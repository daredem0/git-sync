// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for payload proof domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Pack-level proof metrics emitted for pre-transfer completeness auditing.
pub struct PayloadPackProof {
    /// Explicit verification result for PACK completeness/integrity checks.
    pub verification_status: String,
    /// PACK format version parsed from pack header.
    pub pack_version: u32,
    /// Legacy alias of `entries_declared`, kept for compatibility.
    pub declared_object_count: usize,
    /// Legacy alias of `entries_parsed`, kept for compatibility.
    pub processed_object_count: usize,
    /// Number of PACK entries declared by pack header.
    pub entries_declared: usize,
    /// Number of PACK entries parsed from raw stream.
    pub entries_parsed: usize,
    /// Number of entries with full object bytes materialized.
    pub entries_materialized: usize,
    /// Number of unique materialized object IDs.
    pub unique_objects_materialized: usize,
    /// Duplicate count among materialized entries (`entries_materialized - unique_objects_materialized`).
    pub duplicate_entry_count_materialized: usize,
    /// Whether computed checksum and trailer checksum matched.
    pub checksum_verified: bool,
    /// Whether external delta-base dependencies were detected while parsing.
    pub thin_pack_detected: bool,
    /// Number of entries resolved via baseline repository ODB.
    pub baseline_resolutions_count: usize,
    /// Whether transfer is allowed by entry-materialization gate.
    pub transfer_allowed: bool,
    /// Optional blocked-transfer reason when gate is closed.
    pub blocked_reason: Option<String>,
    /// Hash algorithm used for pack trailer/object IDs.
    pub hash_algorithm: String,
    /// SHA-1 of all pack bytes except trailer (computed locally).
    pub computed_pack_checksum: String,
    /// SHA-1 trailer checksum embedded in PACK payload.
    pub trailer_pack_checksum: String,
}

impl PayloadPackProof {
    /// Builds proof counters and deterministic transfer-gate status from entry metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn from_entry_counters(
        pack_version: u32,
        entries_declared: usize,
        entries_parsed: usize,
        entries_materialized: usize,
        unique_objects_materialized: usize,
        duplicate_entry_count_materialized: usize,
        checksum_verified: bool,
        thin_pack_detected: bool,
        baseline_resolutions_count: usize,
        hash_algorithm: String,
        computed_pack_checksum: String,
        trailer_pack_checksum: String,
    ) -> Self {
        let transfer_allowed = entries_materialized == entries_declared;
        let blocked_reason = if transfer_allowed {
            None
        } else {
            Some(format!(
                "materialized entries below declared count: materialized={}, declared={}",
                entries_materialized, entries_declared
            ))
        };

        Self {
            verification_status: if !checksum_verified {
                "failed".to_string()
            } else if transfer_allowed {
                "ok".to_string()
            } else {
                "blocked".to_string()
            },
            pack_version,
            declared_object_count: entries_declared,
            processed_object_count: entries_parsed,
            entries_declared,
            entries_parsed,
            entries_materialized,
            unique_objects_materialized,
            duplicate_entry_count_materialized,
            checksum_verified,
            thin_pack_detected,
            baseline_resolutions_count,
            transfer_allowed,
            blocked_reason,
            hash_algorithm,
            computed_pack_checksum,
            trailer_pack_checksum,
        }
    }
}
