// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for payload materialized domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use super::{PackEntryLedger, PayloadObjectKind, PayloadPackProof};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structured result of PACK-proof verification with entry-level ledger truth.
pub struct PayloadPackVerification {
    /// Summary proof counters/checksums.
    pub proof: PayloadPackProof,
    /// Authoritative parsed PACK entry ledger.
    pub ledger: PackEntryLedger,
    /// Deduplicated materialized objects derived from ledger result rows.
    pub materialized_index: MaterializedObjectIndex,
    /// Verifier-owned materialized object content store keyed by object id.
    pub materialized_store: MaterializedObjectStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Deduplicated object index derived from materialized ledger entries.
pub struct MaterializedObjectIndex {
    /// Unique object rows in deterministic order.
    pub objects: Vec<MaterializedObjectRecord>,
    /// Number of materialized ledger entries (before deduplication).
    pub materialized_entry_count: usize,
    /// Number of unique materialized objects (after deduplication).
    pub unique_object_count: usize,
    /// Materialized duplicate-entry count (`materialized_entry_count - unique_object_count`).
    pub duplicate_entry_count_materialized: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One unique materialized object row derived from PACK ledger entries.
pub struct MaterializedObjectRecord {
    /// Canonical object id.
    pub oid: git2::Oid,
    /// Canonical object kind.
    pub kind: PayloadObjectKind,
    /// Uncompressed object size in bytes.
    pub size_bytes: usize,
    /// First ledger entry index where this object was observed.
    pub first_entry_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Verifier-owned materialized object store for payload detail rendering.
pub struct MaterializedObjectStore {
    /// Materialized objects in deterministic OID order.
    pub objects: Vec<MaterializedObjectData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One materialized object payload entry retained by verifier.
pub struct MaterializedObjectData {
    /// Canonical object id.
    pub oid: git2::Oid,
    /// Canonical object kind.
    pub kind: PayloadObjectKind,
    /// Canonical uncompressed object size in bytes.
    pub size_bytes: usize,
    /// Stored bytes (full content or bounded preview when truncated).
    pub content_bytes: Vec<u8>,
    /// Whether `content_bytes` is truncated preview instead of full bytes.
    pub content_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structured fail-closed error for payload PACK verification.
pub struct PayloadAuditError {
    /// Human-readable reason string.
    pub reason: String,
    /// Optional zero-based index of the blocked entry.
    pub blocked_entry_idx: Option<usize>,
    /// Optional partial ledger captured before failure.
    pub ledger_partial: Option<PackEntryLedger>,
}

impl std::fmt::Display for PayloadAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.blocked_entry_idx, &self.ledger_partial) {
            (Some(idx), Some(ledger)) => write!(
                f,
                "{} (blocked_entry_idx={}, entries_parsed={}, entries_declared={})",
                self.reason,
                idx,
                ledger.entries.len(),
                ledger.declared_entry_count
            ),
            (Some(idx), None) => write!(f, "{} (blocked_entry_idx={})", self.reason, idx),
            (None, Some(ledger)) => write!(
                f,
                "{} (entries_parsed={}, entries_declared={})",
                self.reason,
                ledger.entries.len(),
                ledger.declared_entry_count
            ),
            (None, None) => f.write_str(&self.reason),
        }
    }
}

impl std::error::Error for PayloadAuditError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::types::PackEntryLedger;

    fn sample_ledger(entries_parsed: usize, entries_declared: usize) -> PackEntryLedger {
        PackEntryLedger {
            pack_version: 2,
            declared_entry_count: entries_declared,
            entries: (0..entries_parsed)
                .map(|idx| crate::git::types::PackEntryRecord {
                    idx,
                    offset: 12 + idx,
                    kind: crate::git::types::PackEntryKind::Blob,
                    out_size: 1,
                    reconstructed_size: Some(1),
                    base_ref: None,
                    result_oid: Some(
                        git2::Oid::from_str("1111111111111111111111111111111111111111")
                            .expect("must parse test oid"),
                    ),
                    result_kind: Some(crate::git::types::PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(crate::git::types::ResolutionSource::InPack),
                    note: None,
                })
                .collect(),
        }
    }

    #[test]
    fn payload_audit_error_display_with_blocked_index_and_partial_ledger() {
        let error = PayloadAuditError {
            reason: "failure".to_string(),
            blocked_entry_idx: Some(3),
            ledger_partial: Some(sample_ledger(2, 7)),
        };
        let text = error.to_string();
        assert!(text.contains("failure"));
        assert!(text.contains("blocked_entry_idx=3"));
        assert!(text.contains("entries_parsed=2"));
        assert!(text.contains("entries_declared=7"));
    }

    #[test]
    fn payload_audit_error_display_with_blocked_index_only() {
        let error = PayloadAuditError {
            reason: "failure".to_string(),
            blocked_entry_idx: Some(1),
            ledger_partial: None,
        };
        let text = error.to_string();
        assert!(text.contains("failure"));
        assert!(text.contains("blocked_entry_idx=1"));
        assert!(!text.contains("entries_parsed="));
    }

    #[test]
    fn payload_audit_error_display_with_partial_ledger_only() {
        let error = PayloadAuditError {
            reason: "failure".to_string(),
            blocked_entry_idx: None,
            ledger_partial: Some(sample_ledger(4, 9)),
        };
        let text = error.to_string();
        assert!(text.contains("failure"));
        assert!(text.contains("entries_parsed=4"));
        assert!(text.contains("entries_declared=9"));
        assert!(!text.contains("blocked_entry_idx="));
    }

    #[test]
    fn payload_audit_error_display_with_reason_only() {
        let error = PayloadAuditError {
            reason: "failure-only".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        };
        assert_eq!(error.to_string(), "failure-only");
    }
}
