// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Keyboard input handling for actions behavior.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::router::{DiffAction, HelpAction, KeyAction, PayloadObjectAction};
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
            if state.show_help {
                state.close_help();
                false
            } else if state.is_diff_open() {
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
            state.toggle_help();
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

pub(super) fn apply_help_action(state: &mut AppState, action: HelpAction) {
    match action {
        HelpAction::NextPage => state.next_help_page(),
        HelpAction::PreviousPage => state.previous_help_page(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::tests::support::sample_model;
    use crate::ui::types::{DiffViewState, PayloadObjectViewState};
    use crossterm::event::KeyCode;
    use ratatui::text::Line;

    fn oid(hex: &str) -> git2::Oid {
        git2::Oid::from_str(hex).expect("must parse test oid")
    }

    #[test]
    fn apply_key_action_handles_quit_and_escape_paths() {
        let model = sample_model(2, 1);
        let mut state = AppState::new(&model);
        assert!(
            apply_key_action(&mut state, &model, KeyAction::Quit),
            "quit action should request application exit"
        );

        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: oid("1111111111111111111111111111111111111111"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 8,
            scroll_y: 0,
            scroll_x: 0,
        });
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::Escape),
            "escape should close diff before exiting"
        );
        assert!(state.diff_view.is_none());

        state.payload_object_view = Some(PayloadObjectViewState {
            oid: oid("2222222222222222222222222222222222222222"),
            kind: crate::git::PayloadObjectKind::Blob,
            syntax_name: "none".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 8,
            scroll_y: 0,
            scroll_x: 0,
        });
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::Escape),
            "escape should close payload object view before exiting"
        );
        assert!(state.payload_object_view.is_none());

        state.page_index = 1;
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::Escape),
            "escape should return to first page from commit pages"
        );
        assert_eq!(state.page_index, 0);

        assert!(
            apply_key_action(&mut state, &model, KeyAction::Escape),
            "escape on overview without overlays should exit"
        );
    }

    #[test]
    fn apply_key_action_handles_main_view_and_help_toggles() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);

        assert!(!state.show_help);
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::ToggleHelp),
            "help toggle should not exit"
        );
        assert!(state.show_help);

        assert_eq!(state.main_view, MainView::History);
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::ToggleMainView),
            "main view toggle should not exit"
        );
        assert_eq!(state.main_view, MainView::Payload);
    }

    #[test]
    fn apply_key_action_routes_history_and_payload_navigation_actions() {
        let model = sample_model(2, 1);
        let mut state = AppState::new(&model);

        assert!(
            !apply_key_action(&mut state, &model, KeyAction::HistoryOpenSelection),
            "history open action should not exit"
        );
        assert_eq!(
            state.page_index, 1,
            "history open from overview should enter commit page"
        );

        state.main_view = MainView::Payload;
        state.payload_selected_index = 0;
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionDown(1)),
            "payload down action should not exit"
        );
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionUp(1)),
            "payload up action should not exit"
        );
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionDown(10)),
            "payload pagedown-style move should not exit"
        );
        assert!(
            !apply_key_action(&mut state, &model, KeyAction::PayloadMoveSelectionUp(10)),
            "payload pageup-style move should not exit"
        );
    }

    #[test]
    fn apply_diff_and_payload_object_actions_update_scroll_state() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: oid("3333333333333333333333333333333333333333"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 12,
            scroll_y: 0,
            scroll_x: 0,
        });
        apply_diff_action(&mut state, DiffAction::ScrollDown(1));
        apply_diff_action(&mut state, DiffAction::ScrollRight(2));
        apply_diff_action(&mut state, DiffAction::Reset);
        let diff = state
            .diff_view
            .as_ref()
            .expect("diff view should remain open");
        assert_eq!(diff.scroll_y, 0);
        assert_eq!(diff.scroll_x, 0);

        state.payload_object_view = Some(PayloadObjectViewState {
            oid: oid("4444444444444444444444444444444444444444"),
            kind: crate::git::PayloadObjectKind::Blob,
            syntax_name: "none".to_string(),
            lines: vec![Line::from("line"), Line::from("line2")],
            max_line_width: 12,
            scroll_y: 0,
            scroll_x: 0,
        });
        apply_payload_object_action(&mut state, PayloadObjectAction::ScrollDown(1));
        apply_payload_object_action(&mut state, PayloadObjectAction::ScrollRight(2));
        apply_payload_object_action(&mut state, PayloadObjectAction::Reset);
        let payload_object = state
            .payload_object_view
            .as_ref()
            .expect("payload object view should remain open");
        assert_eq!(payload_object.scroll_y, 0);
        assert_eq!(payload_object.scroll_x, 0);
        // Keep KeyCode imported to avoid unused warnings across test cfg permutations.
        let _ = KeyCode::Null;
    }
}
