// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/delta.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::apply_git_delta;

#[test]
fn apply_git_delta_rejects_source_size_mismatch() {
    let base = b"abc";
    let delta = [4u8, 0u8];
    let error = apply_git_delta(base, &delta).expect_err("source-size mismatch must fail");
    assert!(
        error.to_string().contains("delta source size mismatch"),
        "error should explain source-size mismatch"
    );
}

#[test]
fn apply_git_delta_rejects_invalid_opcode_zero() {
    let base: [u8; 0] = [];
    let delta = [0u8, 0u8, 0u8];
    let error = apply_git_delta(&base, &delta).expect_err("opcode 0x00 must fail");
    assert!(
        error.to_string().contains("invalid delta opcode 0x00"),
        "error should report invalid opcode 0x00"
    );
}

#[test]
fn apply_git_delta_rejects_truncated_varint() {
    let base: [u8; 0] = [];
    let delta = [0x80u8];
    let error = apply_git_delta(&base, &delta).expect_err("truncated varint must fail");
    assert!(
        error
            .to_string()
            .contains("delta varint byte: truncated data"),
        "error should point to truncated varint byte"
    );
}

#[test]
fn apply_git_delta_rejects_truncated_literal_chunk() {
    let base: [u8; 0] = [];
    let delta = [0u8, 3u8, 3u8, b'a', b'b'];
    let error = apply_git_delta(&base, &delta).expect_err("truncated literal chunk must fail");
    assert!(
        error
            .to_string()
            .contains("delta literal chunk: truncated data"),
        "error should identify truncated literal chunk"
    );
}

#[test]
fn apply_git_delta_rejects_truncated_copy_offset_bytes() {
    let base = b"abcd";
    let delta = [4u8, 1u8, 0x81u8];
    let error = apply_git_delta(base, &delta).expect_err("truncated copy-offset byte must fail");
    assert!(
        error
            .to_string()
            .contains("delta copy offset byte 0: truncated data"),
        "error should identify truncated copy-offset byte"
    );
}

#[test]
fn apply_git_delta_rejects_copy_range_out_of_bounds() {
    let base = b"abcd";
    let delta = [4u8, 2u8, 0x91u8, 3u8, 2u8];
    let error = apply_git_delta(base, &delta).expect_err("out-of-bounds copy must fail");
    assert!(
        error
            .to_string()
            .contains("delta copy range exceeds base object"),
        "error should report out-of-bounds copy range"
    );
}

#[test]
fn apply_git_delta_rejects_target_size_mismatch() {
    let base = b"abc";
    let delta = [3u8, 4u8, 3u8, b'a', b'b', b'c'];
    let error = apply_git_delta(base, &delta).expect_err("target-size mismatch must fail");
    assert!(
        error.to_string().contains("delta result size mismatch"),
        "error should report target-size mismatch"
    );
}
