//! Git-layer payload audit functionality.

use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::types::{
    BundleHead, BundleInspection, BundleVersion, MaterializedObjectData, MaterializedObjectIndex,
    MaterializedObjectRecord, MaterializedObjectStore, PackEntryBaseRef, PackEntryKind,
    PackEntryLedger, PackEntryRecord, PayloadAudit, PayloadAuditDocument,
    PayloadAuditDocumentEntryLedger, PayloadAuditDocumentHead, PayloadAuditDocumentObjectDetail,
    PayloadAuditDocumentPackEntry, PayloadAuditDocumentPackObject,
    PayloadAuditDocumentTransportEntry, PayloadAuditError, PayloadAuditLedgerMode,
    PayloadAuditPackSummary, PayloadObjectDetail, PayloadObjectEntry, PayloadObjectKind,
    PayloadPackProof, PayloadPackVerification, PayloadResolveMode, PayloadTransportEntry,
    ResolutionSource,
};
use crate::git::util::{
    bundle_version_code, current_hostname, current_unix_timestamp_secs, current_username,
    sha256_hex,
};
use anyhow::{Result, anyhow, bail};
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

const BLOB_PATH_SCAN_LIMIT: usize = 12;
const LEDGER_SUMMARY_EDGE_COUNT: usize = 20;
const MAX_BLOB_STORE_BYTES: usize = 4 * 1024 * 1024;
const LARGE_BLOB_PREVIEW_BYTES: usize = 8192;

/// Reusable payload session for fast object-detail queries.
#[derive(Debug)]
pub struct PayloadSession {
    inspection: BundleInspection,
    payload: PayloadAudit,
    materialized_store_by_oid: HashMap<git2::Oid, MaterializedObjectData>,
    blob_paths_by_oid: HashMap<git2::Oid, Vec<String>>,
    bundle_path: String,
    bundle_size_bytes: u64,
    bundle_sha256: String,
}

#[derive(Debug, Clone)]
struct ParsedPackObject {
    kind: PayloadObjectKind,
    content: Vec<u8>,
}

#[derive(Debug)]
struct BundlePayload<'a> {
    inspection: BundleInspection,
    pack_data: &'a [u8],
}

