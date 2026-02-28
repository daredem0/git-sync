//! Payload page composition and top-level sections.

mod detail;
mod layout;
mod preview;
mod tables;
mod util;

use super::render_footer_text;
use crate::ui::types::{AppState, AuditModel, PayloadModel};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Renders payload page tables or selected payload-object detail view.
pub(crate) fn render_payload_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.payload_object_view.is_some() {
        detail::render_payload_object_detail(frame, state);
        return;
    }

    let page_layout = layout::split_payload_page(frame.area());
    let title = Paragraph::new(payload_title_text(model, state))
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
                 entries: {}/{} | materialized: {}/{}\n\
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
