// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload table rendering for entries data.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::super::util::{payload_entry_base_ref_label, payload_entry_kind_label, short_oid};
use crate::ui::types::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

const FOCUS_ACCENT: Color = Color::Cyan;

/// Renders payload entry-ledger table with selected-row highlight.
pub(in crate::ui::render::payload) fn render_entries_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    state: &AppState,
    area: Rect,
) {
    let entries = &payload.entry_ledger.entries;
    let rows: Vec<Row<'_>> = if entries.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no entries)"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from((entry.idx + 1).to_string()),
                    Cell::from(entry.offset.to_string()),
                    Cell::from(payload_entry_kind_label(entry.kind)),
                    Cell::from(entry.out_size.to_string()),
                    Cell::from(
                        entry
                            .reconstructed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    Cell::from(payload_entry_base_ref_label(entry.base_ref.as_ref())),
                    Cell::from(
                        entry
                            .result_oid
                            .map(short_oid)
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    Cell::from(if entry.resolved {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    })
                    .style(if entry.resolved {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    }),
                ])
            })
            .collect()
    };
    let title = format!(
        "Pack Entries ({} parsed / {} declared) [active]",
        entries.len(),
        payload.entry_ledger.declared_entry_count
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec![
            "#",
            "OFFSET",
            "KIND",
            "HDR_SIZE",
            "RECON_SIZE",
            "BASE",
            "OID",
            "RESOLVED",
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(
        Style::default()
            .fg(FOCUS_ACCENT)
            .add_modifier(Modifier::REVERSED),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(FOCUS_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
            .title_style(
                Style::default()
                    .fg(FOCUS_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
            .title(title),
    )
    .column_spacing(1);

    let mut table_state = TableState::default();
    if !entries.is_empty() {
        table_state.select(Some(std::cmp::min(
            state.payload_selected_index,
            entries.len() - 1,
        )));
    }
    frame.render_stateful_widget(table, area, &mut table_state);
}
