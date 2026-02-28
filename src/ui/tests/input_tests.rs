//! Unit tests for input tests.

// Focus: keyboard event handling, page/diff key behavior, and exit/help toggles.

use super::super::input::{handle_diff_keys, handle_key_press, handle_page_keys};
use super::super::types::{DiffViewState, MainView, PayloadModel, PayloadSubView};
use super::support::*;
use crate::git::PayloadObjectKind;
use crossterm::event::KeyCode;
use ratatui::text::Line;

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

    handle_page_keys(&mut state, &model, KeyCode::Char('v'));
    assert_eq!(
        state.main_view,
        MainView::Payload,
        "key v should toggle from history to payload view"
    );

    handle_page_keys(&mut state, &model, KeyCode::Tab);
    assert_eq!(
        state.main_view,
        MainView::History,
        "Tab should toggle from payload back to history view"
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

// Verifies that view-switch shortcuts are ignored outside the main page context.
#[test]
fn handle_page_keys_ignores_view_switch_shortcuts_off_main_page() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;
    state.main_view = MainView::History;

    handle_page_keys(&mut state, &model, KeyCode::Char('2'));
    assert_eq!(
        state.main_view,
        MainView::History,
        "view-switch shortcut should be ignored on commit pages"
    );

    handle_page_keys(&mut state, &model, KeyCode::Char('v'));
    assert_eq!(
        state.main_view,
        MainView::History,
        "toggle shortcut should be ignored on commit pages"
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
