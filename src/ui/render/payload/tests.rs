// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/render/payload/mod.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::payload_title_text;
use crate::ui::tests::support::sample_model;
use crate::ui::types::PayloadModel;

#[test]
fn payload_title_text_for_ok_payload_reports_integrity_and_transfer_status() {
    let mut model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload data");
    };
    payload.pack_proof.verification_status = "ok".to_string();
    payload.pack_proof.checksum_verified = true;
    payload.pack_proof.transfer_allowed = false;
    payload.pack_proof.blocked_reason = Some("policy gate".to_string());
    payload.pack_proof.thin_pack_detected = true;
    payload.pack_proof.baseline_resolutions_count = 2;

    let state = crate::ui::types::AppState::new(&model);
    let title = payload_title_text(&model, &state);
    assert!(
        title.contains("status: ok"),
        "title should include overall proof status"
    );
    assert!(
        title.contains("transfer: blocked (policy gate)"),
        "title should include blocked transfer reason"
    );
    assert!(title.contains("thin pack: yes"));
    assert!(title.contains("baseline resolutions: 2"));
}

#[test]
fn payload_title_text_for_failed_payload_uses_fallback_text() {
    let mut model = sample_model(1, 1);
    model.payload = PayloadModel::Failed("payload unavailable".to_string());
    let state = crate::ui::types::AppState::new(&model);

    let title = payload_title_text(&model, &state);
    assert!(
        title.contains("Payload View"),
        "fallback title should still identify payload view"
    );
    assert!(
        title.contains("Transport package entries"),
        "fallback text should include high-level payload page guidance"
    );
}
