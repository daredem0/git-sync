//! Key-to-action routing for UI input handling.

use crate::ui::types::{AppState, MainView};
use crossterm::event::KeyCode;

const DIFF_SCROLL_VERTICAL_STEP: usize = 1;
const DIFF_SCROLL_HORIZONTAL_STEP: usize = 2;
const DIFF_SCROLL_PAGE_STEP: usize = 20;
const PAYLOAD_SELECT_PAGE_STEP: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyAction {
    GoHistoryOverview,
    GoPayloadOverview,
    GoCommitDetailPage,
    Quit,
    Escape,
    ToggleHelp,
    ToggleMainView,
    ToggleOverviewFocus,
    HistoryNextPage,
    HistoryPreviousPage,
    HistoryMoveSelectionDown,
    HistoryMoveSelectionUp,
    HistoryFirstPage,
    HistoryLastPage,
    HistoryOpenSelection,
    PayloadMoveSelectionDown(usize),
    PayloadMoveSelectionUp(usize),
    PayloadCycleSort,
    PayloadToggleSubview,
    PayloadOpenSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffAction {
    ScrollDown(usize),
    ScrollUp(usize),
    ScrollRight(usize),
    ScrollLeft(usize),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PayloadObjectAction {
    ScrollDown(usize),
    ScrollUp(usize),
    ScrollRight(usize),
    ScrollLeft(usize),
    Reset,
}

pub(super) fn global_action(code: KeyCode) -> Option<KeyAction> {
    primary_navigation_action(code).or(match code {
        KeyCode::Char('q') => Some(KeyAction::Quit),
        KeyCode::Esc => Some(KeyAction::Escape),
        KeyCode::Char('?') => Some(KeyAction::ToggleHelp),
        _ => None,
    })
}

pub(super) fn action_for_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    if let Some(action) = primary_navigation_action(code) {
        return Some(action);
    }

    let on_main_page = state.page_index == 0;
    let on_history_overview = on_main_page && state.main_view == MainView::History;
    match code {
        KeyCode::Char('v') if on_main_page => return Some(KeyAction::ToggleMainView),
        KeyCode::Tab if on_history_overview => return Some(KeyAction::ToggleOverviewFocus),
        _ => {}
    }

    if state.main_view == MainView::Payload {
        return action_for_payload_page_key(state, code);
    }
    action_for_history_page_key(state, code)
}

pub(super) fn action_for_diff_key(code: KeyCode) -> Option<DiffAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            Some(DiffAction::ScrollDown(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::ScrollUp(DIFF_SCROLL_VERTICAL_STEP)),
        KeyCode::Right | KeyCode::Char('l') => {
            Some(DiffAction::ScrollRight(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::Left | KeyCode::Char('h') => {
            Some(DiffAction::ScrollLeft(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::PageDown => Some(DiffAction::ScrollDown(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::PageUp => Some(DiffAction::ScrollUp(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::Home => Some(DiffAction::Reset),
        _ => None,
    }
}

pub(super) fn action_for_payload_object_key(code: KeyCode) -> Option<PayloadObjectAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            Some(PayloadObjectAction::ScrollDown(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Up | KeyCode::Char('k') => {
            Some(PayloadObjectAction::ScrollUp(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Right | KeyCode::Char('l') => Some(PayloadObjectAction::ScrollRight(
            DIFF_SCROLL_HORIZONTAL_STEP,
        )),
        KeyCode::Left | KeyCode::Char('h') => {
            Some(PayloadObjectAction::ScrollLeft(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::PageDown => Some(PayloadObjectAction::ScrollDown(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::PageUp => Some(PayloadObjectAction::ScrollUp(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::Home => Some(PayloadObjectAction::Reset),
        _ => None,
    }
}

fn primary_navigation_action(code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Char('1') => Some(KeyAction::GoHistoryOverview),
        KeyCode::Char('2') => Some(KeyAction::GoPayloadOverview),
        KeyCode::Char('3') => Some(KeyAction::GoCommitDetailPage),
        _ => None,
    }
}

fn action_for_payload_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => Some(KeyAction::PayloadMoveSelectionDown(1)),
        KeyCode::Up | KeyCode::Char('k') => Some(KeyAction::PayloadMoveSelectionUp(1)),
        KeyCode::PageDown => Some(KeyAction::PayloadMoveSelectionDown(
            PAYLOAD_SELECT_PAGE_STEP,
        )),
        KeyCode::PageUp => Some(KeyAction::PayloadMoveSelectionUp(PAYLOAD_SELECT_PAGE_STEP)),
        KeyCode::Char('s') if state.is_payload_objects_view() => Some(KeyAction::PayloadCycleSort),
        KeyCode::Char('e') => Some(KeyAction::PayloadToggleSubview),
        KeyCode::Enter => Some(KeyAction::PayloadOpenSelection),
        _ => None,
    }
}

fn action_for_history_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Right | KeyCode::Char('l') if state.page_index > 0 => {
            Some(KeyAction::HistoryNextPage)
        }
        KeyCode::Left | KeyCode::Char('h') if state.page_index > 1 => {
            Some(KeyAction::HistoryPreviousPage)
        }
        KeyCode::Down | KeyCode::Char('j') => Some(KeyAction::HistoryMoveSelectionDown),
        KeyCode::Up | KeyCode::Char('k') => Some(KeyAction::HistoryMoveSelectionUp),
        KeyCode::Char('g') => Some(KeyAction::HistoryFirstPage),
        KeyCode::Char('G') => Some(KeyAction::HistoryLastPage),
        KeyCode::Enter => Some(KeyAction::HistoryOpenSelection),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
}
