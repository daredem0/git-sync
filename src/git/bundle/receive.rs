//! Git-layer receive functionality.

use super::inspect::inspect_bundle;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::metadata::verify_bundle_metadata_integrity_input;
use crate::git::util::path_to_string;
use crate::git::{
    BundleHead, BundleInspection, CommitAuditEntry, CommitAuditIdentity, FileLineStat,
    ReceiveBundleOptions, ReceiveBundleResult,
};
use anyhow::{Result, anyhow, bail};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Receives a bundle input using default receive options.
///
/// Equivalent to calling [`receive_bundle_input_with_options`] with
/// [`ReceiveBundleOptions::default`].
///
/// # Errors
///
/// Returns an error when bundle parsing/import fails.
pub fn receive_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options(
        bundle_input_path,
        receiver_repo_path,
        ReceiveBundleOptions::default(),
    )
}

/// Receives a bundle input (`.bundle` or packaged `.zip`) into a repository.
///
/// When `dry_run` is enabled, import and diff analysis run against a temporary
/// bare mirror and do not mutate the receiver.
///
/// # Errors
///
/// Returns an error when metadata verification fails (if enabled), archive
/// extraction fails, or bundle import cannot be applied.
pub fn receive_bundle_input_with_options(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
) -> Result<ReceiveBundleResult> {
    if options.verify_metadata {
        verify_bundle_metadata_integrity_input(bundle_input_path)?;
    }

    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        receive_bundle(&extracted.bundle_path, receiver_repo_path, options.dry_run)
    } else {
        receive_bundle(bundle_input_path, receiver_repo_path, options.dry_run)
    }
}

/// Collects commit-level audit entries for a bundle input.
///
/// This imports into a temporary mirror and computes commit/file summaries
/// without mutating the receiver repository.
///
/// # Errors
///
/// Returns an error when bundle inspection/import or commit traversal fails.
pub fn collect_commit_audit_entries_for_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<Vec<CommitAuditEntry>> {
    with_imported_bundle_input_repo(bundle_input_path, receiver_repo_path, |repo, inspection| {
        collect_commit_audit_entries(repo, inspection)
    })
}

/// Collects a unified patch for one file in a bundle commit.
///
/// # Errors
///
/// Returns an error when the commit/path is unavailable, when the file is not
/// changed in the target commit, or when a textual patch is unavailable.
pub fn collect_commit_file_patch_for_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    commit_id: git2::Oid,
    path: &str,
) -> Result<String> {
    with_imported_bundle_input_repo(
        bundle_input_path,
        receiver_repo_path,
        |repo, _inspection| collect_commit_file_patch(repo, commit_id, path),
    )
}

/// Applies a bundle to the receiver repository or to a dry-run mirror.
fn receive_bundle(
    bundle_path: &Path,
    receiver_repo_path: &Path,
    dry_run: bool,
) -> Result<ReceiveBundleResult> {
    let inspection = inspect_bundle(bundle_path)?;
    if inspection.heads.is_empty() {
        bail!("bundle does not contain any heads to import");
    }

    let repo = git2::Repository::open(receiver_repo_path)?;
    if inspection
        .heads
        .iter()
        .map(|head| is_head_already_applied(&repo, head))
        .collect::<Result<Vec<bool>>>()?
        .into_iter()
        .all(std::convert::identity)
    {
        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts: true,
            line_stats: Vec::new(),
        });
    }

    if dry_run {
        // Dry-run operates on a temporary mirror so we can safely import and diff.
        let temp_repo = TempBareRepo::from_existing(receiver_repo_path)?;
        let dry_run_repo = git2::Repository::open_bare(&temp_repo.path)?;
        apply_bundle_to_repo(&dry_run_repo, bundle_path, &inspection.heads)?;
        let line_stats = collect_bundle_line_stats(&dry_run_repo, &inspection)?;

        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts: true,
            line_stats,
        });
    }

    apply_bundle_to_repo(&repo, bundle_path, &inspection.heads)?;

    Ok(ReceiveBundleResult {
        bundle_version: inspection.version,
        imported_heads: inspection.heads,
        can_apply_without_conflicts: true,
        line_stats: Vec::new(),
    })
}

