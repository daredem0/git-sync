//! Git-layer patch functionality.

use crate::git::archive::patch_sidecar_path;
use crate::git::digest::sha256_hex;
use crate::git::types::CreateBundleAuditPatchSidecar;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Writes a unified-diff patch sidecar for the audited commit range.
///
/// # Errors
///
/// Returns an error when commit/tree diffing, sidecar writing, or hash
/// computation fails.
pub(crate) fn write_patch_sidecar(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
    bundle_path: &Path,
) -> Result<CreateBundleAuditPatchSidecar> {
    let base_commit = repo.find_commit(base_commit_id)?;
    let tip_commit = repo.find_commit(tip_commit_id)?;
    let base_tree = base_commit.tree()?;
    let tip_tree = tip_commit.tree()?;

    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut patch_bytes = Vec::<u8>::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        patch_bytes.extend_from_slice(line.content());
        true
    })?;

    let patch_path = patch_sidecar_path(bundle_path);
    fs::write(&patch_path, &patch_bytes)?;

    Ok(CreateBundleAuditPatchSidecar {
        path: patch_path.display().to_string(),
        format: "unified-diff".to_string(),
        size_bytes: patch_bytes.len() as u64,
        sha256: sha256_hex(&patch_bytes)?,
    })
}
