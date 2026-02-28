//! TUI-layer payload-page functionality.

use super::render_footer_text;
use crate::git::PayloadObjectKind;
use crate::ui::types::{AppState, AuditModel, PayloadModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

/// Renders payload page tables or selected payload-object detail view.
pub(crate) fn render_payload_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.payload_object_view.is_some() {
        render_payload_object_detail(frame, state);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = Paragraph::new(
        "Payload View\n\
         Transport entries and full bundle object listing (including unreachable objects)\n\
         Use j/k to select object rows and Enter to open object detail",
    )
    .block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(title, chunks[0]);

    match &model.payload {
        PayloadModel::Failed(err) => {
            let body = Paragraph::new(format!(
                "Payload data is unavailable.\n\
                 error: {err}\n\
                 \n\
                 Verify the bundle input and retry."
            ))
            .block(Block::default().borders(Borders::ALL).title("Payload"));
            frame.render_widget(body, chunks[1]);
        }
        PayloadModel::Ok(payload) => {
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(chunks[1]);
            render_transport_entries_table(frame, payload, body_chunks[0]);
            render_objects_table(frame, payload, state, body_chunks[1]);
        }
    }

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

/// Renders payload transport entry table.
fn render_transport_entries_table(
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

/// Renders payload object table with selected-row highlight.
fn render_objects_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    state: &AppState,
    area: Rect,
) {
    let rows: Vec<Row<'_>> = if payload.objects.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no objects)"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        payload
            .objects
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from(entry.oid.to_string()),
                    Cell::from(payload_kind_label(entry.kind)),
                    Cell::from(entry.size_bytes.to_string()),
                    Cell::from(if entry.reachable_from_heads {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }),
                ])
            })
            .collect()
    };

    let title = format!(
        "Pack Objects ({} total, {} heads)",
        payload.objects.len(),
        payload.heads.len()
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(40),
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
    if !payload.objects.is_empty() {
        table_state.select(Some(std::cmp::min(
            state.payload_selected_index,
            payload.objects.len() - 1,
        )));
    }
    frame.render_stateful_widget(table, area, &mut table_state);
}

/// Renders selected payload object detail with scroll offsets.
fn render_payload_object_detail(frame: &mut Frame<'_>, state: &AppState) {
    let Some(view) = &state.payload_object_view else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let header = Paragraph::new(format!(
        "Payload Object Detail\n\
         oid: {}\n\
         type: {}",
        view.oid,
        payload_kind_label(view.kind)
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(header, chunks[0]);

    let detail_text = ratatui::text::Text::from(view.lines.clone());
    let detail = Paragraph::new(detail_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Object Content"),
        )
        .scroll((
            u16::try_from(view.scroll_y).unwrap_or(u16::MAX),
            u16::try_from(view.scroll_x).unwrap_or(u16::MAX),
        ));
    frame.render_widget(detail, chunks[1]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

/// Returns compact display label for payload object kind.
fn payload_kind_label(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

/// Returns shortened digest prefix for compact table output.
fn short_sha256(digest: &str) -> String {
    if digest.len() <= 12 {
        digest.to_string()
    } else {
        digest[..12].to_string()
    }
}
