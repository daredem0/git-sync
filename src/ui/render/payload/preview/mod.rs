//! Payload preview panel rendering.

mod clip;
mod syntax;

use super::util::{payload_entry_base_ref_label, payload_entry_kind_label, payload_kind_label};
use crate::ui::types::{AppState, AuditModel, PayloadModel};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Renders selected pack object preview (commit/tree/blob/tag) on payload page.
pub(super) fn render_pack_preview(
    frame: &mut Frame<'_>,
    model: &AuditModel,
    state: &AppState,
    area: Rect,
) {
    let lines: Vec<Line<'_>> = match &model.payload {
        PayloadModel::Failed(_) => vec![
            Line::from("Payload data is unavailable."),
            Line::from("Preview cannot be rendered."),
        ],
        PayloadModel::Ok(payload) if state.is_payload_entries_view() => {
            if let Some(entry) = state.payload_selected_entry(payload) {
                let raw_lines = vec![
                    format!("entry #{}", entry.idx + 1),
                    format!("offset: {}", entry.offset),
                    format!("kind: {}", payload_entry_kind_label(entry.kind)),
                    format!("header size: {}", entry.out_size),
                    format!(
                        "reconstructed size: {}",
                        entry
                            .reconstructed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ),
                    format!(
                        "base: {}",
                        payload_entry_base_ref_label(entry.base_ref.as_ref())
                    ),
                    format!("resolved: {}", if entry.resolved { "yes" } else { "no" }),
                    format!(
                        "resolved via: {}",
                        match entry.resolved_via {
                            Some(crate::git::ResolutionSource::InPack) => "in-pack",
                            Some(crate::git::ResolutionSource::Baseline) => "baseline",
                            None => "-",
                        }
                    ),
                    format!(
                        "oid: {}",
                        entry
                            .result_oid
                            .map(|oid| oid.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ),
                    format!("note: {}", entry.note.as_deref().unwrap_or("-")),
                ];
                clip::render_preview_lines_to_area(
                    raw_lines,
                    area,
                    None,
                    None,
                    &model.syntax_highlighter,
                )
            } else {
                vec![
                    Line::from("No entry selected."),
                    Line::from("Use j/k to select a ledger entry row."),
                ]
            }
        }
        PayloadModel::Ok(payload) => {
            let selected_object = {
                let sorted = state.payload_sorted_objects(payload);
                sorted
                    .get(std::cmp::min(
                        state.payload_selected_index,
                        sorted.len().saturating_sub(1),
                    ))
                    .copied()
            };
            match &state.payload_preview {
                Some(preview) => {
                    let mut raw_lines = vec![format!(
                        "selected: {} ({})",
                        preview.oid,
                        payload_kind_label(preview.kind)
                    )];
                    if let Some(entry) = selected_object {
                        raw_lines.push(format!(
                            "reachable from heads: {}",
                            if entry.reachable_from_heads {
                                "yes"
                            } else {
                                "no"
                            }
                        ));
                        raw_lines.push(format!(
                            "context head: {}",
                            entry
                                .context_head_index
                                .map(|index| format!("#{}", index + 1))
                                .unwrap_or_else(|| "-".to_string())
                        ));
                        raw_lines.push(format!(
                            "context commit order: {}",
                            entry
                                .context_commit_order
                                .map(|order| order.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        ));
                        raw_lines.push(format!(
                            "context path: {}",
                            entry.context_path.as_deref().unwrap_or("-")
                        ));
                    }
                    raw_lines.push(String::new());
                    let prefix_len = raw_lines.len();
                    raw_lines.extend(preview.lines.iter().cloned());
                    let syntax_start = preview.syntax_start_index.map(|index| index + prefix_len);
                    clip::render_preview_lines_to_area(
                        raw_lines,
                        area,
                        preview.syntax_path_hint.as_deref(),
                        syntax_start,
                        &model.syntax_highlighter,
                    )
                }
                None => vec![
                    Line::from("No preview loaded."),
                    Line::from("Use j/k to select a pack object row."),
                    Line::from("Enter opens full object detail."),
                ],
            }
        }
    };

    let preview = Paragraph::new(ratatui::text::Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title("Pack Preview"));
    frame.render_widget(preview, area);
}
