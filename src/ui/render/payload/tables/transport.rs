//! Transport-entry table rendering.

use super::super::util::short_sha256;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

/// Renders payload transport entry table.
pub(in crate::ui::render::payload) fn render_transport_entries_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    area: Rect,
) {
    let rows: Vec<Row<'_>> = if payload.transport_entries.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no transport entries)"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        payload
            .transport_entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from(entry.name.clone()),
                    Cell::from(entry.size_bytes.to_string()),
                    Cell::from(short_sha256(&entry.sha256)),
                ])
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Min(22),
            Constraint::Length(10),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["ENTRY", "SIZE", "SHA256"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Transport Entries"),
    )
    .column_spacing(1);
    frame.render_widget(table, area);
}
