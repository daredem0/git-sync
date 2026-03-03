// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for audit flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

use crate::app::{AppConfig, output};
use crate::cli::{
    OutputFormat, PayloadDetailMode, PayloadLedgerMode,
    PayloadResolveMode as CliPayloadResolveMode, resolve_payload_audit_target,
};
use crate::git::{
    PayloadAuditLedgerMode, PayloadAuditObjectDetailMode, PayloadResolveMode,
    build_payload_audit_document_for_bundle_input_with_options,
    build_payload_audit_document_for_bundle_input_with_options_and_detail_mode,
    collect_payload_audit_for_bundle_input_with_resolve_mode,
    verify_bundle_metadata_against_repo_input,
};
use crate::ui;

pub(super) fn run(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    verify_metadata: bool,
    format: Option<OutputFormat>,
    payload_ledger: PayloadLedgerMode,
    payload_detail: PayloadDetailMode,
    resolve: CliPayloadResolveMode,
) -> Result<()> {
    if verify_metadata {
        let repo_path = repo.ok_or_else(|| anyhow!("metadata verification requires --repo"))?;
        let bundle_path =
            bundle.ok_or_else(|| anyhow!("metadata verification requires --bundle"))?;

        verify_bundle_metadata_against_repo_input(&bundle_path, &repo_path)?;
        println!("metadata verification passed");
        return Ok(());
    }

    if let Some(format) = format {
        return run_non_interactive(
            repo,
            bundle,
            format,
            payload_ledger,
            payload_detail,
            resolve,
        );
    }

    run_interactive(repo, bundle, resolve)
}

fn run_interactive(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    resolve: CliPayloadResolveMode,
) -> Result<()> {
    run_interactive_with(repo, bundle, resolve, ui::run)
}

fn run_interactive_with<F>(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    resolve: CliPayloadResolveMode,
    runner: F,
) -> Result<()>
where
    F: FnOnce(&AppConfig) -> Result<()>,
{
    if !matches!(resolve, CliPayloadResolveMode::PackOnly) {
        return Err(anyhow!(
            "interactive audit currently supports only --resolve pack-only"
        ));
    }

    let repo_path = repo.ok_or_else(|| anyhow!("interactive audit requires --repo"))?;
    let bundle_path = bundle.ok_or_else(|| anyhow!("interactive audit requires --bundle"))?;

    let config = AppConfig {
        repo_path,
        bundle_path,
        base_ref: "sync/last".to_string(),
        tip_ref: None,
    };
    runner(&config)
}

fn run_non_interactive(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    format: OutputFormat,
    payload_ledger: PayloadLedgerMode,
    payload_detail: PayloadDetailMode,
    resolve: CliPayloadResolveMode,
) -> Result<()> {
    let resolve_mode = match resolve {
        CliPayloadResolveMode::PackOnly => PayloadResolveMode::PackOnly,
        CliPayloadResolveMode::Baseline => PayloadResolveMode::Baseline,
    };

    let target = resolve_payload_audit_target(repo, bundle)?;
    match format {
        OutputFormat::Table => {
            let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
                &target.bundle_path,
                &target.repo_path,
                resolve_mode,
            )?;
            let table = output::render_payload_audit_table(&payload);
            println!("{table}");
            Ok(())
        }
        OutputFormat::Json => {
            let ledger_mode = match payload_ledger {
                PayloadLedgerMode::None => PayloadAuditLedgerMode::None,
                PayloadLedgerMode::Summary => PayloadAuditLedgerMode::Summary,
                PayloadLedgerMode::Full => PayloadAuditLedgerMode::Full,
            };
            let payload_document = match payload_detail {
                PayloadDetailMode::Full => {
                    build_payload_audit_document_for_bundle_input_with_options(
                        &target.bundle_path,
                        &target.repo_path,
                        ledger_mode,
                        resolve_mode,
                    )?
                }
                PayloadDetailMode::Light => {
                    build_payload_audit_document_for_bundle_input_with_options_and_detail_mode(
                        &target.bundle_path,
                        &target.repo_path,
                        ledger_mode,
                        PayloadAuditObjectDetailMode::Light,
                        resolve_mode,
                    )?
                }
            };
            let payload_json = output::render_payload_audit_json(&payload_document)?;
            println!("{payload_json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
