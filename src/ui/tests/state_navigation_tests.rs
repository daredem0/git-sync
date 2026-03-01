// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for state navigation behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: AppState page navigation and file selection movement invariants.

use super::super::types::OverviewFocus;
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

// Verifies that overview up/down navigation changes selected head and updates available commit-page count.
#[test]
fn app_state_overview_head_navigation_updates_selected_head_and_total_pages() {
    let model = sample_multi_head_model(&[1, 3]);
    let mut state = super::super::types::AppState::new(&model);

    assert_eq!(state.page_index, 0, "precondition: start on overview");
    assert_eq!(
        state.total_pages(&model),
        2,
        "first head with one commit should expose overview + one commit page"
    );

    state.move_selection_down(&model);
    assert_eq!(
        state.selected_head_index, 1,
        "overview down should select next head"
    );
    assert_eq!(
        state.total_pages(&model),
        4,
        "second head with three commits should expose overview + three commit pages"
    );

    state.move_selection_up(&model);
    assert_eq!(
        state.selected_head_index, 0,
        "overview up should move back to previous head"
    );
}

// Verifies that overview focus starts on heads table and Tab-style toggling switches between heads and would-change.
#[test]
fn app_state_overview_focus_defaults_and_toggles() {
    let model = sample_multi_head_model(&[2, 2]);
    let mut state = super::super::types::AppState::new(&model);
    assert_eq!(
        state.overview_focus,
        OverviewFocus::Heads,
        "new app state should default overview focus to heads table"
    );

    state.toggle_overview_focus();
    assert_eq!(
        state.overview_focus,
        OverviewFocus::WouldChange,
        "focus toggle should switch to would-change table"
    );

    state.toggle_overview_focus();
    assert_eq!(
        state.overview_focus,
        OverviewFocus::Heads,
        "focus toggle should switch back to heads table"
    );
}
