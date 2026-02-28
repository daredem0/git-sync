//! TUI payload-view selection and object drill-down state transitions.

use crate::git;
use crate::ui::format::single_line_error;
use crate::ui::types::{AppState, AuditModel, PayloadModel, PayloadObjectViewState};
use ratatui::text::Line;

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
                let lines = detail
                    .lines
                    .into_iter()
                    .map(Line::from)
                    .collect::<Vec<Line<'static>>>();
                let max_line_width = lines.iter().map(|line| line.width()).max().unwrap_or(0);
                self.payload_object_view = Some(PayloadObjectViewState {
                    oid: detail.oid,
                    kind: detail.kind,
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
