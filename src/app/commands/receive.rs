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

    render_preflight_plan(&result.preflight_plan);
    render_plan_outcome(
        &result.preflight_plan,
        integrate_policy,
        dry_run,
        result.can_apply_without_conflicts,
    );

    if dry_run {
        render_dry_run_line_stats(&result.line_stats);
        println!();
        println!("dry-run completed successfully.");
    } else {
        println!();
        println!("receive completed successfully.");
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

fn render_preflight_plan(plan: &[ReceivePlanEntry]) {
    println!();
    println!("preflight checks:");

    if plan.is_empty() {
        println!("(no advertised heads)");
        return;
    }

    for (index, row) in plan.iter().enumerate() {
        println!("[{}/{}] {}", index + 1, plan.len(), row.target_ref);
        println!(
            "  status       : {} ({})",
            receive_plan_status_label(row.status),
            row.status.as_str()
        );
        println!(
            "  target oid   : {}",
            format_optional_short_oid(row.target_oid)
        );
        println!("  incoming oid : {}", short_oid(row.incoming_oid));
        println!(
            "  merge base   : {}",
            format_optional_short_oid(row.merge_base_oid)
        );
        println!("  preserved ref: {}", row.preserved_incoming_ref);
    }
}

fn short_oid(oid: git2::Oid) -> String {
    let full = oid.to_string();
    full.chars().take(12).collect()
}

fn format_optional_short_oid(oid: Option<git2::Oid>) -> String {
    oid.map(short_oid).unwrap_or_else(|| "-".to_string())
}

fn receive_plan_status_label(status: crate::git::ReceivePlanStatus) -> &'static str {
    match status {
        crate::git::ReceivePlanStatus::AlreadyPresent => "already present",
        crate::git::ReceivePlanStatus::TargetMissing => "target missing",
        crate::git::ReceivePlanStatus::FastForwardOk => "fast-forward ok",
        crate::git::ReceivePlanStatus::DivergedMergeRequired => "diverged, merge required",
    }
}

fn render_plan_outcome(
    plan: &[ReceivePlanEntry],
    policy: ReceiveIntegratePolicy,
    dry_run: bool,
    can_apply_without_conflicts: bool,
) {
    let mut already_present = 0usize;
    let mut target_missing = 0usize;
    let mut fast_forward_ok = 0usize;
    let mut diverged_merge_required = 0usize;

    for row in plan {
        match row.status {
            crate::git::ReceivePlanStatus::AlreadyPresent => already_present += 1,
            crate::git::ReceivePlanStatus::TargetMissing => target_missing += 1,
            crate::git::ReceivePlanStatus::FastForwardOk => fast_forward_ok += 1,
            crate::git::ReceivePlanStatus::DivergedMergeRequired => diverged_merge_required += 1,
        }
    }

    let policy_label = match policy {
        ReceiveIntegratePolicy::CreateRefsOnly => "create-refs-only",
        ReceiveIntegratePolicy::FastForwardOnly => "fast-forward-only",
    };
    println!();
    println!(
        "plan summary: total={}, already_present={}, target_missing={}, fast_forward_ok={}, diverged_merge_required={}",
        plan.len(),
        already_present,
        target_missing,
        fast_forward_ok,
        diverged_merge_required,
    );

    if dry_run {
        if can_apply_without_conflicts {
            println!("plan result : dry-run check passed");
        } else {
            println!("plan result : dry-run check failed (manual merge required)");
        }
        return;
    }

    match policy {
        ReceiveIntegratePolicy::FastForwardOnly => {
            println!(
                "plan result : receive applied with policy {} (strict fast-forward integration)",
                policy_label
            );
        }
        ReceiveIntegratePolicy::CreateRefsOnly => {
            println!(
                "plan result : receive applied with policy {} (target refs preserved)",
                policy_label
            );
        }
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
