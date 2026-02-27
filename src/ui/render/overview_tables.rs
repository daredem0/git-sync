use crate::git::{self, BundleVersion};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};

pub(super) fn render_heads_table(
    frame: &mut Frame<'_>,
    result: &git::ReceiveBundleResult,
    area: Rect,
) {
    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    let rows: Vec<Row<'_>> = result
        .imported_heads
        .iter()
        .map(|head| {
            Row::new(vec![
                Cell::from(head.oid.to_string()),
                Cell::from(head.reference.clone()),
            ])
        })
        .collect();
    let heads_table = Table::new(rows, [Constraint::Length(40), Constraint::Min(20)])
        .header(Row::new(vec!["OID", "REF"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Heads To Import (bundle {version})")),
        )
        .column_spacing(2);
    frame.render_widget(heads_table, area);
}

pub(super) fn render_changes_table(
    frame: &mut Frame<'_>,
    result: &git::ReceiveBundleResult,
    area: Rect,
) {
    let rows: Vec<Row<'_>> = if result.line_stats.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no file content changes)"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        result
            .line_stats
            .iter()
            .map(|stat| {
                Row::new(vec![
                    Cell::from(stat.path.clone()),
                    Cell::from(stat.additions.to_string()),
                    Cell::from(stat.deletions.to_string()),
                ])
            })
            .collect()
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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Would Change (per-file line diff summary)"),
    )
    .column_spacing(2);
    frame.render_widget(changes_table, area);
}
