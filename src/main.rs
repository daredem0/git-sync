mod app;
mod cli;
mod git;
mod ui;

use anyhow::Result;
use app::AppConfig;
use clap::Parser;
use cli::{AuditTarget, Cli, Command, OutputFormat, resolve_audit_target};
use git::{
    CreateBundleOptions, collect_changed_files, create_bundle, create_bundle_with_options,
    render_bundle_inspection_json, render_bundle_inspection_tsv, render_manifest,
    render_manifest_json, resolve_repo_audit_range,
};

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
            println!(
                "bundle created: path={}, caudit={}, patch_audit={}, from={}, to={}, tip_ref={}",
                result.bundle_path.display(),
                result.audit_path.display(),
                patch_audit_display,
                result.from_commit_id,
                result.to_commit_id,
                result.tip_ref_name
            );
        }
        Some(Command::Audit {
            repo,
            bundle,
            from,
            to,
            format,
        }) => match resolve_audit_target(repo, bundle, from, to)? {
            AuditTarget::RepoRange {
                repo_path,
                from_rev,
                to_rev,
            } => {
                let range = resolve_repo_audit_range(&repo_path, &from_rev, &to_rev)?;
                let changes =
                    collect_changed_files(&repo_path, range.base_commit_id, range.tip_commit_id)?;
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
                let inspection = git::inspect_bundle(&bundle_path)?;
                match format {
                    OutputFormat::Tsv => {
                        let output = render_bundle_inspection_tsv(&inspection);
                        print!("{output}");
                    }
                    OutputFormat::Json => {
                        let output = render_bundle_inspection_json(&inspection)?;
                        println!("{output}");
                    }
                }
            }
        },
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
            git::open_context(&config)?;
            ui::run(&config)?;
            println!("UI command scaffold is ready.");
            println!("Implementation is intentionally pending.");
        }
        None => {
            println!("git-sync-audit scaffold is ready.");
            println!("Use --help to inspect planned commands.");
        }
    }

    Ok(())
}
