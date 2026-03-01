// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload rendering module for detail views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::super::render_footer_text;
use super::layout;
use super::util::{line_number_width, payload_kind_label};
use crate::ui::types::AppState;
use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Renders selected payload object detail with scroll offsets.
pub(super) fn render_payload_object_detail(frame: &mut Frame<'_>, state: &AppState) {
    let Some(view) = &state.payload_object_view else {
        return;
    };

    let chunks = layout::split_payload_detail(frame.area());
    let header = Paragraph::new(format!(
        "Payload Object Detail\n\
         oid: {}\n\
         type: {}\n\
         syntax: {}",
        view.oid,
        payload_kind_label(view.kind),
        view.syntax_name
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(header, chunks.header);

    let detail_text = ratatui::text::Text::from(numbered_lines(&view.lines));
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
    frame.render_widget(detail, chunks.content);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks.footer);
}

/// Adds line-number gutters to rendered payload object detail lines.
fn numbered_lines(lines: &[Line<'static>]) -> Vec<Line<'static>> {
    let width = line_number_width(lines.len());
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::styled(
                format!("{:>width$} │ ", index + 1),
                Style::default().fg(Color::DarkGray),
            ));
            spans.extend(line.spans.clone());
            Line::from(spans)
        })
        .collect()
}
