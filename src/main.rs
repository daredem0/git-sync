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
use cli::{
    Cli, Command, OutputFormat, PayloadLedgerMode, PayloadResolveMode as CliPayloadResolveMode,
    resolve_payload_audit_target,
};
use git::{
    CreateBundleOptions, PayloadAuditLedgerMode, PayloadResolveMode, ReceiveBundleOptions,
    build_payload_audit_document_for_bundle_input_with_options,
    collect_payload_audit_for_bundle_input_with_resolve_mode, create_bundle,
    create_bundle_with_options, receive_bundle_input, receive_bundle_input_with_options,
    remove_unarchived_bundle_artifacts, verify_bundle_metadata_against_repo_input,
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
            verify_metadata,
            format,
            payload_ledger,
            resolve,
        }) => {
            if verify_metadata {
                let repo_path =
                    repo.ok_or_else(|| anyhow::anyhow!("metadata verification requires --repo"))?;
                let bundle_path = bundle
                    .ok_or_else(|| anyhow::anyhow!("metadata verification requires --bundle"))?;

                verify_bundle_metadata_against_repo_input(&bundle_path, &repo_path)?;
                println!("metadata verification passed");
                return Ok(());
            }

            // `audit` without `--format` enters interactive TUI mode.
            if format.is_none() {
                if !matches!(resolve, CliPayloadResolveMode::PackOnly) {
                    return Err(anyhow::anyhow!(
                        "interactive audit currently supports only --resolve pack-only"
                    ));
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
                    let table = render_payload_audit_table(&payload);
                    println!("{table}");
                }
                OutputFormat::Json => {
                    let payload_document =
                        build_payload_audit_document_for_bundle_input_with_options(
                            &target.bundle_path,
                            &target.repo_path,
                            match payload_ledger {
                                PayloadLedgerMode::Summary => PayloadAuditLedgerMode::Summary,
                                PayloadLedgerMode::Full => PayloadAuditLedgerMode::Full,
                            },
                            resolve_mode,
                        )?;
                    let payload_json = render_payload_audit_json(&payload_document)?;
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
    let transport_name_header = "NAME";
    let transport_size_header = "SIZE";
    let transport_sha_header = "SHA256";
    let transport_name_width = std::cmp::max(
        transport_name_header.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.name.len())
            .max()
            .unwrap_or(0),
    );
    let transport_size_width = std::cmp::max(
        transport_size_header.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );

    let mut out = String::new();
    let proof_ok = payload.pack_proof.entries_declared == payload.pack_proof.entries_parsed
        && payload.pack_proof.entries_materialized == payload.pack_proof.entries_declared
        && payload.pack_proof.computed_pack_checksum == payload.pack_proof.trailer_pack_checksum;
    let transfer_status = if payload.pack_proof.transfer_allowed {
        "allowed".to_string()
    } else {
        format!(
            "blocked ({})",
            payload
                .pack_proof
                .blocked_reason
                .as_deref()
                .unwrap_or("entries not fully materialized")
        )
    };
    out.push_str(&format!(
        "PACK PROOF status={} version={} entries={}/{} materialized={}/{} transfer={} hash={}\n",
        if proof_ok { "ok" } else { "failed" },
        payload.pack_proof.pack_version,
        payload.pack_proof.entries_parsed,
        payload.pack_proof.entries_declared,
        payload.pack_proof.entries_materialized,
        payload.pack_proof.entries_declared,
        transfer_status,
        payload.pack_proof.hash_algorithm
    ));
    out.push_str(&format!(
        "PACK CHECKSUM computed={} trailer={}\n",
        payload.pack_proof.computed_pack_checksum, payload.pack_proof.trailer_pack_checksum
    ));
    let unresolved_entries = payload
        .entry_ledger
        .entries
        .iter()
        .filter(|entry| !entry.resolved)
        .count();
    out.push_str(&format!(
        "LEDGER summary declared={} parsed={} unresolved={}\n",
        payload.entry_ledger.declared_entry_count,
        payload.entry_ledger.entries.len(),
        unresolved_entries
    ));
    out.push('\n');
    out.push_str("TRANSPORT ENTRIES\n");
    out.push_str(&format!(
        "{:<transport_name_width$}  {:>transport_size_width$}  {}\n",
        transport_name_header, transport_size_header, transport_sha_header
    ));
    for entry in &payload.transport_entries {
        out.push_str(&format!(
            "{:<transport_name_width$}  {:>transport_size_width$}  {}\n",
            entry.name, entry.size_bytes, entry.sha256
        ));
    }
    if payload.transport_entries.is_empty() {
        out.push_str("(no transport entries)\n");
    }
    out.push('\n');
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

/// Renders non-interactive payload audit document as pretty-printed JSON.
fn render_payload_audit_json(document: &git::PayloadAuditDocument) -> Result<String> {
    Ok(serde_json::to_string_pretty(document)?)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> git2::Oid {
        git2::Oid::from_str(hex).expect("must create valid oid")
    }

    fn sample_payload_for_table() -> git::PayloadAudit {
        git::PayloadAudit {
            bundle_version: git::BundleVersion::V2,
            heads: vec![git::BundleHead {
                oid: oid("1111111111111111111111111111111111111111"),
                reference: "refs/heads/main".to_string(),
            }],
            transport_entries: vec![git::PayloadTransportEntry {
                name: "sync.bundle".to_string(),
                size_bytes: 123,
                sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            }],
            pack_proof: git::PayloadPackProof::from_entry_counters(
                2,
                4,
                4,
                4,
                3,
                1,
                "sha1".to_string(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
            entry_ledger: git::PackEntryLedger {
                pack_version: 2,
                declared_entry_count: 4,
                entries: vec![
                    git::PackEntryRecord {
                        idx: 0,
                        offset: 12,
                        kind: git::PackEntryKind::Commit,
                        out_size: 120,
                        base_ref: None,
                        result_oid: Some(oid("2222222222222222222222222222222222222222")),
                        result_kind: Some(git::PayloadObjectKind::Commit),
                        resolved: true,
                        resolved_via: Some(git::ResolutionSource::InPack),
                        note: None,
                    },
                    git::PackEntryRecord {
                        idx: 1,
                        offset: 44,
                        kind: git::PackEntryKind::Blob,
                        out_size: 40,
                        base_ref: None,
                        result_oid: Some(oid("3333333333333333333333333333333333333333")),
                        result_kind: Some(git::PayloadObjectKind::Blob),
                        resolved: true,
                        resolved_via: Some(git::ResolutionSource::InPack),
                        note: None,
                    },
                    git::PackEntryRecord {
                        idx: 2,
                        offset: 78,
                        kind: git::PackEntryKind::Blob,
                        out_size: 40,
                        base_ref: None,
                        result_oid: Some(oid("3333333333333333333333333333333333333333")),
                        result_kind: Some(git::PayloadObjectKind::Blob),
                        resolved: true,
                        resolved_via: Some(git::ResolutionSource::InPack),
                        note: None,
                    },
                    git::PackEntryRecord {
                        idx: 3,
                        offset: 101,
                        kind: git::PackEntryKind::Tree,
                        out_size: 60,
                        base_ref: None,
                        result_oid: Some(oid("4444444444444444444444444444444444444444")),
                        result_kind: Some(git::PayloadObjectKind::Tree),
                        resolved: true,
                        resolved_via: Some(git::ResolutionSource::InPack),
                        note: None,
                    },
                ],
            },
            objects: vec![git::PayloadObjectEntry {
                oid: oid("2222222222222222222222222222222222222222"),
                kind: git::PayloadObjectKind::Commit,
                size_bytes: 120,
                reachable_from_heads: true,
                context_head_index: Some(0),
                context_commit_order: Some(1),
                context_path: None,
            }],
        }
    }

    // Verifies that non-interactive table output includes entry counters and transfer status.
    #[test]
    fn audit_table_includes_entry_counts_and_transfer_status() {
        let table = render_payload_audit_table(&sample_payload_for_table());
        assert!(
            table.contains("entries=4/4 materialized=4/4"),
            "table proof header should include parsed/materialized entry counters"
        );
        assert!(
            table.contains("transfer=allowed"),
            "table proof header should include transfer status"
        );
        assert!(
            table.contains("LEDGER summary declared=4 parsed=4 unresolved=0"),
            "table should include concise ledger summary line"
        );
    }
}
