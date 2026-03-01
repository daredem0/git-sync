// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/types/payload_materialized.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::git::types::PackEntryLedger;

fn sample_ledger(entries_parsed: usize, entries_declared: usize) -> PackEntryLedger {
    PackEntryLedger {
        pack_version: 2,
        declared_entry_count: entries_declared,
        entries: (0..entries_parsed)
            .map(|idx| crate::git::types::PackEntryRecord {
                idx,
                offset: 12 + idx,
                kind: crate::git::types::PackEntryKind::Blob,
                out_size: 1,
                reconstructed_size: Some(1),
                base_ref: None,
                result_oid: Some(
                    git2::Oid::from_str("1111111111111111111111111111111111111111")
                        .expect("must parse test oid"),
                ),
                result_kind: Some(crate::git::types::PayloadObjectKind::Blob),
                resolved: true,
                resolved_via: Some(crate::git::types::ResolutionSource::InPack),
                note: None,
            })
            .collect(),
    }
}

#[test]
fn payload_audit_error_display_with_blocked_index_and_partial_ledger() {
    let error = PayloadAuditError {
        reason: "failure".to_string(),
        blocked_entry_idx: Some(3),
        ledger_partial: Some(sample_ledger(2, 7)),
    };
    let text = error.to_string();
    assert!(text.contains("failure"));
    assert!(text.contains("blocked_entry_idx=3"));
    assert!(text.contains("entries_parsed=2"));
    assert!(text.contains("entries_declared=7"));
}

#[test]
fn payload_audit_error_display_with_blocked_index_only() {
    let error = PayloadAuditError {
        reason: "failure".to_string(),
        blocked_entry_idx: Some(1),
        ledger_partial: None,
    };
    let text = error.to_string();
    assert!(text.contains("failure"));
    assert!(text.contains("blocked_entry_idx=1"));
    assert!(!text.contains("entries_parsed="));
}

#[test]
fn payload_audit_error_display_with_partial_ledger_only() {
    let error = PayloadAuditError {
        reason: "failure".to_string(),
        blocked_entry_idx: None,
        ledger_partial: Some(sample_ledger(4, 9)),
    };
    let text = error.to_string();
    assert!(text.contains("failure"));
    assert!(text.contains("entries_parsed=4"));
    assert!(text.contains("entries_declared=9"));
    assert!(!text.contains("blocked_entry_idx="));
}

#[test]
fn payload_audit_error_display_with_reason_only() {
    let error = PayloadAuditError {
        reason: "failure-only".to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    };
    assert_eq!(error.to_string(), "failure-only");
}
