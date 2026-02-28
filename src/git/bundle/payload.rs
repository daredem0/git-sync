//! Git-layer payload audit functionality.

use super::inspect::inspect_bundle;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::types::{
    BundleInspection, PayloadAudit, PayloadObjectDetail, PayloadObjectEntry, PayloadObjectKind,
    PayloadTransportEntry,
};
use crate::git::util::sha256_hex;
use anyhow::{Result, anyhow, bail};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

/// Collects transport-entry and pack-object payload audit data for a bundle input.
///
/// # Errors
///
/// Returns an error when the bundle input cannot be parsed/imported or objects
/// cannot be enumerated.
pub fn collect_payload_audit_for_bundle_input(
    bundle_input_path: &Path,
    repo_path: &Path,
) -> Result<PayloadAudit> {
    with_imported_payload_repo(
        bundle_input_path,
        repo_path,
        |repo, inspection, transport_entries| {
            let reachable = collect_reachable_objects(repo, &inspection.heads)?;
            let objects = collect_payload_objects(repo, &reachable)?;
            Ok(PayloadAudit {
                bundle_version: inspection.version,
                heads: inspection.heads.clone(),
                transport_entries: transport_entries.to_vec(),
                objects,
            })
        },
    )
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
    with_imported_payload_repo(
        bundle_input_path,
        repo_path,
        |repo, inspection, _transport_entries| {
            let odb = repo.odb()?;
            let (size_bytes, kind) = odb.read_header(object_id)?;
            let kind = payload_kind_from_git(kind);
            let object = repo.find_object(object_id, None)?;
            let (lines, is_text_blob) = object_detail_lines(&object)?;
            let syntax_path_hint = if is_text_blob {
                find_blob_path_hint(repo, &inspection.heads, object_id)?
                    .or_else(|| Some("blob.txt".to_string()))
            } else {
                None
            };

            Ok(PayloadObjectDetail {
                oid: object_id,
                kind,
                size_bytes,
                syntax_path_hint,
                lines,
            })
        },
    )
}

/// Executes a callback against a temporary bare repository containing only imported bundle payload objects.
fn with_imported_payload_repo<T>(
    bundle_input_path: &Path,
    repo_path: &Path,
    func: impl FnOnce(&git2::Repository, &BundleInspection, &[PayloadTransportEntry]) -> Result<T>,
) -> Result<T> {
    let source_repo = git2::Repository::open(repo_path)?;
    let source_odb = source_repo.odb()?;
    if is_zip_bundle_input_path(bundle_input_path) {
        let transport_entries = collect_transport_entries_for_zip(bundle_input_path)?;
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let inspection = inspect_bundle(&extracted.bundle_path)?;
        let temp_repo = TempBareRepo::new()?;
        let repo = git2::Repository::open_bare(&temp_repo.path)?;
        import_bundle_pack_to_repo(
            &repo,
            &extracted.bundle_path,
            &inspection,
            Some(&source_odb),
        )?;
        return func(&repo, &inspection, &transport_entries);
    }

    let transport_entries = collect_transport_entries_for_plain_bundle(bundle_input_path)?;
    let inspection = inspect_bundle(bundle_input_path)?;
    let temp_repo = TempBareRepo::new()?;
    let repo = git2::Repository::open_bare(&temp_repo.path)?;
    import_bundle_pack_to_repo(&repo, bundle_input_path, &inspection, Some(&source_odb))?;
    func(&repo, &inspection, &transport_entries)
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
        objects.push(PayloadObjectEntry {
            oid,
            kind: payload_kind_from_git(kind),
            size_bytes,
            reachable_from_heads: reachable.contains(&oid),
        });
    }

    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });
    Ok(objects)
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

/// Renders object-specific detail lines for payload drill-down view.
///
/// Returns `(lines, is_text_blob)` where `is_text_blob` is `true` only for
/// UTF-8 blob content that can be syntax highlighted.
fn object_detail_lines(object: &git2::Object<'_>) -> Result<(Vec<String>, bool)> {
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
            Ok((lines, false))
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
            Ok((lines, false))
        }
        Some(git2::ObjectType::Blob) => {
            let blob = object.peel_to_blob()?;
            let bytes = blob.content();
            if let Ok(text) = std::str::from_utf8(bytes) {
                let mut lines = vec![
                    format!("text blob {}", object.id()),
                    format!("size: {} bytes", bytes.len()),
                    String::new(),
                ];
                lines.extend(text.lines().map(str::to_string));
                return Ok((lines, true));
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
            Ok((lines, false))
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
            Ok((lines, false))
        }
        _ => Ok((
            vec![
                format!("object {}", object.id()),
                "unsupported object type for detail rendering".to_string(),
            ],
            false,
        )),
    }
}

/// Finds one reachable tree path for a blob object id to use as syntax hint.
fn find_blob_path_hint(
    repo: &git2::Repository,
    heads: &[crate::git::BundleHead],
    blob_oid: git2::Oid,
) -> Result<Option<String>> {
    let mut seen_trees = HashSet::new();
    for head in heads {
        let Ok(commit) = repo.find_commit(head.oid) else {
            continue;
        };
        if let Some(path) =
            find_blob_in_tree(repo, commit.tree_id(), "", blob_oid, &mut seen_trees)?
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Recursively searches a tree for a blob object id and returns first matching path.
fn find_blob_in_tree(
    repo: &git2::Repository,
    tree_id: git2::Oid,
    prefix: &str,
    blob_oid: git2::Oid,
    seen_trees: &mut HashSet<git2::Oid>,
) -> Result<Option<String>> {
    if !seen_trees.insert(tree_id) {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id)?;

    for entry in &tree {
        let name = entry.name().unwrap_or("<invalid-utf8>");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(git2::ObjectType::Blob) if entry.id() == blob_oid => return Ok(Some(path)),
            Some(git2::ObjectType::Tree) => {
                if let Some(found) =
                    find_blob_in_tree(repo, entry.id(), &path, blob_oid, seen_trees)?
                {
                    return Ok(Some(found));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

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
