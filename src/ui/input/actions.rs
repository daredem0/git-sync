//! Action reducers for routed UI input events.

use super::router::{DiffAction, KeyAction, PayloadObjectAction};
use crate::ui::types::{AppState, AuditModel, MainView};

pub(super) fn apply_key_action(
    state: &mut AppState,
    model: &AuditModel,
    action: KeyAction,
) -> bool {
    match action {
        KeyAction::GoHistoryOverview => {
            state.close_diff();
            state.close_payload_object();
            state.show_history_view();
            state.first_page();
            false
        }
        KeyAction::GoPayloadOverview => {
            state.close_diff();
            state.close_payload_object();
            state.show_payload_view();
            state.refresh_payload_preview(model);
            false
        }
        KeyAction::GoCommitDetailPage => {
            state.close_diff();
            state.close_payload_object();
            state.show_history_view();
            state.first_page();
            state.enter_selected_head(model);
            false
        }
        KeyAction::Quit => true,
        KeyAction::Escape => {
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
        KeyAction::ToggleHelp => {
            state.show_help = !state.show_help;
            false
        }
        KeyAction::ToggleMainView => {
            state.toggle_main_view();
            if state.main_view == MainView::Payload {
                state.refresh_payload_preview(model);
            }
            false
        }
        KeyAction::ToggleOverviewFocus => {
            state.toggle_overview_focus();
            false
        }
        KeyAction::HistoryNextPage => {
            state.next_page(model);
            false
        }
        KeyAction::HistoryPreviousPage => {
            state.previous_page();
            false
        }
        KeyAction::HistoryMoveSelectionDown => {
            state.move_selection_down(model);
            false
        }
        KeyAction::HistoryMoveSelectionUp => {
            state.move_selection_up(model);
            false
        }
        KeyAction::HistoryFirstPage => {
            state.first_page();
            false
        }
        KeyAction::HistoryLastPage => {
            state.last_page(model);
            false
        }
        KeyAction::HistoryOpenSelection => {
            if state.page_index == 0 {
                state.enter_selected_head(model);
            } else {
                state.open_selected_diff(model);
            }
            false
        }
        KeyAction::PayloadMoveSelectionDown(step) => {
            if step == 1 {
                state.move_payload_selection_down(model);
            } else {
                state.move_payload_selection_down_by(model, step);
            }
            false
        }
        KeyAction::PayloadMoveSelectionUp(step) => {
            if step == 1 {
                state.move_payload_selection_up(model);
            } else {
                state.move_payload_selection_up_by(model, step);
            }
            false
        }
        KeyAction::PayloadCycleSort => {
            state.cycle_payload_sort_mode(model);
            false
        }
        KeyAction::PayloadToggleSubview => {
            state.toggle_payload_sub_view(model);
            false
        }
        KeyAction::PayloadOpenSelection => {
            state.open_selected_payload_object(model);
            false
        }
    }
}

pub(super) fn apply_diff_action(state: &mut AppState, action: DiffAction) {
    match action {
        DiffAction::ScrollDown(step) => state.scroll_diff_down(step),
        DiffAction::ScrollUp(step) => state.scroll_diff_up(step),
        DiffAction::ScrollRight(step) => state.scroll_diff_right(step),
        DiffAction::ScrollLeft(step) => state.scroll_diff_left(step),
        DiffAction::Reset => state.reset_diff_scroll(),
    }
}

pub(super) fn apply_payload_object_action(state: &mut AppState, action: PayloadObjectAction) {
    match action {
        PayloadObjectAction::ScrollDown(step) => state.scroll_payload_object_down(step),
        PayloadObjectAction::ScrollUp(step) => state.scroll_payload_object_up(step),
        PayloadObjectAction::ScrollRight(step) => state.scroll_payload_object_right(step),
        PayloadObjectAction::ScrollLeft(step) => state.scroll_payload_object_left(step),
        PayloadObjectAction::Reset => state.reset_payload_object_scroll(),
    }
}
