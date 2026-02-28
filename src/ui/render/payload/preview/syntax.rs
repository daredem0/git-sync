//! Syntax-highlighted preview-line rendering.

use super::super::util::line_number_width;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;

/// Renders visible plain preview lines with optional syntax highlighting.
pub(super) fn render_visible_plain_lines(
    lines: &[String],
    syntax_path_hint: Option<&str>,
    syntax_start_index: Option<usize>,
    highlighter: &crate::ui::types::SyntaxHighlighter,
) -> Vec<Line<'static>> {
    let (Some(path_hint), Some(start_index)) = (syntax_path_hint, syntax_start_index) else {
        return lines
            .iter()
            .map(|line| Line::from(line.to_string()))
            .collect::<Vec<_>>();
    };
    let line_no_width = line_number_width(lines.len().saturating_sub(start_index));

    let (syntax, _syntax_name) = highlighter.resolve_syntax_for_path(path_hint);
    let mut syntax_state = HighlightLines::new(syntax, &highlighter.theme);
    let mut rendered = Vec::with_capacity(lines.len());

    for (index, raw_line) in lines.iter().enumerate() {
        if index < start_index {
            rendered.push(Line::from(raw_line.to_string()));
            continue;
        }
        let line_no = index - start_index + 1;
        let mut highlight_input = String::with_capacity(raw_line.len() + 1);
        highlight_input.push_str(raw_line);
        highlight_input.push('\n');
        let spans = match syntax_state.highlight_line(&highlight_input, &highlighter.syntax_set) {
            Ok(regions) if !regions.is_empty() => {
                let last = regions.len() - 1;
                let mut spans = Vec::new();
                for (region_index, (style, segment)) in regions.into_iter().enumerate() {
                    let text = if region_index == last {
                        segment.strip_suffix('\n').unwrap_or(segment)
                    } else {
                        segment
                    };
                    if text.is_empty() {
                        continue;
                    }
                    spans.push(Span::styled(
                        text.to_string(),
                        syntect_style_to_ratatui(style),
                    ));
                }
                if spans.is_empty() {
                    vec![Span::raw(String::new())]
                } else {
                    spans
                }
            }
            _ => vec![Span::raw(raw_line.to_string())],
        };
        rendered.push(numbered_styled_line(line_no, line_no_width, spans));
    }
    rendered
}

/// Prefixes a styled line with a line-number gutter.
fn numbered_styled_line(
    line_no: usize,
    width: usize,
    content: Vec<Span<'static>>,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(content.len() + 1);
    spans.push(Span::styled(
        format!("{line_no:>width$} │ "),
        Style::default().fg(ratatui::style::Color::DarkGray),
    ));
    spans.extend(content);
    Line::from(spans)
}

/// Converts a syntect style span into an equivalent ratatui style.
fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut result = Style::default().fg(ratatui::style::Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        result = result.add_modifier(ratatui::style::Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        result = result.add_modifier(ratatui::style::Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        result = result.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }
    result
}
