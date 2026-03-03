// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload view rendering module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod detail;
mod layout;
mod preview;
mod tables;
mod util;

use super::render_footer_text;
use crate::ui::types::{AppState, AuditModel, PayloadModel};
use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

const FOCUS_ACCENT: Color = Color::Cyan;

/// Renders payload page tables or selected payload-object detail view.
pub(crate) fn render_payload_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.payload_object_view.is_some() {
        detail::render_payload_object_detail(frame, state);
        return;
    }

    let page_layout = layout::split_payload_page(frame.area());
    let title = Paragraph::new(Text::from(payload_title_lines(model, state)))
        .block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(title, page_layout.title);

    match &model.payload {
        PayloadModel::Failed(err) => {
            let body = Paragraph::new(format!(
                "Payload data is unavailable.\n\
                 error: {err}\n\
                 \n\
                 Verify the bundle input and retry."
            ))
            .block(Block::default().borders(Borders::ALL).title("Payload"));
            frame.render_widget(body, page_layout.body);
        }
        PayloadModel::Ok(payload) => {
            let body_layout = layout::split_payload_body(page_layout.body);
            tables::render_transport_entries_table(frame, payload, body_layout.transport_entries);
            if state.is_payload_entries_view() {
                tables::render_entries_table(frame, payload, state, body_layout.left_table);
            } else {
                tables::render_objects_table(frame, payload, state, body_layout.left_table);
            }
            preview::render_pack_preview(frame, model, state, body_layout.preview);
        }
    }

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, page_layout.footer);
}

/// Builds payload-page top summary including pack-proof invariants.
fn payload_title_text(model: &AuditModel, state: &AppState) -> String {
    match &model.payload {
        PayloadModel::Ok(payload) => {
            let proof = &payload.pack_proof;
            let commit_count = payload
                .objects
                .iter()
                .filter(|entry| matches!(entry.kind, crate::git::PayloadObjectKind::Commit))
                .count();
            let counts_match = proof.entries_declared == proof.entries_parsed;
            let checksums_match = proof.computed_pack_checksum == proof.trailer_pack_checksum;
            let proof_status = if proof.verification_status.eq_ignore_ascii_case("ok")
                && counts_match
                && checksums_match
            {
                "ok"
            } else {
                "failed"
            };
            let transfer_line = if proof.transfer_allowed {
                "transfer: allowed".to_string()
            } else {
                format!(
                    "transfer: blocked ({})",
                    proof
                        .blocked_reason
                        .as_deref()
                        .unwrap_or("entries not fully materialized")
                )
            };
            format!(
                "Payload View\n\
                 Press 1 main | 2 payload | 3 commit\n\
                 status: {proof_status} | pack version: {}\n\
                 entries: {}/{} | materialized: {}/{} | commits: {}\n\
                 unique objects: {} | duplicates: {}\n\
                 {transfer_line} | hash: {} | checksum: {}\n\
                 thin pack: {} | baseline resolutions: {}\n\
                 computed checksum: {}\n\
                 trailer checksum: {}\n\
                 subview: {} (toggle: e)",
                proof.pack_version,
                proof.entries_parsed,
                proof.entries_declared,
                proof.entries_materialized,
                proof.entries_declared,
                commit_count,
                proof.unique_objects_materialized,
                proof.duplicate_entry_count_materialized,
                proof.hash_algorithm,
                if proof.checksum_verified {
                    "ok"
                } else {
                    "failed"
                },
                if proof.thin_pack_detected {
                    "yes"
                } else {
                    "no"
                },
                proof.baseline_resolutions_count,
                proof.computed_pack_checksum,
                proof.trailer_pack_checksum,
                state.payload_sub_view_label()
            )
        }
        PayloadModel::Failed(_) => "Payload View\n\
            Press 1 main | 2 payload | 3 commit\n\
            Transport package entries, selected-object preview, and full pack object listing\n\
            Use j/k to select object rows and Enter to open object detail"
            .to_string(),
    }
}

/// Builds payload-page lines and colors pass/fail fields for quick scanning.
fn payload_title_lines(model: &AuditModel, state: &AppState) -> Vec<Line<'static>> {
    payload_title_text(model, state)
        .lines()
        .map(style_payload_title_line)
        .collect()
}

/// Styles payload-summary fields while preserving the existing text content.
fn style_payload_title_line(line: &str) -> Line<'static> {
    if let Some(rest) = line.strip_prefix("status: ")
        && let Some((status, pack_version)) = rest.split_once(" | pack version: ")
    {
        return Line::from(vec![
            Span::raw("status: "),
            Span::styled(
                status.to_string(),
                status_style(status.eq_ignore_ascii_case("ok")),
            ),
            Span::raw(" | pack version: "),
            Span::raw(pack_version.to_string()),
        ]);
    }

    if let Some(rest) = line.strip_prefix("entries: ")
        && let Some((entries_ratio, materialized_tail)) = rest.split_once(" | materialized: ")
    {
        let (materialized_ratio, commit_count) = if let Some((materialized_ratio, commit_count)) =
            materialized_tail.split_once(" | commits: ")
        {
            (materialized_ratio, Some(commit_count))
        } else {
            (materialized_tail, None)
        };
        let mut spans = vec![
            Span::raw("entries: "),
            Span::styled(
                entries_ratio.to_string(),
                status_style(ratio_matches_declared(entries_ratio)),
            ),
            Span::raw(" | materialized: "),
            Span::styled(
                materialized_ratio.to_string(),
                status_style(ratio_matches_declared(materialized_ratio)),
            ),
        ];
        if let Some(commit_count) = commit_count {
            spans.push(Span::raw(" | commits: "));
            spans.push(Span::raw(commit_count.to_string()));
        }
        return Line::from(spans);
    }

    if let Some(rest) = line.strip_prefix("transfer: ")
        && let Some((transfer_value, hash_and_checksum)) = rest.split_once(" | hash: ")
        && let Some((hash_value, checksum_value)) = hash_and_checksum.split_once(" | checksum: ")
    {
        return Line::from(vec![
            Span::raw("transfer: "),
            Span::styled(
                transfer_value.to_string(),
                status_style(transfer_value == "allowed"),
            ),
            Span::raw(" | hash: "),
            Span::raw(hash_value.to_string()),
            Span::raw(" | checksum: "),
            Span::styled(
                checksum_value.to_string(),
                status_style(checksum_value.eq_ignore_ascii_case("ok")),
            ),
        ]);
    }

    if let Some(rest) = line.strip_prefix("subview: ")
        && let Some((subview_value, toggle_suffix)) = rest.split_once(" (toggle: e)")
    {
        return Line::from(vec![
            Span::raw("subview: "),
            Span::styled(subview_value.to_string(), focus_style()),
            Span::raw(format!(" (toggle: e){toggle_suffix}")),
        ]);
    }

    Line::from(line.to_string())
}

/// Returns `true` when a `N/N`-style ratio has matching parsed and declared counts.
fn ratio_matches_declared(value: &str) -> bool {
    let Some((lhs, rhs)) = value.split_once('/') else {
        return false;
    };
    let Ok(lhs) = lhs.trim().parse::<u64>() else {
        return false;
    };
    let Ok(rhs) = rhs.trim().parse::<u64>() else {
        return false;
    };
    lhs == rhs
}

/// Returns semantic status styling for pass/fail values in payload title lines.
fn status_style(passed: bool) -> Style {
    if passed {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    }
}

/// Returns shared cyan accent style used for focused navigation cues.
fn focus_style() -> Style {
    Style::default()
        .fg(FOCUS_ACCENT)
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests;
