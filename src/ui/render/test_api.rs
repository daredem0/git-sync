// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Test-only adapter helpers for ui rendering behavior.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Exposes focused helpers for validating help text behavior.

use super::{HelpContext, help_hotkeys_text};

/// Returns contextual key help for page mode or diff mode.
pub(crate) fn help_text_for_mode(in_diff_view: bool) -> &'static str {
    if in_diff_view {
        help_hotkeys_text(HelpContext::Diff)
    } else {
        help_hotkeys_text(HelpContext::HistoryOverview)
    }
}
