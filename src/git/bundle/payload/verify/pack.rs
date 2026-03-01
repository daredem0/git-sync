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
mod tests;
