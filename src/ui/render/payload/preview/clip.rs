//! Preview clipping helper based on current preview panel height.

use super::syntax::render_visible_plain_lines;
use ratatui::layout::Rect;
use ratatui::text::Line;

/// Clips plain preview lines to panel height and highlights only visible lines.
pub(super) fn render_preview_lines_to_area(
    lines: Vec<String>,
    area: Rect,
    syntax_path_hint: Option<&str>,
    syntax_start_index: Option<usize>,
    highlighter: &crate::ui::types::SyntaxHighlighter,
) -> Vec<Line<'static>> {
    let max_rows = usize::from(area.height.saturating_sub(2));
    if max_rows == 0 {
        return Vec::new();
    }

    if lines.len() <= max_rows {
        return render_visible_plain_lines(
            &lines,
            syntax_path_hint,
            syntax_start_index,
            highlighter,
        );
    }
    if max_rows == 1 {
        return vec![Line::from(format!("... ({} more lines)", lines.len()))];
    }

    let shown = max_rows - 1;
    let hidden = lines.len().saturating_sub(shown);
    let mut clipped = render_visible_plain_lines(
        &lines[..shown],
        syntax_path_hint,
        syntax_start_index,
        highlighter,
    );
    clipped.push(Line::from(format!("... ({} more lines)", hidden)));
    clipped
}
