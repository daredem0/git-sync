//! PACK preflight parsing and checksum validation.

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
mod tests {
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
}