/// Imports a bundle PACK stream into the repository object database and refs.
///
/// # Errors
///
/// Returns an error when the PACK payload cannot be located/imported, when
/// imported head commits are missing, or ref updates fail.
fn apply_bundle_to_repo(
    repo: &git2::Repository,
    bundle_path: &Path,
    heads: &[BundleHead],
) -> Result<()> {
    let bundle_bytes = fs::read(bundle_path)?;
    // Git bundle payload starts at the embedded PACK stream.
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .ok_or_else(|| anyhow!("bundle does not contain PACK payload"))?;
    let pack_data = &bundle_bytes[pack_offset..];

    let odb = repo.odb()?;
    let pack_dir = repo.path().join("objects").join("pack");
    fs::create_dir_all(&pack_dir)?;
    let mut indexer = git2::Indexer::new(Some(&odb), &pack_dir, 0o644, true)?;
    indexer.write_all(pack_data)?;
    indexer.commit()?;

    for head in heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    for head in heads {
        if is_head_already_applied(&repo, head)? {
            continue;
        }
        repo.reference(&head.reference, head.oid, true, "receive bundle import")?;
    }

    Ok(())
}

/// Aggregates per-file line deltas across all imported heads.
///
/// Aggregation is keyed by path and sums additions/deletions from each head.
fn collect_bundle_line_stats(
    repo: &git2::Repository,
    inspection: &BundleInspection,
) -> Result<Vec<FileLineStat>> {
    let mut aggregated = std::collections::BTreeMap::<String, (usize, usize)>::new();

    for head in &inspection.heads {
        let stats_for_head = collect_line_stats_for_head(repo, head, &inspection.prerequisites)?;
        for stat in stats_for_head {
            let entry = aggregated.entry(stat.path).or_insert((0, 0));
            entry.0 += stat.additions;
            entry.1 += stat.deletions;
        }
    }

    Ok(aggregated
        .into_iter()
        .map(|(path, (additions, deletions))| FileLineStat {
            path,
            additions,
            deletions,
        })
        .collect())
}

/// Computes per-file line stats for a single imported head.
///
/// Non-text changes are represented as `0/0` line deltas.
fn collect_line_stats_for_head(
    repo: &git2::Repository,
    head: &BundleHead,
    prerequisites: &[git2::Oid],
) -> Result<Vec<FileLineStat>> {
    let head_commit = repo.find_commit(head.oid)?;
    let tip_tree = head_commit.tree()?;
    let base_tree = resolve_base_tree_for_head(repo, &head_commit, prerequisites)?;

    let mut diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut stats = Vec::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = path_to_string(delta.new_file().path().or(delta.old_file().path()))?;
        let (additions, deletions) = if is_non_text_delta(&delta) {
            (0, 0)
        } else {
            match git2::Patch::from_diff(&diff, index)? {
                Some(patch) => {
                    let (_, additions, deletions) = patch.line_stats()?;
                    (additions, deletions)
                }
                None => (0, 0),
            }
        };
        stats.push(FileLineStat {
            path,
            additions,
            deletions,
        });
    }
    Ok(stats)
}

/// Builds commit-page entries from imported bundle objects.
fn collect_commit_audit_entries(
    repo: &git2::Repository,
    inspection: &BundleInspection,
) -> Result<Vec<CommitAuditEntry>> {
    let commit_ids = collect_imported_commit_ids(repo, inspection)?;
    let mut entries = Vec::new();
    for commit_id in commit_ids {
        let commit = repo.find_commit(commit_id)?;
        let tip_tree = commit.tree()?;
        let base_tree = if commit.parent_count() == 0 {
            None
        } else {
            Some(commit.parent(0)?.tree()?)
        };

        let files = collect_line_stats_for_tree_diff(repo, base_tree.as_ref(), &tip_tree)?;
        let committer = commit.committer();
        let author = commit.author();
        entries.push(CommitAuditEntry {
            commit_id,
            subject: commit
                .summary()
                .map(std::string::ToString::to_string)
                .unwrap_or_else(|| "<no subject>".to_string()),
            committer: CommitAuditIdentity {
                name: committer.name().unwrap_or("<unknown>").to_string(),
                email: committer.email().unwrap_or("<unknown>").to_string(),
                time_seconds: committer.when().seconds(),
                offset_minutes: committer.when().offset_minutes(),
            },
            author: CommitAuditIdentity {
                name: author.name().unwrap_or("<unknown>").to_string(),
                email: author.email().unwrap_or("<unknown>").to_string(),
                time_seconds: author.when().seconds(),
                offset_minutes: author.when().offset_minutes(),
            },
            files,
        });
    }

    Ok(entries)
}

