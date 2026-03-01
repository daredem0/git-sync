//! Git-layer diff functionality.

use crate::git::ChangeStatus;
use crate::git::types::DiffEntry;
use crate::git::util::{oid_or_none, path_to_string};
use anyhow::{Result, bail};

/// Collects rich diff entries including mode/binary metadata.
///
/// The output is path-sorted for deterministic audit artifact generation.
///
/// # Errors
///
/// Returns an error when commits/trees cannot be loaded or when an unsupported
/// diff delta kind is encountered.
pub(crate) fn collect_diff_entries(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<DiffEntry>> {
    let base_commit = repo.find_commit(base_commit_id)?;
    let tip_commit = repo.find_commit(tip_commit_id)?;
    let base_tree = base_commit.tree()?;
    let tip_tree = tip_commit.tree()?;

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.include_typechange(true);
    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), Some(&mut diff_opts))?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut entries = Vec::new();
    for delta in diff.deltas() {
        let old_file = delta.old_file();
        let new_file = delta.new_file();

        let (status, path, old_path) = match delta.status() {
            git2::Delta::Unmodified => continue,
            git2::Delta::Added => (ChangeStatus::Added, path_to_string(new_file.path())?, None),
            git2::Delta::Modified => (
                ChangeStatus::Modified,
                path_to_string(new_file.path().or(old_file.path()))?,
                None,
            ),
            git2::Delta::Deleted => (
                ChangeStatus::Deleted,
                path_to_string(old_file.path())?,
                None,
            ),
            git2::Delta::Renamed => (
                ChangeStatus::Renamed,
                path_to_string(new_file.path())?,
                Some(path_to_string(old_file.path())?),
            ),
            git2::Delta::Copied => (
                ChangeStatus::Copied,
                path_to_string(new_file.path())?,
                Some(path_to_string(old_file.path())?),
            ),
            git2::Delta::Typechange => (
                ChangeStatus::TypeChanged,
                path_to_string(new_file.path().or(old_file.path()))?,
                None,
            ),
            other => bail!("unsupported diff delta status for tree diff: {other:?}"),
        };

        let is_binary = old_file.is_binary() || new_file.is_binary();
        entries.push(DiffEntry {
            status,
            path,
            old_path,
            old_oid: oid_or_none(old_file.id()),
            new_oid: oid_or_none(new_file.id()),
            old_mode: old_file.exists().then(|| u32::from(old_file.mode())),
            new_mode: new_file.exists().then(|| u32::from(new_file.mode())),
            is_binary,
        });
    }

    // Deterministic order is required for stable audit artifacts.
    entries.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.old_path.cmp(&b.old_path))
    });
    Ok(entries)
}
