// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/detail.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::collect_payload_object_detail_from_store;
use crate::git::types::{MaterializedObjectData, PayloadObjectKind};
use std::collections::HashMap;

fn oid(value: &str) -> git2::Oid {
    git2::Oid::from_str(value).expect("must parse test oid")
}

fn object(
    kind: PayloadObjectKind,
    oid: git2::Oid,
    bytes: &[u8],
    truncated: bool,
) -> MaterializedObjectData {
    MaterializedObjectData {
        oid,
        kind,
        size_bytes: if truncated {
            bytes.len() + 16
        } else {
            bytes.len()
        },
        content_bytes: bytes.to_vec(),
        content_truncated: truncated,
    }
}

#[test]
fn collect_payload_object_detail_returns_error_for_unknown_object_id() {
    let store = HashMap::new();
    let paths = HashMap::new();
    let missing = oid("ffffffffffffffffffffffffffffffffffffffff");
    let error = collect_payload_object_detail_from_store(&store, &paths, missing)
        .expect_err("missing payload object id should fail detail lookup");
    assert!(
        error
            .to_string()
            .contains("is not available in materialized store"),
        "error should explain missing materialized object"
    );
}

#[test]
fn collect_payload_object_detail_renders_commit_headers_and_message() {
    let commit_oid = oid("1111111111111111111111111111111111111111");
    let tree_oid = oid("2222222222222222222222222222222222222222");
    let parent_oid = oid("3333333333333333333333333333333333333333");
    let commit = format!(
        "tree {tree_oid}\nparent {parent_oid}\nauthor A <a@example.com> 1 +0000\ncommitter C <c@example.com> 2 +0000\n\nsubject\nbody line\n"
    );

    let mut store = HashMap::new();
    store.insert(
        commit_oid,
        object(
            PayloadObjectKind::Commit,
            commit_oid,
            commit.as_bytes(),
            false,
        ),
    );

    let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), commit_oid)
        .expect("commit detail rendering should succeed");
    assert_eq!(detail.lines[0], format!("commit {commit_oid}"));
    assert!(detail.lines.iter().any(|line| line.starts_with("tree ")));
    assert!(detail.lines.iter().any(|line| line.starts_with("author ")));
    assert!(detail.lines.iter().any(|line| line == "subject"));
    assert!(detail.lines.iter().any(|line| line == "body line"));
}

#[test]
fn collect_payload_object_detail_renders_tree_entries() {
    let tree_oid = oid("4444444444444444444444444444444444444444");
    let blob_oid = oid("5555555555555555555555555555555555555555");
    let mut tree_bytes = Vec::new();
    tree_bytes.extend_from_slice(b"100644 file.txt\0");
    tree_bytes.extend_from_slice(blob_oid.as_bytes());

    let mut store = HashMap::new();
    store.insert(
        tree_oid,
        object(PayloadObjectKind::Tree, tree_oid, &tree_bytes, false),
    );

    let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), tree_oid)
        .expect("tree detail rendering should succeed");
    assert_eq!(detail.lines[0], format!("tree {tree_oid}"));
    assert!(
        detail.lines.iter().any(|line| line.contains("100644")
            && line.contains("blob")
            && line.contains("file.txt")),
        "tree detail should include formatted entry row"
    );
}

#[test]
fn collect_payload_object_detail_rejects_malformed_tree_content() {
    let tree_oid = oid("6666666666666666666666666666666666666666");
    let malformed = b"100644-without-space-or-nul";
    let mut store = HashMap::new();
    store.insert(
        tree_oid,
        object(PayloadObjectKind::Tree, tree_oid, malformed, false),
    );

    let error = collect_payload_object_detail_from_store(&store, &HashMap::new(), tree_oid)
        .expect_err("malformed tree bytes should fail");
    assert!(
        error.to_string().contains("tree entry"),
        "tree parse failures should mention tree entry decoding"
    );
}

