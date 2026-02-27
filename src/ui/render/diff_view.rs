use super::render_footer_text;
use crate::ui::types::{AppState, DiffViewState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(crate) fn render_diff_view(frame: &mut Frame<'_>, state: &AppState) {
    let Some(diff_view) = &state.diff_view else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let header = Paragraph::new(format!(
        "Commit {}/{} | {}\n{}\nfile: {}\nsyntax: {} | selected file index: {}",
        diff_view.commit_index + 1,
        diff_view.commit_total,
        diff_view.commit_id,
        diff_view.commit_subject,
        diff_view.file_path,
        diff_view.syntax_name,
        diff_view.file_index + 1
    ))
    .block(Block::default().borders(Borders::ALL).title("Diff View"))
    .wrap(Wrap { trim: false });
    frame.render_widget(header, chunks[0]);

    render_patch_block(frame, diff_view, chunks[1]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

fn render_patch_block(frame: &mut Frame<'_>, diff_view: &DiffViewState, area: Rect) {
    let diff_text = ratatui::text::Text::from(diff_view.lines.clone());
    let diff_paragraph = Paragraph::new(diff_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Patch (first-parent commit diff)"),
        )
        .scroll((
            u16::try_from(diff_view.scroll_y).unwrap_or(u16::MAX),
            u16::try_from(diff_view.scroll_x).unwrap_or(u16::MAX),
        ));
    frame.render_widget(diff_paragraph, area);
}
