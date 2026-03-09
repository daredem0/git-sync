// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for input behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: keyboard event handling, page/diff key behavior, and exit/help toggles.

use super::super::input::{
    handle_diff_keys, handle_key_press, handle_page_keys, handle_payload_object_keys,
};
use super::super::types::{
    DiffViewState, ExportNotice, MainView, OverviewFocus, PayloadModel, PayloadSubView,
};
use super::support::*;
use crate::git::PayloadObjectKind;
use crossterm::event::KeyCode;
use ratatui::text::Line;
use std::path::PathBuf;

// Verifies that Esc closes diff view without requesting app exit, and Esc exits when no diff is open.
#[test]
fn handle_key_press_esc_closes_diff_then_exits() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 10,
        scroll_y: 0,
        scroll_x: 0,
    });

    let should_exit_with_diff = handle_key_press(&mut state, &model, KeyCode::Esc);
    assert!(
        !should_exit_with_diff,
        "Esc should close active diff view instead of exiting application"
    );
    assert!(
        state.diff_view.is_none(),
        "Esc should clear diff view state when diff is open"
    );

    let should_exit_without_diff = handle_key_press(&mut state, &model, KeyCode::Esc);
    assert!(
        should_exit_without_diff,
        "Esc should request exit when no diff view is active"
    );
}

// Verifies that pressing q through unified key handler requests app termination.
#[test]
fn handle_key_press_q_requests_exit() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    assert!(
        handle_key_press(&mut state, &model, KeyCode::Char('q')),
        "q should request loop exit"
    );
}

// Verifies that pressing ? through unified key handler toggles the help flag.
#[test]
fn handle_key_press_question_toggles_help() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    assert!(
        !state.show_help,
        "precondition: help starts hidden for new app state"
    );
    let should_exit = handle_key_press(&mut state, &model, KeyCode::Char('?'));
    assert!(!should_exit, "toggling help should not request app exit");
    assert!(state.show_help, "help flag should flip to true");
    assert_eq!(
        state.help_page_index, 0,
        "opening help should reset to the first help page"
    );
}

// Verifies that help-page navigation keys are consumed by the help overlay and do not mutate page selection.
#[test]
fn handle_key_press_help_overlay_consumes_navigation_for_help_paging() {
    let model = sample_multi_head_model(&[2, 2]);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;
    state.selected_head_index = 1;
    state.selected_file_indices[1][0] = 1;

    let opened = handle_key_press(&mut state, &model, KeyCode::Char('?'));
    assert!(!opened, "opening help should not exit");
    assert!(state.show_help, "help should be visible after '?'");

    let next_page = handle_key_press(&mut state, &model, KeyCode::PageDown);
    assert!(!next_page, "help page switch should not exit");
    assert_eq!(
        state.help_page_index, 1,
        "help PageDown should advance to second help page"
    );
    assert_eq!(
        state.page_index, 1,
        "help paging should not trigger commit-page navigation"
    );
    assert_eq!(
        state.selected_file_indices[1][0], 1,
        "help paging should not change selected file row"
    );

    let third_page = handle_key_press(&mut state, &model, KeyCode::PageDown);
    assert!(!third_page, "help page switch should not exit");
    assert_eq!(
        state.help_page_index, 2,
        "second help PageDown should advance to third help page"
    );

    let clamped = handle_key_press(&mut state, &model, KeyCode::PageDown);
    assert!(!clamped, "help page switch should not exit");
    assert_eq!(
        state.help_page_index, 2,
        "help paging should clamp at last available page"
    );

    let prev_page = handle_key_press(&mut state, &model, KeyCode::PageUp);
    assert!(!prev_page, "help page switch should not exit");
    assert_eq!(
        state.help_page_index, 1,
        "help PageUp should move back to second help page"
    );
}

