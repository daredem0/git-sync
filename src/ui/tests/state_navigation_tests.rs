//! Unit tests for state navigation tests.

// Focus: AppState page navigation and file selection movement invariants.

use super::support::*;

// Verifies that total_pages returns one overview page plus one page per commit.
#[test]
fn app_state_total_pages_counts_overview_and_commits() {
    let model = sample_model(3, 2);
    let state = super::super::types::AppState::new(&model);
    assert_eq!(state.total_pages(&model), 4);
}

// Verifies that page navigation clamps at the first and last available page.
#[test]
fn app_state_page_navigation_is_bounded() {
    let model = sample_model(2, 1);
    let mut state = super::super::types::AppState::new(&model);

    state.previous_page();
    assert_eq!(state.page_index, 0);

    state.next_page(&model);
    state.next_page(&model);
    state.next_page(&model);
    assert_eq!(state.page_index, 2);

    state.first_page();
    assert_eq!(state.page_index, 0);

    state.last_page(&model);
    assert_eq!(state.page_index, 2);
}

// Verifies that file selection movement on commit pages stays within valid row bounds.
#[test]
fn app_state_selection_movement_is_bounded() {
    let model = sample_model(1, 2);
    let mut state = super::super::types::AppState::new(&model);
    state.next_page(&model);
    assert_eq!(state.page_index, 1);

    state.move_selection_down(&model);
    assert_eq!(state.selected_file_index(0), 1);

    state.move_selection_down(&model);
    assert_eq!(state.selected_file_index(0), 1);

    state.move_selection_up(&model);
    assert_eq!(state.selected_file_index(0), 0);

    state.move_selection_up(&model);
    assert_eq!(state.selected_file_index(0), 0);
}