#[test]
fn collect_payload_object_detail_renders_text_blob_with_line_count_and_path_hint() {
    let blob_oid = oid("7777777777777777777777777777777777777777");
    let mut store = HashMap::new();
    store.insert(
        blob_oid,
        object(
            PayloadObjectKind::Blob,
            blob_oid,
            b"line one\nline two\n",
            false,
        ),
    );
    let mut paths = HashMap::new();
    paths.insert(blob_oid, vec!["src/lib.rs".to_string()]);

    let detail = collect_payload_object_detail_from_store(&store, &paths, blob_oid)
        .expect("text blob detail rendering should succeed");
    assert_eq!(detail.text_line_count, Some(2));
    assert_eq!(detail.syntax_path_hint.as_deref(), Some("src/lib.rs"));
    assert!(detail.lines.iter().any(|line| line == "text lines: 2"));
}

#[test]
fn collect_payload_object_detail_uses_default_path_hint_for_text_blob_without_paths() {
    let blob_oid = oid("8888888888888888888888888888888888888888");
    let mut store = HashMap::new();
    store.insert(
        blob_oid,
        object(PayloadObjectKind::Blob, blob_oid, b"single line\n", false),
    );

    let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
        .expect("text blob detail rendering should succeed");
    assert_eq!(detail.syntax_path_hint.as_deref(), Some("blob.txt"));
}

#[test]
fn collect_payload_object_detail_marks_truncated_text_blob_preview() {
    let blob_oid = oid("9999999999999999999999999999999999999999");
    let mut store = HashMap::new();
    store.insert(
        blob_oid,
        object(PayloadObjectKind::Blob, blob_oid, b"head\npreview\n", true),
    );

    let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
        .expect("truncated text blob detail should succeed");
    assert_eq!(
        detail.text_line_count, None,
        "truncated text preview should not claim full text-line count"
    );
    assert!(
        detail
            .lines
            .iter()
            .any(|line| line.contains("preview only:")),
        "truncated text preview should include preview-only marker"
    );
    assert!(
        detail
            .lines
            .iter()
            .any(|line| line.contains("full content not retained in-memory")),
        "truncated text preview should explain retention policy"
    );
}

#[test]
fn collect_payload_object_detail_renders_binary_blob_hex_preview() {
    let blob_oid = oid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let mut store = HashMap::new();
    store.insert(
        blob_oid,
        object(
            PayloadObjectKind::Blob,
            blob_oid,
            &[0xff, 0x00, 0x41, 0x7f],
            false,
        ),
    );

    let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
        .expect("binary blob detail rendering should succeed");
    assert!(
        detail.lines[0].starts_with("binary blob"),
        "binary blob detail header should identify binary blob"
    );
    assert!(
        detail.lines.iter().any(|line| line.contains("hex preview")),
        "binary blob detail should include hex-preview label"
    );
}

#[test]
fn collect_payload_object_detail_renders_tag_and_unknown_objects() {
    let tag_oid = oid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let unknown_oid = oid("cccccccccccccccccccccccccccccccccccccccc");
    let mut store = HashMap::new();
    store.insert(
        tag_oid,
        object(
            PayloadObjectKind::Tag,
            tag_oid,
            b"object deadbeef\ntype commit\ntag v1\n\nmessage\n",
            false,
        ),
    );
    store.insert(
        unknown_oid,
        object(PayloadObjectKind::Unknown, unknown_oid, b"", false),
    );

    let tag_detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), tag_oid)
        .expect("tag detail rendering should succeed");
    assert_eq!(tag_detail.lines[0], format!("tag {tag_oid}"));
    assert!(tag_detail.lines.iter().any(|line| line == "tag v1"));

    let unknown_detail =
        collect_payload_object_detail_from_store(&store, &HashMap::new(), unknown_oid)
            .expect("unknown-kind detail rendering should succeed");
    assert!(
        unknown_detail
            .lines
            .iter()
            .any(|line| line.contains("unsupported object type")),
        "unknown-kind details should emit unsupported-type explanation"
    );
}
