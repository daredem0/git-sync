// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Test-only adapter helpers for ui input handling.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Provides narrow entry points for focused input unit tests.

use super::actions;
use super::router;
use crate::ui::types::{AppState, AuditModel};
use crossterm::event::KeyCode;

/// Handles navigation keys while the app is in page mode.
pub(crate) fn handle_page_keys(state: &mut AppState, model: &AuditModel, code: KeyCode) {
    if let Some(action) = router::action_for_page_key(state, code) {
        let _ = actions::apply_key_action(state, model, action);
    }
}

/// Handles scrolling/navigation keys while diff view is open.
pub(crate) fn handle_diff_keys(state: &mut AppState, code: KeyCode) {
    if let Some(action) = router::action_for_diff_key(code) {
        actions::apply_diff_action(state, action);
    }
}

/// Handles scrolling/navigation keys while payload object detail view is open.
pub(crate) fn handle_payload_object_keys(state: &mut AppState, code: KeyCode) {
    if let Some(action) = router::action_for_payload_object_key(code) {
        actions::apply_payload_object_action(state, action);
    }
}
