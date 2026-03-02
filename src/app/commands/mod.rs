// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Command dispatch module for CLI subcommands.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

mod audit;
mod create;
mod receive;
mod ui;

use anyhow::Result;

use crate::cli::Command;

/// Dispatches one parsed command.
///
/// # Errors
///
/// Returns an error when any selected subcommand operation fails.
pub fn dispatch(command: Option<Command>) -> Result<()> {
    match command {
        Some(Command::Create {
            repo,
            from,
            to,
            output,
            with_patches,
        }) => create::run(repo, from, to, output, with_patches),
        Some(Command::Audit {
            repo,
            bundle,
            verify_metadata,
            format,
            payload_ledger,
            resolve,
        }) => audit::run(
            repo,
            bundle,
            verify_metadata,
            format,
            payload_ledger,
            resolve,
        ),
        Some(Command::Ui {
            repo,
            bundle,
            base,
            tip,
        }) => ui::run(repo, bundle, base, tip),
        Some(Command::Receive {
            repo,
            bundle,
            verify_metadata,
            dry_run,
            integrate,
            incoming_as_branches,
            check_mergeability,
            format,
        }) => receive::run(
            repo,
            bundle,
            verify_metadata,
            dry_run,
            integrate,
            incoming_as_branches,
            check_mergeability,
            format,
        ),
        None => {
            println!("git-sync scaffold is ready.");
            println!("Use --help to inspect planned commands.");
            Ok(())
        }
    }
}