/// Collects transport-entry and pack-object payload audit data for a bundle input with explicit resolve mode.
///
/// # Errors
///
/// Returns an error when the bundle input cannot be parsed/imported or objects
/// cannot be enumerated.
pub fn collect_payload_audit_for_bundle_input_with_resolve_mode(
    bundle_input_path: &Path,
    repo_path: &Path,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadAudit> {
    let session =
        open_payload_session_with_resolve_mode(bundle_input_path, repo_path, resolve_mode)?;
    Ok(payload_audit_from_session(&session))
}

/// Builds a serialized payload-audit JSON document with explicit ledger and resolve modes.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails or object-detail
/// materialization fails.
pub fn build_payload_audit_document_for_bundle_input_with_options(
    bundle_input_path: &Path,
    repo_path: &Path,
    ledger_mode: PayloadAuditLedgerMode,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadAuditDocument> {
    let session =
        open_payload_session_with_resolve_mode(bundle_input_path, repo_path, resolve_mode)?;
    payload_audit_document_from_session_with_ledger_mode(&session, ledger_mode)
}

/// Collects detail lines for one selected payload object.
///
/// # Errors
///
/// Returns an error when the object is unavailable in the imported payload.
pub fn collect_payload_object_detail_for_bundle_input(
    bundle_input_path: &Path,
    repo_path: &Path,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    let session = open_payload_session_with_resolve_mode(
        bundle_input_path,
        repo_path,
        PayloadResolveMode::PackOnly,
    )?;
    collect_payload_object_detail_for_session(&session, object_id)
}

/// Opens an imported payload session that can be reused across many detail lookups.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails.
pub fn open_payload_session(bundle_input_path: &Path, repo_path: &Path) -> Result<PayloadSession> {
    open_payload_session_with_resolve_mode(
        bundle_input_path,
        repo_path,
        PayloadResolveMode::PackOnly,
    )
}

/// Opens an imported payload session that can be reused across many detail lookups with explicit resolve mode.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails.
pub fn open_payload_session_with_resolve_mode(
    bundle_input_path: &Path,
    repo_path: &Path,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadSession> {
    ensure_supported_repo_object_format(repo_path)?;

    let baseline_repo = if matches!(resolve_mode, PayloadResolveMode::Baseline) {
        Some(git2::Repository::open(repo_path)?)
    } else {
        None
    };
    let resolve_odb = baseline_repo
        .as_ref()
        .map(git2::Repository::odb)
        .transpose()?;

    let (bundle_path, bundle_bytes, transport_entries) =
        if is_zip_bundle_input_path(bundle_input_path) {
            let transport_entries = collect_transport_entries_for_zip(bundle_input_path)?;
            let extracted = extract_bundle_archive(bundle_input_path)?;
            let bundle_bytes = fs::read(&extracted.bundle_path)?;
            let bundle_path = extracted
                .bundle_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| extracted.bundle_path.display().to_string());
            (bundle_path, bundle_bytes, transport_entries)
        } else {
            let transport_entries = collect_transport_entries_for_plain_bundle(bundle_input_path)?;
            let bundle_bytes = fs::read(bundle_input_path)?;
            let bundle_path = bundle_input_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| bundle_input_path.display().to_string());
            (bundle_path, bundle_bytes, transport_entries)
        };

    let parsed_bundle = parse_bundle_payload(&bundle_bytes)?;
    let inspection = parsed_bundle.inspection.clone();
    let verification = verify_pack_payload_with_ledger_and_baseline_odb(
        parsed_bundle.pack_data,
        resolve_odb.as_ref(),
    )
    .map_err(anyhow::Error::from)?;
    ensure_materialized_index_matches_pack_proof(
        &verification.materialized_index,
        &verification.proof,
    )?;

    let materialized_store_by_oid = verification
        .materialized_store
        .objects
        .iter()
        .cloned()
        .map(|entry| (entry.oid, entry))
        .collect::<HashMap<_, _>>();

    let (reachable, context_map, blob_paths_by_oid) =
        collect_reachability_context_from_materialized(
            &inspection.heads,
            &materialized_store_by_oid,
        );
    let objects = collect_payload_objects_from_materialized_index(
        &verification.materialized_index,
        &reachable,
        &context_map,
    );

    let payload = PayloadAudit {
        bundle_version: inspection.version,
        heads: inspection.heads.clone(),
        transport_entries,
        pack_proof: verification.proof.clone(),
        entry_ledger: verification.ledger,
        objects,
    };

    Ok(PayloadSession {
        inspection,
        payload,
        materialized_store_by_oid,
        blob_paths_by_oid,
        bundle_path,
        bundle_size_bytes: bundle_bytes.len() as u64,
        bundle_sha256: sha256_hex(&bundle_bytes)?,
    })
}

/// Ensures payload audit runs only for repositories using SHA-1 object format.
fn ensure_supported_repo_object_format(repo_path: &Path) -> Result<()> {
    let repo = git2::Repository::open(repo_path)?;
    let config = repo.config()?;
    let object_format = config
        .get_string("extensions.objectformat")
        .unwrap_or_else(|_| "sha1".to_string())
        .to_ascii_lowercase();
    if object_format != "sha1" {
        bail!(
            "unsupported repository object format '{}' at {}: payload audit currently supports only sha1",
            object_format,
            repo_path.display()
        );
    }
    Ok(())
}

/// Returns a payload-audit snapshot captured in the provided session.
pub fn payload_audit_from_session(session: &PayloadSession) -> PayloadAudit {
    session.payload.clone()
}

/// Builds a serialized payload-audit JSON document from a reusable session using a selected ledger mode.
///
/// # Errors
///
/// Returns an error when object detail collection fails for any payload object.
pub fn payload_audit_document_from_session_with_ledger_mode(
    session: &PayloadSession,
    ledger_mode: PayloadAuditLedgerMode,
) -> Result<PayloadAuditDocument> {
    let mut pack_objects = Vec::<PayloadAuditDocumentPackObject>::new();
    let mut object_details = Vec::<PayloadAuditDocumentObjectDetail>::new();

    let mut reachable_objects = 0usize;
    let mut commit_objects = 0usize;
    let mut tree_objects = 0usize;
    let mut blob_objects = 0usize;
    let mut tag_objects = 0usize;
    let mut unknown_objects = 0usize;

    for object in &session.payload.objects {
        if object.reachable_from_heads {
            reachable_objects += 1;
        }

        match object.kind {
            PayloadObjectKind::Commit => commit_objects += 1,
            PayloadObjectKind::Tree => tree_objects += 1,
            PayloadObjectKind::Blob => blob_objects += 1,
            PayloadObjectKind::Tag => tag_objects += 1,
            PayloadObjectKind::Unknown => unknown_objects += 1,
        }

        pack_objects.push(PayloadAuditDocumentPackObject {
            oid: object.oid.to_string(),
            kind: payload_kind_code(object.kind).to_string(),
            size_bytes: object.size_bytes,
            reachable_from_heads: object.reachable_from_heads,
            context_head_index: object.context_head_index,
            context_commit_order: object.context_commit_order,
            context_path: object.context_path.clone(),
        });

        let detail = collect_payload_object_detail_for_session(session, object.oid)?;
        object_details.push(PayloadAuditDocumentObjectDetail {
            oid: detail.oid.to_string(),
            kind: payload_kind_code(detail.kind).to_string(),
            size_bytes: detail.size_bytes,
            syntax_path_hint: detail.syntax_path_hint,
            blob_paths: detail.blob_paths,
            text_line_count: detail.text_line_count,
            lines: detail.lines,
        });
    }

    let total_objects = session.payload.objects.len();
    let summary = PayloadAuditPackSummary {
        total_objects,
        reachable_objects,
        unreachable_objects: total_objects.saturating_sub(reachable_objects),
        commit_objects,
        tree_objects,
        blob_objects,
        tag_objects,
        unknown_objects,
    };

    Ok(PayloadAuditDocument {
        schema_version: "1".to_string(),
        tool_version: crate::version::APP_VERSION.to_string(),
        generated_at_unix_secs: current_unix_timestamp_secs()?,
        generated_by_username: current_username(),
        generated_by_hostname: current_hostname(),
        bundle_path: session.bundle_path.clone(),
        bundle_size_bytes: session.bundle_size_bytes,
        bundle_sha256: session.bundle_sha256.clone(),
        bundle_header_version: bundle_version_code(session.inspection.version).to_string(),
        prerequisites: session
            .inspection
            .prerequisites
            .iter()
            .map(|oid| oid.to_string())
            .collect(),
        heads: session
            .inspection
            .heads
            .iter()
            .map(|head| PayloadAuditDocumentHead {
                oid: head.oid.to_string(),
                reference: head.reference.clone(),
            })
            .collect(),
        transport_entries: session
            .payload
            .transport_entries
            .iter()
            .map(|entry| PayloadAuditDocumentTransportEntry {
                name: entry.name.clone(),
                size_bytes: entry.size_bytes,
                sha256: entry.sha256.clone(),
            })
            .collect(),
        pack_proof: session.payload.pack_proof.clone(),
        entry_ledger: build_document_entry_ledger(&session.payload.entry_ledger, ledger_mode),
        pack_summary: summary,
        pack_objects,
        object_details,
    })
}

/// Builds serialized entry-ledger export section according to requested mode.
fn build_document_entry_ledger(
    ledger: &PackEntryLedger,
    mode: PayloadAuditLedgerMode,
) -> PayloadAuditDocumentEntryLedger {
    let unresolved_entry_rows = ledger
        .entries
        .iter()
        .filter(|entry| !entry.resolved)
        .map(document_pack_entry_row)
        .collect::<Vec<_>>();
    let parsed_entries = ledger.entries.len();
    let declared_entries = ledger.declared_entry_count;
    let unresolved_entries = unresolved_entry_rows.len();
    let first_entries = ledger
        .entries
        .iter()
        .take(LEDGER_SUMMARY_EDGE_COUNT)
        .map(document_pack_entry_row)
        .collect::<Vec<_>>();
    let last_entries = ledger
        .entries
        .iter()
        .skip(parsed_entries.saturating_sub(LEDGER_SUMMARY_EDGE_COUNT))
        .map(document_pack_entry_row)
        .collect::<Vec<_>>();
    let entries = match mode {
        PayloadAuditLedgerMode::Summary => Vec::new(),
        PayloadAuditLedgerMode::Full => ledger
            .entries
            .iter()
            .map(document_pack_entry_row)
            .collect::<Vec<_>>(),
    };

    PayloadAuditDocumentEntryLedger {
        mode: payload_ledger_mode_code(mode).to_string(),
        declared_entries,
        parsed_entries,
        unresolved_entries,
        first_entries,
        last_entries,
        unresolved_entry_rows,
        entries,
    }
}

/// Converts one in-memory ledger row into serialized payload-audit document row.
fn document_pack_entry_row(entry: &PackEntryRecord) -> PayloadAuditDocumentPackEntry {
    PayloadAuditDocumentPackEntry {
        idx: entry.idx,
        offset: entry.offset,
        kind: payload_entry_kind_code(entry.kind).to_string(),
        out_size: entry.out_size,
        reconstructed_size: entry.reconstructed_size,
        base: entry.base_ref.as_ref().map(payload_entry_base_ref_code),
        result_oid: entry.result_oid.map(|oid| oid.to_string()),
        result_kind: entry
            .result_kind
            .map(|kind| payload_kind_code(kind).to_string()),
        resolved: entry.resolved,
        resolved_via: entry
            .resolved_via
            .map(|value| resolution_source_code(value).to_string()),
        note: entry.note.clone(),
    }
}

/// Collects detail lines for one selected payload object from a reusable session.
///
/// # Errors
///
/// Returns an error when the object is unavailable in session object storage.
pub fn collect_payload_object_detail_for_session(
    session: &PayloadSession,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    collect_payload_object_detail_from_store(
        &session.materialized_store_by_oid,
        &session.blob_paths_by_oid,
        object_id,
    )
}

/// Collects payload detail lines from verifier-owned materialized object store.
fn collect_payload_object_detail_from_store(
    store: &HashMap<git2::Oid, MaterializedObjectData>,
    blob_paths_by_oid: &HashMap<git2::Oid, Vec<String>>,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    let stored = store.get(&object_id).ok_or_else(|| {
        anyhow!(
            "payload object {} is not available in materialized store",
            object_id
        )
    })?;
    let kind = stored.kind;
    let size_bytes = stored.size_bytes;
    let detail_lines = object_detail_lines_from_materialized(stored)?;
    let blob_paths = if kind == PayloadObjectKind::Blob {
        blob_paths_by_oid
            .get(&object_id)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let syntax_path_hint = if detail_lines.is_text_blob {
        blob_paths
            .first()
            .cloned()
            .or_else(|| Some("blob.txt".to_string()))
    } else {
        None
    };

    Ok(PayloadObjectDetail {
        oid: object_id,
        kind,
        size_bytes,
        syntax_path_hint,
        blob_paths,
        text_line_count: detail_lines.text_line_count,
        lines: detail_lines.lines,
    })
}

/// Collects transport-entry metadata for a plain `.bundle` input.
fn collect_transport_entries_for_plain_bundle(
    bundle_input_path: &Path,
) -> Result<Vec<PayloadTransportEntry>> {
    if !bundle_input_path.exists() {
        bail!(
            "bundle input path does not exist: {}",
            bundle_input_path.display()
        );
    }
    if !bundle_input_path.is_file() {
        bail!(
            "bundle input path is not a file: {}",
            bundle_input_path.display()
        );
    }
    let bytes = fs::read(bundle_input_path)?;
    let name = bundle_input_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| bundle_input_path.display().to_string());
    Ok(vec![PayloadTransportEntry {
        name,
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes)?,
    }])
}

