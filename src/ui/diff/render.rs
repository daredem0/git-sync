//! TUI-layer render functionality.

use super::parse::{classify_patch_line, line_number_columns, parse_hunk_header};
use super::style::render_patch_content_line;
use crate::ui::types::{RenderedDiff, SyntaxHighlighter};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;

/// Renders a unified patch into styled ratatui lines.
///
/// Each row contains old/new line number columns plus syntax-highlighted
/// content for textual lines.
pub(crate) fn render_patch_with_syntax(
    path: &str,
    patch: &str,
    highlighter: &SyntaxHighlighter,
) -> RenderedDiff {
    let (syntax, syntax_name) = highlighter.resolve_syntax_for_path(path);
    let mut syntax_state = HighlightLines::new(syntax, &highlighter.theme);

    let mut old_line: Option<usize> = None;
    let mut new_line: Option<usize> = None;
    let mut lines = Vec::new();
    let mut max_line_width = 0usize;

    for raw_line in patch.lines() {
        if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
            old_line = Some(old_start);
            new_line = Some(new_start);
        }

        let kind = classify_patch_line(raw_line);
        let (old_display, new_display) = line_number_columns(kind, &mut old_line, &mut new_line);
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("{:>6} {:>6} │ ", old_display, new_display),
            Style::default().fg(Color::DarkGray),
        ));

        spans.extend(render_patch_content_line(
            raw_line,
            kind,
            &mut syntax_state,
            &highlighter.syntax_set,
        ));

        let visual_width = raw_line.chars().count() + 18;
        max_line_width = std::cmp::max(max_line_width, visual_width);
        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(patch contains no renderable text lines)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    RenderedDiff {
        syntax_name,
        lines,
        max_line_width,
    }
}
