//! TUI-layer payload-page functionality.

use super::render_footer_text;
use crate::ui::types::{AppState, AuditModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Renders the payload main page placeholder used before full payload browser implementation.
pub(crate) fn render_payload_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(2)])
        .split(frame.area());

    let body = Paragraph::new(format!(
        "Payload View\n\
         This view will host authoritative transport-payload auditing.\n\
         Step 2 currently implements only top-level History/Payload switching.\n\
         \n\
         repo: {}\n\
         bundle: {}\n\
         \n\
         Use 1 to return to History view.",
        model.overview.repo_path, model.overview.bundle_path
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"))
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[1]);
}