/// Collects transport-entry metadata for a packaged `.zip` input.
fn collect_transport_entries_for_zip(archive_path: &Path) -> Result<Vec<PayloadTransportEntry>> {
    let archive_file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().ends_with('/') {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        entries.push(PayloadTransportEntry {
            name: entry.name().to_string(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes)?,
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

/// Parses bundle header and returns structured inspection metadata plus exact PACK payload slice.
fn parse_bundle_payload(bundle_bytes: &[u8]) -> Result<BundlePayload<'_>> {
    let mut cursor = 0usize;
    let version_line = read_bundle_header_line(bundle_bytes, &mut cursor)?
        .ok_or_else(|| anyhow!("bundle payload is missing version line"))?;
    let version = match version_line.as_str() {
        "# v2 git bundle" => BundleVersion::V2,
        "# v3 git bundle" => BundleVersion::V3,
        _ => bail!("bundle file is not a valid git bundle header"),
    };

    let mut prerequisites = Vec::<git2::Oid>::new();
    let mut heads = Vec::<BundleHead>::new();
    loop {
        let line = read_bundle_header_line(bundle_bytes, &mut cursor)?
            .ok_or_else(|| anyhow!("bundle header terminated before PACK payload"))?;
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix('-') {
            let oid_token = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("invalid bundle prerequisite line: '{line}'"))?;
            prerequisites.push(git2::Oid::from_str(oid_token)?);
            continue;
        }

        let mut parts = line.splitn(2, ' ');
        let oid_token = parts
            .next()
            .ok_or_else(|| anyhow!("invalid bundle head line: '{line}'"))?;
        let reference = parts
            .next()
            .ok_or_else(|| anyhow!("bundle head line missing reference: '{line}'"))?;
        heads.push(BundleHead {
            oid: git2::Oid::from_str(oid_token)?,
            reference: reference.to_string(),
        });
    }

    if bundle_bytes.len().saturating_sub(cursor) < 4 {
        bail!("bundle header is not followed by PACK payload");
    }
    if &bundle_bytes[cursor..cursor + 4] != b"PACK" {
        bail!("bundle header terminator is not followed by PACK payload");
    }

    Ok(BundlePayload {
        inspection: BundleInspection {
            version,
            prerequisites,
            heads,
        },
        pack_data: &bundle_bytes[cursor..],
    })
}

/// Reads one bundle-header line as UTF-8 and advances cursor.
fn read_bundle_header_line(bundle_bytes: &[u8], cursor: &mut usize) -> Result<Option<String>> {
    if *cursor >= bundle_bytes.len() {
        return Ok(None);
    }
    let start = *cursor;
    let rel_end = bundle_bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|value| start + value);
    let end = rel_end.unwrap_or(bundle_bytes.len());
    *cursor = if rel_end.is_some() { end + 1 } else { end };

    let mut line = &bundle_bytes[start..end];
    if line.ends_with(b"\r") {
        line = &line[..line.len() - 1];
    }
    let text = std::str::from_utf8(line)
        .map_err(|_| anyhow!("bundle header contains non-utf8 line bytes"))?
        .to_string();
    Ok(Some(text))
}

/// Verifies pack proof + ledger directly from a bundle or packaged zip input.
#[cfg(test)]
pub fn verify_pack_payload_for_bundle_input(
    bundle_input_path: &Path,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    verify_pack_payload_for_bundle_input_with_resolve_mode(bundle_input_path, None)
}

/// Verifies pack proof + ledger from bundle input with optional baseline resolve repository (tests only).
#[cfg(test)]
pub fn verify_pack_payload_for_bundle_input_with_resolve_mode(
    bundle_input_path: &Path,
    baseline_repo_path: Option<&Path>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    let baseline_repo = baseline_repo_path.and_then(|path| git2::Repository::open(path).ok());
    let baseline_odb = baseline_repo.as_ref().and_then(|repo| repo.odb().ok());
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted =
            extract_bundle_archive(bundle_input_path).map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: None,
                ledger_partial: None,
            })?;
        let bundle_bytes = fs::read(&extracted.bundle_path).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
        let parsed_bundle =
            parse_bundle_payload(&bundle_bytes).map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: None,
                ledger_partial: None,
            })?;
        return verify_pack_payload_with_ledger_and_baseline_odb(
            parsed_bundle.pack_data,
            baseline_odb.as_ref(),
        );
    }

    let bundle_bytes = fs::read(bundle_input_path).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    let parsed_bundle = parse_bundle_payload(&bundle_bytes).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    verify_pack_payload_with_ledger_and_baseline_odb(parsed_bundle.pack_data, baseline_odb.as_ref())
}

