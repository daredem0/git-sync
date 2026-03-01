// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/pack.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::{parse_ofs_delta_base_distance, parse_pack_entry_header, read_be_u32};
use crate::git::PackEntryKind;

fn encode_pack_entry_header(kind: PackEntryKind, size: usize) -> Vec<u8> {
    let kind_code = match kind {
        PackEntryKind::Commit => 1u8,
        PackEntryKind::Tree => 2u8,
        PackEntryKind::Blob => 3u8,
        PackEntryKind::Tag => 4u8,
        PackEntryKind::OfsDelta => 6u8,
        PackEntryKind::RefDelta => 7u8,
    };
    let mut out = Vec::new();
    let mut remaining = size >> 4;
    let mut first = (kind_code << 4) | ((size & 0x0f) as u8);
    if remaining != 0 {
        first |= 0x80;
    }
    out.push(first);
    while remaining != 0 {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }
    out
}

#[test]
fn read_be_u32_reads_value_and_rejects_out_of_bounds() {
    let bytes = [0x12u8, 0x34, 0x56, 0x78, 0x9a];
    let value = read_be_u32(&bytes, 0).expect("must read big-endian u32");
    assert_eq!(value, 0x12345678);

    let error = read_be_u32(&bytes, 2).expect_err("out-of-bounds read must fail");
    assert!(
        error.to_string().contains("u32 read out of bounds"),
        "error should report out-of-bounds u32 read"
    );
}

#[test]
fn parse_pack_entry_header_parses_kind_and_size() {
    let header = encode_pack_entry_header(PackEntryKind::Blob, 0x123);
    let (kind, size, cursor) =
        parse_pack_entry_header(&header, 0).expect("header parsing should succeed");
    assert_eq!(kind, PackEntryKind::Blob);
    assert_eq!(size, 0x123);
    assert_eq!(cursor, header.len());
}

#[test]
fn parse_pack_entry_header_rejects_invalid_and_truncated_headers() {
    let invalid = [0x50u8];
    let invalid_error =
        parse_pack_entry_header(&invalid, 0).expect_err("invalid entry kind must fail");
    assert!(
        invalid_error
            .to_string()
            .contains("unsupported/invalid pack entry type code"),
        "error should report invalid entry kind code"
    );

    let truncated = [0xB0u8];
    let truncated_error =
        parse_pack_entry_header(&truncated, 0).expect_err("truncated multi-byte header must fail");
    assert!(
        truncated_error
            .to_string()
            .contains("pack entry header is truncated"),
        "error should report truncated pack header"
    );
}

#[test]
fn parse_ofs_delta_base_distance_parses_and_rejects_truncated_data() {
    let (distance, consumed) = parse_ofs_delta_base_distance(&[0x81, 0x00], 0)
        .expect("ofs-delta distance parsing should succeed");
    assert_eq!(distance, 256);
    assert_eq!(consumed, 2);

    let error = parse_ofs_delta_base_distance(&[0x81], 0)
        .expect_err("truncated ofs-delta distance should fail");
    assert!(
        error
            .to_string()
            .contains("ofs-delta base encoding is truncated"),
        "error should report truncated ofs-delta base encoding"
    );
}
