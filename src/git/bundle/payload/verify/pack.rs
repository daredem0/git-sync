// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for pack.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::PackEntryKind;
use anyhow::{Result, anyhow, bail};

/// Reads one big-endian `u32` from `bytes` at `offset`.
#[allow(dead_code)]
pub(super) fn read_be_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow!("u32 read offset overflow"))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| anyhow!("u32 read out of bounds"))?;
    Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Parses a PACK object header at `offset`.
pub(super) fn parse_pack_entry_header(
    pack_data: &[u8],
    offset: usize,
) -> Result<(PackEntryKind, usize, usize)> {
    if offset >= pack_data.len() {
        bail!("pack entry header offset is out of bounds");
    }

    let first = pack_data[offset];
    let entry_kind = match (first >> 4) & 0x07 {
        1 => PackEntryKind::Commit,
        2 => PackEntryKind::Tree,
        3 => PackEntryKind::Blob,
        4 => PackEntryKind::Tag,
        6 => PackEntryKind::OfsDelta,
        7 => PackEntryKind::RefDelta,
        code => bail!("unsupported/invalid pack entry type code: {code}"),
    };

    let mut size = (first & 0x0f) as usize;
    let mut shift = 4u32;
    let mut cursor = offset + 1;
    let mut byte = first;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("pack entry header is truncated");
        }
        byte = pack_data[cursor];
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        cursor += 1;
    }

    Ok((entry_kind, size, cursor))
}

/// Parses OFS-delta encoded backward base distance.
pub(super) fn parse_ofs_delta_base_distance(
    pack_data: &[u8],
    offset: usize,
) -> Result<(usize, usize)> {
    if offset >= pack_data.len() {
        bail!("ofs-delta base encoding offset out of bounds");
    }
    let mut cursor = offset;
    let mut byte = pack_data[cursor];
    cursor += 1;

    let mut distance = (byte & 0x7f) as usize;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("ofs-delta base encoding is truncated");
        }
        byte = pack_data[cursor];
        cursor += 1;
        distance = ((distance + 1) << 7) + (byte & 0x7f) as usize;
    }

    Ok((distance, cursor - offset))
}

#[cfg(test)]
mod tests {
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
        let truncated_error = parse_pack_entry_header(&truncated, 0)
            .expect_err("truncated multi-byte header must fail");
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
}
