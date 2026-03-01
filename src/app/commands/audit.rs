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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_interactive_rejects_non_pack_only_resolve_mode() {
        let result = run_interactive(
            Some(PathBuf::from(".")),
            Some(PathBuf::from("sync.bundle.zip")),
            CliPayloadResolveMode::Baseline,
        );
        let error = result.expect_err("interactive mode should reject baseline resolve mode");
        assert!(
            error
                .to_string()
                .contains("interactive audit currently supports only --resolve pack-only"),
            "error should explain interactive resolve-mode constraint"
        );
    }

    #[test]
    fn run_verify_metadata_requires_repo_and_bundle_arguments() {
        let missing_repo = run(
            None,
            Some(PathBuf::from("sync.bundle.zip")),
            true,
            None,
            PayloadLedgerMode::Summary,
            CliPayloadResolveMode::PackOnly,
        );
        assert!(
            missing_repo
                .expect_err("verify-metadata mode should require repo")
                .to_string()
                .contains("metadata verification requires --repo")
        );

        let missing_bundle = run(
            Some(PathBuf::from(".")),
            None,
            true,
            None,
            PayloadLedgerMode::Summary,
            CliPayloadResolveMode::PackOnly,
        );
        assert!(
            missing_bundle
                .expect_err("verify-metadata mode should require bundle")
                .to_string()
                .contains("metadata verification requires --bundle")
        );
    }

    #[test]
    fn run_non_interactive_propagates_target_resolution_error() {
        let result = run_non_interactive(
            None,
            None,
            OutputFormat::Table,
            PayloadLedgerMode::Summary,
            CliPayloadResolveMode::PackOnly,
        );
        assert!(
            result
                .expect_err("non-interactive mode should require target input")
                .to_string()
                .contains("payload audit requires both --repo and --bundle"),
            "target resolution error should be preserved"
        );
    }
}
