// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/input/actions.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::ui::tests::support::sample_model;
use crate::ui::types::{DiffViewState, ExportNotice, PayloadObjectViewState};
use crossterm::event::KeyCode;
use ratatui::text::Line;
use std::path::PathBuf;

fn oid(hex: &str) -> git2::Oid {
    git2::Oid::from_str(hex).expect("must parse test oid")
}

#[test]
fn apply_key_action_handles_quit_and_escape_paths() {
    let model = sample_model(2, 1);
    let mut state = AppState::new(&model);
    assert!(
        apply_key_action(&mut state, &model, KeyAction::Quit),
        "quit action should request application exit"
    );

    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: oid("1111111111111111111111111111111111111111"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line")],
        max_line_width: 8,
        scroll_y: 0,
        scroll_x: 0,
    });
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape should close diff before exiting"
    );
    assert!(state.diff_view.is_none());
    assert!(
        state.take_full_redraw_request(),
        "closing diff view should request one-shot full redraw"
    );

    state.payload_object_view = Some(PayloadObjectViewState {
        oid: oid("2222222222222222222222222222222222222222"),
        kind: crate::git::PayloadObjectKind::Blob,
        syntax_name: "none".to_string(),
        lines: vec![Line::from("line")],
        max_line_width: 8,
        scroll_y: 0,
        scroll_x: 0,
    });
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape should close payload object view before exiting"
    );
    assert!(state.payload_object_view.is_none());
    assert!(
        state.take_full_redraw_request(),
        "closing payload object view should request one-shot full redraw"
    );

    state.export_notice = Some(ExportNotice {
        path: PathBuf::from("sync.paudit.json"),
        exported_at_human_utc: "2026-03-03 12:34:56 UTC".to_string(),
    });
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape should close export notice before page navigation or exit"
    );
    assert!(
        state.export_notice.is_none(),
        "escape should clear export notice overlay state"
    );
    assert!(
        state.take_full_redraw_request(),
        "closing export notice should request one-shot full redraw"
    );

    state.page_index = 1;
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape should return to first page from commit pages"
    );
    assert_eq!(state.page_index, 0);

    assert!(
        apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape on overview without overlays should exit"
    );
}

#[test]
fn apply_key_action_handles_main_view_and_help_toggles() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);

    assert!(!state.show_help);
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::ToggleHelp),
        "help toggle should not exit"
    );
    assert!(state.show_help);
    assert!(
        !state.take_full_redraw_request(),
        "opening help overlay should not force a full redraw"
    );
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape should close help overlay before other navigation"
    );
    assert!(!state.show_help);
    assert!(
        state.take_full_redraw_request(),
        "closing help overlay should request one-shot full redraw"
    );

    assert_eq!(state.main_view, MainView::History);
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::ToggleMainView),
        "main view toggle should not exit"
    );
    assert_eq!(state.main_view, MainView::Payload);
}

#[test]
fn apply_key_action_escape_returns_to_graph_when_commit_opened_from_graph() {
    let model = sample_model(3, 1);
    let mut state = AppState::new(&model);
    state.show_history_graph_view();
    state.scroll_history_graph_down(&model, 1);
    let selected_graph_index = state.history_graph_scroll_y;

    assert!(
        !apply_key_action(&mut state, &model, KeyAction::HistoryOpenSelection),
        "opening selected graph commit should not exit"
    );
    assert!(
        !state.is_history_graph_view(),
        "opening selected graph commit should switch to commit pages"
    );
    assert!(
        state.should_return_to_graph_from_commit_page(),
        "graph-opened commit pages should mark graph as the escape return target"
    );

    assert!(
        !apply_key_action(&mut state, &model, KeyAction::Escape),
        "escape from graph-opened commit pages should not exit"
    );
    assert!(
        state.is_history_graph_view(),
        "escape should return to graph mode after opening commit detail from graph"
    );
    assert_eq!(
        state.page_index, 0,
        "escape should return to graph main page"
    );
    assert_eq!(
        state.history_graph_scroll_y, selected_graph_index,
        "escape should preserve graph selection row when returning"
    );
}

#[test]
fn apply_key_action_export_payload_audit_sets_failure_message_when_export_fails() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);

    assert!(
        !apply_key_action(&mut state, &model, KeyAction::ExportPayloadAuditJsonFull),
        "export action should never request app exit"
    );
    assert!(
        state
            .action_message
            .as_deref()
            .is_some_and(|message| message.contains("failed to export paudit")),
        "export failures should be surfaced as action messages"
    );

    assert!(
        !apply_key_action(&mut state, &model, KeyAction::ExportPayloadAuditJsonLight),
        "light export action should never request app exit"
    );
}

#[test]
fn apply_key_action_routes_history_and_payload_navigation_actions() {
    let model = sample_model(2, 1);
    let mut state = AppState::new(&model);

    assert!(
        !apply_key_action(&mut state, &model, KeyAction::HistoryOpenSelection),
        "history open action should not exit"
    );
    assert_eq!(
        state.page_index, 1,
        "history open from overview should enter commit page"
    );

    state.main_view = MainView::Payload;
    state.payload_selected_index = 0;
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionDown(1)),
        "payload down action should not exit"
    );
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionUp(1)),
        "payload up action should not exit"
    );
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionDown(10)),
        "payload pagedown-style move should not exit"
    );
    assert!(
        !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionUp(10)),
        "payload pageup-style move should not exit"
    );
}

#[test]
fn apply_diff_and_payload_object_actions_update_scroll_state() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: oid("3333333333333333333333333333333333333333"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line")],
        max_line_width: 12,
        scroll_y: 0,
        scroll_x: 0,
    });
    apply_diff_action(&mut state, DiffAction::ScrollDown(1));
    apply_diff_action(&mut state, DiffAction::ScrollRight(2));
    apply_diff_action(&mut state, DiffAction::Reset);
    let diff = state
        .diff_view
        .as_ref()
        .expect("diff view should remain open");
    assert_eq!(diff.scroll_y, 0);
    assert_eq!(diff.scroll_x, 0);

    state.payload_object_view = Some(PayloadObjectViewState {
        oid: oid("4444444444444444444444444444444444444444"),
        kind: crate::git::PayloadObjectKind::Blob,
        syntax_name: "none".to_string(),
        lines: vec![Line::from("line"), Line::from("line2")],
        max_line_width: 12,
        scroll_y: 0,
        scroll_x: 0,
    });
    apply_payload_object_action(&mut state, PayloadObjectAction::ScrollDown(1));
    apply_payload_object_action(&mut state, PayloadObjectAction::ScrollRight(2));
    apply_payload_object_action(&mut state, PayloadObjectAction::Reset);
    let payload_object = state
        .payload_object_view
        .as_ref()
        .expect("payload object view should remain open");
    assert_eq!(payload_object.scroll_y, 0);
    assert_eq!(payload_object.scroll_x, 0);
    // Keep KeyCode imported to avoid unused warnings across test cfg permutations.
    let _ = KeyCode::Null;
}
