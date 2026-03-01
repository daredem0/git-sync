// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/state/payload_ops.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::git::{PayloadObjectDetail, PayloadObjectKind};
use crate::ui::tests::support::sample_model;
use crate::ui::types::PayloadModel;

fn oid(value: &str) -> git2::Oid {
    git2::Oid::from_str(value).expect("must parse test oid")
}

fn preview_state_for(oid: git2::Oid, kind: PayloadObjectKind) -> PayloadPreviewState {
    PayloadPreviewState {
        oid,
        kind,
        lines: vec!["cached preview".to_string()],
        syntax_path_hint: None,
        syntax_start_index: None,
    }
}

#[test]
fn refresh_payload_preview_clears_preview_for_unresolved_entries_row() {
    let mut model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload data");
    };
    payload.entry_ledger.entries[0].result_oid = None;
    payload.entry_ledger.entries[0].result_kind = None;
    payload.entry_ledger.entries[0].resolved = false;

    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Entries;

    state.refresh_payload_preview(&model);
    assert!(
        state.payload_preview.is_none(),
        "preview should be absent when selected entry has no resolved object id"
    );
}

#[test]
fn refresh_payload_preview_clears_preview_when_payload_model_is_failed() {
    let mut model = sample_model(1, 1);
    model.payload = PayloadModel::Failed("payload failed".to_string());
    let mut state = AppState::new(&model);
    state.payload_preview = Some(preview_state_for(
        oid("1111111111111111111111111111111111111111"),
        PayloadObjectKind::Blob,
    ));

    state.refresh_payload_preview(&model);
    assert!(
        state.payload_preview.is_none(),
        "preview must be cleared when payload data is unavailable"
    );
}

#[test]
fn open_selected_payload_object_in_entries_reports_unresolved_row() {
    let mut model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload data");
    };
    payload.entry_ledger.entries[0].result_oid = None;
    payload.entry_ledger.entries[0].result_kind = None;
    payload.entry_ledger.entries[0].resolved = false;

    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Entries;

    state.open_selected_payload_object(&model);
    assert!(
        state
            .action_message
            .as_deref()
            .is_some_and(|message| message.contains("selected entry is unresolved")),
        "entries subview should explain when selected row has no resolved object detail"
    );
}

#[test]
fn open_selected_payload_object_in_entries_opens_detail_for_resolved_row() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Entries;

    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload data");
    };
    let oid = payload.entry_ledger.entries[0]
        .result_oid
        .expect("first entry should resolve to object id");
    state.payload_detail_cache.insert(
        oid,
        PayloadObjectDetail {
            oid,
            kind: PayloadObjectKind::Commit,
            size_bytes: 32,
            syntax_path_hint: None,
            blob_paths: Vec::new(),
            text_line_count: Some(1),
            lines: vec!["commit".to_string(), String::new(), "body".to_string()],
        },
    );

    state.open_selected_payload_object(&model);
    assert!(
        state.payload_object_view.is_some(),
        "entries subview should open detail for resolved rows"
    );
    assert!(
        state.action_message.is_none(),
        "resolved entries should not produce an action error"
    );
}

#[test]
fn open_selected_payload_object_reports_unavailable_payload_data() {
    let mut model = sample_model(1, 1);
    model.payload = PayloadModel::Failed("payload failed".to_string());
    let mut state = AppState::new(&model);

    state.open_selected_payload_object(&model);
    assert!(
        state
            .action_message
            .as_deref()
            .is_some_and(|message| message.contains("payload audit data is unavailable")),
        "missing payload data should be surfaced in action message"
    );
}

#[test]
fn open_selected_payload_object_reports_empty_objects_list() {
    let mut model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload data");
    };
    payload.objects.clear();
    let mut state = AppState::new(&model);

    state.open_selected_payload_object(&model);
    assert!(
        state
            .action_message
            .as_deref()
            .is_some_and(|message| message.contains("payload contains no importable objects")),
        "empty payload list should emit a clear action message"
    );
}