/// Asserts that ledger-derived materialization counts align with pack-proof counters.
fn ensure_materialized_index_matches_pack_proof(
    index: &MaterializedObjectIndex,
    pack_proof: &PayloadPackProof,
) -> Result<()> {
    if index.materialized_entry_count != pack_proof.entries_materialized {
        bail!(
            "materialized entry count mismatch: materialized={}, proof_materialized={}",
            index.materialized_entry_count,
            pack_proof.entries_materialized
        );
    }
    if !pack_proof.transfer_allowed {
        bail!(
            "transfer blocked by proof gate: {}",
            pack_proof
                .blocked_reason
                .as_deref()
                .unwrap_or("entries are not fully materialized")
        );
    }
    Ok(())
}

/// Verifies payload PACK bytes and returns proof + entry ledger truth with optional baseline ODB resolution.
pub fn verify_pack_payload_with_ledger_and_baseline_odb(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    verify_pack_payload_impl(pack_data, baseline_odb)
}

fn verify_pack_payload_impl(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    if pack_data.len() < 32 {
        return Err(PayloadAuditError {
            reason: "pack payload is too small".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }
    if &pack_data[..4] != b"PACK" {
        return Err(PayloadAuditError {
            reason: "pack payload does not start with PACK header".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    let pack_version = read_be_u32(pack_data, 4).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    if pack_version != 2 && pack_version != 3 {
        return Err(PayloadAuditError {
            reason: format!("unsupported pack version: {pack_version}"),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }
    let declared_object_count = read_be_u32(pack_data, 8).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })? as usize;

    let trailer_offset = pack_data
        .len()
        .checked_sub(20)
        .ok_or_else(|| PayloadAuditError {
            reason: "pack payload missing trailer checksum".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    let computed_checksum =
        sha1_hex(&pack_data[..trailer_offset]).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    let trailer_checksum = hex_encode(&pack_data[trailer_offset..]);
    if computed_checksum != trailer_checksum {
        return Err(PayloadAuditError {
            reason: format!(
                "pack trailer checksum mismatch: computed={}, trailer={}",
                computed_checksum, trailer_checksum
            ),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    let mut ledger = PackEntryLedger {
        pack_version,
        declared_entry_count: declared_object_count,
        entries: Vec::with_capacity(declared_object_count),
    };
    let mut offset = 12usize;
    let mut processed_object_count = 0usize;
    let mut thin_pack_detected = false;
    let mut baseline_resolutions_count = 0usize;
    let mut objects_by_offset = HashMap::<usize, ParsedPackObject>::new();
    let mut objects_by_oid = HashMap::<git2::Oid, ParsedPackObject>::new();

    while processed_object_count < declared_object_count {
        if offset >= trailer_offset {
            return Err(PayloadAuditError {
                reason: format!(
                    "pack ended before declared object count was processed: declared={}, processed={}",
                    declared_object_count, processed_object_count
                ),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger),
            });
        }

        let entry_offset = offset;
        let (entry_kind, expected_header_size, header_end) =
            parse_pack_entry_header(pack_data, offset)
            .map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger.clone()),
            })?;
        let mut entry_record = PackEntryRecord {
            idx: processed_object_count,
            offset: entry_offset,
            kind: entry_kind,
            out_size: expected_header_size,
            reconstructed_size: None,
            base_ref: None,
            result_oid: None,
            result_kind: None,
            resolved: false,
            resolved_via: None,
            note: None,
        };
        offset = header_end;

        let mut entry_resolved_via = ResolutionSource::InPack;
        let parsed = match entry_kind {
            PackEntryKind::Commit
            | PackEntryKind::Tree
            | PackEntryKind::Blob
            | PackEntryKind::Tag => {
                let expected_object_size = expected_header_size;
                let (consumed, content) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress pack object #{} ({:?}) at offset {}: {err}",
                                processed_object_count + 1,
                                entry_kind,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
                    .checked_add(consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "pack entry offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                if content.len() != expected_object_size {
                    return Err(PayloadAuditError {
                        reason: format!(
                            "pack object size mismatch at object {}: header={}, actual={}",
                            processed_object_count + 1,
                            expected_object_size,
                            content.len()
                        ),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: pack_entry_kind_to_payload_kind(entry_kind),
                    content,
                }
            }
            PackEntryKind::OfsDelta => {
                let (base_offset_distance, consumed) =
                    parse_ofs_delta_base_distance(pack_data, offset).map_err(|err| {
                        PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        }
                    })?;
                offset = offset
                    .checked_add(consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ofs-delta offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let base_entry_offset =
                    entry_offset
                        .checked_sub(base_offset_distance)
                        .ok_or_else(|| PayloadAuditError {
                            reason: "ofs-delta base offset underflow".to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                entry_record.base_ref = Some(PackEntryBaseRef::BaseOffset {
                    distance: base_offset_distance,
                    base_offset: Some(base_entry_offset),
                });
                let Some(base) = objects_by_offset.get(&base_entry_offset) else {
                    let reason = format!(
                        "ofs-delta references unresolved base at distance {} (external dependency/thin pack)",
                        base_offset_distance
                    );
                    entry_record.note = Some(reason.clone());
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason,
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                };
                let (delta_consumed, delta_bytes) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress ofs-delta object #{} at offset {}: {err}",
                                processed_object_count + 1,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
                    .checked_add(delta_consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ofs-delta data offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let expected_delta_stream_len = expected_header_size;
                // PACK delta headers encode delta-stream byte length for ofs/ref-delta entries.
                // Spec: https://git-scm.com/docs/pack-format (Size encoding section).
                if delta_bytes.len() != expected_delta_stream_len {
                    return Err(PayloadAuditError {
                        reason: format!(
                            "ofs-delta stream size mismatch at object {}: header={}, actual={}",
                            processed_object_count + 1,
                            expected_delta_stream_len,
                            delta_bytes.len()
                        ),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let content = apply_git_delta(&base.content, &delta_bytes).map_err(|err| {
                    PayloadAuditError {
                        reason: err.to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    }
                })?;
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: base.kind,
                    content,
                }
            }
            PackEntryKind::RefDelta => {
                if trailer_offset.saturating_sub(offset) < 20 {
                    entry_record.note =
                        Some("ref-delta entry is missing base object id bytes".to_string());
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason: "ref-delta entry is missing base object id bytes".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let base_oid =
                    git2::Oid::from_bytes(&pack_data[offset..offset + 20]).map_err(|err| {
                        PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        }
                    })?;
                entry_record.base_ref = Some(PackEntryBaseRef::BaseOid(base_oid));
                offset += 20;

                let in_pack_base = objects_by_oid.get(&base_oid).cloned();
                let baseline_base = if in_pack_base.is_none() {
                    baseline_odb.and_then(|odb| load_parsed_object_from_odb(odb, base_oid).ok())
                } else {
                    None
                };
                let Some(base) = in_pack_base.or(baseline_base) else {
                    let reason = format!(
                        "ref-delta references unresolved base object {} (external dependency/thin pack)",
                        base_oid
                    );
                    entry_record.note = Some(reason.clone());
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason,
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                };
                if objects_by_oid.contains_key(&base_oid) {
                    entry_resolved_via = ResolutionSource::InPack;
                } else {
                    thin_pack_detected = true;
                    baseline_resolutions_count += 1;
                    entry_resolved_via = ResolutionSource::Baseline;
                }
                let (delta_consumed, delta_bytes) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress ref-delta object #{} at offset {}: {err}",
                                processed_object_count + 1,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
                    .checked_add(delta_consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ref-delta data offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let expected_delta_stream_len = expected_header_size;
                // PACK delta headers encode delta-stream byte length for ofs/ref-delta entries.
                // Spec: https://git-scm.com/docs/pack-format (Size encoding section).
                if delta_bytes.len() != expected_delta_stream_len {
                    return Err(PayloadAuditError {
                        reason: format!(
                            "ref-delta stream size mismatch at object {}: header={}, actual={}",
                            processed_object_count + 1,
                            expected_delta_stream_len,
                            delta_bytes.len()
                        ),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let content = apply_git_delta(&base.content, &delta_bytes).map_err(|err| {
                    PayloadAuditError {
                        reason: err.to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    }
                })?;
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: base.kind,
                    content,
                }
            }
        };

        let object_oid = object_oid_for_content(parsed.kind, &parsed.content).map_err(|err| {
            PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger.clone()),
            }
        })?;
        entry_record.result_oid = Some(object_oid);
        entry_record.result_kind = Some(parsed.kind);
        entry_record.resolved = true;
        entry_record.resolved_via = Some(entry_resolved_via);
        ledger.entries.push(entry_record);
        objects_by_offset.insert(entry_offset, parsed.clone());
        objects_by_oid.insert(object_oid, parsed);
        processed_object_count += 1;
    }

    if offset != trailer_offset {
        return Err(PayloadAuditError {
            reason: format!(
                "pack contains trailing or unconsumed bytes before trailer: consumed={}, trailer={}",
                offset, trailer_offset
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger),
        });
    }
    if processed_object_count != declared_object_count {
        return Err(PayloadAuditError {
            reason: format!(
                "pack object count mismatch: declared={}, processed={}",
                declared_object_count, processed_object_count
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger),
        });
    }

    let materialized_index = build_materialized_object_index_from_ledger(&ledger);
    let materialized_store = build_materialized_object_store(&objects_by_oid);
    let proof = PayloadPackProof::from_entry_counters(
        pack_version,
        declared_object_count,
        processed_object_count,
        materialized_index.materialized_entry_count,
        materialized_index.unique_object_count,
        materialized_index.duplicate_entry_count_materialized,
        true,
        thin_pack_detected,
        baseline_resolutions_count,
        "sha1".to_string(),
        computed_checksum,
        trailer_checksum,
    );
    Ok(PayloadPackVerification {
        proof,
        ledger,
        materialized_index,
        materialized_store,
    })
}

/// Parses a PACK object header at `offset`.
fn parse_pack_entry_header(
    pack_data: &[u8],
    offset: usize,
) -> Result<(PackEntryKind, usize, usize)> {
    if offset >= pack_data.len() {
        bail!("pack entry header offset is out of bounds");
    }

    let first = pack_data[offset];
    let entry_kind = match (first >> 4) & 0x07 {
        1 => PackEntryKind::Commit,
        2 => PackEntryKind::Tree,
        3 => PackEntryKind::Blob,
        4 => PackEntryKind::Tag,
        6 => PackEntryKind::OfsDelta,
        7 => PackEntryKind::RefDelta,
        code => bail!("unsupported/invalid pack entry type code: {code}"),
    };

    let mut size = (first & 0x0f) as usize;
    let mut shift = 4u32;
    let mut cursor = offset + 1;
    let mut byte = first;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("pack entry header is truncated");
        }
        byte = pack_data[cursor];
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        cursor += 1;
    }

    Ok((entry_kind, size, cursor))
}

/// Parses OFS-delta encoded backward base distance.
fn parse_ofs_delta_base_distance(pack_data: &[u8], offset: usize) -> Result<(usize, usize)> {
    if offset >= pack_data.len() {
        bail!("ofs-delta base encoding offset out of bounds");
    }
    let mut cursor = offset;
    let mut byte = pack_data[cursor];
    cursor += 1;

    let mut distance = (byte & 0x7f) as usize;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("ofs-delta base encoding is truncated");
        }
        byte = pack_data[cursor];
        cursor += 1;
        distance = ((distance + 1) << 7) + (byte & 0x7f) as usize;
    }

    Ok((distance, cursor - offset))
}

/// Decompresses one zlib stream from the start of `bytes`, returning consumed input bytes.
fn decompress_zlib_stream(bytes: &[u8]) -> Result<(usize, Vec<u8>)> {
    let mut decompressor = Decompress::new(true);
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut no_progress_streak = 0usize;

    loop {
        if consumed_total >= bytes.len() {
            bail!("unexpected end of zlib stream while reading pack entry");
        }
        let input = &bytes[consumed_total..];
        out.reserve(16 * 1024);
        let before_in = decompressor.total_in();
        let before_out = decompressor.total_out();
        let status = decompressor.decompress_vec(input, &mut out, FlushDecompress::None)?;
        let consumed = (decompressor.total_in() - before_in) as usize;
        let produced = (decompressor.total_out() - before_out) as usize;
        consumed_total += consumed;

        match status {
            Status::StreamEnd => break,
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    no_progress_streak += 1;
                    if no_progress_streak >= 8 {
                        bail!("zlib stream made no progress while parsing pack entry");
                    }
                } else {
                    no_progress_streak = 0;
                }
            }
        }
    }

    Ok((consumed_total, out))
}

/// Applies a git delta bytecode stream to `base`, producing reconstructed object content.
fn apply_git_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = 0usize;
    let (source_size, source_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += source_size_consumed;
    if source_size != base.len() {
        bail!(
            "delta source size mismatch: expected={}, actual={}",
            source_size,
            base.len()
        );
    }

    let (target_size, target_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += target_size_consumed;
    let mut out = Vec::with_capacity(target_size);

    while cursor < delta.len() {
        let opcode = delta[cursor];
        cursor += 1;

        if (opcode & 0x80) != 0 {
            let mut copy_offset = 0usize;
            let mut copy_size = 0usize;

            if (opcode & 0x01) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 0")?;
                copy_offset |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x02) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 1")?;
                copy_offset |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x04) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 2")?;
                copy_offset |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if (opcode & 0x08) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 3")?;
                copy_offset |= (delta[cursor] as usize) << 24;
                cursor += 1;
            }

            if (opcode & 0x10) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 0")?;
                copy_size |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x20) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 1")?;
                copy_size |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x40) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 2")?;
                copy_size |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if copy_size == 0 {
                copy_size = 0x10000;
            }

            let copy_end = copy_offset
                .checked_add(copy_size)
                .ok_or_else(|| anyhow!("delta copy range overflow"))?;
            if copy_end > base.len() {
                bail!(
                    "delta copy range exceeds base object: offset={}, size={}, base={}",
                    copy_offset,
                    copy_size,
                    base.len()
                );
            }
            out.extend_from_slice(&base[copy_offset..copy_end]);
        } else if opcode != 0 {
            let literal_size = opcode as usize;
            ensure_remaining(delta, cursor, literal_size, "delta literal chunk")?;
            out.extend_from_slice(&delta[cursor..cursor + literal_size]);
            cursor += literal_size;
        } else {
            bail!("invalid delta opcode 0x00");
        }
    }

    if out.len() != target_size {
        bail!(
            "delta result size mismatch: expected={}, actual={}",
            target_size,
            out.len()
        );
    }
    Ok(out)
}

/// Parses one git delta varint from `bytes` at `offset`.
fn parse_git_delta_varint(bytes: &[u8], offset: usize) -> Result<(usize, usize)> {
    if offset >= bytes.len() {
        bail!("delta varint is out of bounds");
    }
    let mut cursor = offset;
    let mut value = 0usize;
    let mut shift = 0u32;
    loop {
        ensure_remaining(bytes, cursor, 1, "delta varint byte")?;
        let byte = bytes[cursor];
        cursor += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, cursor - offset))
}

/// Returns the canonical object id for `(kind, content)` as SHA-1.
fn object_oid_for_content(kind: PayloadObjectKind, content: &[u8]) -> Result<git2::Oid> {
    let type_name = match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => bail!("cannot hash unknown pack object kind"),
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(format!("{type_name} {}\0", content.len()).as_bytes());
    bytes.extend_from_slice(content);
    let digest_hex = sha1_hex(&bytes)?;
    Ok(git2::Oid::from_str(&digest_hex)?)
}

/// Loads one baseline object by OID for external ref-delta base resolution.
fn load_parsed_object_from_odb(odb: &git2::Odb<'_>, oid: git2::Oid) -> Result<ParsedPackObject> {
    let object = odb.read(oid)?;
    let kind = payload_kind_from_git(object.kind());
    if matches!(kind, PayloadObjectKind::Unknown) {
        bail!("baseline base object has unsupported type for delta resolution: {oid}");
    }
    Ok(ParsedPackObject {
        kind,
        content: object.data().to_vec(),
    })
}

/// Converts one pack entry kind into payload object kind.
fn pack_entry_kind_to_payload_kind(kind: PackEntryKind) -> PayloadObjectKind {
    match kind {
        PackEntryKind::Commit => PayloadObjectKind::Commit,
        PackEntryKind::Tree => PayloadObjectKind::Tree,
        PackEntryKind::Blob => PayloadObjectKind::Blob,
        PackEntryKind::Tag => PayloadObjectKind::Tag,
        PackEntryKind::OfsDelta | PackEntryKind::RefDelta => PayloadObjectKind::Unknown,
    }
}

/// Reads one big-endian `u32` from `bytes` at `offset`.
fn read_be_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow!("u32 read offset overflow"))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| anyhow!("u32 read out of bounds"))?;
    Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Ensures `len` bytes are available from `offset`.
fn ensure_remaining(bytes: &[u8], offset: usize, len: usize, context: &str) -> Result<()> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| anyhow!("{context}: offset overflow"))?;
    if end > bytes.len() {
        bail!("{context}: truncated data");
    }
    Ok(())
}

