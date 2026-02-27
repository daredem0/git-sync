use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleVersion {
    V2,
    V3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleHead {
    pub oid: git2::Oid,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleInspection {
    pub version: BundleVersion,
    pub prerequisites: Vec<git2::Oid>,
    pub heads: Vec<BundleHead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenContext {
    pub base_commit_id: git2::Oid,
    pub tip_commit_id: Option<git2::Oid>,
    pub bundle_version: BundleVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub status: ChangeStatus,
    pub path: String,
    pub old_path: Option<String>,
    pub old_oid: Option<git2::Oid>,
    pub new_oid: Option<git2::Oid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBundleResult {
    pub from_commit_id: git2::Oid,
    pub to_commit_id: git2::Oid,
    pub tip_ref_name: String,
    pub bundle_path: PathBuf,
    pub audit_path: PathBuf,
    pub patch_audit_path: Option<PathBuf>,
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoAuditRange {
    pub base_commit_id: git2::Oid,
    pub tip_commit_id: git2::Oid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveBundleResult {
    pub bundle_version: BundleVersion,
    pub imported_heads: Vec<BundleHead>,
    pub can_apply_without_conflicts: bool,
    pub line_stats: Vec<FileLineStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLineStat {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitAuditEntry {
    pub commit_id: git2::Oid,
    pub subject: String,
    pub committer: CommitAuditIdentity,
    pub author: CommitAuditIdentity,
    pub files: Vec<FileLineStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitAuditIdentity {
    pub name: String,
    pub email: String,
    pub time_seconds: i64,
    pub offset_minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CreateBundleOptions {
    pub include_patch_sidecar: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReceiveBundleOptions {
    pub verify_metadata: bool,
    pub dry_run: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffEntry {
    pub(crate) status: ChangeStatus,
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
    pub(crate) old_oid: Option<git2::Oid>,
    pub(crate) new_oid: Option<git2::Oid>,
    pub(crate) old_mode: Option<u32>,
    pub(crate) new_mode: Option<u32>,
    pub(crate) is_binary: bool,
}
