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
