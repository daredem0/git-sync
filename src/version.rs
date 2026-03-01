// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Build-time and runtime version reporting utilities.
//!
//! Resolves build and runtime version identity for CLI, UI, and emitted audit artifacts.
//! Keeps provenance information consistent across human and machine-facing surfaces.

/// Human-readable app version embedded by `build.rs`.
pub const APP_VERSION: &str = env!("GIT_SYNC_VERSION");