/// Returns lowercase hex for arbitrary bytes.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns SHA-1 digest hex for bytes.
fn sha1_hex(bytes: &[u8]) -> Result<String> {
    let mut ctx = std::mem::MaybeUninit::<openssl_sys::SHA_CTX>::uninit();
    let init_ok = unsafe { openssl_sys::SHA1_Init(ctx.as_mut_ptr()) } == 1;
    if !init_ok {
        bail!("failed to initialize SHA-1 context");
    }
    let mut ctx = unsafe { ctx.assume_init() };
    let update_ok =
        unsafe { openssl_sys::SHA1_Update(&mut ctx, bytes.as_ptr().cast(), bytes.len()) } == 1;
    if !update_ok {
        bail!("failed to update SHA-1 digest");
    }
    let mut digest = [0u8; 20];
    let final_ok = unsafe { openssl_sys::SHA1_Final(digest.as_mut_ptr(), &mut ctx) } == 1;
    if !final_ok {
        bail!("failed to finalize SHA-1 digest");
    }
    Ok(hex_encode(&digest))
}

/// Builds deduplicated materialized object index directly from ledger result rows.
fn build_materialized_object_index_from_ledger(
    ledger: &PackEntryLedger,
) -> MaterializedObjectIndex {
    let mut by_oid = HashMap::<git2::Oid, MaterializedObjectRecord>::new();
    let mut materialized_entry_count = 0usize;

    for entry in &ledger.entries {
        if !is_materialized_entry(entry) {
            continue;
        }
        let (Some(oid), Some(kind)) = (entry.result_oid, entry.result_kind) else {
            continue;
        };
        materialized_entry_count += 1;
        let reconstructed_size = entry.reconstructed_size.unwrap_or(entry.out_size);
        by_oid.entry(oid).or_insert(MaterializedObjectRecord {
            oid,
            kind,
            size_bytes: reconstructed_size,
            first_entry_idx: entry.idx,
        });
    }

    let unique_object_count = by_oid.len();
    let duplicate_entry_count_materialized =
        materialized_entry_count.saturating_sub(unique_object_count);
    let mut objects = by_oid.into_values().collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });

    MaterializedObjectIndex {
        objects,
        materialized_entry_count,
        unique_object_count,
        duplicate_entry_count_materialized,
    }
}

