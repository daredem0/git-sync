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
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn run_with_passes_cli_values_into_app_config() {
        let repo = PathBuf::from("/tmp/repo");
        let bundle = PathBuf::from("/tmp/sync.bundle.zip");
        let base = "refs/heads/main".to_string();
        let tip = Some("refs/heads/topic".to_string());

        let mut captured: Option<AppConfig> = None;
        let result = run_with(
            repo.clone(),
            bundle.clone(),
            base.clone(),
            tip.clone(),
            |config| {
                captured = Some(config.clone());
                Ok(())
            },
        );

        assert!(result.is_ok(), "runner wrapper should return success");
        let captured = captured.expect("runner should receive one config value");
        assert_eq!(captured.repo_path, repo);
        assert_eq!(captured.bundle_path, bundle);
        assert_eq!(captured.base_ref, base);
        assert_eq!(captured.tip_ref, tip);
    }

    #[test]
    fn run_with_propagates_runner_error() {
        let result = run_with(
            PathBuf::from("/tmp/repo"),
            PathBuf::from("/tmp/sync.bundle.zip"),
            "sync/last".to_string(),
            None,
            |_config| Err(anyhow!("simulated ui error")),
        );

        let err = result.expect_err("runner error should bubble up");
        assert!(
            err.to_string().contains("simulated ui error"),
            "error text should preserve runner failure reason"
        );
    }
}
