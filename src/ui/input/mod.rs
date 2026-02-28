//! TUI-layer input handling entrypoints.

use super::types::{AppState, AuditModel};
use crossterm::event::KeyCode;

mod actions;
mod router;

/// Handles one key press and returns `true` when the app should exit.
pub(crate) fn handle_key_press(state: &mut AppState, model: &AuditModel, code: KeyCode) -> bool {
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
