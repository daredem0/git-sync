// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI test-support module wiring.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: shared UI test fixtures, model builders, and render capture helpers.

mod fixtures;
mod helpers;
mod models;
mod render_capture;

pub(crate) use fixtures::{create_diff_fixture, create_non_text_diff_fixture};
pub(crate) use models::{
    build_model_from_fixture, sample_model, sample_multi_head_model, sample_overview_model,
};
pub(crate) use render_capture::render_and_capture_text;
