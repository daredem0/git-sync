//! Shared application configuration types.

use std::path::PathBuf;

pub mod commands;
pub mod output;

#[derive(Debug, Clone)]
/// Runtime configuration shared across CLI and TUI flows.
pub struct AppConfig {
    /// Path to the repository being audited or receiving a bundle.
    pub repo_path: PathBuf,
    /// Path to the bundle input (`.bundle` or packaged `.zip`).
    pub bundle_path: PathBuf,
    /// Base revision used for repository-context validation.
    pub base_ref: String,
    /// Optional tip revision; when present it must be at/after `base_ref`.
    pub tip_ref: Option<String>,
}