/// Builds verifier-owned materialized object store from parsed object map.
fn build_materialized_object_store(
    objects_by_oid: &HashMap<git2::Oid, ParsedPackObject>,
) -> MaterializedObjectStore {
    let mut objects = objects_by_oid
        .iter()
        .map(|(oid, parsed)| {
            if parsed.kind == PayloadObjectKind::Blob && parsed.content.len() > MAX_BLOB_STORE_BYTES
            {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content
                        [..LARGE_BLOB_PREVIEW_BYTES.min(parsed.content.len())]
                        .to_vec(),
                    content_truncated: true,
                }
            } else {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content.clone(),
                    content_truncated: false,
                }
            }
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| left.oid.cmp(&right.oid));
    MaterializedObjectStore { objects }
}

/// Returns true when an entry is fully materialized and exportable as exact object bytes.
fn is_materialized_entry(entry: &PackEntryRecord) -> bool {
    entry.resolved
        && entry.result_oid.is_some()
        && entry.result_kind.is_some()
        && entry.reconstructed_size.is_some()
        && entry.resolved_via.is_some()
}

/// Builds payload object rows from the deduplicated materialized index.
fn collect_payload_objects_from_materialized_index(
    materialized_index: &MaterializedObjectIndex,
    reachable: &HashSet<git2::Oid>,
    context_map: &HashMap<git2::Oid, PayloadObjectContext>,
) -> Vec<PayloadObjectEntry> {
    let mut objects = Vec::new();
    for row in &materialized_index.objects {
        let oid = row.oid;
        let context = context_map.get(&oid);
        objects.push(PayloadObjectEntry {
            oid,
            kind: row.kind,
            size_bytes: row.size_bytes,
            reachable_from_heads: reachable.contains(&oid),
            context_head_index: context.map(|value| value.head_index),
            context_commit_order: context.map(|value| value.commit_order),
            context_path: context.and_then(|value| value.path.clone()),
        });
    }

    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });
    objects
}

