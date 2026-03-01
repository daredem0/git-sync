//! Core PACK stream verification/materialization orchestrator.

use crate::git::types::{
    PackEntryLedger, PayloadAuditError, PayloadPackProof, PayloadPackVerification,
};
use std::collections::HashMap;

use super::entry::{EntryProcessingState, process_next_entry};
use super::materialized::{
    build_materialized_object_index_from_ledger, build_materialized_object_store,
};
use super::object::ParsedPackObject;
use super::preflight::run_pack_preflight;

pub(super) fn verify_pack_payload_impl(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    let preflight = run_pack_preflight(pack_data)?;

    let mut ledger = PackEntryLedger {
        pack_version: preflight.pack_version,
        declared_entry_count: preflight.declared_entry_count,
        entries: Vec::with_capacity(preflight.declared_entry_count),
    };
    let mut offset = 12usize;
    let mut processed_object_count = 0usize;
    let mut thin_pack_detected = false;
    let mut baseline_resolutions_count = 0usize;
    let mut objects_by_offset = HashMap::<usize, ParsedPackObject>::new();
    let mut objects_by_oid = HashMap::<git2::Oid, ParsedPackObject>::new();

    while processed_object_count < preflight.declared_entry_count {
        if offset >= preflight.trailer_offset {
            return Err(PayloadAuditError {
                reason: format!(
                    "pack ended before declared object count was processed: declared={}, processed={}",
                    preflight.declared_entry_count, processed_object_count
                ),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger),
            });
        }

        let outcome = process_next_entry(
            pack_data,
            preflight.trailer_offset,
            baseline_odb,
            offset,
            processed_object_count,
            EntryProcessingState {
                ledger: &mut ledger,
                objects_by_offset: &mut objects_by_offset,
                objects_by_oid: &mut objects_by_oid,
                thin_pack_detected: &mut thin_pack_detected,
                baseline_resolutions_count: &mut baseline_resolutions_count,
            },
        )?;

        offset = outcome.next_offset;
        processed_object_count += 1;
    }

    if offset != preflight.trailer_offset {
        return Err(PayloadAuditError {
            reason: format!(
                "pack contains trailing or unconsumed bytes before trailer: consumed={}, trailer={}",
                offset, preflight.trailer_offset
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger),
        });
    }

    let materialized_index = build_materialized_object_index_from_ledger(&ledger);
    let materialized_store = build_materialized_object_store(&objects_by_oid);
    let proof = PayloadPackProof::from_entry_counters(
        preflight.pack_version,
        preflight.declared_entry_count,
        processed_object_count,
        materialized_index.materialized_entry_count,
        materialized_index.unique_object_count,
        materialized_index.duplicate_entry_count_materialized,
        true,
        thin_pack_detected,
        baseline_resolutions_count,
        "sha1".to_string(),
        preflight.computed_checksum,
        preflight.trailer_checksum,
    );

    Ok(PayloadPackVerification {
        proof,
        ledger,
        materialized_index,
        materialized_store,
    })
}

#[cfg(test)]
mod tests {
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
        let error = verify_pack_payload_impl(&pack, None)
            .expect_err("pack with unconsumed bytes must fail");
        assert!(
            error
                .reason
                .contains("pack contains trailing or unconsumed bytes before trailer"),
            "error should report trailing/unconsumed bytes"
        );
    }
}
