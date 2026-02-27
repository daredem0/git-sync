//! Git-layer range functionality.

use crate::git::RepoAuditRange;
use anyhow::{Result, bail};
use std::path::Path;

/// Resolves a linear commit range from two revision expressions.
///
/// The returned range is valid only when `to_rev` equals `from_rev` or is a
/// descendant of it.
///
/// # Errors
///
/// Returns an error when the repository cannot be opened, either revision
/// cannot be resolved to a commit, or the range is non-linear.
pub fn resolve_repo_audit_range(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
) -> Result<RepoAuditRange> {
    let repo = git2::Repository::open(repo_path)?;

    let base_obj = repo.revparse_single(from_rev)?;
    let base_commit = base_obj.peel_to_commit()?;
    let base_commit_id = base_commit.id();

    let tip_obj = repo.revparse_single(to_rev)?;
    let tip_commit = tip_obj.peel_to_commit()?;
    let tip_commit_id = tip_commit.id();

    if tip_commit_id != base_commit_id
        && !repo.graph_descendant_of(tip_commit_id, base_commit_id)?
    {
        bail!(
            "to rev '{}' must be the same commit as from rev '{}' or a descendant of it",
            to_rev,
            from_rev
        );
    }

    Ok(RepoAuditRange {
        base_commit_id,
        tip_commit_id,
    })
}
