//! TUI-layer overview functionality.

use super::overview_tables::{render_changes_table, render_heads_table};
use super::render_footer_text;
use crate::ui::format::{render_dry_run_status, render_status_line};
use crate::ui::types::{AppState, AuditModel, DryRunLine};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Renders the overview page with validation, heads, and dry-run summaries.
pub(crate) fn render_overview_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    let overview = &model.overview;
    let page_label = format!("page {}/{}", state.page_index + 1, state.total_pages(model));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = Paragraph::new(format!(
        "Audit Overview ({})\n\
             This page shows package validity, import heads, and would-change summary\n\
             Use h/l or left/right to move pages",
        page_label
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"))
    .wrap(Wrap { trim: false });
    frame.render_widget(title, chunks[0]);

    let general_lines = vec![
        format!("tool version: {}", overview.app_version),
        format!("repo: {}", overview.repo_path),
        format!("bundle: {}", overview.bundle_path),
        format!(
            "base_ref: {} | tip_ref: {}",
            overview.base_ref, overview.tip_ref
        ),
        format!(
            "metadata verification: {}",
            render_status_line(&overview.metadata_verification)
        ),
        format!(
            "dry-run applicability: {}",
            render_dry_run_status(&overview.dry_run)
        ),
    ];
    let general = Paragraph::new(general_lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("General"));
    frame.render_widget(general, chunks[1]);

    match &overview.dry_run {
        DryRunLine::Ok(result) => {
            let detail_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(chunks[2]);
            render_heads_table(frame, result, detail_chunks[0]);
            render_changes_table(frame, result, detail_chunks[1]);
        }
        DryRunLine::Failed(err) => {
            let failure = Paragraph::new(format!(
                "Dry-run failed, so no per-file summary is available.\nerror: {err}"
            ))
            .block(Block::default().borders(Borders::ALL).title("Would Change"))
            .wrap(Wrap { trim: false });
            frame.render_widget(failure, chunks[2]);
        }
    }

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[3]);
}