#[test]
fn toggle_payload_sub_view_with_failed_payload_resets_preview_and_selection() {
    let mut model = sample_model(1, 1);
    model.payload = PayloadModel::Failed("payload failed".to_string());
    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Objects;
    state.payload_selected_index = 9;
    state.payload_preview = Some(preview_state_for(
        oid("2222222222222222222222222222222222222222"),
        PayloadObjectKind::Blob,
    ));

    state.toggle_payload_sub_view(&model);
    assert_eq!(
        state.payload_sub_view,
        PayloadSubView::Entries,
        "subview toggle should still occur even when payload model is failed"
    );
    assert_eq!(
        state.payload_selected_index, 0,
        "failed payload model should reset payload selection index"
    );
    assert!(
        state.payload_preview.is_none(),
        "failed payload model should clear stale preview content"
    );
}

#[test]
fn payload_selected_entry_returns_none_when_not_in_entries_subview() {
    let model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload data");
    };
    let state = AppState::new(&model);

    let selected = state.payload_selected_entry(payload.as_ref());
    assert!(
        selected.is_none(),
        "selected entry lookup should be disabled outside entries subview"
    );
}

#[test]
fn payload_selected_entry_clamps_to_last_entry_row() {
    let model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload data");
    };
    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Entries;
    state.payload_selected_index = usize::MAX;

    let selected = state
        .payload_selected_entry(payload.as_ref())
        .expect("entries subview should provide selected ledger row");
    let last = payload
        .entry_ledger
        .entries
        .last()
        .expect("fixture ledger should contain at least one row");
    assert_eq!(
        selected.idx, last.idx,
        "selected ledger row should clamp to final entry index"
    );
}

#[test]
fn cycle_payload_sort_mode_is_noop_in_entries_subview() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);
    state.payload_sub_view = PayloadSubView::Entries;
    state.payload_sort_mode = PayloadSortMode::Canonical;

    state.cycle_payload_sort_mode(&model);
    assert_eq!(
        state.payload_sort_mode,
        PayloadSortMode::Canonical,
        "sort mode should not change outside objects subview"
    );
}

#[test]
fn line_number_width_returns_expected_digit_counts() {
    assert_eq!(line_number_width(0), 1);
    assert_eq!(line_number_width(9), 1);
    assert_eq!(line_number_width(10), 2);
    assert_eq!(line_number_width(123), 3);
}

#[test]
fn build_payload_preview_state_reports_unreachable_blob_without_paths() {
    let detail = PayloadObjectDetail {
        oid: oid("3333333333333333333333333333333333333333"),
        kind: PayloadObjectKind::Blob,
        size_bytes: 8,
        syntax_path_hint: None,
        blob_paths: Vec::new(),
        text_line_count: None,
        lines: vec![
            "binary blob".to_string(),
            "size: 8 bytes".to_string(),
            "hex preview:".to_string(),
            String::new(),
            "ff ee".to_string(),
        ],
    };

    let preview = build_payload_preview_state(&detail, false);
    assert!(
        preview
            .lines
            .iter()
            .any(|line| line.contains("unreachable from advertised heads")),
        "preview should explain missing blob paths for unreachable object"
    );
    assert_eq!(
        preview.syntax_start_index, None,
        "syntax start should be absent without syntax path hint"
    );
}

#[test]
fn build_payload_preview_state_sets_syntax_start_for_text_blob_preview_body() {
    let detail = PayloadObjectDetail {
        oid: oid("4444444444444444444444444444444444444444"),
        kind: PayloadObjectKind::Blob,
        size_bytes: 14,
        syntax_path_hint: Some("src/lib.rs".to_string()),
        blob_paths: vec!["src/lib.rs".to_string()],
        text_line_count: Some(1),
        lines: vec![
            "text blob".to_string(),
            "size: 14 bytes".to_string(),
            "text lines: 1".to_string(),
            String::new(),
            "fn main() {}".to_string(),
        ],
    };

    let preview = build_payload_preview_state(&detail, true);
    let syntax_start = preview
        .syntax_start_index
        .expect("text blob preview should expose syntax body start index");
    assert_eq!(preview.lines[syntax_start], "fn main() {}");
}
