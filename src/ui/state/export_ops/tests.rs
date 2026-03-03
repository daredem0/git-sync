// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/state/export_ops.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::ui::tests::support::{build_model_from_fixture, create_diff_fixture};
use std::fs;
use std::path::PathBuf;

fn is_iso8601_basic_utc(token: &str) -> bool {
    if token.len() != 16 {
        return false;
    }
    let bytes = token.as_bytes();
    if bytes[8] != b'T' || bytes[15] != b'Z' {
        return false;
    }

    token
        .chars()
        .enumerate()
        .all(|(index, ch)| (index == 8 || index == 15) || ch.is_ascii_digit())
}

#[test]
fn sanitize_file_name_token_replaces_unsupported_characters() {
    assert_eq!(
        sanitize_file_name_token("repo name/with spaces"),
        "repo-name-with-spaces"
    );
    assert_eq!(sanitize_file_name_token("___"), "___");
    assert_eq!(sanitize_file_name_token("   "), "unknown");
}

#[test]
fn with_collision_suffix_preserves_paudit_suffix_shape() {
    let path = PathBuf::from("20260303T123456Z_repo_sync.bundle.paudit.json");
    let suffixed = with_collision_suffix(&path, 3).expect("must append collision suffix");
    assert_eq!(
        suffixed,
        PathBuf::from("20260303T123456Z_repo_sync.bundle-3.paudit.json")
    );
}

#[test]
fn write_payload_audit_export_writes_timestamped_paudit_file_to_current_pwd() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let expected_dir = std::env::current_dir().expect("must resolve current working directory");

    let notice = write_payload_audit_export(&model, PayloadAuditObjectDetailMode::Full)
        .expect("must export payload audit json");
    let export_path = notice.path.clone();
    let file_name = notice
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("exported file name should be valid utf-8");

    let mut pieces = file_name.splitn(3, '_');
    let timestamp = pieces.next().expect("timestamp prefix should exist");
    let repo = pieces.next().expect("repo token should exist");
    let tail = pieces.next().expect("bundle tail should exist");

    assert!(
        is_iso8601_basic_utc(timestamp),
        "timestamp prefix should use ISO-8601 basic UTC shape: {timestamp}"
    );
    assert!(!repo.is_empty(), "repo token should not be empty");
    assert!(
        tail.ends_with(".paudit.json"),
        "export should keep .paudit.json suffix"
    );
    assert!(
        tail.contains("_full.paudit.json"),
        "full export should include mode token in file name"
    );
    assert_eq!(
        notice.path.parent(),
        Some(expected_dir.as_path()),
        "export path should be rooted in the process current working directory"
    );
    assert!(
        notice.exported_at_human_utc.ends_with(" UTC"),
        "human timestamp should include explicit UTC suffix"
    );

    let content = fs::read_to_string(&export_path).expect("must read exported json");
    assert!(
        content.contains("\"schema_version\""),
        "export should contain payload-audit json fields"
    );

    let _ = fs::remove_file(&export_path);
}

#[test]
fn write_payload_audit_export_light_mode_omits_object_details() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);

    let notice = write_payload_audit_export(&model, PayloadAuditObjectDetailMode::Light)
        .expect("must export payload audit json in light mode");
    let content = fs::read_to_string(&notice.path).expect("must read exported json");

    assert!(
        content.contains("\"object_detail_mode\": \"light\""),
        "light export should mark object detail mode in payload document"
    );
    assert!(
        content.contains("\"object_details\": []"),
        "light export should omit object details"
    );
    assert!(
        content.contains("\"mode\": \"none\""),
        "light export should use none ledger mode for minimal paudit output"
    );
    assert!(
        content.contains("\"pack_objects\": []"),
        "light export should omit pack object rows for minimal paudit output"
    );
    assert!(
        content.contains("\"first_entries\": []")
            && content.contains("\"last_entries\": []")
            && content.contains("\"unresolved_entry_rows\": []")
            && content.contains("\"entries\": []"),
        "light export should omit all ledger row arrays in none mode"
    );

    let _ = fs::remove_file(&notice.path);
}
