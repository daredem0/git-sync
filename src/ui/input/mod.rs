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

#[cfg(test)]
pub(crate) use test_api::{handle_diff_keys, handle_page_keys, handle_payload_object_keys};
#[cfg(test)]
mod test_api;

#[cfg(test)]
mod tests;
