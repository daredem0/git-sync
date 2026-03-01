//! Proof-boundary types and invariant enforcement.

use crate::git::types::{PackEntryLedger, PayloadAuditError, PayloadPackVerification};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::git::bundle::payload) struct VerifiedPayload {
    verification: PayloadPackVerification,
}

impl VerifiedPayload {
    pub(in crate::git::bundle::payload) fn into_verification(self) -> PayloadPackVerification {
        self.verification
    }
}

pub(in crate::git::bundle::payload) fn verify_payload_invariants(
    verification: PayloadPackVerification,
) -> std::result::Result<VerifiedPayload, PayloadAuditError> {
    validate_proof_invariants(&verification)?;
    Ok(VerifiedPayload { verification })
}

fn validate_proof_invariants(
    verification: &PayloadPackVerification,
) -> std::result::Result<(), PayloadAuditError> {
    let proof = &verification.proof;
    let ledger = &verification.ledger;
    let index = &verification.materialized_index;

    if proof.entries_declared != ledger.declared_entry_count {
        return Err(invariant_violation(
            format!(
                "proof/ledger declared mismatch: proof={}, ledger={}",
                proof.entries_declared, ledger.declared_entry_count
            ),
            ledger,
        ));
    }
    if proof.entries_parsed != ledger.entries.len() {
        return Err(invariant_violation(
            format!(
                "proof/ledger parsed mismatch: proof={}, ledger={}",
                proof.entries_parsed,
                ledger.entries.len()
            ),
            ledger,
        ));
    }
    if proof.entries_parsed != proof.entries_declared {
        return Err(invariant_violation(
            format!(
                "parsed entries below declared count: parsed={}, declared={}",
                proof.entries_parsed, proof.entries_declared
            ),
            ledger,
        ));
    }
    if index.materialized_entry_count != proof.entries_materialized {
        return Err(invariant_violation(
            format!(
                "materialized entry count mismatch: index={}, proof={}",
                index.materialized_entry_count, proof.entries_materialized
            ),
            ledger,
        ));
    }
    if index.unique_object_count != proof.unique_objects_materialized {
        return Err(invariant_violation(
            format!(
                "unique object count mismatch: index={}, proof={}",
                index.unique_object_count, proof.unique_objects_materialized
            ),
            ledger,
        ));
    }
    if index.duplicate_entry_count_materialized != proof.duplicate_entry_count_materialized {
        return Err(invariant_violation(
            format!(
                "duplicate materialized count mismatch: index={}, proof={}",
                index.duplicate_entry_count_materialized, proof.duplicate_entry_count_materialized
            ),
            ledger,
        ));
    }
    if !proof.checksum_verified {
        return Err(invariant_violation(
            "pack checksum verification failed".to_string(),
            ledger,
        ));
    }
    if !proof.transfer_allowed {
        return Err(invariant_violation(
            proof
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "transfer gate blocked".to_string()),
            ledger,
        ));
    }

    Ok(())
}

