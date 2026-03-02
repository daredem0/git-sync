// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for receive flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::{Result, bail};
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use crate::cli::{OutputFormat, ReceiveIntegratePolicy as CliReceiveIntegratePolicy};
use crate::git::{
    BundleVersion, ReceiveBundleOptions, ReceiveIntegratePolicy, ReceiveMergeabilityCheck,
    ReceivePlanEntry, receive_bundle_input, receive_bundle_input_with_options_and_policy,
    receive_bundle_input_with_options_policy_and_branch_mirror,
    receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check,
};

pub(super) fn run(
    repo: PathBuf,
    bundle: PathBuf,
    verify_metadata: bool,
    dry_run: bool,
    integrate: CliReceiveIntegratePolicy,
    incoming_as_branches: bool,
    check_mergeability: bool,
    format: Option<OutputFormat>,
) -> Result<()> {
    let effective_dry_run = dry_run || check_mergeability;
    if !effective_dry_run && format.is_some() {
        bail!("receive --format is supported only with --dry-run");
    }

    let integrate_policy = match integrate {
        CliReceiveIntegratePolicy::CreateRefsOnly => ReceiveIntegratePolicy::CreateRefsOnly,
        CliReceiveIntegratePolicy::FastForwardOnly => ReceiveIntegratePolicy::FastForwardOnly,
        CliReceiveIntegratePolicy::Merge => ReceiveIntegratePolicy::Merge,
    };
    let result = if !incoming_as_branches
        && !verify_metadata
        && !effective_dry_run
        && integrate_policy == ReceiveIntegratePolicy::FastForwardOnly
        && !check_mergeability
    {
        receive_bundle_input(&bundle, &repo)?
    } else if check_mergeability {
        receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check(
            &bundle,
            &repo,
            ReceiveBundleOptions {
                verify_metadata,
                dry_run: effective_dry_run,
            },
            integrate_policy,
            incoming_as_branches,
            true,
        )?
    } else if incoming_as_branches {
        receive_bundle_input_with_options_policy_and_branch_mirror(
            &bundle,
            &repo,
            ReceiveBundleOptions {
                verify_metadata,
                dry_run: effective_dry_run,
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
                dry_run: effective_dry_run,
            },
            integrate_policy,
        )?
    };

    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    if effective_dry_run && matches!(format, Some(OutputFormat::Json)) {
        println!("{}", render_dry_run_json(version, &result));
        return Ok(());
    }

    if effective_dry_run {
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
    render_plan_actions(&result.preflight_plan, integrate_policy, effective_dry_run);
    render_mergeability_checks(&result.mergeability_checks, check_mergeability);
    render_plan_outcome(
        &result.preflight_plan,
        integrate_policy,
        effective_dry_run,
        result.can_apply_without_conflicts,
        result.apply_backend,
        check_mergeability,
    );

    if effective_dry_run {
        render_dry_run_line_stats(&result.line_stats);
        println!();
        if check_mergeability {
            println!("mergeability check completed successfully.");
        } else {
            println!("dry-run completed successfully.");
        }
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

    let mergeability_rows = result
        .mergeability_checks
        .iter()
        .map(|row| {
            serde_json::json!({
                "target_ref": row.target_ref,
                "target_oid": row.target_oid.map(|oid| oid.to_string()),
                "target_summary": row.target_summary,
                "incoming_oid": row.incoming_oid.to_string(),
                "incoming_summary": row.incoming_summary,
                "merge_base_oid": row.merge_base_oid.map(|oid| oid.to_string()),
                "merge_base_summary": row.merge_base_summary,
                "status": row.status.as_str(),
                "detail": row.detail,
                "conflict_paths": row.conflict_paths,
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&serde_json::json!({
        "bundle_version": version,
        "would_import_heads": result.imported_heads.len(),
        "can_apply_without_conflicts": result.can_apply_without_conflicts,
        "preflight_plan": plan_rows,
        "mergeability_checks": mergeability_rows,
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
    apply_backend: Option<crate::git::ReceiveApplyBackend>,
    check_mergeability: bool,
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
        ReceiveIntegratePolicy::Merge => "merge",
    };
    println!();
    println!(
        "summary: checked {} ref(s): {} fast-forwardable, {} missing target(s), {} already up to date, {} requiring manual merge.",
        plan.len(),
        fast_forward_ok,
        target_missing,
        already_present,
        diverged_merge_required,
    );

    if dry_run {
        if check_mergeability {
            println!("result : mergeability analysis finished; target refs were not updated.");
        } else if can_apply_without_conflicts {
            println!("result : dry-run passed; the selected policy can be applied safely.");
        } else {
            println!(
                "result : dry-run failed; manual merge is required for at least one target ref."
            );
        }
        return;
    }

    if let Some(backend) = apply_backend {
        match backend {
            crate::git::ReceiveApplyBackend::RefTransaction => {
                println!(
                    "safety: target refs were updated through a locked ref transaction (no partial target updates)."
                );
            }
            crate::git::ReceiveApplyBackend::ManualCasRollback => {
                println!(
                    "safety: target refs were updated with compare-and-swap checks and rollback protection."
                );
            }
        }
    }

    match policy {
        ReceiveIntegratePolicy::FastForwardOnly => {
            println!(
                "result : receive applied with policy {} (strict fast-forward integration).",
                policy_label
            );
        }
        ReceiveIntegratePolicy::CreateRefsOnly => {
            println!(
                "result : receive applied with policy {} (target refs preserved).",
                policy_label
            );
        }
        ReceiveIntegratePolicy::Merge => {
            println!(
                "result : receive applied with policy {} (diverged refs merged when clean).",
                policy_label
            );
        }
    }
}

fn render_mergeability_checks(checks: &[ReceiveMergeabilityCheck], check_requested: bool) {
    if !check_requested {
        return;
    }

    println!();
    println!("mergeability checks:");

    if checks.is_empty() {
        println!("no diverged refs detected; merge simulation was not needed.");
        return;
    }

    for (index, row) in checks.iter().enumerate() {
        println!("[{}/{}] {}", index + 1, checks.len(), row.target_ref);
        println!(
            "  status       : {} ({})",
            receive_mergeability_status_label(row.status),
            row.status.as_str()
        );
        let target_display = format_commit_display(row.target_oid, row.target_summary.as_deref());
        let incoming_display =
            format_commit_display(Some(row.incoming_oid), row.incoming_summary.as_deref());
        let merge_base_display =
            format_commit_display(row.merge_base_oid, row.merge_base_summary.as_deref());

        println!("  merge context:");
        println!("    target   : {target_display}");
        println!("    incoming : {incoming_display}");
        println!("    base     : {merge_base_display}");
        println!("    graph    :");
        if let (Some(target_oid), Some(base_oid)) = (row.target_oid, row.merge_base_oid) {
            let target_oid = colorize(&short_oid(target_oid), "31");
            let incoming_oid = colorize(&short_oid(row.incoming_oid), "32");
            let base_oid = colorize(&short_oid(base_oid), "36");
            println!(
                "      * {} {}",
                target_oid,
                row.target_summary
                    .as_deref()
                    .unwrap_or("(subject unavailable)")
            );
            println!(
                "      | * {} {}",
                incoming_oid,
                row.incoming_summary
                    .as_deref()
                    .unwrap_or("(subject unavailable)")
            );
            println!("      |/");
            println!(
                "      * {} {}",
                base_oid,
                row.merge_base_summary
                    .as_deref()
                    .unwrap_or("(subject unavailable)")
            );
        } else {
            println!("      (graph unavailable: merge base or target oid missing)");
        }

        if let Some(detail) = &row.detail {
            println!("  detail       : {}", detail);
        }
        if row.conflict_paths.is_empty() {
            println!("  conflict files: none");
        } else {
            println!("  conflict files:");
            for path in &row.conflict_paths {
                println!("    - {path}");
            }
        }
    }
}

fn receive_mergeability_status_label(
    status: crate::git::ReceiveMergeabilityStatus,
) -> &'static str {
    match status {
        crate::git::ReceiveMergeabilityStatus::Clean => "would merge cleanly",
        crate::git::ReceiveMergeabilityStatus::Conflicted => "would conflict",
        crate::git::ReceiveMergeabilityStatus::Unknown => "unknown (check failed)",
    }
}

fn format_commit_display(oid: Option<git2::Oid>, summary: Option<&str>) -> String {
    match oid {
        Some(oid) => {
            let short = short_oid(oid);
            match summary {
                Some(subject) if !subject.trim().is_empty() => format!("{short} {subject}"),
                _ => short,
            }
        }
        None => "-".to_string(),
    }
}

fn colorize(text: &str, code: &str) -> String {
    if colors_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn colors_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn render_plan_actions(plan: &[ReceivePlanEntry], policy: ReceiveIntegratePolicy, dry_run: bool) {
    println!();
    println!("changes:");

    if plan.is_empty() {
        println!("- none");
        return;
    }

    for row in plan {
        let action_prefix = if dry_run { "would" } else { "did" };
        match policy {
            ReceiveIntegratePolicy::CreateRefsOnly => {
                println!(
                    "- {} keep {} unchanged (create-refs-only mode)",
                    action_prefix, row.target_ref
                );
            }
            ReceiveIntegratePolicy::FastForwardOnly => match row.status {
                crate::git::ReceivePlanStatus::TargetMissing
                | crate::git::ReceivePlanStatus::FastForwardOk => {
                    println!(
                        "- {} {} from {} to {}",
                        if dry_run { "would update" } else { "updated" },
                        row.target_ref,
                        format_optional_short_oid(row.target_oid),
                        short_oid(row.incoming_oid)
                    );
                }
                crate::git::ReceivePlanStatus::AlreadyPresent => {
                    println!(
                        "- {} {} unchanged (already up to date)",
                        if dry_run { "would keep" } else { "kept" },
                        row.target_ref
                    );
                }
                crate::git::ReceivePlanStatus::DivergedMergeRequired => {
                    println!(
                        "- cannot auto-update {} (diverged history, manual merge required)",
                        row.target_ref
                    );
                }
            },
            ReceiveIntegratePolicy::Merge => match row.status {
                crate::git::ReceivePlanStatus::TargetMissing
                | crate::git::ReceivePlanStatus::FastForwardOk => {
                    println!(
                        "- {} {} from {} to {} (merge policy; no merge commit needed)",
                        if dry_run { "would update" } else { "updated" },
                        row.target_ref,
                        format_optional_short_oid(row.target_oid),
                        short_oid(row.incoming_oid)
                    );
                }
                crate::git::ReceivePlanStatus::AlreadyPresent => {
                    println!(
                        "- {} {} unchanged (already up to date)",
                        if dry_run { "would keep" } else { "kept" },
                        row.target_ref
                    );
                }
                crate::git::ReceivePlanStatus::DivergedMergeRequired => {
                    println!(
                        "- {} merge {} with incoming head (merge policy)",
                        if dry_run { "would" } else { "did" },
                        row.target_ref
                    );
                }
            },
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