/// Enumerates commits carried by imported bundle heads, excluding prerequisites.
///
/// Returned OIDs are ordered oldest-first for stable page progression.
fn collect_imported_commit_ids(
    repo: &git2::Repository,
    inspection: &BundleInspection,
) -> Result<Vec<git2::Oid>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME | git2::Sort::REVERSE)?;

    let mut heads = inspection.heads.clone();
    heads.sort_by(|left, right| {
        left.reference
            .cmp(&right.reference)
            .then_with(|| left.oid.cmp(&right.oid))
    });
    for head in heads {
        revwalk.push(head.oid)?;
    }
    for prerequisite in &inspection.prerequisites {
        revwalk.hide(*prerequisite)?;
    }

    let mut commits = Vec::new();
    for oid_result in revwalk {
        commits.push(oid_result?);
    }
    Ok(commits)
}

/// Returns a textual patch for one file in a single commit.
///
/// # Errors
///
/// Returns an error for missing commits/paths, non-text changes, or when the
/// file is not part of the commit diff.
fn collect_commit_file_patch(
    repo: &git2::Repository,
    commit_id: git2::Oid,
    path: &str,
) -> Result<String> {
    let commit = repo.find_commit(commit_id)?;
    let tip_tree = commit.tree()?;
    let base_tree = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };

    let mut diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    for (index, delta) in diff.deltas().enumerate() {
        let old_path = path_to_string(delta.old_file().path())?;
        let new_path = path_to_string(delta.new_file().path())?;
        if old_path != path && new_path != path {
            continue;
        }
        if is_non_text_delta(&delta) {
            bail!("textual diff unavailable for non-text path '{path}'");
        }

        let patch = git2::Patch::from_diff(&diff, index)?;
        let Some(mut patch) = patch else {
            return Ok(format!(
                "diff --git a/{old_path} b/{new_path}\nBinary file changed; textual diff unavailable.\n"
            ));
        };

        let patch_buf = patch.to_buf()?;
        let patch_text = String::from_utf8_lossy(patch_buf.as_ref()).to_string();
        return Ok(patch_text);
    }

    bail!("file '{path}' is not changed in commit '{commit_id}'")
}

/// Runs a callback against a temporary repo with the bundle input imported.
///
/// This isolates read/analysis operations from the live receiver repository.
fn with_imported_bundle_input_repo<T>(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    func: impl FnOnce(&git2::Repository, &BundleInspection) -> Result<T>,
) -> Result<T> {
    // Analysis helpers run against a temporary imported repo to avoid mutating the receiver.
    let temp_repo = TempBareRepo::from_existing(receiver_repo_path)?;
    let repo = git2::Repository::open_bare(&temp_repo.path)?;

    let inspection = if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let inspection = inspect_bundle(&extracted.bundle_path)?;
        apply_bundle_to_repo(&repo, &extracted.bundle_path, &inspection.heads)?;
        inspection
    } else {
        let inspection = inspect_bundle(bundle_input_path)?;
        apply_bundle_to_repo(&repo, bundle_input_path, &inspection.heads)?;
        inspection
    };

    func(&repo, &inspection)
}

