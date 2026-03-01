// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for commit table views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::git::CommitAuditEntry;
use crate::ui::types::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

/// Renders the per-commit file stats table with selection highlighting.
pub(super) fn render_commit_files_table(
    frame: &mut Frame<'_>,
    entry: &CommitAuditEntry,
    commit_index: usize,
    state: &AppState,
    area: Rect,
) {
    if entry.files.is_empty() {
        let empty = Paragraph::new("(no file content changes in this commit)").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Changed Files"),
        );
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row<'_>> = entry
        .files
        .iter()
        .map(|stat| {
            Row::new(vec![
                Cell::from(stat.path.clone()),
                Cell::from(stat.additions.to_string()).style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(stat.deletions.to_string()).style(
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        })
        .collect();

    let files_table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["PATH", "+LINES", "-LINES"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Changed Files (this commit)"),
    )
    .column_spacing(2);

    let mut table_state = TableState::default();
    table_state.select(Some(
        state
            .selected_file_index(commit_index)
            .min(entry.files.len() - 1),
    ));
    frame.render_stateful_widget(files_table, area, &mut table_state);
}
