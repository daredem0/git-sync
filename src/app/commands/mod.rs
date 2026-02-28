//! CLI command dispatch and per-command orchestration.

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
        }) => receive::run(repo, bundle, verify_metadata, dry_run),
        None => {
            println!("git-sync scaffold is ready.");
            println!("Use --help to inspect planned commands.");
            Ok(())
        }
    }
}