// Verifies that Esc closes help overlay before normal escape routing.
#[test]
fn handle_key_press_esc_closes_help_before_page_navigation_or_exit() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.show_help = true;
    state.help_page_index = 1;

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Esc);
    assert!(
        !should_exit,
        "Esc should close help overlay first instead of exiting"
    );
    assert!(!state.show_help, "Esc should hide help overlay");
    assert_eq!(
        state.help_page_index, 0,
        "closing help should reset paging to first page"
    );
}

// Verifies that Esc closes export notice overlay before normal escape routing.
#[test]
fn handle_key_press_esc_closes_export_notice_before_exit() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.export_notice = Some(ExportNotice {
        path: PathBuf::from("sync.paudit.json"),
        exported_at_human_utc: "2026-03-03 12:34:56 UTC".to_string(),
    });

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Esc);
    assert!(
        !should_exit,
        "Esc should close export notice overlay first instead of exiting"
    );
    assert!(
        state.export_notice.is_none(),
        "Esc should hide export notice overlay"
    );
}

// Verifies that Esc from a commit page returns to overview instead of exiting immediately.
#[test]
fn handle_key_press_esc_on_commit_page_returns_to_overview() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Esc);

    assert!(
        !should_exit,
        "Esc on a commit page should navigate back to overview"
    );
    assert_eq!(state.page_index, 0, "Esc should return to overview page");
}

// Verifies that Enter on commit pages sets an error message when patch loading fails.
#[test]
fn handle_page_keys_enter_sets_error_when_patch_load_fails() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;
    handle_page_keys(&mut state, &model, KeyCode::Enter);
    assert!(
        state
            .action_message
            .as_deref()
            .is_some_and(|msg| msg.contains("failed to open patch view")),
        "enter should expose a helpful failure message when patch load cannot be performed"
    );
}

// Verifies that Enter on overview enters commit-page mode instead of attempting to open a diff.
#[test]
fn handle_page_keys_enter_on_overview_enters_commit_pages() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(state.page_index, 0, "precondition: start on overview page");

    handle_page_keys(&mut state, &model, KeyCode::Enter);

    assert_eq!(
        state.page_index, 1,
        "Enter on overview should navigate to first commit page"
    );
    assert!(
        state.diff_view.is_none(),
        "Enter on overview should not directly open a diff view"
    );
}

// Verifies that page-mode shortcuts switch between history and payload top-level views.
#[test]
fn handle_page_keys_view_switch_shortcuts_toggle_and_select_views() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 0;

    handle_page_keys(&mut state, &model, KeyCode::Char('2'));
    assert_eq!(
        state.main_view,
        MainView::Payload,
        "key 2 should switch to payload view"
    );
    assert_eq!(
        state.page_index, 0,
        "switching to payload view should reset to main page index"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('1'));
    assert_eq!(
        state.main_view,
        MainView::History,
        "key 1 should switch back to history view"
    );
    assert_eq!(state.page_index, 0, "key 1 should return to overview page");

    handle_page_keys(&mut state, &model, KeyCode::Char('v'));
    assert_eq!(
        state.main_view,
        MainView::Payload,
        "key v should toggle from history to payload view"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    assert_eq!(
        state.main_view,
        MainView::Payload,
        "Tab should no longer toggle payload/history views"
    );
}

// Verifies that key `1` from commit-detail page returns to overview page in history view.
#[test]
fn handle_page_keys_key_one_from_commit_page_returns_overview() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 1;

    handle_page_keys(&mut state, &model, KeyCode::Char('1'));

    assert_eq!(
        state.main_view,
        MainView::History,
        "key 1 should keep history view active"
    );
    assert_eq!(
        state.page_index, 0,
        "key 1 should return from commit detail to overview page"
    );
}

// Verifies that payload subview defaults to Objects in fresh app state.
#[test]
fn payload_subview_defaults_to_objects() {
    let model = sample_model(1, 1);
    let state = super::super::types::AppState::new(&model);
    assert_eq!(
        state.payload_sub_view,
        PayloadSubView::Objects,
        "new app state should default payload subview to Objects"
    );
}

