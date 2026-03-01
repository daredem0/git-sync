// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload table rendering for objects data.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::super::util::{payload_kind_label, short_oid};
use crate::ui::types::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

/// Renders payload object table with selected-row highlight.
pub(in crate::ui::render::payload) fn render_objects_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    state: &AppState,
    area: Rect,
) {
    let sorted = state.payload_sorted_objects(payload);
    let rows: Vec<Row<'_>> = if sorted.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no objects)"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        sorted
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from(short_oid(entry.oid)),
                    Cell::from(payload_kind_label(entry.kind)),
                    Cell::from(entry.size_bytes.to_string()),
                    Cell::from(if entry.reachable_from_heads {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    })
                    .style(if entry.reachable_from_heads {
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
        "Pack Objects ({} total, {} heads, sort: {})",
        payload.objects.len(),
        payload.heads.len(),
        state.payload_sort_mode_label()
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(11),
        ],
    )
    .header(
        Row::new(vec!["OID", "TYPE", "SIZE", "REACHABLE"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(Block::default().borders(Borders::ALL).title(title))
    .column_spacing(1);

    let mut table_state = TableState::default();
    if !sorted.is_empty() {
        table_state.select(Some(std::cmp::min(
            state.payload_selected_index,
            sorted.len() - 1,
        )));
    }
    frame.render_stateful_widget(table, area, &mut table_state);
}
