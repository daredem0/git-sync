// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/materialized.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

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
