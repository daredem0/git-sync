// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for cli.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;

// Verifies that resolve_payload_audit_target accepts payload mode with --repo and --bundle.
#[test]
fn resolve_payload_audit_target_accepts_bundle_with_repo() {
    let repo_path = PathBuf::from(".");
    let bundle_path = PathBuf::from("sync.bundle");
    let result = resolve_payload_audit_target(Some(repo_path.clone()), Some(bundle_path.clone()))
        .expect("payload audit input should be accepted");
    assert_eq!(
        result,
        PayloadAuditTarget {
            repo_path,
            bundle_path,
        }
    );
}

// Verifies that resolve_payload_audit_target requires both --repo and --bundle.
#[test]
fn resolve_payload_audit_target_requires_repo_and_bundle() {
    let missing_bundle = resolve_payload_audit_target(Some(PathBuf::from(".")), None);
    assert!(
        missing_bundle.is_err(),
        "payload audit target resolution must reject missing --bundle"
    );

    let missing_repo = resolve_payload_audit_target(None, Some(PathBuf::from("sync.bundle")));
    assert!(
        missing_repo.is_err(),
        "payload audit target resolution must reject missing --repo"
    );
}
