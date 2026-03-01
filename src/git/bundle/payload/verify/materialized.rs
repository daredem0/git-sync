// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for materialized.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::{
    MaterializedObjectData, MaterializedObjectIndex, MaterializedObjectRecord,
    MaterializedObjectStore, PackEntryLedger, PackEntryRecord, PayloadObjectKind,
};
use std::collections::HashMap;

use super::object::ParsedPackObject;

const MAX_BLOB_STORE_BYTES: usize = 4 * 1024 * 1024;
const LARGE_BLOB_PREVIEW_BYTES: usize = 8192;

/// Builds deduplicated materialized object index directly from ledger result rows.
pub(super) fn build_materialized_object_index_from_ledger(
    ledger: &PackEntryLedger,
) -> MaterializedObjectIndex {
    let mut by_oid = HashMap::<git2::Oid, MaterializedObjectRecord>::new();
    let mut materialized_entry_count = 0usize;

    for entry in &ledger.entries {
        if !is_materialized_entry(entry) {
            continue;
        }
        let (Some(oid), Some(kind)) = (entry.result_oid, entry.result_kind) else {
            continue;
        };
        materialized_entry_count += 1;
        let reconstructed_size = entry.reconstructed_size.unwrap_or(entry.out_size);
        by_oid.entry(oid).or_insert(MaterializedObjectRecord {
            oid,
            kind,
            size_bytes: reconstructed_size,
            first_entry_idx: entry.idx,
        });
    }

    let unique_object_count = by_oid.len();
    let duplicate_entry_count_materialized =
        materialized_entry_count.saturating_sub(unique_object_count);
    let mut objects = by_oid.into_values().collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });

    MaterializedObjectIndex {
        objects,
        materialized_entry_count,
        unique_object_count,
        duplicate_entry_count_materialized,
    }
}

/// Builds verifier-owned materialized object store from parsed object map.
pub(super) fn build_materialized_object_store(
    objects_by_oid: &HashMap<git2::Oid, ParsedPackObject>,
) -> MaterializedObjectStore {
    let mut objects = objects_by_oid
        .iter()
        .map(|(oid, parsed)| {
            if parsed.kind == PayloadObjectKind::Blob && parsed.content.len() > MAX_BLOB_STORE_BYTES
            {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content
                        [..LARGE_BLOB_PREVIEW_BYTES.min(parsed.content.len())]
                        .to_vec(),
                    content_truncated: true,
                }
            } else {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content.clone(),
                    content_truncated: false,
                }
            }
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| left.oid.cmp(&right.oid));
    MaterializedObjectStore { objects }
}

/// Returns true when an entry is fully materialized and exportable as exact object bytes.
fn is_materialized_entry(entry: &PackEntryRecord) -> bool {
    entry.resolved
        && entry.result_oid.is_some()
        && entry.result_kind.is_some()
        && entry.reconstructed_size.is_some()
        && entry.resolved_via.is_some()
}

fn payload_kind_rank(kind: PayloadObjectKind) -> u8 {
    match kind {
        PayloadObjectKind::Commit => 0,
        PayloadObjectKind::Tree => 1,
        PayloadObjectKind::Blob => 2,
        PayloadObjectKind::Tag => 3,
        PayloadObjectKind::Unknown => 4,
    }
}

#[cfg(test)]
mod tests;
