// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI test module wiring.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

mod diff_tests;
mod format_tests;
mod input_tests;
mod model_tests;
mod render_diff_help_tests;
mod render_overview_commit_tests;
mod state_diff_open_tests;
mod state_diff_scroll_tests;
mod state_navigation_tests;
pub(crate) mod support;
