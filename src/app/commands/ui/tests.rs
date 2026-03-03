// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for app/commands/ui.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use anyhow::anyhow;

fn ok_runner(_config: &AppConfig) -> anyhow::Result<()> {
    Ok(())
}

fn err_runner(_config: &AppConfig) -> anyhow::Result<()> {
    Err(anyhow!("simulated default ui runner error"))
}

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

#[test]
fn run_calls_default_ui_runner_path() {
    set_test_ui_runner(Some(ok_runner));
    let result = run(
        PathBuf::from("/tmp/repo"),
        PathBuf::from("/tmp/sync.bundle.zip"),
        "sync/last".to_string(),
        None,
    );

    set_test_ui_runner(None);
    assert!(
        result.is_ok(),
        "default ui runner path should route through configured injected runner in tests"
    );
}

#[test]
fn run_propagates_error_from_injected_default_ui_runner() {
    set_test_ui_runner(Some(err_runner));
    let result = run(
        PathBuf::from("/tmp/repo"),
        PathBuf::from("/tmp/sync.bundle.zip"),
        "sync/last".to_string(),
        None,
    );

    set_test_ui_runner(None);
    let err = result.expect_err("injected default ui runner error should bubble up");
    assert!(
        err.to_string()
            .contains("simulated default ui runner error"),
        "error text should preserve injected default ui runner failure details"
    );
}
