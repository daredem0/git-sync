// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/core.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::verify_pack_payload_impl;
use crate::git::digest::sha1_bytes;

fn build_pack(version: u32, declared_entries: u32, body: &[u8]) -> Vec<u8> {
    let mut pack = Vec::new();
    pack.extend_from_slice(b"PACK");
    pack.extend_from_slice(&version.to_be_bytes());
    pack.extend_from_slice(&declared_entries.to_be_bytes());
    pack.extend_from_slice(body);
    let trailer = sha1_bytes(&pack).expect("must compute test pack checksum");
    pack.extend_from_slice(&trailer);
    pack
}

#[test]
fn verify_pack_payload_impl_rejects_when_pack_ends_before_declared_entries() {
    let pack = build_pack(2, 1, &[]);
    let error = verify_pack_payload_impl(&pack, None)
        .expect_err("pack with missing declared entry payload should fail");
    assert!(
        error
            .reason
            .contains("pack ended before declared object count was processed"),
        "error should report ended-before-declared mismatch"
    );
}

#[test]
fn verify_pack_payload_impl_rejects_trailing_bytes_before_trailer() {
    let pack = build_pack(2, 0, &[0xde, 0xad, 0xbe, 0xef]);
    let error =
        verify_pack_payload_impl(&pack, None).expect_err("pack with unconsumed bytes must fail");
    assert!(
        error
            .reason
            .contains("pack contains trailing or unconsumed bytes before trailer"),
        "error should report trailing/unconsumed bytes"
    );
}
