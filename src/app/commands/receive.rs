// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for receive flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::{Result, bail};
use std::path::PathBuf;

use crate::cli::{OutputFormat, ReceiveIntegratePolicy as CliReceiveIntegratePolicy};
use crate::git::{
    BundleVersion, ReceiveBundleOptions, ReceiveIntegratePolicy, ReceivePlanEntry,
    receive_bundle_input, receive_bundle_input_with_options_and_policy,
    receive_bundle_input_with_options_policy_and_branch_mirror,
};

pub(super) fn run(
    repo: PathBuf,
    bundle: PathBuf,
    verify_metadata: bool,
    dry_run: bool,
    integrate: CliReceiveIntegratePolicy,
    incoming_as_branches: bool,
    format: Option<OutputFormat>,
) -> Result<()> {
    if !dry_run && format.is_some() {
        bail!("receive --format is supported only with --dry-run");
    }

    let integrate_policy = match integrate {
        CliReceiveIntegratePolicy::CreateRefsOnly => ReceiveIntegratePolicy::CreateRefsOnly,
        CliReceiveIntegratePolicy::FastForwardOnly => ReceiveIntegratePolicy::FastForwardOnly,
    };
    let result = if !incoming_as_branches
        && !verify_metadata
        && !dry_run
        && integrate_policy == ReceiveIntegratePolicy::FastForwardOnly
    {
        receive_bundle_input(&bundle, &repo)?
    } else if incoming_as_branches {
        receive_bundle_input_with_options_policy_and_branch_mirror(
            &bundle,
            &repo,
            ReceiveBundleOptions {
                verify_metadata,
                dry_run,
            },
            integrate_policy,
            incoming_as_branches,
        )?
    } else {
        receive_bundle_input_with_options_and_policy(
            &bundle,
            &repo,
            ReceiveBundleOptions {
                verify_metadata,
                dry_run,
            },
            integrate_policy,
        )?
    };

    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    if dry_run && matches!(format, Some(OutputFormat::Json)) {
        println!("{}", render_dry_run_json(version, &result));
        return Ok(());
    }

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
        render_dry_run_preflight_plan(&result.preflight_plan);
        render_dry_run_line_stats(&result.line_stats);
    }

    Ok(())
}

fn render_dry_run_json(version: &str, result: &crate::git::ReceiveBundleResult) -> String {
    let plan_rows = result
        .preflight_plan
        .iter()
        .map(|row| {
            serde_json::json!({
                "target_ref": row.target_ref,
                "status": row.status.as_str(),
                "target_oid": row.target_oid.map(|oid| oid.to_string()),
                "incoming_oid": row.incoming_oid.to_string(),
                "merge_base_oid": row.merge_base_oid.map(|oid| oid.to_string()),
                "preserved_incoming_ref": row.preserved_incoming_ref,
            })
        })
        .collect::<Vec<_>>();

    let line_rows = result
        .line_stats
        .iter()
        .map(|row| {
            serde_json::json!({
                "path": row.path,
                "additions": row.additions,
                "deletions": row.deletions,
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&serde_json::json!({
        "bundle_version": version,
        "would_import_heads": result.imported_heads.len(),
        "can_apply_without_conflicts": result.can_apply_without_conflicts,
        "preflight_plan": plan_rows,
        "line_stats": line_rows,
    }))
    .expect("receive dry-run json rendering should always be serializable")
}

fn render_dry_run_preflight_plan(plan: &[ReceivePlanEntry]) {
    println!();
    println!("preflight plan (per-ref integration status):");

    let ref_header = "TARGET_REF";
    let status_header = "STATUS";
    let target_header = "TARGET_OID";
    let incoming_header = "INCOMING_OID";
    let merge_base_header = "MERGE_BASE_OID";

    let ref_width = std::cmp::max(
        ref_header.len(),
        plan.iter()
            .map(|row| row.target_ref.len())
            .max()
            .unwrap_or(0),
    );
    let status_width = std::cmp::max(
        status_header.len(),
        plan.iter()
            .map(|row| row.status.as_str().len())
            .max()
            .unwrap_or(0),
    );

    println!(
        "{:<ref_width$}  {:<status_width$}  {:<40}  {:<40}  {:<40}",
        ref_header, status_header, target_header, incoming_header, merge_base_header
    );

    if plan.is_empty() {
        println!("(no advertised heads)");
        return;
    }

    for row in plan {
        let target_oid = row
            .target_oid
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "-".to_string());
        let merge_base_oid = row
            .merge_base_oid
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<ref_width$}  {:<status_width$}  {:<40}  {:<40}  {:<40}",
            row.target_ref,
            row.status.as_str(),
            target_oid,
            row.incoming_oid,
            merge_base_oid,
        );
    }
}

fn render_dry_run_line_stats(line_stats: &[crate::git::FileLineStat]) {
    println!();
    println!("would change (per-file line diff summary):");

    let path_header = "PATH";
    let adds_header = "+LINES";
    let dels_header = "-LINES";

    let path_width = std::cmp::max(
        path_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.path.len())
            .max()
            .unwrap_or(0),
    );
    let adds_width = std::cmp::max(
        adds_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.additions.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let dels_width = std::cmp::max(
        dels_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.deletions.to_string().len())
            .max()
            .unwrap_or(0),
    );

    println!(
        "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
        path_header, adds_header, dels_header
    );

    if line_stats.is_empty() {
        println!("(no file content changes)");
        return;
    }

    for stat in line_stats {
        println!(
            "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
            stat.path, stat.additions, stat.deletions
        );
    }
}
