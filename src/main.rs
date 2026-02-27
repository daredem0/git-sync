//! CLI entrypoint and command dispatch for git-sync-audit.
#![doc = include_str!("../README.md")]
#![doc = "\n\n---\n\n"]
#![doc = include_str!("../SDD_SAD.md")]

mod app;
mod cli;
mod git;
mod ui;
mod version;

use anyhow::Result;
use app::AppConfig;
use clap::Parser;
use cli::{AuditTarget, Cli, Command, OutputFormat, resolve_audit_target};
use git::{
    CreateBundleOptions, ReceiveBundleOptions, collect_changed_files,
    collect_changed_files_from_bundle_input, create_bundle, create_bundle_with_options,
    receive_bundle_input, receive_bundle_input_with_options, remove_unarchived_bundle_artifacts,
    render_manifest, render_manifest_json, resolve_repo_audit_range,
    verify_bundle_metadata_against_repo_input,
};

/// Entrypoint for CLI parsing and subcommand dispatch.
///
/// # Errors
///
/// Returns an error when any selected subcommand operation fails.
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Create {
            repo,
            from,
            to,
            output,
            with_patches,
        }) => {
            let result = if with_patches {
                create_bundle_with_options(
                    &repo,
                    &from,
                    &to,
                    &output,
                    CreateBundleOptions {
                        include_patch_sidecar: true,
                    },
                )?
            } else {
                create_bundle(&repo, &from, &to, &output)?
            };
            let patch_audit_display = result
                .patch_audit_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            remove_unarchived_bundle_artifacts(&result)?;
            println!(
                "bundle package created: archive={}, from={}, to={}, tip_ref={}, included_patch={}",
                result.archive_path.display(),
                result.from_commit_id,
                result.to_commit_id,
                result.tip_ref_name,
                if patch_audit_display == "-" {
                    "no"
                } else {
                    "yes"
                }
            );
        }
        Some(Command::Audit {
            repo,
            bundle,
            from,
            to,
            verify_metadata,
            format,
        }) => {
            // `audit` without `--format` enters interactive TUI mode.
            if format.is_none() {
                if verify_metadata {
                    anyhow::bail!(
                        "interactive audit does not accept --verify-metadata; metadata is shown in the TUI overview"
                    );
                }
                if from.is_some() || to.is_some() {
                    anyhow::bail!(
                        "interactive audit does not accept --from/--to; use --format for non-interactive repo-range audit"
                    );
                }
                let repo_path =
                    repo.ok_or_else(|| anyhow::anyhow!("interactive audit requires --repo"))?;
                let bundle_path =
                    bundle.ok_or_else(|| anyhow::anyhow!("interactive audit requires --bundle"))?;
                let config = AppConfig {
                    repo_path,
                    bundle_path,
                    base_ref: "sync/last".to_string(),
                    tip_ref: None,
                };
                ui::run(&config)?;
                return Ok(());
            }

            let format = format.expect("format should be present in non-interactive audit mode");

            if verify_metadata {
                let repo_path =
                    repo.ok_or_else(|| anyhow::anyhow!("metadata verification requires --repo"))?;
                let bundle_path = bundle
                    .ok_or_else(|| anyhow::anyhow!("metadata verification requires --bundle"))?;
                if from.is_some() || to.is_some() {
                    anyhow::bail!("metadata verification does not accept --from or --to");
                }

                verify_bundle_metadata_against_repo_input(&bundle_path, &repo_path)?;
                match format {
                    OutputFormat::Tsv => {
                        println!("VERIFY\tOK");
                    }
                    OutputFormat::Json => {
                        println!("{{\"verification\":\"ok\"}}");
                    }
                }
                return Ok(());
            }

            match resolve_audit_target(repo, bundle, from, to)? {
                AuditTarget::RepoRange {
                    repo_path,
                    from_rev,
                    to_rev,
                } => {
                    let range = resolve_repo_audit_range(&repo_path, &from_rev, &to_rev)?;
                    let changes = collect_changed_files(
                        &repo_path,
                        range.base_commit_id,
                        range.tip_commit_id,
                    )?;
                    match format {
                        OutputFormat::Tsv => {
                            let manifest = render_manifest(&changes);
                            print!("{manifest}");
                        }
                        OutputFormat::Json => {
                            let manifest = render_manifest_json(&changes)?;
                            println!("{manifest}");
                        }
                    }
                }
                AuditTarget::Bundle { bundle_path } => {
                    let changes = collect_changed_files_from_bundle_input(&bundle_path)?;
                    match format {
                        OutputFormat::Tsv => {
                            let manifest = render_manifest(&changes);
                            print!("{manifest}");
                        }
                        OutputFormat::Json => {
                            let manifest = render_manifest_json(&changes)?;
                            println!("{manifest}");
                        }
                    }
                }
            }
        }
        Some(Command::Ui {
            repo,
            bundle,
            base,
            tip,
        }) => {
            let config = AppConfig {
                repo_path: repo,
                bundle_path: bundle,
                base_ref: base,
                tip_ref: tip,
            };
            ui::run(&config)?;
        }
        Some(Command::Receive {
            repo,
            bundle,
            verify_metadata,
            dry_run,
        }) => {
            let result = if verify_metadata || dry_run {
                receive_bundle_input_with_options(
                    &bundle,
                    &repo,
                    ReceiveBundleOptions {
                        verify_metadata,
                        dry_run,
                    },
                )?
            } else {
                receive_bundle_input(&bundle, &repo)?
            };
            let version = match result.bundle_version {
                git::BundleVersion::V2 => "v2",
                git::BundleVersion::V3 => "v3",
            };
            if dry_run {
                println!(
                    "bundle can be applied without conflicts: version={}, would_import_heads={}",
                    version,
                    result.imported_heads.len()
                );
            } else {
                println!(
                    "bundle received: version={}, imported_heads={}",
                    version,
                    result.imported_heads.len()
                );
            }
            for head in &result.imported_heads {
                println!("HEAD\t{}\t{}", head.oid, head.reference);
            }
            if dry_run {
                println!();
                println!("would change (per-file line diff summary):");
                let path_header = "PATH";
                let adds_header = "+LINES";
                let dels_header = "-LINES";
                let path_width = std::cmp::max(
                    path_header.len(),
                    result
                        .line_stats
                        .iter()
                        .map(|stat| stat.path.len())
                        .max()
                        .unwrap_or(0),
                );
                let adds_width = std::cmp::max(
                    adds_header.len(),
                    result
                        .line_stats
                        .iter()
                        .map(|stat| stat.additions.to_string().len())
                        .max()
                        .unwrap_or(0),
                );
                let dels_width = std::cmp::max(
                    dels_header.len(),
                    result
                        .line_stats
                        .iter()
                        .map(|stat| stat.deletions.to_string().len())
                        .max()
                        .unwrap_or(0),
                );

                println!(
                    "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
                    path_header, adds_header, dels_header
                );
                if result.line_stats.is_empty() {
                    println!("(no file content changes)");
                } else {
                    for stat in &result.line_stats {
                        println!(
                            "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
                            stat.path, stat.additions, stat.deletions
                        );
                    }
                }
            }
        }
        None => {
            println!("git-sync-audit scaffold is ready.");
            println!("Use --help to inspect planned commands.");
        }
    }

    Ok(())
}
