//! `ui` command handler.

use anyhow::Result;
use std::path::PathBuf;

use crate::{app::AppConfig, ui};

pub(super) fn run(repo: PathBuf, bundle: PathBuf, base: String, tip: Option<String>) -> Result<()> {
    let config = AppConfig {
        repo_path: repo,
        bundle_path: bundle,
        base_ref: base,
        tip_ref: tip,
    };
    ui::run(&config)
}
