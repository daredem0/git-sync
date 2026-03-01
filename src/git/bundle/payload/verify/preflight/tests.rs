// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/preflight.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::run_pack_preflight;
use crate::git::digest::{hex_encode, sha1_bytes};

fn build_pack(version: u32, declared_entries: u32, body: &[u8]) -> Vec<u8> {
    let mut pack = Vec::new();
    pack.extend_from_slice(b"PACK");
    pack.extend_from_slice(&version.to_be_bytes());
    pack.extend_from_slice(&declared_entries.to_be_bytes());
    pack.extend_from_slice(body);
    let trailer = sha1_bytes(&pack).expect("must compute pack trailer checksum");
    pack.extend_from_slice(&trailer);
    pack
}

#[test]
fn run_pack_preflight_rejects_payloads_smaller_than_header_plus_trailer() {
    let too_small = vec![0u8; 31];
    let error = run_pack_preflight(&too_small).expect_err("small pack payload must fail");
    assert!(
        error.reason.contains("pack payload is too small"),
        "error should explain minimum pack size requirement"
    );
}

#[test]
fn run_pack_preflight_rejects_invalid_magic() {
    let mut invalid = vec![0u8; 32];
    invalid[..4].copy_from_slice(b"PAXK");
    let error = run_pack_preflight(&invalid).expect_err("invalid magic must fail");
    assert!(
        error
            .reason
            .contains("pack payload does not start with PACK header"),
        "error should report invalid pack magic"
    );
}

#[test]
fn run_pack_preflight_rejects_unsupported_pack_versions() {
    let pack = build_pack(4, 0, &[]);
    let error = run_pack_preflight(&pack).expect_err("unsupported version must fail");
    assert!(
        error.reason.contains("unsupported pack version: 4"),
        "error should report unsupported version value"
    );
}

#[test]
fn run_pack_preflight_rejects_checksum_mismatch() {
    let mut pack = build_pack(2, 0, &[]);
    let last = pack.len() - 1;
    pack[last] ^= 0x01;

    let error = run_pack_preflight(&pack).expect_err("tampered trailer must fail");
    assert!(
        error.reason.contains("pack trailer checksum mismatch"),
        "error should report trailer checksum mismatch"
    );
}

#[test]
fn run_pack_preflight_returns_validated_metadata_for_valid_pack() {
    let pack = build_pack(2, 0, &[]);
    let preflight = run_pack_preflight(&pack).expect("valid pack preflight should pass");
    assert_eq!(preflight.pack_version, 2);
    assert_eq!(preflight.declared_entry_count, 0);
    assert_eq!(preflight.trailer_offset, 12);
    assert_eq!(preflight.computed_checksum, preflight.trailer_checksum);
    assert_eq!(
        preflight.trailer_checksum,
        hex_encode(&pack[preflight.trailer_offset..]),
        "reported trailer checksum should match trailer bytes"
    );
}
