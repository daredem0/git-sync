// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for ui flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::Result;
use std::path::PathBuf;

use crate::{app::AppConfig, ui};

type UiRunner = fn(&AppConfig) -> Result<()>;

pub(super) fn run(repo: PathBuf, bundle: PathBuf, base: String, tip: Option<String>) -> Result<()> {
    run_with(repo, bundle, base, tip, default_ui_runner())
}

fn run_with<F>(
    repo: PathBuf,
    bundle: PathBuf,
    base: String,
    tip: Option<String>,
    runner: F,
) -> Result<()>
where
    F: FnOnce(&AppConfig) -> Result<()>,
{
    let config = AppConfig {
        repo_path: repo,
        bundle_path: bundle,
        base_ref: base,
        tip_ref: tip,
    };
    runner(&config)
}

#[cfg(not(test))]
fn default_ui_runner() -> UiRunner {
    ui::run
}

#[cfg(test)]
thread_local! {
    static TEST_UI_RUNNER: std::cell::RefCell<Option<UiRunner>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn default_ui_runner() -> UiRunner {
    TEST_UI_RUNNER.with(|slot| slot.borrow().as_ref().copied().unwrap_or(ui::run))
}

#[cfg(test)]
pub(super) fn set_test_ui_runner(runner: Option<UiRunner>) {
    TEST_UI_RUNNER.with(|slot| {
        *slot.borrow_mut() = runner;
    });
}

#[cfg(test)]
mod tests;
