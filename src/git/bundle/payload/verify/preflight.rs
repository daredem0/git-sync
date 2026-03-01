// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for preflight.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::digest::{hex_encode, sha1_hex};
use crate::git::types::PayloadAuditError;

#[derive(Debug, Clone)]
pub(super) struct PackPreflight {
    pub(super) pack_version: u32,
    pub(super) declared_entry_count: usize,
    pub(super) trailer_offset: usize,
    pub(super) computed_checksum: String,
    pub(super) trailer_checksum: String,
}

pub(super) fn run_pack_preflight(
    pack_data: &[u8],
) -> std::result::Result<PackPreflight, PayloadAuditError> {
    if pack_data.len() < 32 {
        return Err(PayloadAuditError {
            reason: "pack payload is too small".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }
    if &pack_data[..4] != b"PACK" {
        return Err(PayloadAuditError {
            reason: "pack payload does not start with PACK header".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    // Header bytes are guaranteed present by the minimum-size check above.
    let pack_version = u32::from_be_bytes(pack_data[4..8].try_into().expect("slice length"));
    if pack_version != 2 && pack_version != 3 {
        return Err(PayloadAuditError {
            reason: format!("unsupported pack version: {pack_version}"),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    // Header bytes are guaranteed present by the minimum-size check above.
    let declared_entry_count =
        u32::from_be_bytes(pack_data[8..12].try_into().expect("slice length")) as usize;

    // `pack_data.len() >= 32` guarantees a trailer is present.
    let trailer_offset = pack_data.len() - 20;

    let computed_checksum =
        sha1_hex(&pack_data[..trailer_offset]).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    let trailer_checksum = hex_encode(&pack_data[trailer_offset..]);
    if computed_checksum != trailer_checksum {
        return Err(PayloadAuditError {
            reason: format!(
                "pack trailer checksum mismatch: computed={}, trailer={}",
                computed_checksum, trailer_checksum
            ),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    Ok(PackPreflight {
        pack_version,
        declared_entry_count,
        trailer_offset,
        computed_checksum,
        trailer_checksum,
    })
}

#[cfg(test)]
mod tests;
