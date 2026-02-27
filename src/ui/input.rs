use super::types::{AppState, AuditModel};
use crossterm::event::KeyCode;

const DIFF_SCROLL_VERTICAL_STEP: usize = 1;
const DIFF_SCROLL_HORIZONTAL_STEP: usize = 2;
const DIFF_SCROLL_PAGE_STEP: usize = 20;

pub(crate) fn handle_key_press(state: &mut AppState, model: &AuditModel, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => true,
        KeyCode::Esc => {
            if state.is_diff_open() {
                state.close_diff();
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
            } else {
                handle_page_keys(state, model, code);
            }
            false
        }
    }
}

pub(crate) fn handle_page_keys(state: &mut AppState, model: &AuditModel, code: KeyCode) {
    match code {
        KeyCode::Right | KeyCode::Char('l') => state.next_page(model),
        KeyCode::Left | KeyCode::Char('h') => state.previous_page(),
        KeyCode::Down | KeyCode::Char('j') => state.move_selection_down(model),
        KeyCode::Up | KeyCode::Char('k') => state.move_selection_up(model),
        KeyCode::Char('g') => state.first_page(),
        KeyCode::Char('G') => state.last_page(model),
        KeyCode::Enter => state.open_selected_diff(model),
        _ => {}
    }
}

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
