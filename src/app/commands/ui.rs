// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for ui flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::Result;
use std::path::PathBuf;

use crate::{app::AppConfig, ui};

pub(super) fn run(repo: PathBuf, bundle: PathBuf, base: String, tip: Option<String>) -> Result<()> {
    run_with(repo, bundle, base, tip, ui::run)
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

#[cfg(test)]
mod tests;
