// Focus: keyboard event handling, page/diff key behavior, and exit/help toggles.

use super::super::input::{handle_diff_keys, handle_key_press, handle_page_keys};
use super::super::types::DiffViewState;
use super::support::*;
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
