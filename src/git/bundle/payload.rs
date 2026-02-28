//! Git-layer payload audit functionality.

use super::inspect::inspect_bundle;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::types::{
    BundleInspection, PayloadAudit, PayloadAuditDocument, PayloadAuditDocumentHead,
    PayloadAuditDocumentObjectDetail, PayloadAuditDocumentPackObject,
    PayloadAuditDocumentTransportEntry, PayloadAuditPackSummary, PayloadObjectDetail,
    PayloadObjectEntry, PayloadObjectKind, PayloadTransportEntry,
};
use crate::git::util::{
    bundle_version_code, current_hostname, current_unix_timestamp_secs, current_username,
    sha256_hex,
};
use anyhow::{Result, anyhow, bail};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

const BLOB_PATH_SCAN_LIMIT: usize = 12;

/// Reusable imported payload session for fast object-detail queries.
#[derive(Debug)]
pub struct PayloadSession {
    temp_repo: TempBareRepo,
    inspection: BundleInspection,
    payload: PayloadAudit,
    bundle_path: String,
    bundle_size_bytes: u64,
    bundle_sha256: String,
}

/// Collects transport-entry and pack-object payload audit data for a bundle input.
///
/// # Errors
///
/// Returns an error when the bundle input cannot be parsed/imported or objects
/// cannot be enumerated.
#[allow(dead_code)]
pub fn collect_payload_audit_for_bundle_input(
    bundle_input_path: &Path,
    repo_path: &Path,
) -> Result<PayloadAudit> {
    let session = open_payload_session(bundle_input_path, repo_path)?;
    Ok(payload_audit_from_session(&session))
}

/// Builds a serialized payload-audit JSON document for non-interactive CLI output.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails or object-detail
/// materialization fails.
pub fn build_payload_audit_document_for_bundle_input(
    bundle_input_path: &Path,
    repo_path: &Path,
) -> Result<PayloadAuditDocument> {
    let session = open_payload_session(bundle_input_path, repo_path)?;
    payload_audit_document_from_session(&session)
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
    let session = open_payload_session(bundle_input_path, repo_path)?;
    collect_payload_object_detail_for_session(&session, object_id)
}

/// Opens an imported payload session that can be reused across many detail lookups.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails.
pub fn open_payload_session(bundle_input_path: &Path, repo_path: &Path) -> Result<PayloadSession> {
    let source_repo = git2::Repository::open(repo_path)?;
    let source_odb = source_repo.odb()?;
    if is_zip_bundle_input_path(bundle_input_path) {
        let transport_entries = collect_transport_entries_for_zip(bundle_input_path)?;
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let bundle_bytes = fs::read(&extracted.bundle_path)?;
        let bundle_path = extracted
            .bundle_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| extracted.bundle_path.display().to_string());
        let inspection = inspect_bundle(&extracted.bundle_path)?;
        let temp_repo = TempBareRepo::new()?;
        let repo = git2::Repository::open_bare(&temp_repo.path)?;
        import_bundle_pack_to_repo(
            &repo,
            &extracted.bundle_path,
            &inspection,
            Some(&source_odb),
        )?;
        let reachable = collect_reachable_objects(&repo, &inspection.heads)?;
        let context_map = collect_object_context_map(&repo, &inspection.heads)?;
        let objects = collect_payload_objects(&repo, &reachable, &context_map)?;
        let payload = PayloadAudit {
            bundle_version: inspection.version,
            heads: inspection.heads.clone(),
            transport_entries,
            objects,
        };
        return Ok(PayloadSession {
            temp_repo,
            inspection,
            payload,
            bundle_path,
            bundle_size_bytes: bundle_bytes.len() as u64,
            bundle_sha256: sha256_hex(&bundle_bytes)?,
        });
    }

    let transport_entries = collect_transport_entries_for_plain_bundle(bundle_input_path)?;
    let bundle_bytes = fs::read(bundle_input_path)?;
    let bundle_path = bundle_input_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| bundle_input_path.display().to_string());
    let inspection = inspect_bundle(bundle_input_path)?;
    let temp_repo = TempBareRepo::new()?;
    let repo = git2::Repository::open_bare(&temp_repo.path)?;
    import_bundle_pack_to_repo(&repo, bundle_input_path, &inspection, Some(&source_odb))?;
    let reachable = collect_reachable_objects(&repo, &inspection.heads)?;
    let context_map = collect_object_context_map(&repo, &inspection.heads)?;
    let objects = collect_payload_objects(&repo, &reachable, &context_map)?;
    let payload = PayloadAudit {
        bundle_version: inspection.version,
        heads: inspection.heads.clone(),
        transport_entries,
        objects,
    };

    Ok(PayloadSession {
        temp_repo,
        inspection,
        payload,
        bundle_path,
        bundle_size_bytes: bundle_bytes.len() as u64,
        bundle_sha256: sha256_hex(&bundle_bytes)?,
    })
}

