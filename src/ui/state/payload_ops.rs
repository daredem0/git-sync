//! TUI payload-view selection and object drill-down state transitions.

use crate::git;
use crate::ui::format::single_line_error;
use crate::ui::types::{
    AppState, AuditModel, PayloadModel, PayloadObjectViewState, PayloadPreviewState,
    SyntaxHighlighter,
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

    /// Refreshes cached payload preview content for the selected object row.
    pub(crate) fn refresh_payload_preview(&mut self, model: &AuditModel) {
        let PayloadModel::Ok(payload) = &model.payload else {
            self.payload_preview = None;
            return;
        };
        if payload.objects.is_empty() {
            self.payload_preview = None;
            return;
        }

        let index = std::cmp::min(self.payload_selected_index, payload.objects.len() - 1);
        self.payload_selected_index = index;
        let selected = &payload.objects[index];
        if let Some(cached) = self.payload_preview_cache.get(&selected.oid).cloned() {
            self.payload_preview = Some(cached);
            return;
        }

        match self.load_payload_object_detail_cached(model, selected.oid) {
            Ok(detail) => {
                let preview = build_payload_preview_state(&detail, selected.reachable_from_heads);
                self.payload_preview_cache
                    .insert(selected.oid, preview.clone());
                self.payload_preview = Some(preview);
            }
            Err(err) => {
                self.payload_preview = Some(PayloadPreviewState {
                    oid: selected.oid,
                    kind: selected.kind,
                    lines: vec![
                        format!("preview unavailable for object {}", selected.oid),
                        format!("error: {}", single_line_error(&err)),
                    ],
                    syntax_path_hint: None,
                    syntax_start_index: None,
                });
            }
        }
    }

    /// Moves payload object selection down by one row.
    pub(crate) fn move_payload_selection_down(&mut self, model: &AuditModel) {
        self.move_payload_selection_down_by(model, 1);
    }

    /// Moves payload object selection up by one row.
    pub(crate) fn move_payload_selection_up(&mut self, model: &AuditModel) {
        self.move_payload_selection_up_by(model, 1);
    }

    /// Moves payload object selection down by `step` rows.
    pub(crate) fn move_payload_selection_down_by(&mut self, model: &AuditModel, step: usize) {
        let PayloadModel::Ok(payload) = &model.payload else {
            return;
        };
        if payload.objects.is_empty() {
            return;
        }
        self.payload_selected_index = std::cmp::min(
            self.payload_selected_index.saturating_add(step),
            payload.objects.len() - 1,
        );
        self.refresh_payload_preview(model);
    }

    /// Moves payload object selection up by `step` rows.
    pub(crate) fn move_payload_selection_up_by(&mut self, model: &AuditModel, step: usize) {
        self.payload_selected_index = self.payload_selected_index.saturating_sub(step);
        self.refresh_payload_preview(model);
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
        match self.load_payload_object_detail_cached(model, selected.oid) {
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

    /// Loads one payload object detail using cached data or reusable payload session.
    fn load_payload_object_detail_cached(
        &mut self,
        model: &AuditModel,
        object_id: git2::Oid,
    ) -> anyhow::Result<git::PayloadObjectDetail> {
        if let Some(cached) = self.payload_detail_cache.get(&object_id).cloned() {
            return Ok(cached);
        }

        let detail = if let Some(session) = model.payload_session.as_ref() {
            git::collect_payload_object_detail_for_session(session, object_id)?
        } else {
            git::collect_payload_object_detail_for_bundle_input(
                &model.bundle_path,
                &model.repo_path,
                object_id,
            )?
        };
        self.payload_detail_cache.insert(object_id, detail.clone());
        Ok(detail)
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

/// Builds a compact preview payload for the selected object on the payload main page.
fn build_payload_preview_state(
    detail: &git::PayloadObjectDetail,
    reachable_from_heads: bool,
) -> PayloadPreviewState {
    if detail.kind != git::PayloadObjectKind::Blob {
        return PayloadPreviewState {
            oid: detail.oid,
            kind: detail.kind,
            lines: detail.lines.clone(),
            syntax_path_hint: None,
            syntax_start_index: None,
        };
    }

    let mut lines = vec![
        format!("blob {}", detail.oid),
        format!("size: {} bytes", detail.size_bytes),
        format!(
            "content: {}",
            if detail.text_line_count.is_some() {
                "text"
            } else {
                "binary"
            }
        ),
        format!(
            "text lines: {}",
            detail
                .text_line_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
    ];

    if detail.blob_paths.is_empty() {
        if reachable_from_heads {
            lines.push("blob paths: (none found in advertised-head trees)".to_string());
        } else {
            lines.push(
                "blob paths: (none; object is unreachable from advertised heads)".to_string(),
            );
        }
    } else {
        lines.push(format!("blob paths: {}", detail.blob_paths.len()));
        for path in detail.blob_paths.iter().take(8) {
            lines.push(format!("  - {path}"));
        }
        if detail.blob_paths.len() > 8 {
            lines.push(format!("  ... and {} more", detail.blob_paths.len() - 8));
        }
    }

    let content_start = detail
        .lines
        .iter()
        .position(|line| line.is_empty())
        .map_or(0, |index| index + 1);
    let preview_body = &detail.lines[content_start..];
    let mut syntax_start_index = None;
    if !preview_body.is_empty() {
        lines.push(String::new());
        lines.push("content preview:".to_string());
        syntax_start_index = detail.syntax_path_hint.as_ref().map(|_| lines.len());
        lines.extend(preview_body.iter().cloned());
    }

    PayloadPreviewState {
        oid: detail.oid,
        kind: detail.kind,
        lines,
        syntax_path_hint: detail.syntax_path_hint.clone(),
        syntax_start_index,
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
