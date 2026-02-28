//! Build-time version metadata exposed across CLI and UI.

/// Human-readable app version embedded by `build.rs`.
pub const APP_VERSION: &str = env!("GIT_SYNC_VERSION");