/// Collects reachability/context/blob-path metadata directly from materialized object bytes.
fn collect_reachability_context_from_materialized(
    heads: &[BundleHead],
    store_by_oid: &HashMap<git2::Oid, MaterializedObjectData>,
) -> (
    HashSet<git2::Oid>,
    HashMap<git2::Oid, PayloadObjectContext>,
    HashMap<git2::Oid, Vec<String>>,
) {
    let mut reachable = HashSet::<git2::Oid>::new();
    let mut context = HashMap::<git2::Oid, PayloadObjectContext>::new();
    let mut blob_paths = HashMap::<git2::Oid, Vec<String>>::new();
    let mut seen_commits = HashSet::<git2::Oid>::new();
    let mut seen_trees = HashSet::<git2::Oid>::new();

    for (head_index, head) in heads.iter().enumerate() {
        let mut commit_order = 0usize;
        let mut stack = vec![head.oid];
        while let Some(commit_id) = stack.pop() {
            if !seen_commits.insert(commit_id) {
                continue;
            }
            let Some(commit_data) = store_by_oid.get(&commit_id) else {
                continue;
            };
            if commit_data.kind != PayloadObjectKind::Commit {
                continue;
            }
            commit_order += 1;
            reachable.insert(commit_id);
            context
                .entry(commit_id)
                .or_insert_with(|| PayloadObjectContext {
                    head_index,
                    commit_order,
                    path: None,
                });

            let (tree_oid, parents) = parse_commit_tree_and_parents(&commit_data.content_bytes);
            if let Some(tree_oid) = tree_oid {
                walk_tree_from_materialized(
                    store_by_oid,
                    tree_oid,
                    "",
                    head_index,
                    commit_order,
                    &mut reachable,
                    &mut context,
                    &mut blob_paths,
                    &mut seen_trees,
                );
            }
            for parent_id in parents.into_iter().rev() {
                if !seen_commits.contains(&parent_id) {
                    stack.push(parent_id);
                }
            }
        }
    }

    for paths in blob_paths.values_mut() {
        paths.sort();
        paths.dedup();
        paths.truncate(BLOB_PATH_SCAN_LIMIT);
    }

    (reachable, context, blob_paths)
}

/// Recursively traverses materialized tree bytes and records context/path metadata.
fn walk_tree_from_materialized(
    store_by_oid: &HashMap<git2::Oid, MaterializedObjectData>,
    tree_oid: git2::Oid,
    prefix: &str,
    head_index: usize,
    commit_order: usize,
    reachable: &mut HashSet<git2::Oid>,
    context: &mut HashMap<git2::Oid, PayloadObjectContext>,
    blob_paths: &mut HashMap<git2::Oid, Vec<String>>,
    seen_trees: &mut HashSet<git2::Oid>,
) {
    if !seen_trees.insert(tree_oid) {
        return;
    }
    let Some(tree_data) = store_by_oid.get(&tree_oid) else {
        return;
    };
    if tree_data.kind != PayloadObjectKind::Tree {
        return;
    }
    reachable.insert(tree_oid);
    context
        .entry(tree_oid)
        .or_insert_with(|| PayloadObjectContext {
            head_index,
            commit_order,
            path: if prefix.is_empty() {
                None
            } else {
                Some(prefix.to_string())
            },
        });

    let Ok(entries) = parse_tree_entries(&tree_data.content_bytes) else {
        return;
    };
    for entry in entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        reachable.insert(entry.oid);
        context
            .entry(entry.oid)
            .or_insert_with(|| PayloadObjectContext {
                head_index,
                commit_order,
                path: Some(path.clone()),
            });
        if entry.kind == PayloadObjectKind::Tree {
            walk_tree_from_materialized(
                store_by_oid,
                entry.oid,
                &path,
                head_index,
                commit_order,
                reachable,
                context,
                blob_paths,
                seen_trees,
            );
        } else if entry.kind == PayloadObjectKind::Blob {
            let paths = blob_paths.entry(entry.oid).or_default();
            if paths.len() < BLOB_PATH_SCAN_LIMIT {
                paths.push(path);
            }
        }
    }
}

/// Returns stable display rank for object-kind ordering.
fn payload_kind_rank(kind: PayloadObjectKind) -> u8 {
    match kind {
        PayloadObjectKind::Commit => 0,
        PayloadObjectKind::Tree => 1,
        PayloadObjectKind::Blob => 2,
        PayloadObjectKind::Tag => 3,
        PayloadObjectKind::Unknown => 4,
    }
}

/// Maps git2 object type into payload object kind enum.
fn payload_kind_from_git(kind: git2::ObjectType) -> PayloadObjectKind {
    match kind {
        git2::ObjectType::Commit => PayloadObjectKind::Commit,
        git2::ObjectType::Tree => PayloadObjectKind::Tree,
        git2::ObjectType::Blob => PayloadObjectKind::Blob,
        git2::ObjectType::Tag => PayloadObjectKind::Tag,
        _ => PayloadObjectKind::Unknown,
    }
}

/// Returns stable string code for payload object kinds.
fn payload_kind_code(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

/// Returns stable string code for payload ledger export mode.
fn payload_ledger_mode_code(mode: PayloadAuditLedgerMode) -> &'static str {
    match mode {
        PayloadAuditLedgerMode::Summary => "summary",
        PayloadAuditLedgerMode::Full => "full",
    }
}

/// Returns stable string code for pack entry kinds.
fn payload_entry_kind_code(kind: PackEntryKind) -> &'static str {
    match kind {
        PackEntryKind::Commit => "commit",
        PackEntryKind::Tree => "tree",
        PackEntryKind::Blob => "blob",
        PackEntryKind::Tag => "tag",
        PackEntryKind::OfsDelta => "ofs-delta",
        PackEntryKind::RefDelta => "ref-delta",
    }
}

