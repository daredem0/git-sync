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
mod tests;
