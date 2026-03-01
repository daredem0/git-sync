// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/object.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::{load_parsed_object_from_odb, object_oid_for_content, pack_entry_kind_to_payload_kind};
use crate::git::{PackEntryKind, PayloadObjectKind};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("git-sync-{prefix}-{nanos}"))
}

#[test]
fn object_oid_for_content_matches_git_blob_hash_and_rejects_unknown_kind() {
    let repo_dir = temp_repo_dir("verify-object-oid");
    std::fs::create_dir_all(&repo_dir).expect("must create temporary repo directory");
    let repo = git2::Repository::init(&repo_dir).expect("must initialize temporary repository");
    let blob_bytes = b"hello world\n";
    let expected = repo
        .blob(blob_bytes)
        .expect("must write blob into temporary repository");

    let actual = object_oid_for_content(PayloadObjectKind::Blob, blob_bytes)
        .expect("blob oid should be computed");
    assert_eq!(
        actual, expected,
        "computed blob oid should match git's canonical object id"
    );

    let error = object_oid_for_content(PayloadObjectKind::Unknown, blob_bytes)
        .expect_err("unknown object kind should be rejected");
    assert!(
        error
            .to_string()
            .contains("cannot hash unknown pack object kind"),
        "error should report unsupported unknown object kind"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn load_parsed_object_from_odb_reads_blob_kind_and_content() {
    let repo_dir = temp_repo_dir("verify-object-load");
    std::fs::create_dir_all(&repo_dir).expect("must create temporary repo directory");
    let repo = git2::Repository::init(&repo_dir).expect("must initialize temporary repository");
    let blob_bytes = b"blob-bytes";
    let blob_oid = repo
        .blob(blob_bytes)
        .expect("must write blob into temporary repository");
    let odb = repo.odb().expect("must open object database");

    let parsed = load_parsed_object_from_odb(&odb, blob_oid)
        .expect("blob object should be loadable from baseline odb");
    assert_eq!(parsed.kind, PayloadObjectKind::Blob);
    assert_eq!(parsed.content, blob_bytes);

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn pack_entry_kind_to_payload_kind_maps_delta_entries_to_unknown() {
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::Commit),
        PayloadObjectKind::Commit
    );
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::Tree),
        PayloadObjectKind::Tree
    );
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::Blob),
        PayloadObjectKind::Blob
    );
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::Tag),
        PayloadObjectKind::Tag
    );
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::OfsDelta),
        PayloadObjectKind::Unknown
    );
    assert_eq!(
        pack_entry_kind_to_payload_kind(PackEntryKind::RefDelta),
        PayloadObjectKind::Unknown
    );
}