fn invariant_violation(reason: String, ledger: &PackEntryLedger) -> PayloadAuditError {
    PayloadAuditError {
        reason,
        blocked_entry_idx: if ledger.entries.is_empty() {
            None
        } else {
            Some(ledger.entries.len() - 1)
        },
        ledger_partial: Some(ledger.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::types::{
        MaterializedObjectIndex, MaterializedObjectStore, PackEntryKind, PackEntryLedger,
        PackEntryRecord, PayloadObjectKind, PayloadPackProof, PayloadPackVerification,
        ResolutionSource,
    };

    fn empty_verification() -> PayloadPackVerification {
        PayloadPackVerification {
            proof: PayloadPackProof::from_entry_counters(
                2,
                0,
                0,
                0,
                0,
                0,
                true,
                false,
                0,
                "sha1".to_string(),
                "deadbeef".to_string(),
                "deadbeef".to_string(),
            ),
            ledger: PackEntryLedger {
                pack_version: 2,
                declared_entry_count: 0,
                entries: Vec::new(),
            },
            materialized_index: MaterializedObjectIndex {
                objects: Vec::new(),
                materialized_entry_count: 0,
                unique_object_count: 0,
                duplicate_entry_count_materialized: 0,
            },
            materialized_store: MaterializedObjectStore {
                objects: Vec::new(),
            },
        }
    }

    fn sample_record(idx: usize) -> PackEntryRecord {
        PackEntryRecord {
            idx,
            offset: 12 + idx,
            kind: PackEntryKind::Blob,
            out_size: 1,
            reconstructed_size: Some(1),
            base_ref: None,
            result_oid: Some(
                git2::Oid::from_str("1111111111111111111111111111111111111111")
                    .expect("must parse sample object id"),
            ),
            result_kind: Some(PayloadObjectKind::Blob),
            resolved: true,
            resolved_via: Some(ResolutionSource::InPack),
            note: None,
        }
    }

    // Verifies that the proof boundary accepts a self-consistent verification result.
    #[test]
    fn verify_payload_invariants_accepts_consistent_verification() {
        let verification = empty_verification();
        let verified = verify_payload_invariants(verification);
        assert!(verified.is_ok());
    }

    // Verifies that the proof boundary fails closed when proof counters disagree with index counters.
    #[test]
    fn verify_payload_invariants_rejects_counter_mismatch() {
        let mut verification = empty_verification();
        verification.proof.entries_materialized = 1;

        let error = verify_payload_invariants(verification).unwrap_err();
        assert!(error.reason.contains("materialized entry count mismatch"));
        assert!(error.ledger_partial.is_some());
    }

    #[test]
    fn verify_payload_invariants_rejects_declared_count_mismatch() {
        let mut verification = empty_verification();
        verification.proof.entries_declared = 1;

        let error = verify_payload_invariants(verification)
            .expect_err("proof/ledger declared count mismatch should fail invariant validation");
        assert!(error.reason.contains("proof/ledger declared mismatch"));
    }

    #[test]
    fn verify_payload_invariants_rejects_parsed_count_mismatch() {
        let mut verification = empty_verification();
        verification.proof.entries_parsed = 1;

        let error = verify_payload_invariants(verification)
            .expect_err("proof/ledger parsed count mismatch should fail invariant validation");
        assert!(error.reason.contains("proof/ledger parsed mismatch"));
    }

    #[test]
    fn verify_payload_invariants_rejects_when_parsed_is_below_declared() {
        let mut verification = empty_verification();
        verification.proof.entries_declared = 2;
        verification.ledger.declared_entry_count = 2;
        verification.proof.entries_parsed = 1;
        verification.ledger.entries = vec![sample_record(0)];

        let error = verify_payload_invariants(verification)
            .expect_err("parsed-below-declared mismatch should fail invariant validation");
        assert!(error.reason.contains("parsed entries below declared count"));
    }

    #[test]
    fn verify_payload_invariants_rejects_unique_object_count_mismatch() {
        let mut verification = empty_verification();
        verification.materialized_index.unique_object_count = 1;

        let error = verify_payload_invariants(verification)
            .expect_err("unique-object mismatch should fail invariant validation");
        assert!(error.reason.contains("unique object count mismatch"));
    }

    #[test]
    fn verify_payload_invariants_rejects_duplicate_count_mismatch() {
        let mut verification = empty_verification();
        verification
            .materialized_index
            .duplicate_entry_count_materialized = 1;

        let error = verify_payload_invariants(verification)
            .expect_err("duplicate-count mismatch should fail invariant validation");
        assert!(
            error
                .reason
                .contains("duplicate materialized count mismatch")
        );
    }

    #[test]
    fn verify_payload_invariants_rejects_failed_checksum() {
        let mut verification = empty_verification();
        verification.proof.checksum_verified = false;

        let error = verify_payload_invariants(verification)
            .expect_err("checksum failure should fail invariant validation");
        assert!(error.reason.contains("pack checksum verification failed"));
    }

    #[test]
    fn verify_payload_invariants_rejects_blocked_transfer_gate() {
        let mut verification = empty_verification();
        verification.proof.transfer_allowed = false;
        verification.proof.blocked_reason = Some("blocked by policy".to_string());

        let error = verify_payload_invariants(verification)
            .expect_err("blocked transfer gate should fail invariant validation");
        assert!(error.reason.contains("blocked by policy"));
    }
}
