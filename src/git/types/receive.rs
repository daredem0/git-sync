// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for receive domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use super::{BundleHead, BundleVersion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Integration policy applied to target refs during receive.
pub enum ReceiveIntegratePolicy {
    /// Only create/update incoming namespace refs; never touch target refs.
    CreateRefsOnly,
    /// Update target refs only when updates are strict fast-forwards.
    #[default]
    FastForwardOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Result of receiving a bundle or running receive in dry-run mode.
pub struct ReceiveBundleResult {
    /// Detected version of the imported bundle.
    pub bundle_version: BundleVersion,
    /// Heads that were (or would be) imported.
    pub imported_heads: Vec<BundleHead>,
    /// Whether the import can be applied cleanly.
    pub can_apply_without_conflicts: bool,
    /// Backend used for target-ref updates during non-dry-run receive.
    pub apply_backend: Option<ReceiveApplyBackend>,
    /// Deterministic preflight integration plan for each imported head.
    pub preflight_plan: Vec<ReceivePlanEntry>,
    /// Per-file additions/deletions produced during dry-run analysis.
    pub line_stats: Vec<FileLineStat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Execution backend used for applying receive target-ref updates.
pub enum ReceiveApplyBackend {
    /// Uses libgit2 reference transactions.
    RefTransaction,
    /// Uses sequential compare-and-swap updates with rollback-on-failure.
    ManualCasRollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Preflight integration status for one incoming head.
pub enum ReceivePlanStatus {
    /// Target ref already points to the incoming object.
    AlreadyPresent,
    /// Target ref does not currently exist.
    TargetMissing,
    /// Target ref can be advanced via strict fast-forward.
    FastForwardOk,
    /// Target and incoming have diverged; manual merge is required.
    DivergedMergeRequired,
}

impl ReceivePlanStatus {
    /// Returns a stable machine-friendly label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyPresent => "already_present",
            Self::TargetMissing => "target_missing",
            Self::FastForwardOk => "fast_forward_ok",
            Self::DivergedMergeRequired => "diverged_merge_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One row in the receive preflight integration plan.
pub struct ReceivePlanEntry {
    /// Target ref that may be updated depending on integration policy.
    pub target_ref: String,
    /// Current target OID before receive, if the ref exists.
    pub target_oid: Option<git2::Oid>,
    /// Incoming OID imported from the bundle head.
    pub incoming_oid: git2::Oid,
    /// Merge-base between target and incoming, when both are resolvable.
    pub merge_base_oid: Option<git2::Oid>,
    /// Preserve-location for manual follow-up.
    pub preserved_incoming_ref: String,
    /// Computed integration status.
    pub status: ReceivePlanStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Compact per-file line-delta summary.
pub struct FileLineStat {
    /// Repository-relative file path.
    pub path: String,
    /// Number of added lines.
    pub additions: usize,
    /// Number of deleted lines.
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Commit-level entry used by commit pages in the TUI.
pub struct CommitAuditEntry {
    /// Commit object ID.
    pub commit_id: git2::Oid,
    /// First line of commit message.
    pub subject: String,
    /// Committer identity/time metadata.
    pub committer: CommitAuditIdentity,
    /// Author identity/time metadata.
    pub author: CommitAuditIdentity,
    /// Changed files summary for this commit.
    pub files: Vec<FileLineStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Head-scoped commit and line-stat entries used by multi-head UI views.
pub struct HeadAuditEntry {
    /// Bundle head this entry represents.
    pub head: BundleHead,
    /// Per-head dry-run file summary.
    pub line_stats: Vec<FileLineStat>,
    /// Commit entries reachable from this head.
    pub commits: Vec<CommitAuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Canonicalized git identity and timestamp offset information.
pub struct CommitAuditIdentity {
    /// Identity name component.
    pub name: String,
    /// Identity email component.
    pub email: String,
    /// Unix timestamp in seconds.
    pub time_seconds: i64,
    /// UTC offset in minutes.
    pub offset_minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Options controlling receive/verification behavior.
pub struct ReceiveBundleOptions {
    /// When true, validate metadata integrity before import.
    pub verify_metadata: bool,
    /// When true, apply into a temporary mirror and report would-change stats.
    pub dry_run: bool,
}
