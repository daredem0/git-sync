// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI module facade and submodule wiring.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod diff;
mod format;
mod input;
mod model;
mod render;
mod runtime;
mod state;
mod syntax;
mod types;

/// Runs the interactive terminal audit UI.
pub use runtime::run;

#[cfg(test)]
mod tests;
