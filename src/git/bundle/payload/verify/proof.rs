// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for proof.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

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
mod tests;
