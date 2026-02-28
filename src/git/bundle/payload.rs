//! Git-layer payload audit functionality.

use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::types::{
    BundleInspection, MaterializedObjectData, MaterializedObjectIndex, PayloadAudit,
    PayloadAuditDocument, PayloadAuditError, PayloadObjectDetail, PayloadPackProof,
    PayloadPackVerification, PayloadResolveMode, PayloadTransportEntry,
};
use crate::git::util::sha256_hex;
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

mod context;
mod detail;
mod document;
mod parse;
mod verify;

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
    ledger_mode: crate::git::PayloadAuditLedgerMode,
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

    let (bundle_path, bundle_bytes, transport_entries) = if is_zip_bundle_input_path(bundle_input_path)
    {
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

    let parsed_bundle = parse::parse_bundle_payload(&bundle_bytes)?;
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
        context::collect_reachability_context_from_materialized(
            &inspection.heads,
            &materialized_store_by_oid,
        );
    let objects = context::collect_payload_objects_from_materialized_index(
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
    ledger_mode: crate::git::PayloadAuditLedgerMode,
) -> Result<PayloadAuditDocument> {
    document::payload_audit_document_from_session_with_ledger_mode(session, ledger_mode)
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
    detail::collect_payload_object_detail_from_store(
        &session.materialized_store_by_oid,
        &session.blob_paths_by_oid,
        object_id,
    )
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
        let parsed_bundle = parse::parse_bundle_payload(&bundle_bytes).map_err(|err| PayloadAuditError {
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
    let parsed_bundle = parse::parse_bundle_payload(&bundle_bytes).map_err(|err| PayloadAuditError {
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
    verify::verify_pack_payload_impl(pack_data, baseline_odb)
}