// Verifies that key `e` toggles payload subview between Objects and Entries on payload main page.
#[test]
fn payload_key_e_toggles_objects_entries() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.page_index = 0;

    handle_page_keys(&mut state, &model, KeyCode::Char('e'));
    assert_eq!(
        state.payload_sub_view,
        PayloadSubView::Entries,
        "first e press should switch payload subview to Entries"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('e'));
    assert_eq!(
        state.payload_sub_view,
        PayloadSubView::Objects,
        "second e press should switch payload subview back to Objects"
    );
}

// Verifies that direct view shortcuts remain available even when currently on a commit page.
#[test]
fn handle_page_keys_view_switch_shortcuts_work_off_main_page() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;
    state.main_view = MainView::History;

    handle_page_keys(&mut state, &model, KeyCode::Char('2'));
    assert_eq!(
        state.main_view,
        MainView::Payload,
        "key 2 should switch to payload view from commit pages"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('1'));
    assert_eq!(
        state.main_view,
        MainView::History,
        "key 1 should switch back to history view from any page mode"
    );
}

// Verifies that key `3` opens commit detail flow from overview by entering selected head.
#[test]
fn handle_page_keys_key_three_opens_commit_detail_from_overview() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(state.page_index, 0, "precondition: overview page");

    handle_page_keys(&mut state, &model, KeyCode::Char('3'));

    assert_eq!(
        state.page_index, 1,
        "key 3 should enter first commit detail page for selected head"
    );
    assert!(
        state.diff_view.is_none(),
        "key 3 should not open inline diff directly"
    );
    assert!(
        state.action_message.is_none(),
        "key 3 should not set a diff-load failure message"
    );
}

// Verifies that key `3` from payload view returns to history and opens first commit detail page.
#[test]
fn handle_page_keys_key_three_from_payload_switches_to_history_commit_detail() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    handle_page_keys(&mut state, &model, KeyCode::Char('3'));

    assert_eq!(
        state.main_view,
        MainView::History,
        "key 3 should switch from payload to history to open commit detail context"
    );
    assert_eq!(
        state.page_index, 1,
        "key 3 should enter first commit page while opening commit detail shortcut"
    );
    assert!(
        state.diff_view.is_none(),
        "key 3 should not directly open file diff from payload view"
    );
}

// Verifies that key `4` opens commit graph mode from history overview.
#[test]
fn handle_page_keys_key_four_opens_commit_graph_mode() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(state.page_index, 0, "precondition: overview page");
    assert!(
        !state.is_history_graph_view(),
        "precondition: graph mode should start disabled"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('4'));

    assert_eq!(
        state.main_view,
        MainView::History,
        "key 4 should keep/switch to history view"
    );
    assert!(
        state.is_history_graph_view(),
        "key 4 should activate history graph mode"
    );
}

// Verifies that key `4` from payload switches to history and opens commit graph mode.
#[test]
fn handle_page_keys_key_four_from_payload_switches_to_history_graph_mode() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    handle_page_keys(&mut state, &model, KeyCode::Char('4'));

    assert_eq!(
        state.main_view,
        MainView::History,
        "key 4 should switch back to history view"
    );
    assert!(
        state.is_history_graph_view(),
        "key 4 should activate history graph mode from payload view"
    );
}

