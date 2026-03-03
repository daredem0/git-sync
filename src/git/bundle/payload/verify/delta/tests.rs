// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/delta.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::apply_git_delta;

fn encode_varint(mut value: usize) -> Vec<u8> {
    let mut encoded = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            break;
        }
    }
    encoded
}

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

#[test]
fn apply_git_delta_rejects_out_of_bounds_initial_varint_offset() {
    let base: [u8; 0] = [];
    let error = apply_git_delta(&base, &[]).expect_err("empty delta should fail");
    assert!(
        error.to_string().contains("delta varint is out of bounds"),
        "error should report out-of-bounds varint offset"
    );
}

#[test]
fn apply_git_delta_applies_literal_then_copy_sections() {
    let base = b"ABCDEFGH";
    let mut delta = Vec::new();
    delta.extend_from_slice(&encode_varint(base.len()));
    delta.extend_from_slice(&encode_varint(7));
    delta.push(0x03);
    delta.extend_from_slice(b"xyz");
    delta.push(0x91);
    delta.push(2);
    delta.push(4);

    let out = apply_git_delta(base, &delta).expect("delta with literal+copy should succeed");
    assert_eq!(
        out, b"xyzCDEF",
        "delta should append literal chunk then copied range"
    );
}

#[test]
fn apply_git_delta_supports_full_copy_when_size_bits_are_zero() {
    let base = vec![b'a'; 0x10000];
    let mut delta = Vec::new();
    delta.extend_from_slice(&encode_varint(base.len()));
    delta.extend_from_slice(&encode_varint(base.len()));
    delta.push(0x8f);
    delta.extend_from_slice(&[0, 0, 0, 0]);

    let out = apply_git_delta(&base, &delta).expect("default full-copy delta should succeed");
    assert_eq!(
        out.len(),
        base.len(),
        "full-copy delta should materialize 0x10000 bytes"
    );
    assert_eq!(out, base, "full-copy delta should match base bytes");
}

#[test]
fn apply_git_delta_supports_copy_size_high_bytes() {
    let base = vec![b'z'; 0x020100];
    let mut delta = Vec::new();
    delta.extend_from_slice(&encode_varint(base.len()));
    delta.extend_from_slice(&encode_varint(base.len()));
    delta.push(0xE0);
    delta.push(0x01);
    delta.push(0x02);

    let out = apply_git_delta(&base, &delta).expect("high-byte size copy should succeed");
    assert_eq!(
        out.len(),
        base.len(),
        "copy size bytes 1/2 should reconstruct full target size"
    );
}