/// Returns a payload-audit snapshot captured in the provided session.
pub fn payload_audit_from_session(session: &PayloadSession) -> PayloadAudit {
    session.payload.clone()
}

/// Builds a serialized payload-audit JSON document from a reusable session.
///
/// # Errors
///
/// Returns an error when object detail collection fails for any payload object.
pub fn payload_audit_document_from_session(
    session: &PayloadSession,
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
        pack_summary: summary,
        pack_objects,
        object_details,
    })
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
    let repo = git2::Repository::open_bare(&session.temp_repo.path)?;
    collect_payload_object_detail_for_repo(&repo, &session.inspection, object_id)
}

/// Collects payload detail lines from an already imported repository/inspection pair.
fn collect_payload_object_detail_for_repo(
    repo: &git2::Repository,
    inspection: &BundleInspection,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    let odb = repo.odb()?;
    let (size_bytes, kind) = odb.read_header(object_id)?;
    let kind = payload_kind_from_git(kind);
    let object = repo.find_object(object_id, None)?;
    let detail_lines = object_detail_lines(&object)?;
    let blob_paths = if kind == PayloadObjectKind::Blob {
        collect_blob_paths_with_limit(repo, &inspection.heads, object_id, BLOB_PATH_SCAN_LIMIT)?
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

/// Imports bundle `PACK` payload into a bare repository object database.
fn import_bundle_pack_to_repo(
    repo: &git2::Repository,
    bundle_path: &Path,
    inspection: &BundleInspection,
    resolve_odb: Option<&git2::Odb<'_>>,
) -> Result<()> {
    let bundle_bytes = fs::read(bundle_path)?;
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .ok_or_else(|| anyhow!("bundle does not contain PACK payload"))?;
    let pack_data = &bundle_bytes[pack_offset..];

    let pack_dir = repo.path().join("objects").join("pack");
    fs::create_dir_all(&pack_dir)?;
    let repo_odb = repo.odb()?;
    let indexer_odb = resolve_odb.unwrap_or(&repo_odb);
    let mut indexer = git2::Indexer::new(Some(indexer_odb), &pack_dir, 0o644, true)?;
    indexer.write_all(pack_data)?;
    indexer.commit()?;

    for head in &inspection.heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    Ok(())
}

/// Enumerates all payload objects currently stored in repository object database.
fn collect_payload_objects(
    repo: &git2::Repository,
    reachable: &HashSet<git2::Oid>,
    context_map: &HashMap<git2::Oid, PayloadObjectContext>,
) -> Result<Vec<PayloadObjectEntry>> {
    let odb = repo.odb()?;
    let mut object_ids = Vec::<git2::Oid>::new();
    odb.foreach(|oid| {
        object_ids.push(*oid);
        true
    })?;

    let mut objects = Vec::new();
    for oid in object_ids {
        let (size_bytes, kind) = odb.read_header(oid)?;
        let context = context_map.get(&oid);
        objects.push(PayloadObjectEntry {
            oid,
            kind: payload_kind_from_git(kind),
            size_bytes,
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
    Ok(objects)
}

/// Collects first-seen context metadata for objects while traversing head commit trees.
fn collect_object_context_map(
    repo: &git2::Repository,
    heads: &[crate::git::BundleHead],
) -> Result<HashMap<git2::Oid, PayloadObjectContext>> {
    let mut context = HashMap::<git2::Oid, PayloadObjectContext>::new();
    let mut seen_commits = HashSet::<git2::Oid>::new();
    let mut seen_trees = HashSet::<git2::Oid>::new();

    for (head_index, head) in heads.iter().enumerate() {
        let mut commit_order = 0usize;
        let mut stack = vec![head.oid];
        while let Some(commit_id) = stack.pop() {
            if !seen_commits.insert(commit_id) {
                continue;
            }
            let Ok(commit) = repo.find_commit(commit_id) else {
                continue;
            };
            commit_order += 1;

            context
                .entry(commit.id())
                .or_insert_with(|| PayloadObjectContext {
                    head_index,
                    commit_order,
                    path: None,
                });

            collect_tree_context(
                repo,
                commit.tree_id(),
                "",
                head_index,
                commit_order,
                &mut context,
                &mut seen_trees,
            )?;

            let parents = commit.parent_ids().collect::<Vec<_>>();
            for parent_id in parents.into_iter().rev() {
                if !seen_commits.contains(&parent_id) {
                    stack.push(parent_id);
                }
            }
        }
    }

    Ok(context)
}

/// Recursively records first-seen tree/blob context metadata.
fn collect_tree_context(
    repo: &git2::Repository,
    tree_id: git2::Oid,
    prefix: &str,
    head_index: usize,
    commit_order: usize,
    context: &mut HashMap<git2::Oid, PayloadObjectContext>,
    seen_trees: &mut HashSet<git2::Oid>,
) -> Result<()> {
    if !seen_trees.insert(tree_id) {
        return Ok(());
    }

    context
        .entry(tree_id)
        .or_insert_with(|| PayloadObjectContext {
            head_index,
            commit_order,
            path: if prefix.is_empty() {
                None
            } else {
                Some(prefix.to_string())
            },
        });

    let tree = repo.find_tree(tree_id)?;
    for entry in &tree {
        let name = entry.name().unwrap_or("<invalid-utf8>");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };

        context
            .entry(entry.id())
            .or_insert_with(|| PayloadObjectContext {
                head_index,
                commit_order,
                path: Some(path.clone()),
            });

        if entry.kind() == Some(git2::ObjectType::Tree) {
            collect_tree_context(
                repo,
                entry.id(),
                &path,
                head_index,
                commit_order,
                context,
                seen_trees,
            )?;
        }
    }

    Ok(())
}

/// Collects all objects reachable from bundle heads (commit/tree/blob closure).
fn collect_reachable_objects(
    repo: &git2::Repository,
    heads: &[crate::git::BundleHead],
) -> Result<HashSet<git2::Oid>> {
    let mut reachable = HashSet::new();
    let mut seen_commits = HashSet::new();
    let mut seen_trees = HashSet::new();
    for head in heads {
        mark_commit_reachable(
            repo,
            head.oid,
            &mut reachable,
            &mut seen_commits,
            &mut seen_trees,
        )?;
    }
    Ok(reachable)
}

/// Marks commit graph and associated trees/blobs reachable from a commit id.
fn mark_commit_reachable(
    repo: &git2::Repository,
    commit_id: git2::Oid,
    reachable: &mut HashSet<git2::Oid>,
    seen_commits: &mut HashSet<git2::Oid>,
    seen_trees: &mut HashSet<git2::Oid>,
) -> Result<()> {
    if !seen_commits.insert(commit_id) {
        return Ok(());
    }
    let Ok(commit) = repo.find_commit(commit_id) else {
        return Ok(());
    };
    reachable.insert(commit.id());
    mark_tree_reachable(repo, commit.tree_id(), reachable, seen_trees)?;

    for parent_id in commit.parent_ids() {
        mark_commit_reachable(repo, parent_id, reachable, seen_commits, seen_trees)?;
    }
    Ok(())
}

/// Marks tree entries recursively (tree + blobs) as reachable.
fn mark_tree_reachable(
    repo: &git2::Repository,
    tree_id: git2::Oid,
    reachable: &mut HashSet<git2::Oid>,
    seen_trees: &mut HashSet<git2::Oid>,
) -> Result<()> {
    if !seen_trees.insert(tree_id) {
        return Ok(());
    }
    let Ok(tree) = repo.find_tree(tree_id) else {
        return Ok(());
    };
    reachable.insert(tree.id());
    for entry in &tree {
        reachable.insert(entry.id());
        if entry.kind() == Some(git2::ObjectType::Tree) {
            mark_tree_reachable(repo, entry.id(), reachable, seen_trees)?;
        }
    }
    Ok(())
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

/// Renders object-specific detail lines for payload drill-down/preview view.
fn object_detail_lines(object: &git2::Object<'_>) -> Result<ObjectDetailLines> {
    match object.kind() {
        Some(git2::ObjectType::Commit) => {
            let commit = object.peel_to_commit()?;
            let mut lines = Vec::new();
            lines.push(format!("commit {}", commit.id()));
            lines.push(format!("tree {}", commit.tree_id()));
            for parent in commit.parent_ids() {
                lines.push(format!("parent {parent}"));
            }
            let author = commit.author();
            let committer = commit.committer();
            lines.push(format!(
                "author {} <{}> {} {}",
                author.name().unwrap_or("<unknown>"),
                author.email().unwrap_or("<unknown>"),
                author.when().seconds(),
                author.when().offset_minutes()
            ));
            lines.push(format!(
                "committer {} <{}> {} {}",
                committer.name().unwrap_or("<unknown>"),
                committer.email().unwrap_or("<unknown>"),
                committer.when().seconds(),
                committer.when().offset_minutes()
            ));
            lines.push(String::new());
            lines.extend(
                commit
                    .message()
                    .unwrap_or("<no message>")
                    .lines()
                    .map(str::to_string),
            );
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        Some(git2::ObjectType::Tree) => {
            let tree = object.peel_to_tree()?;
            let mut lines = Vec::new();
            lines.push(format!("tree {}", tree.id()));
            lines.push(String::new());
            for entry in &tree {
                let kind = match entry.kind() {
                    Some(git2::ObjectType::Blob) => "blob",
                    Some(git2::ObjectType::Tree) => "tree",
                    Some(git2::ObjectType::Commit) => "commit",
                    Some(git2::ObjectType::Tag) => "tag",
                    _ => "unknown",
                };
                lines.push(format!(
                    "{:06o} {:<7} {} {}",
                    entry.filemode(),
                    kind,
                    entry.id(),
                    entry.name().unwrap_or("<invalid-utf8>")
                ));
            }
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        Some(git2::ObjectType::Blob) => {
            let blob = object.peel_to_blob()?;
            let bytes = blob.content();
            if let Ok(text) = std::str::from_utf8(bytes) {
                let text_line_count = text.lines().count();
                let mut lines = vec![
                    format!("text blob {}", object.id()),
                    format!("size: {} bytes", bytes.len()),
                    format!("text lines: {text_line_count}"),
                    String::new(),
                ];
                lines.extend(text.lines().map(str::to_string));
                return Ok(ObjectDetailLines {
                    lines,
                    is_text_blob: true,
                    text_line_count: Some(text_line_count),
                });
            }

            let preview_len = bytes.len().min(256);
            let mut lines = vec![
                format!("binary blob {}", object.id()),
                format!("size: {} bytes", bytes.len()),
                format!("hex preview (first {preview_len} bytes):"),
                String::new(),
            ];
            for chunk in bytes[..preview_len].chunks(16) {
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
        Some(git2::ObjectType::Tag) => {
            let tag = object.peel_to_tag()?;
            let mut lines = Vec::new();
            lines.push(format!("tag {}", tag.id()));
            lines.push(format!("name: {}", tag.name().unwrap_or("<unnamed>")));
            lines.push(format!("target: {}", tag.target_id()));
            lines.push(String::new());
            lines.extend(
                tag.message()
                    .unwrap_or("<no message>")
                    .lines()
                    .map(str::to_string),
            );
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        _ => Ok(ObjectDetailLines {
            lines: vec![
                format!("object {}", object.id()),
                "unsupported object type for detail rendering".to_string(),
            ],
            is_text_blob: false,
            text_line_count: None,
        }),
    }
}

/// Collects reachable tree paths for a blob object id, capped at `max_paths`.
fn collect_blob_paths_with_limit(
    repo: &git2::Repository,
    heads: &[crate::git::BundleHead],
    blob_oid: git2::Oid,
    max_paths: usize,
) -> Result<Vec<String>> {
    if max_paths == 0 {
        return Ok(Vec::new());
    }

    let mut seen_trees = HashSet::new();
    let mut seen_commits = HashSet::new();
    let mut commit_stack = heads.iter().map(|head| head.oid).collect::<Vec<_>>();
    let mut paths = Vec::new();

    while let Some(commit_id) = commit_stack.pop() {
        if paths.len() >= max_paths {
            break;
        }
        if !seen_commits.insert(commit_id) {
            continue;
        }
        let Ok(commit) = repo.find_commit(commit_id) else {
            continue;
        };

        collect_blob_paths_in_tree(
            repo,
            commit.tree_id(),
            "",
            blob_oid,
            &mut seen_trees,
            &mut paths,
            max_paths,
        )?;

        for parent_id in commit.parent_ids() {
            if !seen_commits.contains(&parent_id) {
                commit_stack.push(parent_id);
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Recursively searches a tree for all paths that reference a blob object id.
fn collect_blob_paths_in_tree(
    repo: &git2::Repository,
    tree_id: git2::Oid,
    prefix: &str,
    blob_oid: git2::Oid,
    seen_trees: &mut HashSet<git2::Oid>,
    paths: &mut Vec<String>,
    max_paths: usize,
) -> Result<()> {
    if paths.len() >= max_paths {
        return Ok(());
    }
    if !seen_trees.insert(tree_id) {
        return Ok(());
    }
    let tree = repo.find_tree(tree_id)?;

    for entry in &tree {
        if paths.len() >= max_paths {
            break;
        }
        let name = entry.name().unwrap_or("<invalid-utf8>");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(git2::ObjectType::Blob) if entry.id() == blob_oid => paths.push(path),
            Some(git2::ObjectType::Tree) => {
                collect_blob_paths_in_tree(
                    repo,
                    entry.id(),
                    &path,
                    blob_oid,
                    seen_trees,
                    paths,
                    max_paths,
                )?;
            }
            _ => {}
        }
    }

    Ok(())
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

#[derive(Debug)]
struct TempBareRepo {
    path: PathBuf,
}

impl TempBareRepo {
    /// Creates a temporary empty bare repository for payload inspection.
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "git-sync-payload-audit-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| anyhow!("system clock is before unix epoch"))?
                .as_nanos()
        ));
        fs::create_dir_all(&path)?;
        let _repo = git2::Repository::init_bare(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempBareRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
