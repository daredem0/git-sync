//! Git-layer collect functionality.

use crate::git::diff::collect_diff_entries;
use crate::git::types::{
    CreateBundleAuditChangedFile, CreateBundleAuditCommit, CreateBundleAuditSignature,
};
use crate::git::util::status_code;
use anyhow::Result;

/// Collects changed-file entries in the serialized metadata shape.
///
/// # Errors
///
/// Returns an error when diff collection fails for the requested range.
pub(crate) fn collect_changed_files_for_metadata(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<CreateBundleAuditChangedFile>> {
    let diff_entries = collect_diff_entries(repo, base_commit_id, tip_commit_id)?;
    Ok(diff_entries
        .into_iter()
        .map(|entry| CreateBundleAuditChangedFile {
            status: status_code(entry.status).to_string(),
            path: entry.path,
            old_path: entry.old_path,
            old_oid: entry.old_oid.map(|oid| oid.to_string()),
            new_oid: entry.new_oid.map(|oid| oid.to_string()),
            old_mode: entry.old_mode.map(|mode| format!("{mode:06o}")),
            new_mode: entry.new_mode.map(|mode| format!("{mode:06o}")),
            is_binary: entry.is_binary,
        })
        .collect())
}

/// Collects a topologically ordered commit chain for metadata serialization.
///
/// The output order is oldest-to-newest within the audited range.
///
/// # Errors
///
/// Returns an error when rev-walk setup or commit lookup fails.
pub(crate) fn collect_commit_chain_for_metadata(
    repo: &git2::Repository,
    from_commit_id: git2::Oid,
    to_commit_id: git2::Oid,
) -> Result<Vec<CreateBundleAuditCommit>> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;
    walk.push(to_commit_id)?;
    walk.hide(from_commit_id)?;

    let mut commit_chain = Vec::new();
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let parent_oids = commit
            .parent_ids()
            .map(|parent| parent.to_string())
            .collect();

        commit_chain.push(CreateBundleAuditCommit {
            oid: commit.id().to_string(),
            tree_oid: commit.tree_id().to_string(),
            parent_oids,
            subject: commit.summary().unwrap_or("").to_string(),
            author: signature_to_audit_signature(commit.author()),
            committer: signature_to_audit_signature(commit.committer()),
        });
    }

    Ok(commit_chain)
}

/// Converts a libgit2 signature into the serialized audit-signature shape.
pub(crate) fn signature_to_audit_signature(
    signature: git2::Signature<'_>,
) -> CreateBundleAuditSignature {
    let timestamp = signature.when();
    CreateBundleAuditSignature {
        name: signature.name().unwrap_or("").to_string(),
        email: signature.email().unwrap_or("").to_string(),
        time_seconds: timestamp.seconds(),
        offset_minutes: timestamp.offset_minutes(),
    }
}
