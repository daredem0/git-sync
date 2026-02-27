//! Unit tests for format tests.

// Focus: formatting helpers for identities/timestamps/status lines and non-text diff error recognition.

use super::super::format::{
    format_git_timestamp, format_identity, is_non_text_patch_unavailable_error,
};
use crate::git::CommitAuditIdentity;

// Verifies that identity formatting is rendered as "Name <email>" for commit detail display.
#[test]
fn format_identity_renders_name_and_email() {
    let identity = CommitAuditIdentity {
        name: "Florian".to_string(),
        email: "florian@example.com".to_string(),
        time_seconds: 0,
        offset_minutes: 0,
    };
    assert_eq!(
        format_identity(&identity),
        "Florian <florian@example.com>".to_string()
    );
}

// Verifies that timestamp formatting keeps unix seconds and renders timezone offset in UTC form.
#[test]
fn format_git_timestamp_renders_seconds_and_offset() {
    assert_eq!(
        format_git_timestamp(1_700_000_000, -90),
        "1700000000 (UTC-01:30)".to_string()
    );
}

// Verifies that non-text patch-load errors are recognized so Enter can no-op on binary/symlink paths.
#[test]
fn is_non_text_patch_unavailable_error_detects_expected_message() {
    let err = anyhow::anyhow!("textual diff unavailable for non-text path 'link-to-f'");
    assert!(
        is_non_text_patch_unavailable_error(&err),
        "helper should detect non-text patch availability errors"
    );
}
