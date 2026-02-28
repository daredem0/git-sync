//! CLI entrypoint for git-sync.
#![doc = include_str!("../README.md")]
#![doc = "\n\n---\n\n"]
#![doc = include_str!("../SDD_SAD.md")]

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
