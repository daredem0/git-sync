// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/proof.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

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
