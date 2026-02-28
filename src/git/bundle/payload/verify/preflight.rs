//! PACK preflight parsing and checksum validation.

use crate::git::digest::{hex_encode, sha1_hex};
use crate::git::types::PayloadAuditError;

use super::pack::read_be_u32;

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

    let pack_version = read_be_u32(pack_data, 4).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    if pack_version != 2 && pack_version != 3 {
        return Err(PayloadAuditError {
            reason: format!("unsupported pack version: {pack_version}"),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    let declared_entry_count = read_be_u32(pack_data, 8).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })? as usize;

    let trailer_offset = pack_data
        .len()
        .checked_sub(20)
        .ok_or_else(|| PayloadAuditError {
            reason: "pack payload missing trailer checksum".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;

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
