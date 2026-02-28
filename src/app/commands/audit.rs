//! `audit` command handler.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

use crate::app::{AppConfig, output};
use crate::cli::{
    OutputFormat, PayloadLedgerMode, PayloadResolveMode as CliPayloadResolveMode,
    resolve_payload_audit_target,
};
use crate::git::{
    PayloadAuditLedgerMode, PayloadResolveMode,
    build_payload_audit_document_for_bundle_input_with_options,
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

    if format.is_none() {
        return run_interactive(repo, bundle, resolve);
    }

    run_non_interactive(
        repo,
        bundle,
        format.expect("format should be set in non-interactive mode"),
        payload_ledger,
        resolve,
    )
}

fn run_interactive(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    resolve: CliPayloadResolveMode,
) -> Result<()> {
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
    ui::run(&config)
}

fn run_non_interactive(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    format: OutputFormat,
    payload_ledger: PayloadLedgerMode,
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
            let payload_document = build_payload_audit_document_for_bundle_input_with_options(
                &target.bundle_path,
                &target.repo_path,
                match payload_ledger {
                    PayloadLedgerMode::Summary => PayloadAuditLedgerMode::Summary,
                    PayloadLedgerMode::Full => PayloadAuditLedgerMode::Full,
                },
                resolve_mode,
            )?;
            let payload_json = output::render_payload_audit_json(&payload_document)?;
            println!("{payload_json}");
            Ok(())
        }
    }
}
