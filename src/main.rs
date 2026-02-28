//! CLI entrypoint and command dispatch for git-sync.
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
use cli::{Cli, Command, OutputFormat, resolve_payload_audit_target};
use git::{
    CreateBundleOptions, ReceiveBundleOptions, collect_payload_audit_for_bundle_input,
    create_bundle, create_bundle_with_options, receive_bundle_input,
    receive_bundle_input_with_options, remove_unarchived_bundle_artifacts,
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
            if verify_metadata {
                let repo_path =
                    repo.ok_or_else(|| anyhow::anyhow!("metadata verification requires --repo"))?;
                let bundle_path = bundle
                    .ok_or_else(|| anyhow::anyhow!("metadata verification requires --bundle"))?;
                if from.is_some() || to.is_some() {
                    anyhow::bail!("metadata verification does not accept --from or --to");
                }

                verify_bundle_metadata_against_repo_input(&bundle_path, &repo_path)?;
                println!("metadata verification passed");
                return Ok(());
            }

            // `audit` without `--format` enters interactive TUI mode.
            if format.is_none() {
                if from.is_some() || to.is_some() {
                    anyhow::bail!(
                        "interactive audit does not accept --from/--to; use --format for non-interactive payload audit"
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

            let target = resolve_payload_audit_target(repo, bundle, from, to)?;
            let payload =
                collect_payload_audit_for_bundle_input(&target.bundle_path, &target.repo_path)?;
            match format {
                OutputFormat::Table => {
                    let table = render_payload_audit_table(&payload);
                    println!("{table}");
                }
                OutputFormat::Json => {
                    let payload_json = render_payload_audit_json(&payload)?;
                    println!("{payload_json}");
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
            println!("git-sync scaffold is ready.");
            println!("Use --help to inspect planned commands.");
        }
    }

    Ok(())
}

/// Renders non-interactive payload audit as a human-readable aligned table.
fn render_payload_audit_table(payload: &git::PayloadAudit) -> String {
    let oid_header = "OID";
    let type_header = "TYPE";
    let size_header = "SIZE";
    let reachable_header = "REACHABLE";

    let oid_width = std::cmp::max(
        oid_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.oid.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let type_width = std::cmp::max(
        type_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| payload_kind_label(entry.kind).len())
            .max()
            .unwrap_or(0),
    );
    let size_width = std::cmp::max(
        size_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let reachable_width = std::cmp::max(reachable_header.len(), 9);

    let mut out = String::new();
    out.push_str(&format!(
        "PACK OBJECTS (bundle {}, heads={})\n",
        match payload.bundle_version {
            git::BundleVersion::V2 => "v2",
            git::BundleVersion::V3 => "v3",
        },
        payload.heads.len()
    ));
    out.push_str(&format!(
        "{:<oid_width$}  {:<type_width$}  {:>size_width$}  {:<reachable_width$}\n",
        oid_header, type_header, size_header, reachable_header
    ));

    for object in &payload.objects {
        out.push_str(&format!(
            "{:<oid_width$}  {:<type_width$}  {:>size_width$}  {:<reachable_width$}\n",
            object.oid,
            payload_kind_label(object.kind),
            object.size_bytes,
            if object.reachable_from_heads {
                "yes"
            } else {
                "no"
            }
        ));
    }

    if payload.objects.is_empty() {
        out.push_str("(no pack objects)\n");
    }

    out
}

/// Renders non-interactive payload audit as JSON.
///
/// This is a phase-1 contract shape and will be extended in subsequent phases.
fn render_payload_audit_json(payload: &git::PayloadAudit) -> Result<String> {
    let value = serde_json::json!({
        "bundle_header_version": match payload.bundle_version {
            git::BundleVersion::V2 => "v2",
            git::BundleVersion::V3 => "v3",
        },
        "heads": payload.heads.iter().map(|head| serde_json::json!({
            "oid": head.oid.to_string(),
            "reference": head.reference,
        })).collect::<Vec<_>>(),
        "transport_entries": payload.transport_entries.iter().map(|entry| serde_json::json!({
            "name": entry.name,
            "size_bytes": entry.size_bytes,
            "sha256": entry.sha256,
        })).collect::<Vec<_>>(),
        "pack_objects": payload.objects.iter().map(|object| serde_json::json!({
            "oid": object.oid.to_string(),
            "type": payload_kind_label(object.kind),
            "size_bytes": object.size_bytes,
            "reachable_from_heads": object.reachable_from_heads,
            "context_head_index": object.context_head_index,
            "context_commit_order": object.context_commit_order,
            "context_path": object.context_path,
        })).collect::<Vec<_>>(),
    });
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Returns stable labels for payload object kinds.
fn payload_kind_label(kind: git::PayloadObjectKind) -> &'static str {
    match kind {
        git::PayloadObjectKind::Commit => "commit",
        git::PayloadObjectKind::Tree => "tree",
        git::PayloadObjectKind::Blob => "blob",
        git::PayloadObjectKind::Tag => "tag",
        git::PayloadObjectKind::Unknown => "unknown",
    }
}