/// Returns a compact code for pack entry base references.
fn payload_entry_base_ref_code(base_ref: &PackEntryBaseRef) -> String {
    match base_ref {
        PackEntryBaseRef::BaseOffset {
            distance,
            base_offset,
        } => match base_offset {
            Some(offset) => format!("ofs:{distance}@{offset}"),
            None => format!("ofs:{distance}"),
        },
        PackEntryBaseRef::BaseOid(oid) => format!("oid:{oid}"),
    }
}

/// Returns stable string code for pack-entry resolution source.
fn resolution_source_code(source: ResolutionSource) -> &'static str {
    match source {
        ResolutionSource::InPack => "in-pack",
        ResolutionSource::Baseline => "baseline",
    }
}

/// Renders object-specific detail lines for payload drill-down/preview view from materialized bytes.
fn object_detail_lines_from_materialized(
    object: &MaterializedObjectData,
) -> Result<ObjectDetailLines> {
    match object.kind {
        PayloadObjectKind::Commit => {
            let text = String::from_utf8_lossy(&object.content_bytes);
            let mut lines = vec![format!("commit {}", object.oid)];
            let mut in_message = false;
            for line in text.lines() {
                if !in_message {
                    if line.is_empty() {
                        in_message = true;
                        lines.push(String::new());
                        continue;
                    }
                    if line.starts_with("tree ")
                        || line.starts_with("parent ")
                        || line.starts_with("author ")
                        || line.starts_with("committer ")
                    {
                        lines.push(line.to_string());
                    }
                } else {
                    lines.push(line.to_string());
                }
            }
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Tree => {
            let mut lines = vec![format!("tree {}", object.oid), String::new()];
            let entries = parse_tree_entries(&object.content_bytes)?;
            for entry in entries {
                lines.push(format!(
                    "{:>6} {:<7} {} {}",
                    entry.mode,
                    payload_kind_code(entry.kind),
                    entry.oid,
                    entry.name
                ));
            }
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Blob => render_blob_detail_lines(object),
        PayloadObjectKind::Tag => {
            let text = String::from_utf8_lossy(&object.content_bytes);
            let mut lines = vec![format!("tag {}", object.oid), String::new()];
            lines.extend(text.lines().map(str::to_string));
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Unknown => Ok(ObjectDetailLines {
            lines: vec![
                format!("object {}", object.oid),
                "unsupported object type for detail rendering".to_string(),
            ],
            is_text_blob: false,
            text_line_count: None,
        }),
    }
}

/// Renders blob detail lines including preview policy when content is truncated.
fn render_blob_detail_lines(object: &MaterializedObjectData) -> Result<ObjectDetailLines> {
    if let Ok(text) = std::str::from_utf8(&object.content_bytes) {
        let text_line_count = if object.content_truncated {
            None
        } else {
            Some(text.lines().count())
        };
        let mut lines = vec![
            format!("text blob {}", object.oid),
            format!("size: {} bytes", object.size_bytes),
            if object.content_truncated {
                format!(
                    "preview only: {} / {} bytes",
                    object.content_bytes.len(),
                    object.size_bytes
                )
            } else {
                format!("text lines: {}", text_line_count.unwrap_or(0))
            },
            String::new(),
        ];
        lines.extend(text.lines().map(str::to_string));
        if object.content_truncated {
            lines.push(String::new());
            lines.push("full content not retained in-memory (large blob preview mode)".to_string());
        }
        return Ok(ObjectDetailLines {
            lines,
            is_text_blob: true,
            text_line_count,
        });
    }

    let preview_len = object.content_bytes.len().min(256);
    let mut lines = vec![
        format!("binary blob {}", object.oid),
        format!("size: {} bytes", object.size_bytes),
        if object.content_truncated {
            format!(
                "preview only: {} / {} bytes",
                object.content_bytes.len(),
                object.size_bytes
            )
        } else {
            format!("hex preview (first {preview_len} bytes):")
        },
        String::new(),
    ];
    for chunk in object.content_bytes[..preview_len].chunks(16) {
        lines.push(
            chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    Ok(ObjectDetailLines {
        lines,
        is_text_blob: false,
        text_line_count: None,
    })
}

#[derive(Debug, Clone)]
struct ParsedTreeEntry {
    mode: String,
    name: String,
    oid: git2::Oid,
    kind: PayloadObjectKind,
}

/// Parses commit bytes for tree and parent object ids.
fn parse_commit_tree_and_parents(content: &[u8]) -> (Option<git2::Oid>, Vec<git2::Oid>) {
    let mut tree = None;
    let mut parents = Vec::new();
    let text = String::from_utf8_lossy(content);
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("tree ") {
            tree = git2::Oid::from_str(value.trim()).ok();
        } else if let Some(value) = line.strip_prefix("parent ")
            && let Ok(parent) = git2::Oid::from_str(value.trim())
        {
            parents.push(parent);
        }
    }
    (tree, parents)
}

/// Parses raw tree object bytes into structured entries.
fn parse_tree_entries(content: &[u8]) -> Result<Vec<ParsedTreeEntry>> {
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < content.len() {
        let mode_end = content[cursor..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|value| cursor + value)
            .ok_or_else(|| anyhow!("tree entry mode is truncated"))?;
        let mode = std::str::from_utf8(&content[cursor..mode_end])
            .map_err(|_| anyhow!("tree entry mode is non-utf8"))?
            .to_string();
        cursor = mode_end + 1;

        let name_end = content[cursor..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|value| cursor + value)
            .ok_or_else(|| anyhow!("tree entry name is truncated"))?;
        let name = String::from_utf8_lossy(&content[cursor..name_end]).to_string();
        cursor = name_end + 1;
        ensure_remaining(content, cursor, 20, "tree entry object id")?;
        let oid = git2::Oid::from_bytes(&content[cursor..cursor + 20])?;
        cursor += 20;

        entries.push(ParsedTreeEntry {
            mode: mode.clone(),
            name,
            oid,
            kind: tree_entry_kind_from_mode(&mode),
        });
    }
    Ok(entries)
}

/// Maps tree-entry mode to payload object kind.
fn tree_entry_kind_from_mode(mode: &str) -> PayloadObjectKind {
    match mode {
        "40000" | "040000" => PayloadObjectKind::Tree,
        "160000" => PayloadObjectKind::Commit,
        _ => PayloadObjectKind::Blob,
    }
}

struct ObjectDetailLines {
    lines: Vec<String>,
    is_text_blob: bool,
    text_line_count: Option<usize>,
}

#[derive(Debug, Clone)]
struct PayloadObjectContext {
    head_index: usize,
    commit_order: usize,
    path: Option<String>,
}
