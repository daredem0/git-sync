// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/input/router.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::ui::tests::support::sample_model;

#[test]
fn global_action_maps_primary_navigation_and_global_shortcuts() {
    assert_eq!(
        global_action(KeyCode::Char('1')),
        Some(KeyAction::GoHistoryOverview)
    );
    assert_eq!(
        global_action(KeyCode::Char('2')),
        Some(KeyAction::GoPayloadOverview)
    );
    assert_eq!(
        global_action(KeyCode::Char('3')),
        Some(KeyAction::GoCommitDetailPage)
    );
    assert_eq!(
        global_action(KeyCode::Char('4')),
        Some(KeyAction::GoCommitGraphPage)
    );
    assert_eq!(
        global_action(KeyCode::Char('p')),
        Some(KeyAction::ExportPayloadAuditJsonLight)
    );
    assert_eq!(
        global_action(KeyCode::Char('P')),
        Some(KeyAction::ExportPayloadAuditJsonFull)
    );
    assert_eq!(global_action(KeyCode::Char('q')), Some(KeyAction::Quit));
    assert_eq!(global_action(KeyCode::Esc), Some(KeyAction::Escape));
    assert_eq!(
        global_action(KeyCode::Char('?')),
        Some(KeyAction::ToggleHelp)
    );
    assert_eq!(global_action(KeyCode::Char('x')), None);
}

#[test]
fn action_for_page_key_routes_history_overview_controls() {
    let model = sample_model(2, 1);
    let mut state = AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 0;

    assert_eq!(
        action_for_page_key(&state, KeyCode::Tab),
        Some(KeyAction::ToggleOverviewFocus)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('v')),
        Some(KeyAction::ToggleMainView)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Down),
        Some(KeyAction::HistoryMoveSelectionDown)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Enter),
        Some(KeyAction::HistoryOpenSelection)
    );
}

#[test]
fn action_for_page_key_routes_history_commit_page_controls() {
    let model = sample_model(3, 1);
    let mut state = AppState::new(&model);
    state.main_view = MainView::History;
    state.page_index = 2;

    assert_eq!(
        action_for_page_key(&state, KeyCode::Right),
        Some(KeyAction::HistoryNextPage)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Left),
        Some(KeyAction::HistoryPreviousPage)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('g')),
        Some(KeyAction::HistoryFirstPage)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('G')),
        Some(KeyAction::HistoryLastPage)
    );
}

#[test]
fn action_for_page_key_routes_history_graph_controls() {
    let model = sample_model(3, 1);
    let mut state = AppState::new(&model);
    state.main_view = MainView::History;
    state.show_history_graph_view();

    assert_eq!(
        action_for_page_key(&state, KeyCode::Down),
        Some(KeyAction::GraphScrollDown(1))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::PageDown),
        Some(KeyAction::GraphScrollDown(20))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Up),
        Some(KeyAction::GraphScrollUp(1))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::PageUp),
        Some(KeyAction::GraphScrollUp(20))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Enter),
        Some(KeyAction::HistoryOpenSelection)
    );
}

#[test]
fn action_for_page_key_routes_payload_controls_and_sort_guard() {
    let model = sample_model(1, 1);
    let mut state = AppState::new(&model);
    state.main_view = MainView::Payload;
    state.page_index = 0;

    assert_eq!(
        action_for_page_key(&state, KeyCode::Down),
        Some(KeyAction::PayloadMoveSelectionDown(1))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::PageDown),
        Some(KeyAction::PayloadMoveSelectionDown(10))
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('e')),
        Some(KeyAction::PayloadToggleSubview)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Enter),
        Some(KeyAction::PayloadOpenSelection)
    );
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('s')),
        Some(KeyAction::PayloadCycleSort),
        "sort shortcut should be enabled in payload objects view"
    );

    state.payload_sub_view = crate::ui::types::PayloadSubView::Entries;
    assert_eq!(
        action_for_page_key(&state, KeyCode::Char('s')),
        None,
        "sort shortcut should be disabled in payload entries view"
    );
}

#[test]
fn diff_and_payload_object_key_maps_cover_navigation_shortcuts() {
    assert_eq!(
        action_for_diff_key(KeyCode::Down),
        Some(DiffAction::ScrollDown(1))
    );
    assert_eq!(
        action_for_diff_key(KeyCode::PageDown),
        Some(DiffAction::ScrollDown(20))
    );
    assert_eq!(action_for_diff_key(KeyCode::Home), Some(DiffAction::Reset));
    assert_eq!(action_for_diff_key(KeyCode::Char('x')), None);

    assert_eq!(
        action_for_payload_object_key(KeyCode::Down),
        Some(PayloadObjectAction::ScrollDown(1))
    );
    assert_eq!(
        action_for_payload_object_key(KeyCode::PageUp),
        Some(PayloadObjectAction::ScrollUp(20))
    );
    assert_eq!(
        action_for_payload_object_key(KeyCode::Home),
        Some(PayloadObjectAction::Reset)
    );
    assert_eq!(action_for_payload_object_key(KeyCode::Char('x')), None);
}

#[test]
fn action_for_help_key_maps_bidirectional_paging_shortcuts() {
    assert_eq!(
        action_for_help_key(KeyCode::Right),
        Some(HelpAction::NextPage)
    );
    assert_eq!(
        action_for_help_key(KeyCode::PageDown),
        Some(HelpAction::NextPage)
    );
    assert_eq!(
        action_for_help_key(KeyCode::Left),
        Some(HelpAction::PreviousPage)
    );
    assert_eq!(
        action_for_help_key(KeyCode::PageUp),
        Some(HelpAction::PreviousPage)
    );
    assert_eq!(action_for_help_key(KeyCode::Char('x')), None);
}
