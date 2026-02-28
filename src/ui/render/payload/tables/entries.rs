//! Pack-entry ledger table rendering.

use super::super::util::{payload_entry_base_ref_label, payload_entry_kind_label, short_oid};
use crate::ui::types::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

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
                    }),
                ])
            })
            .collect()
    };
    let title = format!(
        "Pack Entries ({} parsed / {} declared)",
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
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(Block::default().borders(Borders::ALL).title(title))
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
