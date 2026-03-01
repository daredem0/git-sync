// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Tests for inspect range behavior and invariants.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::support::*;
use super::*;

// Focus: bundle-header inspection parsing and robustness for malformed inputs.
// Verifies that inspect_bundle parses version, prerequisite, and head entries from a created bundle.
#[test]
fn inspect_bundle_parses_created_bundle_metadata() {
    let repo_dir = temp_repo_dir("inspect-bundle");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let inspection = inspect_bundle(&bundle_path).expect("bundle inspection should succeed");
    assert_eq!(
        inspection.version,
        BundleVersion::V2,
        "created bundle should use v2 bundle format"
    );
    assert_eq!(
        inspection.prerequisites,
        vec![base_commit_id],
        "inspection should parse prerequisite commit list"
    );
    assert_eq!(
        inspection.heads.len(),
        1,
        "inspection should parse one head"
    );
    assert_eq!(
        inspection.heads[0].oid, tip_commit_id,
        "inspection should parse head oid"
    );
    assert_eq!(
        inspection.heads[0].reference, "refs/heads/tip",
        "inspection should parse head reference name"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that inspect_bundle rejects files that do not begin with a valid bundle header signature.
#[test]
fn inspect_bundle_rejects_invalid_header_signature() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-invalid-bundle-header-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(&bundle_path, b"not-a-bundle\nPACK").expect("must write invalid bundle file");

    let result = inspect_bundle(&bundle_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject files with an invalid bundle signature line"
    );

    let _ = std::fs::remove_file(bundle_path);
}

// Verifies that inspect_bundle rejects non-existent bundle paths.
#[test]
fn inspect_bundle_rejects_missing_path() {
    let missing_path = std::env::temp_dir().join(format!(
        "git-sync-missing-inspect-bundle-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = inspect_bundle(&missing_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject non-existent bundle paths"
    );
}

// Verifies that inspect_bundle rejects paths that are directories instead of files.
#[test]
fn inspect_bundle_rejects_directory_path() {
    let bundle_dir = temp_repo_dir("inspect-bundle-dir");
    std::fs::create_dir_all(&bundle_dir).expect("must create bundle directory");

    let result = inspect_bundle(&bundle_dir);
    assert!(
        result.is_err(),
        "inspect_bundle must reject directory paths"
    );

    let _ = std::fs::remove_dir_all(bundle_dir);
}

// Verifies that inspect_bundle rejects malformed prerequisite lines that omit the prerequisite OID.
#[test]
fn inspect_bundle_rejects_prerequisite_line_without_oid() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-invalid-prereq-line-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(&bundle_path, b"# v2 git bundle\n-\n\n")
        .expect("must write malformed prerequisite bundle");

    let result = inspect_bundle(&bundle_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject prerequisite lines that omit OID tokens"
    );

    let _ = std::fs::remove_file(bundle_path);
}

// Verifies that inspect_bundle rejects malformed head lines that omit the reference name.
#[test]
fn inspect_bundle_rejects_head_line_without_reference() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-invalid-head-line-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(
        &bundle_path,
        b"# v2 git bundle\n1111111111111111111111111111111111111111\n\n",
    )
    .expect("must write malformed head bundle");

    let result = inspect_bundle(&bundle_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject head lines missing reference names"
    );

    let _ = std::fs::remove_file(bundle_path);
}

// Verifies that inspect_bundle rejects bundle lines with invalid OID tokens.
#[test]
fn inspect_bundle_rejects_invalid_oid_tokens() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-invalid-head-oid-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(
        &bundle_path,
        b"# v2 git bundle\nnotanod refs/heads/main\n\n",
    )
    .expect("must write malformed oid bundle");

    let result = inspect_bundle(&bundle_path);
    assert!(
        result.is_err(),
        "inspect_bundle must reject invalid OID fields in head lines"
    );

    let _ = std::fs::remove_file(bundle_path);
}

// Verifies that inspect_bundle accepts CRLF line endings for bundle header and metadata lines.
#[test]
fn inspect_bundle_accepts_crlf_line_endings() {
    let bundle_path = std::env::temp_dir().join(format!(
        "git-sync-crlf-header-{}-{}.bundle",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    std::fs::write(
        &bundle_path,
        b"# v2 git bundle\r\n-1111111111111111111111111111111111111111\r\n2222222222222222222222222222222222222222 refs/heads/main\r\n\r\n",
    )
    .expect("must write crlf bundle text");

    let inspection = inspect_bundle(&bundle_path).expect("crlf bundle should parse successfully");
    assert_eq!(
        inspection.version,
        BundleVersion::V2,
        "crlf header should still resolve to v2 bundle version"
    );
    assert_eq!(
        inspection.prerequisites.len(),
        1,
        "crlf prerequisites should parse as normal prerequisite lines"
    );
    assert_eq!(
        inspection.heads.len(),
        1,
        "crlf heads should parse as normal head lines"
    );

    let _ = std::fs::remove_file(bundle_path);
}