/// Computes line stats for a direct tree-to-tree diff.
///
/// Returned rows are path-sorted for stable UI rendering.
fn collect_line_stats_for_tree_diff(
    repo: &git2::Repository,
    base_tree: Option<&git2::Tree<'_>>,
    tip_tree: &git2::Tree<'_>,
) -> Result<Vec<FileLineStat>> {
    let mut diff = repo.diff_tree_to_tree(base_tree, Some(tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut stats = Vec::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = path_to_string(delta.new_file().path().or(delta.old_file().path()))?;
        let (additions, deletions) = if is_non_text_delta(&delta) {
            (0, 0)
        } else {
            match git2::Patch::from_diff(&diff, index)? {
                Some(patch) => {
                    let (_, additions, deletions) = patch.line_stats()?;
                    (additions, deletions)
                }
                None => (0, 0),
            }
        };
        stats.push(FileLineStat {
            path,
            additions,
            deletions,
        });
    }

    stats.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(stats)
}

/// Resolves the baseline tree used for per-head dry-run diffing.
///
/// For multi-prerequisite bundles, this requires exactly one prerequisite that
/// is an ancestor of the head.
fn resolve_base_tree_for_head<'repo>(
    repo: &'repo git2::Repository,
    head_commit: &git2::Commit<'repo>,
    prerequisites: &[git2::Oid],
) -> Result<Option<git2::Tree<'repo>>> {
    if prerequisites.is_empty() {
        return if head_commit.parent_count() == 0 {
            Ok(None)
        } else {
            Ok(Some(head_commit.parent(0)?.tree()?))
        };
    }

    if prerequisites.len() == 1 {
        let base_commit = repo.find_commit(prerequisites[0])?;
        return Ok(Some(base_commit.tree()?));
    }

    let mut matching_prerequisites = Vec::new();
    for prerequisite in prerequisites {
        if repo.graph_descendant_of(head_commit.id(), *prerequisite)? {
            matching_prerequisites.push(*prerequisite);
        }
    }

    if matching_prerequisites.len() != 1 {
        bail!(
            "unable to determine unique dry-run base for head '{}' with {} prerequisites",
            head_commit.id(),
            prerequisites.len()
        );
    }

    let base_commit = repo.find_commit(matching_prerequisites[0])?;
    Ok(Some(base_commit.tree()?))
}

/// Returns `true` when the referenced head already points at an imported commit.
///
/// This guards repeated receive operations from rewriting unchanged refs.
pub(crate) fn is_head_already_applied(repo: &git2::Repository, head: &BundleHead) -> Result<bool> {
    let current_target = match repo.find_reference(&head.reference) {
        Ok(reference) => reference.target().or_else(|| {
            reference
                .resolve()
                .ok()
                .and_then(|resolved| resolved.target())
        }),
        Err(err) if err.code() == git2::ErrorCode::NotFound => None,
        Err(err) => return Err(err.into()),
    };

    let Some(current_target) = current_target else {
        return Ok(false);
    };
    if current_target != head.oid {
        return Ok(false);
    }

    Ok(repo.find_commit(head.oid).is_ok())
}

/// Returns `true` when a diff delta is not a regular text-file change.
fn is_non_text_delta(delta: &git2::DiffDelta<'_>) -> bool {
    let old_file = delta.old_file();
    let new_file = delta.new_file();

    if old_file.is_binary() || new_file.is_binary() {
        return true;
    }

    let old_mode = u32::from(old_file.mode());
    let new_mode = u32::from(new_file.mode());

    let old_regular = !old_file.exists() || is_regular_blob_mode(old_mode);
    let new_regular = !new_file.exists() || is_regular_blob_mode(new_mode);

    !(old_regular && new_regular)
}

/// Returns `true` for standard git regular-file modes.
fn is_regular_blob_mode(mode: u32) -> bool {
    mode == 0o100644 || mode == 0o100755
}

struct TempBareRepo {
    path: PathBuf,
}

impl TempBareRepo {
    /// Creates a temporary bare mirror of the receiver repository.
    ///
    /// The mirror is populated via anonymous remote fetch and deleted on drop.
    fn from_existing(source_repo_path: &Path) -> Result<Self> {
        let temp_path = std::env::temp_dir().join(format!(
            "git-sync-receive-dry-run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| anyhow!("system clock is before unix epoch"))?
                .as_nanos()
        ));
        fs::create_dir_all(&temp_path)?;

        let repo = git2::Repository::init_bare(&temp_path)?;
        let source = source_repo_path.to_string_lossy();
        let mut remote = repo.remote_anonymous(source.as_ref())?;
        remote.fetch(&["+refs/*:refs/*"], None, None)?;

        Ok(Self { path: temp_path })
    }
}

impl Drop for TempBareRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
