//! Materialized index/store helpers built from verified PACK ledger rows.

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
mod tests {
    use super::{build_materialized_object_index_from_ledger, build_materialized_object_store};
    use crate::git::{
        PackEntryKind, PackEntryLedger, PackEntryRecord, PayloadObjectKind, ResolutionSource,
    };
    use std::collections::HashMap;

    fn oid(hex: &str) -> git2::Oid {
        git2::Oid::from_str(hex).expect("must parse test oid")
    }

    #[test]
    fn materialized_index_counts_entries_and_deduplicates_by_oid() {
        let oid1 = oid("1111111111111111111111111111111111111111");
        let oid2 = oid("2222222222222222222222222222222222222222");
        let ledger = PackEntryLedger {
            pack_version: 2,
            declared_entry_count: 4,
            entries: vec![
                PackEntryRecord {
                    idx: 0,
                    offset: 12,
                    kind: PackEntryKind::Commit,
                    out_size: 100,
                    reconstructed_size: Some(100),
                    base_ref: None,
                    result_oid: Some(oid1),
                    result_kind: Some(PayloadObjectKind::Commit),
                    resolved: true,
                    resolved_via: Some(ResolutionSource::InPack),
                    note: None,
                },
                PackEntryRecord {
                    idx: 1,
                    offset: 44,
                    kind: PackEntryKind::Blob,
                    out_size: 10,
                    reconstructed_size: Some(10),
                    base_ref: None,
                    result_oid: Some(oid2),
                    result_kind: Some(PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(ResolutionSource::InPack),
                    note: None,
                },
                // Duplicate materialized OID.
                PackEntryRecord {
                    idx: 2,
                    offset: 77,
                    kind: PackEntryKind::Blob,
                    out_size: 10,
                    reconstructed_size: Some(10),
                    base_ref: None,
                    result_oid: Some(oid2),
                    result_kind: Some(PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(ResolutionSource::InPack),
                    note: None,
                },
                // Unresolved row should be ignored by materialized index.
                PackEntryRecord {
                    idx: 3,
                    offset: 90,
                    kind: PackEntryKind::RefDelta,
                    out_size: 8,
                    reconstructed_size: None,
                    base_ref: None,
                    result_oid: None,
                    result_kind: None,
                    resolved: false,
                    resolved_via: None,
                    note: Some("unresolved".to_string()),
                },
            ],
        };

        let index = build_materialized_object_index_from_ledger(&ledger);
        assert_eq!(index.materialized_entry_count, 3);
        assert_eq!(index.unique_object_count, 2);
        assert_eq!(index.duplicate_entry_count_materialized, 1);
        assert_eq!(index.objects.len(), 2);
    }

    #[test]
    fn materialized_store_truncates_large_blob_and_keeps_small_objects_full() {
        let blob_oid = oid("3333333333333333333333333333333333333333");
        let commit_oid = oid("4444444444444444444444444444444444444444");
        let mut objects = HashMap::new();
        objects.insert(
            blob_oid,
            super::ParsedPackObject {
                kind: PayloadObjectKind::Blob,
                content: vec![b'x'; 4 * 1024 * 1024 + 16],
            },
        );
        objects.insert(
            commit_oid,
            super::ParsedPackObject {
                kind: PayloadObjectKind::Commit,
                content: b"commit-body".to_vec(),
            },
        );

        let store = build_materialized_object_store(&objects);
        let truncated_blob = store
            .objects
            .iter()
            .find(|row| row.oid == blob_oid)
            .expect("truncated blob object must be present in store");
        assert!(truncated_blob.content_truncated);
        assert_eq!(truncated_blob.content_bytes.len(), 8192);

        let commit = store
            .objects
            .iter()
            .find(|row| row.oid == commit_oid)
            .expect("commit object must be present in store");
        assert!(!commit.content_truncated);
        assert_eq!(commit.content_bytes, b"commit-body");
    }
}
