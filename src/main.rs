mod app;
mod cli;
mod git;
mod ui;

use anyhow::Result;
use app::AppConfig;
use clap::Parser;
use cli::{AuditTarget, Cli, Command, OutputFormat, resolve_audit_target};
use git::{
    CreateBundleOptions, collect_changed_files, collect_changed_files_from_bundle_input,
    create_bundle, create_bundle_with_options, remove_unarchived_bundle_artifacts, render_manifest,
    render_manifest_json, resolve_repo_audit_range, verify_bundle_metadata_against_repo_input,
    receive_bundle_input,
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
            git::open_context(&config)?;
            ui::run(&config)?;
            println!("UI command scaffold is ready.");
            println!("Implementation is intentionally pending.");
        }
        Some(Command::Receive { repo, bundle }) => {
            let result = receive_bundle_input(&bundle, &repo)?;
            let version = match result.bundle_version {
                git::BundleVersion::V2 => "v2",
                git::BundleVersion::V3 => "v3",
            };
            println!(
                "bundle received: version={}, imported_heads={}",
                version,
                result.imported_heads.len()
            );
            for head in &result.imported_heads {
                println!("HEAD\t{}\t{}", head.oid, head.reference);
            }
        }
        None => {
            println!("git-sync-audit scaffold is ready.");
            println!("Use --help to inspect planned commands.");
        }
    }

    Ok(())
}