// Verifies that graph page supports vertical scrolling via Up/Down and PageUp/PageDown keys.
#[test]
fn handle_page_keys_graph_scroll_shortcuts_update_graph_offset() {
    let model = sample_model(40, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.show_history_graph_view();

    handle_page_keys(&mut state, &model, KeyCode::Down);
    assert_eq!(
        state.history_graph_scroll_y, 1,
        "Down should move graph offset by one row"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageDown);
    assert_eq!(
        state.history_graph_scroll_y, 21,
        "PageDown should move graph offset by fast-scroll step"
    );

    handle_page_keys(&mut state, &model, KeyCode::Up);
    assert_eq!(
        state.history_graph_scroll_y, 20,
        "Up should move graph offset up by one row"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageUp);
    assert_eq!(
        state.history_graph_scroll_y, 0,
        "PageUp should move graph offset up by fast-scroll step with floor clamp"
    );
}

// Verifies that Tab on overview toggles focus between heads and would-change tables.
#[test]
fn handle_page_keys_tab_toggles_overview_focus() {
    let model = sample_multi_head_model(&[2, 2]);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(
        state.overview_focus,
        OverviewFocus::Heads,
        "precondition: overview focus starts on heads table"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    assert_eq!(
        state.overview_focus,
        OverviewFocus::WouldChange,
        "first Tab should focus would-change table"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    assert_eq!(
        state.overview_focus,
        OverviewFocus::Heads,
        "second Tab should return focus to heads table"
    );
}

// Verifies that overview navigation keys affect only the currently focused table.
#[test]
fn handle_page_keys_overview_navigation_applies_to_focused_table_only() {
    let model = sample_multi_head_model(&[3, 3]);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(
        state.selected_head_index, 0,
        "precondition: head 0 selected"
    );
    assert_eq!(
        state.selected_change_index, 0,
        "precondition: first would-change row selected"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    handle_page_keys(&mut state, &model, KeyCode::Down);
    assert_eq!(
        state.selected_head_index, 0,
        "while would-change is focused, head selection should remain unchanged"
    );
    assert_eq!(
        state.selected_change_index, 1,
        "while would-change is focused, down key should move would-change selection"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    handle_page_keys(&mut state, &model, KeyCode::Down);
    assert_eq!(
        state.selected_head_index, 1,
        "while heads are focused, down key should move head selection"
    );
    assert_eq!(
        state.selected_change_index, 0,
        "changing head selection should reset would-change selection to first row"
    );
}

// Verifies that history page navigation keys are ignored while payload view is selected.
#[test]
fn handle_page_keys_ignores_history_paging_in_payload_view() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.page_index = 0;

    handle_page_keys(&mut state, &model, KeyCode::Right);

    assert_eq!(
        state.page_index, 0,
        "payload mode should ignore history page navigation keys"
    );
}

// Verifies that right-arrow pagination no longer leaves overview for commit pages.
#[test]
fn handle_page_keys_right_arrow_does_not_leave_overview() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 0;

    handle_page_keys(&mut state, &model, KeyCode::Right);

    assert_eq!(
        state.page_index, 0,
        "right arrow on overview should not navigate into commit pages"
    );
}

// Verifies that left-arrow on first commit page does not navigate back to overview.
#[test]
fn handle_page_keys_left_arrow_on_first_commit_stays_on_commit_page() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 1;

    handle_page_keys(&mut state, &model, KeyCode::Left);

    assert_eq!(
        state.page_index, 1,
        "left arrow on first commit page should not navigate back to overview"
    );
}

// Verifies that key `1` from open diff view closes diff and returns to history overview.
#[test]
fn handle_key_press_key_one_from_diff_view_returns_overview() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 1;
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 10,
        scroll_y: 0,
        scroll_x: 0,
    });

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Char('1'));
    assert!(!should_exit, "key 1 should navigate, not exit");
    assert!(
        state.diff_view.is_none(),
        "key 1 should close open diff view when returning to overview"
    );
    assert_eq!(
        state.main_view,
        MainView::History,
        "key 1 should switch/keep history view"
    );
    assert_eq!(
        state.page_index, 0,
        "key 1 should return to overview page from diff view"
    );
}

