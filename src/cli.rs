// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Command-line interface model and argument parsing.
//!
//! Defines user-facing command contracts, flags, and argument relationships.
//! Keeps input validation close to the CLI boundary before deeper workflow execution.

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "git-sync",
    version = crate::version::APP_VERSION,
    about = "Air-gap Git sync audit tool (scaffold)"
)]
/// Top-level CLI parser that dispatches to subcommands.
pub struct Cli {
    /// Optional subcommand; omitted prints scaffold/help guidance.
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
/// Supported `git-sync` command groups.
pub enum Command {
    /// Creates a bundle package for a linear commit range.
    Create {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        with_patches: bool,
    },
    /// Audits either a repository range or a bundle/package input.
    Audit {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        bundle: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        verify_metadata: bool,
        #[arg(long, value_enum)]
        format: Option<OutputFormat>,
        #[arg(long, value_enum, default_value_t = PayloadLedgerMode::Summary)]
        payload_ledger: PayloadLedgerMode,
        #[arg(long, value_enum, default_value_t = PayloadResolveMode::PackOnly)]
        resolve: PayloadResolveMode,
    },
    /// Opens the interactive terminal UI audit view.
    Ui {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value = "sync/last")]
        base: String,
        #[arg(long)]
        tip: Option<String>,
    },
    /// Receives a bundle/package into a repository, optionally as dry-run.
    Receive {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value_t = false)]
        verify_metadata: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
/// Non-interactive output encoding for `audit --format`.
pub enum OutputFormat {
    /// Human-readable aligned payload table.
    Table,
    /// Pretty-printed JSON payload-audit document.
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
/// Entry-ledger export mode used by `audit --format json`.
pub enum PayloadLedgerMode {
    /// Emit bounded first/last/unresolved ledger subsets.
    Summary,
    /// Emit full parsed entry-ledger rows.
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
/// External-base resolve strategy for payload parsing.
pub enum PayloadResolveMode {
    /// Strict in-pack-only resolution.
    PackOnly,
    /// Allow baseline repository ODB as delta-base source.
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Resolved non-interactive payload audit target.
pub struct PayloadAuditTarget {
    /// Repository path used to resolve prerequisite object dependencies.
    pub repo_path: PathBuf,
    /// Bundle/package path being audited.
    pub bundle_path: PathBuf,
}

/// Resolves non-interactive payload-audit inputs.
///
/// # Errors
///
/// Returns an error when `--repo`/`--bundle` are missing or when deprecated
/// repo-range flags are provided.
pub fn resolve_payload_audit_target(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
) -> Result<PayloadAuditTarget> {
    match (repo, bundle) {
        (Some(repo_path), Some(bundle_path)) => Ok(PayloadAuditTarget {
            repo_path,
            bundle_path,
        }),
        (Some(_), None) => {
            bail!("payload audit requires --bundle")
        }
        (None, Some(_)) => {
            bail!("payload audit requires --repo")
        }
        (None, None) => {
            bail!("payload audit requires both --repo and --bundle")
        }
    }
}

#[cfg(test)]
mod tests;
