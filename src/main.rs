// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI entrypoint for git-sync.
//!
//! Combines CLI parsing with command dispatch and includes project docs in rustdoc output.
//! Acts as the binary entry boundary between user input and application workflows.

#![doc = include_str!("../README.md")]
#![doc = "\n\n---\n\n"]
#![doc = include_str!("../docs/GIT_FUNDAMENTALS.md")]
#![doc = "\n\n---\n\n"]
#![doc = include_str!("../docs/SDD_SAD.md")]

mod app;
mod cli;
mod git;
mod ui;
mod version;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

/// Entrypoint for CLI parsing and command dispatch.
///
/// # Errors
///
/// Returns an error when any selected subcommand operation fails.
fn main() -> Result<()> {
    let cli = Cli::parse();
    app::commands::dispatch(cli.command)
}