// Verifies that payload-page navigation keys move object selection and Enter opens object detail view.
#[test]
fn handle_page_keys_payload_navigation_and_enter_open_detail() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    handle_page_keys(&mut state, &model, KeyCode::Down);
    assert!(
        state.payload_selected_index <= 1,
        "payload selection should stay in valid object-row range"
    );

    handle_page_keys(&mut state, &model, KeyCode::Enter);
    assert!(
        state.payload_object_view.is_some(),
        "Enter on payload object row should open object detail view"
    );
}

// Verifies that PageUp/PageDown in payload view jump pack-object selection by ten rows.
#[test]
fn handle_page_keys_payload_pageup_pagedown_jump_by_ten_rows() {
    let fixture = create_diff_fixture();
    let mut model = build_model_from_fixture(&fixture);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload audit data");
    };
    let base_objects = payload.objects.clone();
    while payload.objects.len() < 25 {
        payload.objects.extend(base_objects.clone());
    }
    payload.objects.truncate(25);

    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    handle_page_keys(&mut state, &model, KeyCode::PageDown);
    assert_eq!(
        state.payload_selected_index, 10,
        "PageDown should jump payload selection down by ten rows"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageDown);
    assert_eq!(
        state.payload_selected_index, 20,
        "second PageDown should jump another ten rows"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageDown);
    assert_eq!(
        state.payload_selected_index, 24,
        "PageDown should clamp at final object row"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageUp);
    assert_eq!(
        state.payload_selected_index, 14,
        "PageUp should jump payload selection up by ten rows"
    );

    handle_page_keys(&mut state, &model, KeyCode::PageUp);
    handle_page_keys(&mut state, &model, KeyCode::PageUp);
    assert_eq!(
        state.payload_selected_index, 0,
        "PageUp should clamp at first object row"
    );
}

// Verifies that `s` in payload view cycles list sort mode while preserving selected object identity.
#[test]
fn handle_page_keys_payload_sort_cycle_preserves_selected_object() {
    let mut model = sample_model(1, 1);
    {
        let PayloadModel::Ok(payload) = &mut model.payload else {
            panic!("fixture model must include payload audit data");
        };
        payload.objects = vec![
            crate::git::PayloadObjectEntry {
                oid: git2::Oid::from_str("3000000000000000000000000000000000000000")
                    .expect("valid oid"),
                kind: PayloadObjectKind::Blob,
                size_bytes: 12,
                reachable_from_heads: true,
                context_head_index: Some(1),
                context_commit_order: Some(1),
                context_path: Some("z.txt".to_string()),
            },
            crate::git::PayloadObjectEntry {
                oid: git2::Oid::from_str("1000000000000000000000000000000000000000")
                    .expect("valid oid"),
                kind: PayloadObjectKind::Commit,
                size_bytes: 120,
                reachable_from_heads: true,
                context_head_index: Some(0),
                context_commit_order: Some(1),
                context_path: None,
            },
            crate::git::PayloadObjectEntry {
                oid: git2::Oid::from_str("2000000000000000000000000000000000000000")
                    .expect("valid oid"),
                kind: PayloadObjectKind::Blob,
                size_bytes: 24,
                reachable_from_heads: true,
                context_head_index: Some(0),
                context_commit_order: Some(1),
                context_path: Some("a.txt".to_string()),
            },
        ];
    }
    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit data");
    };

    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.payload_selected_index = 0;
    let expected_selected_oid = payload.objects[0].oid;

    handle_page_keys(&mut state, &model, KeyCode::Char('s'));
    assert_eq!(
        state.payload_sort_mode,
        super::super::types::PayloadSortMode::Context,
        "first sort cycle should switch to context sort"
    );
    let sorted = state.payload_sorted_objects(payload);
    assert_eq!(
        sorted[state.payload_selected_index].oid, expected_selected_oid,
        "sort cycle should preserve selected object identity"
    );
    assert_eq!(
        sorted.first().expect("sorted rows should exist").oid,
        payload.objects[1].oid,
        "context sort should move first-head commit-context object to the top"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('s'));
    assert_eq!(
        state.payload_sort_mode,
        super::super::types::PayloadSortMode::Canonical,
        "second sort cycle should return to canonical sort"
    );
}

