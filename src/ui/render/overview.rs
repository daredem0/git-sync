// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for overview views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::overview_tables::{render_changes_table, render_heads_table};
use super::render_footer_text;
use crate::ui::format::{render_dry_run_status, render_status_line};
use crate::ui::types::{AppState, AuditModel, CommitPagesModel, DryRunLine, PayloadModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const OVERVIEW_COLUMN_SPLIT: [Constraint; 2] =
    [Constraint::Percentage(45), Constraint::Percentage(55)];

/// Renders the overview page with validation, heads, and dry-run summaries.
pub(crate) fn render_overview_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    let overview = &model.overview;
    let page_label = "page 1/1";
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = Paragraph::new(format!(
        "Audit Overview ({})\n\
             This page shows package validity, import heads, and would-change summary\n\
             Press 1 main | 2 payload | 3 commit",
        page_label
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"))
    .wrap(Wrap { trim: false });
    frame.render_widget(title, chunks[0]);

    let (
        pack_proof_status,
        pack_entries_parsed,
        pack_entries_materialized,
        pack_transfer_status,
        pack_checksum_status,
        pack_thin_status,
        pack_baseline_resolutions,
        bundle_reachability_status,
    ) = render_pack_proof_summary(&model.payload);
    let (
        payload_bundle_version,
        payload_heads_count,
        payload_transport_entry_count,
        payload_object_count,
    ) = render_general_payload_summary(&model.payload);
    let general_left_lines = vec![
        format!("tool version: {}", overview.app_version),
        format!("repo: {}", overview.repo_path),
        format!("bundle: {}", overview.bundle_path),
        format!(
            "base_ref: {} | tip_ref: {}",
            overview.base_ref, overview.tip_ref
        ),
        format!("bundle version: {payload_bundle_version}"),
        format!("advertised heads: {payload_heads_count}"),
        format!("transport entries: {payload_transport_entry_count}"),
        format!("payload objects: {payload_object_count}"),
    ];
    let general_right_lines = vec![
        format!(
            "metadata verification: {}",
            render_status_line(&overview.metadata_verification)
        ),
        format!(
            "dry-run applicability: {}",
            render_dry_run_status(&overview.dry_run)
        ),
        format!("pack proof: {pack_proof_status}"),
        format!("pack entries parsed: {pack_entries_parsed}"),
        format!("pack entries materialized: {pack_entries_materialized}"),
        format!("transfer gate: {pack_transfer_status}"),
        format!("pack checksum: {pack_checksum_status}"),
        format!("bundle fully reachable from heads: {bundle_reachability_status}"),
        format!("thin pack detected: {pack_thin_status}"),
        format!("baseline resolutions: {pack_baseline_resolutions}"),
    ];
    let general_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(OVERVIEW_COLUMN_SPLIT)
        .split(chunks[1]);
    let general_left = Paragraph::new(general_left_lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("General"))
        .wrap(Wrap { trim: false });
    frame.render_widget(general_left, general_chunks[0]);
    let general_right = Paragraph::new(general_right_lines.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Bundle Integrity"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(general_right, general_chunks[1]);

    match &overview.dry_run {
        DryRunLine::Ok(result) => {
            let detail_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(OVERVIEW_COLUMN_SPLIT)
                .split(chunks[2]);
            render_heads_table(
                frame,
                result,
                state.selected_head_index,
                state.is_overview_heads_focused(),
                detail_chunks[0],
            );

            let (selected_stats, selected_head_label) = match &model.commit_pages {
                CommitPagesModel::Ok(entries) if !entries.is_empty() => {
                    let selected_head_index =
                        std::cmp::min(state.selected_head_index, entries.len() - 1);
                    (
                        entries[selected_head_index].line_stats.clone(),
                        entries[selected_head_index].head.reference.clone(),
                    )
                }
                _ => {
                    let selected_head_index = if result.imported_heads.is_empty() {
                        0
                    } else {
                        std::cmp::min(state.selected_head_index, result.imported_heads.len() - 1)
                    };
                    let selected_head_label = result
                        .imported_heads
                        .get(selected_head_index)
                        .map(|head| head.reference.clone())
                        .unwrap_or_else(|| "-".to_string());
                    (result.line_stats.clone(), selected_head_label)
                }
            };
            render_changes_table(
                frame,
                &selected_stats,
                &selected_head_label,
                state.selected_change_index(selected_stats.len()),
                state.is_overview_changes_focused(),
                detail_chunks[1],
            );
        }
        DryRunLine::Failed(err) => {
            let failure = Paragraph::new(format!(
                "Dry-run failed, so no per-file summary is available.\nerror: {err}"
            ))
            .block(Block::default().borders(Borders::ALL).title("Would Change"))
            .wrap(Wrap { trim: false });
            frame.render_widget(failure, chunks[2]);
        }
    }

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[3]);
}

/// Returns concise pack-proof status lines for overview's general section.
fn render_pack_proof_summary(
    payload: &PayloadModel,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    match payload {
        PayloadModel::Ok(audit) => {
            let proof = &audit.pack_proof;
            let counts_match = proof.entries_declared == proof.entries_parsed;
            let checksums_match = proof.computed_pack_checksum == proof.trailer_pack_checksum;
            let status = if proof.verification_status.eq_ignore_ascii_case("ok")
                && counts_match
                && checksums_match
            {
                "OK".to_string()
            } else {
                "FAILED".to_string()
            };
            let parsed = format!("{}/{}", proof.entries_parsed, proof.entries_declared);
            let materialized = format!("{}/{}", proof.entries_materialized, proof.entries_declared);
            let transfer = if proof.transfer_allowed {
                "allowed".to_string()
            } else {
                format!(
                    "blocked ({})",
                    proof
                        .blocked_reason
                        .as_deref()
                        .unwrap_or("entries not fully materialized")
                )
            };
            let checksum = if proof.checksum_verified && checksums_match {
                "match"
            } else {
                "mismatch"
            }
            .to_string();
            let thin = if proof.thin_pack_detected {
                "yes"
            } else {
                "no"
            }
            .to_string();
            let baseline_resolutions = proof.baseline_resolutions_count.to_string();
            let total_objects = audit.objects.len();
            let unreachable_objects = audit
                .objects
                .iter()
                .filter(|entry| !entry.reachable_from_heads)
                .count();
            let bundle_reachability = if total_objects == 0 || unreachable_objects == 0 {
                "yes".to_string()
            } else {
                format!("no ({unreachable_objects}/{total_objects} unreachable)")
            };
            (
                status,
                parsed,
                materialized,
                transfer,
                checksum,
                thin,
                baseline_resolutions,
                bundle_reachability,
            )
        }
        PayloadModel::Failed(err) => (
            format!("FAILED (payload unavailable: {err})"),
            "-".to_string(),
            "-".to_string(),
            "blocked".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
        ),
    }
}

/// Returns general payload context lines for the overview's left-side General panel.
fn render_general_payload_summary(payload: &PayloadModel) -> (String, String, String, String) {
    match payload {
        PayloadModel::Ok(audit) => {
            let version = match audit.bundle_version {
                crate::git::BundleVersion::V2 => "v2",
                crate::git::BundleVersion::V3 => "v3",
            }
            .to_string();
            (
                version,
                audit.heads.len().to_string(),
                audit.transport_entries.len().to_string(),
                audit.objects.len().to_string(),
            )
        }
        PayloadModel::Failed(_) => (
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
        ),
    }
}
