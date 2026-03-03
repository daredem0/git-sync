// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for cli.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use clap::Parser;

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

// Verifies that `receive` defaults integration policy to fast-forward-only.
#[test]
fn receive_defaults_to_fast_forward_only_integration_policy() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
    ])
    .expect("receive command with required arguments should parse");

    let Some(Command::Receive {
        integrate,
        incoming_as_branches,
        ..
    }) = cli.command
    else {
        panic!("expected parsed receive command");
    };
    assert_eq!(integrate, ReceiveIntegratePolicy::FastForwardOnly);
    assert!(
        !incoming_as_branches,
        "incoming branch mirroring should default to disabled"
    );
}

// Verifies that `receive --integrate create-refs-only` parses and maps to the expected enum.
#[test]
fn receive_parses_create_refs_only_integration_policy() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--integrate",
        "create-refs-only",
    ])
    .expect("receive command with explicit integration policy should parse");

    let Some(Command::Receive {
        integrate,
        incoming_as_branches,
        ..
    }) = cli.command
    else {
        panic!("expected parsed receive command");
    };
    assert_eq!(integrate, ReceiveIntegratePolicy::CreateRefsOnly);
    assert!(
        !incoming_as_branches,
        "incoming branch mirroring should remain disabled when flag is not set"
    );
}

// Verifies that `receive --integrate merge` parses and maps to the expected enum.
#[test]
fn receive_parses_merge_integration_policy() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--integrate",
        "merge",
    ])
    .expect("receive command with explicit merge integration policy should parse");

    let Some(Command::Receive {
        integrate,
        incoming_as_branches,
        ..
    }) = cli.command
    else {
        panic!("expected parsed receive command");
    };
    assert_eq!(integrate, ReceiveIntegratePolicy::Merge);
    assert!(
        !incoming_as_branches,
        "incoming branch mirroring should remain disabled when flag is not set"
    );
}

// Verifies that `receive --incoming-as-branches` enables incoming branch mirroring.
#[test]
fn receive_parses_incoming_as_branches_flag() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--incoming-as-branches",
    ])
    .expect("receive command with incoming-as-branches should parse");

    let Some(Command::Receive {
        integrate,
        incoming_as_branches,
        ..
    }) = cli.command
    else {
        panic!("expected parsed receive command");
    };
    assert_eq!(integrate, ReceiveIntegratePolicy::FastForwardOnly);
    assert!(
        incoming_as_branches,
        "incoming branch mirroring should be enabled when flag is set"
    );
}

// Verifies that `receive --dry-run --format json` parses with the selected output format.
#[test]
fn receive_parses_dry_run_json_output_format() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--dry-run",
        "--format",
        "json",
    ])
    .expect("receive dry-run with --format json should parse");

    let Some(Command::Receive { format, .. }) = cli.command else {
        panic!("expected parsed receive command");
    };
    assert_eq!(
        format,
        Some(OutputFormat::Json),
        "receive --format json should map to OutputFormat::Json"
    );
}

// Verifies that `receive --check-mergeability` toggles mergeability simulation mode.
#[test]
fn receive_parses_check_mergeability_flag() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--check-mergeability",
    ])
    .expect("receive with --check-mergeability should parse");

    let Some(Command::Receive {
        check_mergeability, ..
    }) = cli.command
    else {
        panic!("expected parsed receive command");
    };
    assert!(
        check_mergeability,
        "receive --check-mergeability should enable mergeability simulation mode"
    );
}

// Verifies that `receive --verbose` enables expanded import diagnostics output.
#[test]
fn receive_parses_verbose_flag() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "receive",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--verbose",
    ])
    .expect("receive with --verbose should parse");

    let Some(Command::Receive { verbose, .. }) = cli.command else {
        panic!("expected parsed receive command");
    };
    assert!(
        verbose,
        "receive --verbose should enable detailed diagnostics"
    );
}

// Verifies that `audit --format json` defaults payload-detail mode to full.
#[test]
fn audit_json_defaults_payload_detail_mode_to_full() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "audit",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--format",
        "json",
    ])
    .expect("audit json command with required arguments should parse");

    let Some(Command::Audit { payload_detail, .. }) = cli.command else {
        panic!("expected parsed audit command");
    };
    assert!(
        matches!(payload_detail, PayloadDetailMode::Full),
        "audit --format json should default payload-detail to full"
    );
}

// Verifies that `audit --payload-detail light` parses and maps to light detail mode.
#[test]
fn audit_json_parses_payload_detail_light() {
    let cli = Cli::try_parse_from([
        "git-sync",
        "audit",
        "--repo",
        ".",
        "--bundle",
        "sync.bundle.zip",
        "--format",
        "json",
        "--payload-detail",
        "light",
    ])
    .expect("audit json command with light payload-detail should parse");

    let Some(Command::Audit { payload_detail, .. }) = cli.command else {
        panic!("expected parsed audit command");
    };
    assert!(
        matches!(payload_detail, PayloadDetailMode::Light),
        "audit --payload-detail light should map to light detail mode"
    );
}
