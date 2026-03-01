// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI input module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::types::{AppState, AuditModel};
use crossterm::event::KeyCode;

mod actions;
mod router;

/// Handles one key press and returns `true` when the app should exit.
pub(crate) fn handle_key_press(state: &mut AppState, model: &AuditModel, code: KeyCode) -> bool {
    if state.show_help {
        if let Some(action) = router::global_action(code) {
            return actions::apply_key_action(state, model, action);
        }
        if let Some(action) = router::action_for_help_key(code) {
            actions::apply_help_action(state, action);
        }
        return false;
    }

    if let Some(action) = router::global_action(code) {
        return actions::apply_key_action(state, model, action);
    }

    if state.is_diff_open() {
        if let Some(action) = router::action_for_diff_key(code) {
            actions::apply_diff_action(state, action);
        }
        return false;
    }

    if state.is_payload_object_open() {
        if let Some(action) = router::action_for_payload_object_key(code) {
            actions::apply_payload_object_action(state, action);
        }
        return false;
    }

    if let Some(action) = router::action_for_page_key(state, code) {
        return actions::apply_key_action(state, model, action);
    }
    false
}

/// Handles navigation keys while the app is in page mode.
#[cfg(test)]
pub(crate) fn handle_page_keys(state: &mut AppState, model: &AuditModel, code: KeyCode) {
    if let Some(action) = router::action_for_page_key(state, code) {
        let _ = actions::apply_key_action(state, model, action);
    }
}

/// Handles scrolling/navigation keys while diff view is open.
#[cfg(test)]
pub(crate) fn handle_diff_keys(state: &mut AppState, code: KeyCode) {
    if let Some(action) = router::action_for_diff_key(code) {
        actions::apply_diff_action(state, action);
    }
}

/// Handles scrolling/navigation keys while payload object detail view is open.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn handle_payload_object_keys(state: &mut AppState, code: KeyCode) {
    if let Some(action) = router::action_for_payload_object_key(code) {
        actions::apply_payload_object_action(state, action);
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_key_press, handle_payload_object_keys};
    use crate::ui::tests::support::sample_model;
    use crate::ui::types::{AppState, DiffViewState, PayloadObjectViewState};
    use crossterm::event::KeyCode;
    use ratatui::text::Line;

    fn oid(hex: &str) -> git2::Oid {
        git2::Oid::from_str(hex).expect("must parse test oid")
    }

    #[test]
    fn handle_key_press_returns_false_for_unmapped_keys() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        assert!(
            !handle_key_press(&mut state, &model, KeyCode::Char('x')),
            "unmapped keys should be ignored without exiting"
        );
    }

    #[test]
    fn handle_payload_object_keys_ignores_unmapped_keys() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.payload_object_view = Some(PayloadObjectViewState {
            oid: oid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            kind: crate::git::PayloadObjectKind::Blob,
            syntax_name: "none".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 8,
            scroll_y: 0,
            scroll_x: 0,
        });

        handle_payload_object_keys(&mut state, KeyCode::Char('x'));
        let view = state
            .payload_object_view
            .as_ref()
            .expect("payload object view should remain open");
        assert_eq!(view.scroll_y, 0);
        assert_eq!(view.scroll_x, 0);
    }

    #[test]
    fn handle_key_press_prioritizes_global_escape_over_diff_and_payload_modes() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: oid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 8,
            scroll_y: 0,
            scroll_x: 0,
        });
        state.payload_object_view = Some(PayloadObjectViewState {
            oid: oid("cccccccccccccccccccccccccccccccccccccccc"),
            kind: crate::git::PayloadObjectKind::Blob,
            syntax_name: "none".to_string(),
            lines: vec![Line::from("line")],
            max_line_width: 8,
            scroll_y: 0,
            scroll_x: 0,
        });

        let should_exit = handle_key_press(&mut state, &model, KeyCode::Esc);
        assert!(
            !should_exit,
            "escape should first close overlays before requesting exit"
        );
        assert!(
            state.diff_view.is_none(),
            "global escape should close diff view with priority"
        );
    }
}
