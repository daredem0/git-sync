use crate::ui::types::PatchLineKind;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::parsing::SyntaxSet;

pub(super) fn render_patch_content_line(
    line: &str,
    kind: PatchLineKind,
    syntax_state: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
) -> Vec<Span<'static>> {
    let semantic_style = semantic_content_style(kind);

    match kind {
        PatchLineKind::Header => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Hunk => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Other => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Added | PatchLineKind::Deleted | PatchLineKind::Context => {
            let prefix_len = line.chars().next().map(char::len_utf8).unwrap_or(0);
            let (prefix, content) = line.split_at(prefix_len);
            let mut spans = vec![Span::styled(
                prefix.to_string(),
                semantic_prefix_style(kind),
            )];

            let mut highlight_input = String::with_capacity(content.len() + 1);
            highlight_input.push_str(content);
            highlight_input.push('\n');

            let regions = syntax_state.highlight_line(&highlight_input, syntax_set);
            match regions {
                Ok(regions) if !regions.is_empty() => {
                    let last = regions.len() - 1;
                    for (index, (style, segment)) in regions.into_iter().enumerate() {
                        let text = if index == last {
                            segment.strip_suffix('\n').unwrap_or(segment)
                        } else {
                            segment
                        };
                        if text.is_empty() {
                            continue;
                        }
                        let span_style = syntect_style_to_ratatui(style).patch(semantic_style);
                        spans.push(Span::styled(text.to_string(), span_style));
                    }
                }
                _ => {
                    spans.push(Span::styled(content.to_string(), semantic_style));
                }
            }

            if spans.len() == 1 {
                spans.push(Span::styled(String::new(), semantic_style));
            }
            spans
        }
    }
}

fn semantic_content_style(kind: PatchLineKind) -> Style {
    match kind {
        PatchLineKind::Added => Style::default().bg(Color::Rgb(18, 46, 20)),
        PatchLineKind::Deleted => Style::default().bg(Color::Rgb(52, 20, 20)),
        PatchLineKind::Hunk => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Header => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn semantic_prefix_style(kind: PatchLineKind) -> Style {
    match kind {
        PatchLineKind::Added => Style::default()
            .fg(Color::Green)
            .bg(Color::Rgb(18, 46, 20))
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Deleted => Style::default()
            .fg(Color::Red)
            .bg(Color::Rgb(52, 20, 20))
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Context => Style::default().fg(Color::DarkGray),
        _ => Style::default(),
    }
}

fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut result = Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        result = result.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        result = result.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        result = result.add_modifier(Modifier::UNDERLINED);
    }

    result
}
