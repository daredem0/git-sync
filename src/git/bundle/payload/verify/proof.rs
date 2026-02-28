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
        MaterializedObjectIndex, MaterializedObjectStore, PackEntryLedger, PayloadPackProof,
        PayloadPackVerification,
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
}