// Verifies that Esc closes payload object detail view without exiting the application.
#[test]
fn handle_key_press_esc_closes_payload_object_detail_view() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.open_selected_payload_object(&model);
    assert!(
        state.payload_object_view.is_some(),
        "precondition: payload object detail should be open"
    );

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Esc);
    assert!(
        !should_exit,
        "Esc should close payload object detail instead of exiting"
    );
    assert!(
        state.payload_object_view.is_none(),
        "Esc should close payload object detail view"
    );
}

// Verifies that payload object detail uses syntax-highlighted rendering for textual blob objects.
#[test]
fn open_selected_payload_object_for_text_blob_sets_syntax_name() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit data");
    };
    let blob_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("fixture payload should include at least one blob object");
    state.payload_selected_index = blob_index;

    state.open_selected_payload_object(&model);
    let detail = state
        .payload_object_view
        .as_ref()
        .expect("payload object detail should open for selected blob object");
    assert!(
        !detail.syntax_name.is_empty(),
        "payload text blob detail should record selected syntax name"
    );
}

// Verifies that payload preview retains syntax hint metadata for textual blob rendering.
#[test]
fn refresh_payload_preview_for_text_blob_sets_syntax_hint() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit data");
    };
    let blob_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("fixture payload should include at least one blob object");
    state.payload_selected_index = blob_index;

    state.refresh_payload_preview(&model);
    let preview = state
        .payload_preview
        .as_ref()
        .expect("payload preview should exist for selected blob object");
    assert!(
        preview.syntax_path_hint.is_some(),
        "payload text blob preview should preserve syntax hint for render-time highlighting"
    );
}

// Verifies that cached payload preview can be reused even when bundle path becomes unavailable.
#[test]
fn refresh_payload_preview_reuses_cached_data_without_bundle_file() {
    let fixture = create_diff_fixture();
    let mut model = build_model_from_fixture(&fixture);
    model.payload_session = None;
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit data");
    };
    let blob_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("fixture payload should include at least one blob object");
    state.payload_selected_index = blob_index;

    state.refresh_payload_preview(&model);
    assert!(
        state.payload_preview.is_some(),
        "precondition: preview should load from bundle path once"
    );

    std::fs::remove_file(&model.bundle_path).expect("must remove fixture bundle archive");
    state.refresh_payload_preview(&model);
    let preview = state
        .payload_preview
        .as_ref()
        .expect("cached preview should remain available after bundle removal");
    assert!(
        !preview
            .lines
            .iter()
            .any(|line| line.contains("preview unavailable")),
        "cached preview should avoid reloading unavailable bundle input"
    );
}

// Verifies that cached object detail can be reopened without bundle file when payload session is absent.
#[test]
fn open_selected_payload_object_reuses_cached_detail_without_bundle_file() {
    let fixture = create_diff_fixture();
    let mut model = build_model_from_fixture(&fixture);
    model.payload_session = None;
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;

    let PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit data");
    };
    let blob_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("fixture payload should include at least one blob object");
    state.payload_selected_index = blob_index;

    state.open_selected_payload_object(&model);
    assert!(
        state.payload_object_view.is_some(),
        "precondition: detail should load while bundle archive exists"
    );

    std::fs::remove_file(&model.bundle_path).expect("must remove fixture bundle archive");
    state.open_selected_payload_object(&model);
    assert!(
        state.payload_object_view.is_some(),
        "cached detail should allow reopening object view after bundle removal"
    );
    assert!(
        state.action_message.is_none(),
        "cached detail path should not emit load error after bundle removal"
    );
}

