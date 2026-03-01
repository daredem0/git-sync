// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for overview tables views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::git::{self, BundleVersion};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

/// Renders the table of bundle heads that would be imported.
pub(super) fn render_heads_table(
    frame: &mut Frame<'_>,
    result: &git::ReceiveBundleResult,
    selected_head_index: usize,
    is_focused: bool,
    area: Rect,
) {
    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    let mut oid_occurrences = std::collections::BTreeMap::<git2::Oid, usize>::new();
    for head in &result.imported_heads {
        *oid_occurrences.entry(head.oid).or_insert(0) += 1;
    }

    let rows: Vec<Row<'_>> = result
        .imported_heads
        .iter()
        .map(|head| {
            let reference = if oid_occurrences.get(&head.oid).copied().unwrap_or(0) > 1 {
                format!("{} (duplicate tip)", head.reference)
            } else {
                head.reference.clone()
            };
            Row::new(vec![
                Cell::from(head.oid.to_string()),
                Cell::from(reference),
            ])
        })
        .collect();
    let heads_title = if is_focused {
        format!("Heads To Import (bundle {version}) [active]")
    } else {
        format!("Heads To Import (bundle {version})")
    };
    let heads_table = Table::new(rows, [Constraint::Length(40), Constraint::Min(20)])
        .header(Row::new(vec!["OID", "REF"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if is_focused {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                })
                .title(heads_title),
        )
        .column_spacing(2);

    let mut table_state = TableState::default();
    if !result.imported_heads.is_empty() {
        table_state.select(Some(std::cmp::min(
            selected_head_index,
            result.imported_heads.len() - 1,
        )));
    }
    frame.render_stateful_widget(heads_table, area, &mut table_state);
}

/// Renders per-file added/deleted line counts from dry-run analysis.
pub(super) fn render_changes_table(
    frame: &mut Frame<'_>,
    line_stats: &[git::FileLineStat],
    selected_head_label: &str,
    selected_change_index: usize,
    is_focused: bool,
    area: Rect,
) {
    let rows: Vec<Row<'_>> = if line_stats.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no file content changes)"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        line_stats
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
            .collect()
    };

    let changes_title = if is_focused {
        format!("Would Change (selected head: {selected_head_label}) [active]")
    } else {
        format!("Would Change (selected head: {selected_head_label})")
    };
    let changes_table = Table::new(
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
            .border_style(if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
            .title(changes_title),
    )
    .column_spacing(2);
    let mut table_state = TableState::default();
    if !line_stats.is_empty() {
        table_state.select(Some(std::cmp::min(
            selected_change_index,
            line_stats.len() - 1,
        )));
    }
    frame.render_stateful_widget(changes_table, area, &mut table_state);
}
