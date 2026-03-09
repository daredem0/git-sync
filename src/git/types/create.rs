// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for create domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Output paths and commit metadata produced by bundle creation.
pub struct CreateBundleResult {
    /// Range start commit encoded as bundle prerequisite.
    pub from_commit_id: git2::Oid,
    /// Range end commit encoded as bundle head target.
    pub to_commit_id: git2::Oid,
    /// Head reference name recorded in the bundle header.
    pub tip_ref_name: String,
    /// Generated raw bundle path.
    pub bundle_path: PathBuf,
    /// Generated metadata sidecar path.
    pub audit_path: PathBuf,
    /// Optional unified-diff sidecar path.
    pub patch_audit_path: Option<PathBuf>,
    /// Final packaged archive path that contains produced artifacts.
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Options controlling bundle creation behavior.
pub struct CreateBundleOptions {
    /// When true, emit a `.caudit.patch` unified-diff sidecar.
    pub include_patch_sidecar: bool,
    /// Extra revisions assumed to already exist at the destination.
    pub assume_present_revs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditMetadata {
    pub(crate) schema_version: String,
    pub(crate) tool_version: String,
    pub(crate) generated_at_unix_secs: u64,
    pub(crate) generated_by_username: String,
    pub(crate) generated_by_hostname: String,
    pub(crate) bundle_path: String,
    pub(crate) bundle_size_bytes: u64,
    pub(crate) bundle_sha256: String,
    pub(crate) bundle_header_version: String,
    pub(crate) prerequisites: Vec<String>,
    pub(crate) heads: Vec<CreateBundleAuditHead>,
    pub(crate) range_from_oid: String,
    pub(crate) range_to_oid: String,
    pub(crate) tip_ref: String,
    pub(crate) commit_chain: Vec<CreateBundleAuditCommit>,
    pub(crate) changed_files: Vec<CreateBundleAuditChangedFile>,
    pub(crate) patch_sidecar: Option<CreateBundleAuditPatchSidecar>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditHead {
    pub(crate) oid: String,
    pub(crate) reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditCommit {
    pub(crate) oid: String,
    pub(crate) tree_oid: String,
    pub(crate) parent_oids: Vec<String>,
    pub(crate) subject: String,
    pub(crate) author: CreateBundleAuditSignature,
    pub(crate) committer: CreateBundleAuditSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditSignature {
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) time_seconds: i64,
    pub(crate) offset_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditChangedFile {
    pub(crate) status: String,
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
    pub(crate) old_oid: Option<String>,
    pub(crate) new_oid: Option<String>,
    pub(crate) old_mode: Option<String>,
    pub(crate) new_mode: Option<String>,
    pub(crate) is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditPatchSidecar {
    pub(crate) path: String,
    pub(crate) format: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}
