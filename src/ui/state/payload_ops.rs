//! TUI payload-view selection and object drill-down state transitions.

use crate::git;
use crate::ui::format::single_line_error;
use crate::ui::types::{
    AppState, AuditModel, PayloadModel, PayloadObjectViewState, SyntaxHighlighter,
};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;

impl AppState {
    /// Returns `true` when payload object detail view is open.
    pub(crate) fn is_payload_object_open(&self) -> bool {
        self.payload_object_view.is_some()
    }

    /// Closes payload object detail view and clears transient message.
    pub(crate) fn close_payload_object(&mut self) {
        self.payload_object_view = None;
        self.action_message = None;
    }

    /// Moves payload object selection down by one row.
    pub(crate) fn move_payload_selection_down(&mut self, model: &AuditModel) {
        let PayloadModel::Ok(payload) = &model.payload else {
            return;
        };
        if payload.objects.is_empty() {
            return;
        }
        self.payload_selected_index =
            std::cmp::min(self.payload_selected_index + 1, payload.objects.len() - 1);
    }

    /// Moves payload object selection up by one row.
    pub(crate) fn move_payload_selection_up(&mut self, _model: &AuditModel) {
        self.payload_selected_index = self.payload_selected_index.saturating_sub(1);
    }

    /// Opens detail view for the currently selected payload object row.
    pub(crate) fn open_selected_payload_object(&mut self, model: &AuditModel) {
        let PayloadModel::Ok(payload) = &model.payload else {
            self.action_message = Some("payload audit data is unavailable".to_string());
            return;
        };
        if payload.objects.is_empty() {
            self.action_message = Some("payload contains no importable objects".to_string());
            return;
        }

        let index = std::cmp::min(self.payload_selected_index, payload.objects.len() - 1);
        let selected = &payload.objects[index];
        match git::collect_payload_object_detail_for_bundle_input(
            &model.bundle_path,
            &model.repo_path,
            selected.oid,
        ) {
            Ok(detail) => {
                let (lines, syntax_name) =
                    if let Some(path_hint) = detail.syntax_path_hint.as_deref() {
                        render_payload_text_with_syntax(
                            &detail.lines,
                            path_hint,
                            &model.syntax_highlighter,
                        )
                    } else {
                        (
                            detail
                                .lines
                                .iter()
                                .map(|line| Line::from(line.to_string()))
                                .collect::<Vec<Line<'static>>>(),
                            "none".to_string(),
                        )
                    };
                let max_line_width = lines.iter().map(|line| line.width()).max().unwrap_or(0);
                self.payload_object_view = Some(PayloadObjectViewState {
                    oid: detail.oid,
                    kind: detail.kind,
                    syntax_name,
                    lines,
                    max_line_width,
                    scroll_y: 0,
                    scroll_x: 0,
                });
                self.action_message = None;
            }
            Err(err) => {
                self.action_message = Some(format!(
                    "failed to open payload object: {}",
                    single_line_error(&err)
                ));
            }
        }
    }

    /// Scrolls payload object detail down by `step` lines.
    pub(crate) fn scroll_payload_object_down(&mut self, step: usize) {
        if let Some(view) = self.payload_object_view.as_mut() {
            let last = view.lines.len().saturating_sub(1);
            view.scroll_y = std::cmp::min(view.scroll_y.saturating_add(step), last);
        }
    }

    /// Scrolls payload object detail up by `step` lines.
    pub(crate) fn scroll_payload_object_up(&mut self, step: usize) {
        if let Some(view) = self.payload_object_view.as_mut() {
            view.scroll_y = view.scroll_y.saturating_sub(step);
        }
    }

    /// Scrolls payload object detail right by `step` columns.
    pub(crate) fn scroll_payload_object_right(&mut self, step: usize) {
        if let Some(view) = self.payload_object_view.as_mut() {
            let max = view.max_line_width.saturating_sub(1);
            view.scroll_x = std::cmp::min(view.scroll_x.saturating_add(step), max);
        }
    }

    /// Scrolls payload object detail left by `step` columns.
    pub(crate) fn scroll_payload_object_left(&mut self, step: usize) {
        if let Some(view) = self.payload_object_view.as_mut() {
            view.scroll_x = view.scroll_x.saturating_sub(step);
        }
    }

    /// Resets payload object detail scroll offsets.
    pub(crate) fn reset_payload_object_scroll(&mut self) {
        if let Some(view) = self.payload_object_view.as_mut() {
            view.scroll_x = 0;
            view.scroll_y = 0;
        }
    }
}

/// Renders plain text lines with syntax highlighting based on a path hint.
fn render_payload_text_with_syntax(
    lines: &[String],
    path_hint: &str,
    highlighter: &SyntaxHighlighter,
) -> (Vec<Line<'static>>, String) {
    let (syntax, syntax_name) = highlighter.resolve_syntax_for_path(path_hint);
    let mut syntax_state = HighlightLines::new(syntax, &highlighter.theme);
    let mut rendered = Vec::new();

    for raw_line in lines {
        let mut highlight_input = String::with_capacity(raw_line.len() + 1);
        highlight_input.push_str(raw_line);
        highlight_input.push('\n');
        let spans = match syntax_state.highlight_line(&highlight_input, &highlighter.syntax_set) {
            Ok(regions) if !regions.is_empty() => {
                let last = regions.len() - 1;
                let mut spans = Vec::new();
                for (index, (style, segment)) in regions.into_iter().enumerate() {
                    let text = if index == last {
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
        rendered.push(Line::from(spans));
    }

    (rendered, syntax_name)
}

/// Converts a syntect style span into an equivalent ratatui style.
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
