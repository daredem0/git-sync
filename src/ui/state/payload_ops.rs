// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI state transition logic for payload ops operations.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::git;
use crate::ui::format::single_line_error;
use crate::ui::types::{
    AppState, AuditModel, PayloadModel, PayloadObjectViewState, PayloadPreviewState,
    PayloadSortMode, PayloadSubView, SyntaxHighlighter,
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

    /// Refreshes cached payload preview content for the selected payload row.
    pub(crate) fn refresh_payload_preview(&mut self, model: &AuditModel) {
        let PayloadModel::Ok(payload) = &model.payload else {
            self.payload_preview = None;
            return;
        };
        let Some((object_id, object_kind, reachable_from_heads)) =
            self.selected_preview_target(payload)
        else {
            self.payload_preview = None;
            return;
        };

        if let Some(cached) = self.payload_preview_cache.get(&object_id).cloned() {
            self.payload_preview = Some(cached);
            return;
        }

        match self.load_payload_object_detail_cached(model, object_id) {
            Ok(detail) => {
                let preview = build_payload_preview_state(&detail, reachable_from_heads);
                self.payload_preview_cache
                    .insert(object_id, preview.clone());
                self.payload_preview = Some(preview);
            }
            Err(err) => {
                self.payload_preview = Some(PayloadPreviewState {
                    oid: object_id,
                    kind: object_kind,
                    lines: vec![
                        format!("preview unavailable for object {object_id}"),
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
        let sorted_len = if self.payload_sub_view == PayloadSubView::Entries {
            payload.entry_ledger.entries.len()
        } else {
            self.payload_sorted_objects(payload).len()
        };
        if sorted_len == 0 {
            return;
        }
        self.payload_selected_index = std::cmp::min(
            self.payload_selected_index.saturating_add(step),
            sorted_len - 1,
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
        let object_id = if self.payload_sub_view == PayloadSubView::Entries {
            let Some(entry) = self.payload_selected_entry(payload) else {
                self.action_message = Some("payload contains no ledger entries".to_string());
                return;
            };
            let Some(oid) = entry.result_oid else {
                self.action_message =
                    Some("selected entry is unresolved; object detail is unavailable".to_string());
                return;
            };
            oid
        } else {
            let sorted = self.payload_sorted_objects(payload);
            if sorted.is_empty() {
                self.action_message = Some("payload contains no importable objects".to_string());
                return;
            }
            let index = std::cmp::min(self.payload_selected_index, sorted.len() - 1);
            sorted[index].oid
        };

        match self.load_payload_object_detail_cached(model, object_id) {
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
                let line_no_width = line_number_width(lines.len());
                let line_no_gutter_width = line_no_width + 3;
                let max_line_width =
                    lines.iter().map(|line| line.width()).max().unwrap_or(0) + line_no_gutter_width;
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

    /// Returns sorted payload objects according to current payload sort mode.
    pub(crate) fn payload_sorted_objects<'a>(
        &self,
        payload: &'a git::PayloadAudit,
    ) -> Vec<&'a git::PayloadObjectEntry> {
        let mut objects = payload.objects.iter().collect::<Vec<_>>();
        if self.payload_sort_mode == PayloadSortMode::Canonical {
            return objects;
        }

        objects.sort_by(|left, right| {
            left.context_head_index
                .is_none()
                .cmp(&right.context_head_index.is_none())
                .then_with(|| left.context_head_index.cmp(&right.context_head_index))
                .then_with(|| left.context_commit_order.cmp(&right.context_commit_order))
                .then_with(|| left.context_path.cmp(&right.context_path))
                .then_with(|| {
                    payload_sort_kind_rank(left.kind).cmp(&payload_sort_kind_rank(right.kind))
                })
                .then_with(|| left.oid.cmp(&right.oid))
        });
        objects
    }

    /// Cycles payload list sorting mode and preserves current object selection when possible.
    pub(crate) fn cycle_payload_sort_mode(&mut self, model: &AuditModel) {
        if self.payload_sub_view != PayloadSubView::Objects {
            return;
        }
        let PayloadModel::Ok(payload) = &model.payload else {
            return;
        };

        let previous_sorted = self.payload_sorted_objects(payload);
        let selected_oid = if previous_sorted.is_empty() {
            None
        } else {
            let index = std::cmp::min(self.payload_selected_index, previous_sorted.len() - 1);
            Some(previous_sorted[index].oid)
        };

        self.payload_sort_mode = match self.payload_sort_mode {
            PayloadSortMode::Canonical => PayloadSortMode::Context,
            PayloadSortMode::Context => PayloadSortMode::Canonical,
        };

        let next_sorted = self.payload_sorted_objects(payload);
        self.payload_selected_index = selected_oid
            .and_then(|oid| next_sorted.iter().position(|entry| entry.oid == oid))
            .unwrap_or(0);
        self.refresh_payload_preview(model);
    }

    /// Toggles payload main-page subview between object rows and raw ledger entry rows.
    pub(crate) fn toggle_payload_sub_view(&mut self, model: &AuditModel) {
        self.payload_sub_view = match self.payload_sub_view {
            PayloadSubView::Objects => PayloadSubView::Entries,
            PayloadSubView::Entries => PayloadSubView::Objects,
        };

        let PayloadModel::Ok(payload) = &model.payload else {
            self.payload_preview = None;
            self.payload_selected_index = 0;
            return;
        };
        let max_index = if self.payload_sub_view == PayloadSubView::Entries {
            payload.entry_ledger.entries.len().saturating_sub(1)
        } else {
            self.payload_sorted_objects(payload).len().saturating_sub(1)
        };
        self.payload_selected_index = std::cmp::min(self.payload_selected_index, max_index);
        self.action_message = None;
        self.refresh_payload_preview(model);
    }

    /// Returns selected ledger entry row while in payload entries subview.
    pub(crate) fn payload_selected_entry<'a>(
        &self,
        payload: &'a git::PayloadAudit,
    ) -> Option<&'a git::PackEntryRecord> {
        if self.payload_sub_view != PayloadSubView::Entries {
            return None;
        }
        payload.entry_ledger.entries.get(std::cmp::min(
            self.payload_selected_index,
            payload.entry_ledger.entries.len().saturating_sub(1),
        ))
    }

    /// Returns human-readable payload subview label.
    pub(crate) fn payload_sub_view_label(&self) -> &'static str {
        match self.payload_sub_view {
            PayloadSubView::Objects => "objects",
            PayloadSubView::Entries => "entries",
        }
    }

    /// Returns whether payload main page currently shows object rows.
    pub(crate) fn is_payload_objects_view(&self) -> bool {
        self.payload_sub_view == PayloadSubView::Objects
    }

    /// Returns whether payload main page currently shows ledger entry rows.
    pub(crate) fn is_payload_entries_view(&self) -> bool {
        self.payload_sub_view == PayloadSubView::Entries
    }

    /// Returns human-readable label for current payload sort mode.
    pub(crate) fn payload_sort_mode_label(&self) -> &'static str {
        match self.payload_sort_mode {
            PayloadSortMode::Canonical => "canonical",
            PayloadSortMode::Context => "context",
        }
    }

    /// Resolves the currently selected row into a previewable object target.
    fn selected_preview_target(
        &mut self,
        payload: &git::PayloadAudit,
    ) -> Option<(git2::Oid, git::PayloadObjectKind, bool)> {
        if self.payload_sub_view == PayloadSubView::Entries {
            let entry = self.payload_selected_entry(payload)?;
            let oid = entry.result_oid?;
            let kind = entry.result_kind.unwrap_or(git::PayloadObjectKind::Unknown);
            let reachable = payload
                .objects
                .iter()
                .find(|object| object.oid == oid)
                .is_some_and(|object| object.reachable_from_heads);
            return Some((oid, kind, reachable));
        }

        let sorted = self.payload_sorted_objects(payload);
        if sorted.is_empty() {
            return None;
        }
        let index = std::cmp::min(self.payload_selected_index, sorted.len() - 1);
        self.payload_selected_index = index;
        let selected = sorted[index];
        Some((selected.oid, selected.kind, selected.reachable_from_heads))
    }
}

/// Computes the number of digits needed for line-number gutters.
fn line_number_width(total_lines: usize) -> usize {
    let mut n = total_lines.max(1);
    let mut digits = 1usize;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

/// Stable kind rank for context-sort tie-breaking.
fn payload_sort_kind_rank(kind: git::PayloadObjectKind) -> u8 {
    match kind {
        git::PayloadObjectKind::Commit => 0,
        git::PayloadObjectKind::Tree => 1,
        git::PayloadObjectKind::Blob => 2,
        git::PayloadObjectKind::Tag => 3,
        git::PayloadObjectKind::Unknown => 4,
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

#[cfg(test)]
mod tests;
