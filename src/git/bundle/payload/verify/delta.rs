// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for delta.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use anyhow::{Result, anyhow, bail};

/// Applies a git delta bytecode stream to `base`, producing reconstructed object content.
pub(super) fn apply_git_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = 0usize;
    let (source_size, source_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += source_size_consumed;
    if source_size != base.len() {
        bail!(
            "delta source size mismatch: expected={}, actual={}",
            source_size,
            base.len()
        );
    }

    let (target_size, target_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += target_size_consumed;
    let mut out = Vec::with_capacity(target_size);

    while cursor < delta.len() {
        let opcode = delta[cursor];
        cursor += 1;

        if (opcode & 0x80) != 0 {
            let mut copy_offset = 0usize;
            let mut copy_size = 0usize;

            if (opcode & 0x01) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 0")?;
                copy_offset |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x02) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 1")?;
                copy_offset |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x04) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 2")?;
                copy_offset |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if (opcode & 0x08) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 3")?;
                copy_offset |= (delta[cursor] as usize) << 24;
                cursor += 1;
            }

            if (opcode & 0x10) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 0")?;
                copy_size |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x20) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 1")?;
                copy_size |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x40) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 2")?;
                copy_size |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if copy_size == 0 {
                copy_size = 0x10000;
            }

            let copy_end = copy_offset
                .checked_add(copy_size)
                .ok_or_else(|| anyhow!("delta copy range overflow"))?;
            if copy_end > base.len() {
                bail!(
                    "delta copy range exceeds base object: offset={}, size={}, base={}",
                    copy_offset,
                    copy_size,
                    base.len()
                );
            }
            out.extend_from_slice(&base[copy_offset..copy_end]);
        } else if opcode != 0 {
            let literal_size = opcode as usize;
            ensure_remaining(delta, cursor, literal_size, "delta literal chunk")?;
            out.extend_from_slice(&delta[cursor..cursor + literal_size]);
            cursor += literal_size;
        } else {
            bail!("invalid delta opcode 0x00");
        }
    }

    if out.len() != target_size {
        bail!(
            "delta result size mismatch: expected={}, actual={}",
            target_size,
            out.len()
        );
    }
    Ok(out)
}

/// Parses one git delta varint from `bytes` at `offset`.
fn parse_git_delta_varint(bytes: &[u8], offset: usize) -> Result<(usize, usize)> {
    if offset >= bytes.len() {
        bail!("delta varint is out of bounds");
    }
    let mut cursor = offset;
    let mut value = 0usize;
    let mut shift = 0u32;
    loop {
        ensure_remaining(bytes, cursor, 1, "delta varint byte")?;
        let byte = bytes[cursor];
        cursor += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, cursor - offset))
}

/// Ensures `len` bytes are available from `offset`.
fn ensure_remaining(bytes: &[u8], offset: usize, len: usize, context: &str) -> Result<()> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| anyhow!("{context}: offset overflow"))?;
    if end > bytes.len() {
        bail!("{context}: truncated data");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
