//! TUI-layer input functionality.

use super::types::{AppState, AuditModel, MainView};
use crossterm::event::KeyCode;

const DIFF_SCROLL_VERTICAL_STEP: usize = 1;
const DIFF_SCROLL_HORIZONTAL_STEP: usize = 2;
const DIFF_SCROLL_PAGE_STEP: usize = 20;
const PAYLOAD_SELECT_PAGE_STEP: usize = 10;

/// Handles one key press and returns `true` when the app should exit.
pub(crate) fn handle_key_press(state: &mut AppState, model: &AuditModel, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => true,
        KeyCode::Esc => {
            if state.is_diff_open() {
                state.close_diff();
                false
            } else if state.is_payload_object_open() {
                state.close_payload_object();
                false
            } else if state.page_index > 0 {
                state.first_page();
                false
            } else {
                true
            }
        }
        KeyCode::Char('?') => {
            state.show_help = !state.show_help;
            false
        }
        _ => {
            if state.is_diff_open() {
                handle_diff_keys(state, code);
            } else if state.is_payload_object_open() {
                handle_payload_object_keys(state, code);
            } else {
                handle_page_keys(state, model, code);
            }
            false
        }
    }
}

/// Handles navigation keys while the app is in page mode.
pub(crate) fn handle_page_keys(state: &mut AppState, model: &AuditModel, code: KeyCode) {
    let on_main_page = state.page_index == 0;
    match code {
        KeyCode::Tab | KeyCode::Char('v') if on_main_page => {
            state.toggle_main_view();
            if state.main_view == MainView::Payload {
                state.refresh_payload_preview(model);
            }
            return;
        }
        KeyCode::Char('1') if on_main_page => {
            state.show_history_view();
            return;
        }
        KeyCode::Char('2') if on_main_page => {
            state.show_payload_view();
            state.refresh_payload_preview(model);
            return;
        }
        _ => {}
    }

    if state.main_view == MainView::Payload {
        match code {
            KeyCode::Down | KeyCode::Char('j') => state.move_payload_selection_down(model),
            KeyCode::Up | KeyCode::Char('k') => state.move_payload_selection_up(model),
            KeyCode::PageDown => {
                state.move_payload_selection_down_by(model, PAYLOAD_SELECT_PAGE_STEP)
            }
            KeyCode::PageUp => state.move_payload_selection_up_by(model, PAYLOAD_SELECT_PAGE_STEP),
            KeyCode::Char('s') => state.cycle_payload_sort_mode(model),
            KeyCode::Enter => state.open_selected_payload_object(model),
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Right | KeyCode::Char('l') => state.next_page(model),
        KeyCode::Left | KeyCode::Char('h') => state.previous_page(),
        KeyCode::Down | KeyCode::Char('j') => state.move_selection_down(model),
        KeyCode::Up | KeyCode::Char('k') => state.move_selection_up(model),
        KeyCode::Char('g') => state.first_page(),
        KeyCode::Char('G') => state.last_page(model),
        KeyCode::Enter => {
            if state.page_index == 0 {
                state.enter_selected_head(model);
            } else {
                state.open_selected_diff(model);
            }
        }
        _ => {}
    }
}

/// Handles scrolling/navigation keys while diff view is open.
pub(crate) fn handle_diff_keys(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Down | KeyCode::Char('j') => state.scroll_diff_down(DIFF_SCROLL_VERTICAL_STEP),
        KeyCode::Up | KeyCode::Char('k') => state.scroll_diff_up(DIFF_SCROLL_VERTICAL_STEP),
        KeyCode::Right | KeyCode::Char('l') => state.scroll_diff_right(DIFF_SCROLL_HORIZONTAL_STEP),
        KeyCode::Left | KeyCode::Char('h') => state.scroll_diff_left(DIFF_SCROLL_HORIZONTAL_STEP),
        KeyCode::PageDown => state.scroll_diff_down(DIFF_SCROLL_PAGE_STEP),
        KeyCode::PageUp => state.scroll_diff_up(DIFF_SCROLL_PAGE_STEP),
        KeyCode::Home => state.reset_diff_scroll(),
        _ => {}
    }
}

/// Handles scrolling/navigation keys while payload object detail view is open.
pub(crate) fn handle_payload_object_keys(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll_payload_object_down(DIFF_SCROLL_VERTICAL_STEP)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll_payload_object_up(DIFF_SCROLL_VERTICAL_STEP)
        }
        KeyCode::Right | KeyCode::Char('l') => {
            state.scroll_payload_object_right(DIFF_SCROLL_HORIZONTAL_STEP)
        }
        KeyCode::Left | KeyCode::Char('h') => {
            state.scroll_payload_object_left(DIFF_SCROLL_HORIZONTAL_STEP)
        }
        KeyCode::PageDown => state.scroll_payload_object_down(DIFF_SCROLL_PAGE_STEP),
        KeyCode::PageUp => state.scroll_payload_object_up(DIFF_SCROLL_PAGE_STEP),
        KeyCode::Home => state.reset_payload_object_scroll(),
        _ => {}
    }
}