// Verifies that unmapped diff keys are no-ops for current scroll position.
#[test]
fn handle_diff_keys_unmapped_input_does_not_change_scroll_state() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "main.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 5,
        scroll_y: 0,
        scroll_x: 0,
    });

    handle_diff_keys(&mut state, KeyCode::Char('x'));
    let diff_view = state.diff_view.as_ref().expect("diff view should exist");
    assert_eq!(diff_view.scroll_y, 0);
    assert_eq!(diff_view.scroll_x, 0);
}

// Verifies that global quit action has precedence over active diff-mode key routing.
#[test]
fn handle_key_press_global_quit_precedes_diff_mode_routing() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "main.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 10,
        scroll_y: 0,
        scroll_x: 0,
    });

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Char('q'));
    assert!(
        should_exit,
        "global quit should request exit even when diff mode is active"
    );
}

// Verifies that diff-mode key routing consumes navigation keys before page-mode routing.
#[test]
fn handle_key_press_diff_mode_consumes_horizontal_navigation_keys() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 2;
    state.diff_view = Some(DiffViewState {
        commit_index: 1,
        commit_total: 2,
        file_index: 0,
        commit_id: git2::Oid::from_str("2222222222222222222222222222222222222222")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "main.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 20,
        scroll_y: 0,
        scroll_x: 6,
    });

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Left);
    assert!(!should_exit, "diff navigation should not request app exit");
    assert_eq!(
        state.page_index, 2,
        "diff-mode key routing must not run history page navigation"
    );
    let diff_view = state
        .diff_view
        .as_ref()
        .expect("diff view should remain open");
    assert_eq!(
        diff_view.scroll_x, 4,
        "left key in diff mode should apply diff horizontal scroll step"
    );
}

// Verifies that payload-object detail key routing consumes scroll keys before page-mode routing.
#[test]
fn handle_key_press_payload_object_mode_consumes_scroll_keys() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.page_index = 1;
    state.payload_object_view = Some(super::super::types::PayloadObjectViewState {
        oid: git2::Oid::from_str("3333333333333333333333333333333333333333").expect("valid oid"),
        kind: PayloadObjectKind::Blob,
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1"), Line::from("line 2")],
        max_line_width: 20,
        scroll_y: 0,
        scroll_x: 0,
    });
    state.payload_selected_index = 0;

    let should_exit = handle_key_press(&mut state, &model, KeyCode::Down);
    assert!(
        !should_exit,
        "payload-object scrolling should not request app exit"
    );
    assert_eq!(
        state.page_index, 1,
        "payload-object key routing must not trigger history page navigation"
    );
    let view = state
        .payload_object_view
        .as_ref()
        .expect("payload-object detail view should remain open");
    assert_eq!(
        view.scroll_y, 1,
        "down key in payload-object mode should scroll detail view"
    );
    assert_eq!(
        state.payload_selected_index, 0,
        "payload-object scroll keys should not mutate payload row selection"
    );
}

// Verifies that direct payload-object key helper applies mapped scroll and reset actions.
#[test]
fn handle_payload_object_keys_scrolls_and_resets_object_view_offsets() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.payload_object_view = Some(super::super::types::PayloadObjectViewState {
        oid: git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("valid oid"),
        kind: PayloadObjectKind::Blob,
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1"), Line::from("line 2")],
        max_line_width: 20,
        scroll_y: 0,
        scroll_x: 0,
    });

    handle_payload_object_keys(&mut state, KeyCode::Down);
    handle_payload_object_keys(&mut state, KeyCode::Right);
    {
        let view = state
            .payload_object_view
            .as_ref()
            .expect("payload-object view must remain open");
        assert_eq!(view.scroll_y, 1);
        assert_eq!(view.scroll_x, 2);
    }

    handle_payload_object_keys(&mut state, KeyCode::Home);
    let view = state
        .payload_object_view
        .as_ref()
        .expect("payload-object view must remain open");
    assert_eq!(view.scroll_y, 0);
    assert_eq!(view.scroll_x, 0);
}
