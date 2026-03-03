// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for app/commands/audit.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;

#[test]
fn run_interactive_rejects_non_pack_only_resolve_mode() {
    let result = run_interactive(
        Some(PathBuf::from(".")),
        Some(PathBuf::from("sync.bundle.zip")),
        CliPayloadResolveMode::Baseline,
    );
    let error = result.expect_err("interactive mode should reject baseline resolve mode");
    assert!(
        error
            .to_string()
            .contains("interactive audit currently supports only --resolve pack-only"),
        "error should explain interactive resolve-mode constraint"
    );
}

#[test]
fn run_verify_metadata_requires_repo_and_bundle_arguments() {
    let missing_repo = run(
        None,
        Some(PathBuf::from("sync.bundle.zip")),
        true,
        None,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        missing_repo
            .expect_err("verify-metadata mode should require repo")
            .to_string()
            .contains("metadata verification requires --repo")
    );

    let missing_bundle = run(
        Some(PathBuf::from(".")),
        None,
        true,
        None,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        missing_bundle
            .expect_err("verify-metadata mode should require bundle")
            .to_string()
            .contains("metadata verification requires --bundle")
    );
}

#[test]
fn run_non_interactive_propagates_target_resolution_error() {
    let result = run_non_interactive(
        None,
        None,
        OutputFormat::Table,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        result
            .expect_err("non-interactive mode should require target input")
            .to_string()
            .contains("payload audit requires both --repo and --bundle"),
        "target resolution error should be preserved"
    );
}
