// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI state transition logic for navigation operations.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::ui::render::history_graph_commit_oids;
use crate::ui::types::{
    AppState, AuditModel, CommitPagesModel, DryRunLine, HistoryViewMode, MainView, OverviewFocus,
    PayloadSortMode, PayloadSubView,
};

const HELP_PAGE_COUNT: usize = 3;

impl AppState {
    /// Creates initial UI state for the provided audit model.
    pub(crate) fn new(model: &AuditModel) -> Self {
        let selected_file_indices = match &model.commit_pages {
            CommitPagesModel::Ok(entries) => entries
                .iter()
                .map(|head_entry| vec![0; head_entry.commits.len()])
                .collect(),
            CommitPagesModel::Failed(_) => Vec::new(),
        };
        Self {
            main_view: MainView::History,
            history_view_mode: HistoryViewMode::CommitPages,
            overview_focus: OverviewFocus::Heads,
            payload_sub_view: PayloadSubView::Objects,
            page_index: 0,
            history_graph_scroll_y: 0,
            selected_head_index: 0,
            selected_change_index: 0,
            selected_file_indices,
            payload_selected_index: 0,
            payload_sort_mode: PayloadSortMode::Canonical,
            show_help: false,
            help_page_index: 0,
            export_notice: None,
            action_message: None,
            payload_detail_cache: std::collections::HashMap::new(),
            payload_preview_cache: std::collections::HashMap::new(),
            payload_preview: None,
            payload_object_view: None,
            diff_view: None,
        }
    }

    /// Returns the total number of renderable pages in the current model.
    pub(crate) fn total_pages(&self, model: &AuditModel) -> usize {
        if self.main_view == MainView::Payload {
            return 1;
        }

        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                if entries.is_empty() {
                    1
                } else {
                    let selected_head_index = self.clamped_selected_head_index(entries.len());
                    let commit_count = entries[selected_head_index].commits.len();
                    1 + commit_count
                }
            }
            CommitPagesModel::Failed(_) => 2,
        }
    }

    /// Advances to the next page, clamped at the last page.
    pub(crate) fn next_page(&mut self, model: &AuditModel) {
        let last = self.total_pages(model).saturating_sub(1);
        self.page_index = std::cmp::min(self.page_index + 1, last);
        self.action_message = None;
        self.payload_object_view = None;
    }

    /// Moves to the previous page, clamped at zero.
    pub(crate) fn previous_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
        self.action_message = None;
        self.payload_object_view = None;
    }

    /// Jumps to the first page.
    pub(crate) fn first_page(&mut self) {
        self.page_index = 0;
        self.action_message = None;
        self.payload_object_view = None;
    }

    /// Jumps to the last page.
    pub(crate) fn last_page(&mut self, model: &AuditModel) {
        self.page_index = self.total_pages(model).saturating_sub(1);
        self.action_message = None;
        self.payload_object_view = None;
    }

    /// Enters commit-page mode from overview, selecting the first commit page.
    pub(crate) fn enter_selected_head(&mut self, model: &AuditModel) {
        if self.page_index != 0 {
            return;
        }

        if self.total_pages(model) > 1 {
            self.page_index = 1;
            self.action_message = None;
        } else {
            self.action_message = Some("selected head has no commits to review".to_string());
        }
    }

    /// Moves file selection down within the current commit page.
    pub(crate) fn move_selection_down(&mut self, model: &AuditModel) {
        if self.is_history_graph_view() {
            return;
        }
        if self.page_index == 0 && self.main_view == MainView::History {
            match self.overview_focus {
                OverviewFocus::Heads => {
                    let head_count = self.available_head_count(model);
                    if head_count > 0 {
                        let selected = self.clamped_selected_head_index(head_count);
                        self.selected_head_index = std::cmp::min(selected + 1, head_count - 1);
                        self.selected_change_index = 0;
                        self.action_message = None;
                    }
                }
                OverviewFocus::WouldChange => {
                    let change_count = self.available_change_count(model);
                    if change_count > 0 {
                        self.selected_change_index =
                            std::cmp::min(self.selected_change_index + 1, change_count - 1);
                        self.action_message = None;
                    }
                }
            }
            return;
        }

        let Some((commit_index, file_count)) = self.current_commit_context(model) else {
            return;
        };
        if file_count == 0 {
            return;
        }
        if let Some(selected) = self
            .selected_file_indices
            .get_mut(self.selected_head_index)
            .and_then(|head_indices| head_indices.get_mut(commit_index))
        {
            *selected = std::cmp::min(*selected + 1, file_count - 1);
        }
    }

    /// Moves file selection up within the current commit page.
    pub(crate) fn move_selection_up(&mut self, model: &AuditModel) {
        if self.is_history_graph_view() {
            return;
        }
        if self.page_index == 0 && self.main_view == MainView::History {
            match self.overview_focus {
                OverviewFocus::Heads => {
                    let head_count = self.available_head_count(model);
                    if head_count > 0 {
                        self.selected_head_index = self
                            .clamped_selected_head_index(head_count)
                            .saturating_sub(1);
                        self.selected_change_index = 0;
                        self.action_message = None;
                    }
                }
                OverviewFocus::WouldChange => {
                    self.selected_change_index = self.selected_change_index.saturating_sub(1);
                    self.action_message = None;
                }
            }
            return;
        }

        let Some((commit_index, _)) = self.current_commit_context(model) else {
            return;
        };
        if let Some(selected) = self
            .selected_file_indices
            .get_mut(self.selected_head_index)
            .and_then(|head_indices| head_indices.get_mut(commit_index))
        {
            *selected = selected.saturating_sub(1);
        }
    }

    /// Returns the selected file row for a commit page.
    ///
    /// Returns `0` when no tracked selection exists for the commit index.
    pub(crate) fn selected_file_index(&self, commit_index: usize) -> usize {
        self.selected_file_indices
            .get(self.selected_head_index)
            .and_then(|head_indices| head_indices.get(commit_index))
            .copied()
            .unwrap_or(0)
    }

    /// Returns `(commit_index, file_count)` for the currently visible commit page.
    pub(crate) fn current_commit_context(&self, model: &AuditModel) -> Option<(usize, usize)> {
        if self.page_index == 0 {
            return None;
        }
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                let selected_head_index = self.clamped_selected_head_index(entries.len());
                let head_entry = entries.get(selected_head_index)?;
                let commit_index = self.page_index - 1;
                let file_count = head_entry.commits.get(commit_index)?.files.len();
                Some((commit_index, file_count))
            }
            CommitPagesModel::Failed(_) => None,
        }
    }

    /// Returns `true` when the inline diff view is currently open.
    pub(crate) fn is_diff_open(&self) -> bool {
        self.diff_view.is_some()
    }

    /// Toggles help overlay visibility and resets help paging when opening.
    pub(crate) fn toggle_help(&mut self) {
        if self.show_help {
            self.close_help();
        } else {
            self.show_help = true;
            self.help_page_index = 0;
        }
    }

    /// Closes help overlay and resets paging state to the first help page.
    pub(crate) fn close_help(&mut self) {
        self.show_help = false;
        self.help_page_index = 0;
    }

    /// Returns `true` when export notice overlay is currently open.
    pub(crate) fn is_export_notice_open(&self) -> bool {
        self.export_notice.is_some()
    }

    /// Closes export notice overlay.
    pub(crate) fn close_export_notice(&mut self) {
        self.export_notice = None;
    }

    /// Advances help overlay to the next help page.
    pub(crate) fn next_help_page(&mut self) {
        self.help_page_index = std::cmp::min(self.help_page_index + 1, HELP_PAGE_COUNT - 1);
    }

    /// Moves help overlay to the previous help page.
    pub(crate) fn previous_help_page(&mut self) {
        self.help_page_index = self.help_page_index.saturating_sub(1);
    }

    /// Closes the diff view and clears transient action messages.
    pub(crate) fn close_diff(&mut self) {
        self.diff_view = None;
        self.action_message = None;
    }

    /// Toggles main page view mode between history and payload.
    pub(crate) fn toggle_main_view(&mut self) {
        let next = match self.main_view {
            MainView::History => MainView::Payload,
            MainView::Payload => MainView::History,
        };
        self.set_main_view(next);
    }

    /// Switches to history main-page view.
    pub(crate) fn show_history_view(&mut self) {
        self.set_main_view(MainView::History);
    }

    /// Switches to history commit-pages mode.
    pub(crate) fn show_history_commit_pages_view(&mut self) {
        self.show_history_view();
        self.history_view_mode = HistoryViewMode::CommitPages;
        self.history_graph_scroll_y = 0;
    }

    /// Switches to history commit-graph mode.
    pub(crate) fn show_history_graph_view(&mut self) {
        self.show_history_view();
        self.history_view_mode = HistoryViewMode::Graph;
        self.page_index = 0;
        self.history_graph_scroll_y = 0;
        self.action_message = None;
    }

    /// Returns `true` when history graph mode is active.
    pub(crate) fn is_history_graph_view(&self) -> bool {
        self.main_view == MainView::History && self.history_view_mode == HistoryViewMode::Graph
    }

    /// Scrolls history graph mode down by `step` rows.
    pub(crate) fn scroll_history_graph_down(&mut self, model: &AuditModel, step: usize) {
        let max = self.history_graph_row_count(model).saturating_sub(1);
        self.history_graph_scroll_y =
            std::cmp::min(self.history_graph_scroll_y.saturating_add(step), max);
    }

    /// Scrolls history graph mode up by `step` rows.
    pub(crate) fn scroll_history_graph_up(&mut self, step: usize) {
        self.history_graph_scroll_y = self.history_graph_scroll_y.saturating_sub(step);
    }

    /// Returns renderable graph row count for active history commits.
    pub(crate) fn history_graph_row_count(&self, model: &AuditModel) -> usize {
        history_graph_commit_oids(model).len()
    }

    /// Opens commit detail page for the currently selected graph commit.
    pub(crate) fn open_selected_graph_commit(&mut self, model: &AuditModel) {
        let commit_oids = history_graph_commit_oids(model);
        if commit_oids.is_empty() {
            self.action_message = Some("graph has no selectable commits".to_string());
            return;
        }
        let selected_index = std::cmp::min(
            self.history_graph_scroll_y,
            commit_oids.len().saturating_sub(1),
        );
        let selected_oid = commit_oids[selected_index];

        let CommitPagesModel::Ok(entries) = &model.commit_pages else {
            self.action_message = Some("commit pages are unavailable".to_string());
            return;
        };

        for (head_index, head_entry) in entries.iter().enumerate() {
            if let Some(commit_index) = head_entry
                .commits
                .iter()
                .position(|commit| commit.commit_id == selected_oid)
            {
                self.show_history_commit_pages_view();
                self.selected_head_index = head_index;
                self.page_index = commit_index + 1;
                self.action_message = None;
                return;
            }
        }

        self.action_message =
            Some("selected graph commit is outside commit-page scope".to_string());
    }

    /// Switches to payload main-page view.
    pub(crate) fn show_payload_view(&mut self) {
        self.set_main_view(MainView::Payload);
    }

    /// Sets main-page view mode and normalizes page-scoped state.
    fn set_main_view(&mut self, view: MainView) {
        if self.main_view == view {
            return;
        }
        self.main_view = view;
        self.page_index = 0;
        if view == MainView::History {
            self.overview_focus = OverviewFocus::Heads;
            self.history_view_mode = HistoryViewMode::CommitPages;
            self.history_graph_scroll_y = 0;
        }
        self.payload_preview = None;
        self.payload_object_view = None;
        self.action_message = None;
    }

    /// Toggles overview focus between heads and would-change tables.
    pub(crate) fn toggle_overview_focus(&mut self) {
        self.overview_focus = match self.overview_focus {
            OverviewFocus::Heads => OverviewFocus::WouldChange,
            OverviewFocus::WouldChange => OverviewFocus::Heads,
        };
        self.action_message = None;
    }

    /// Returns true when overview currently focuses the heads table.
    pub(crate) fn is_overview_heads_focused(&self) -> bool {
        self.overview_focus == OverviewFocus::Heads
    }

    /// Returns true when overview currently focuses the would-change table.
    pub(crate) fn is_overview_changes_focused(&self) -> bool {
        self.overview_focus == OverviewFocus::WouldChange
    }

    /// Returns selected would-change row index clamped to available rows.
    pub(crate) fn selected_change_index(&self, change_count: usize) -> usize {
        if change_count == 0 {
            0
        } else {
            std::cmp::min(self.selected_change_index, change_count - 1)
        }
    }

    /// Returns selected head index clamped to available head count.
    fn clamped_selected_head_index(&self, head_count: usize) -> usize {
        if head_count == 0 {
            0
        } else {
            std::cmp::min(self.selected_head_index, head_count - 1)
        }
    }

    /// Returns number of available heads in the current model.
    fn available_head_count(&self, model: &AuditModel) -> usize {
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => entries.len(),
            CommitPagesModel::Failed(_) => match &model.overview.dry_run {
                DryRunLine::Ok(result) => result.imported_heads.len(),
                DryRunLine::Failed(_) => 0,
            },
        }
    }

    /// Returns number of file rows available in selected would-change table.
    fn available_change_count(&self, model: &AuditModel) -> usize {
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) if !entries.is_empty() => {
                let selected_head_index = self.clamped_selected_head_index(entries.len());
                entries[selected_head_index].line_stats.len()
            }
            _ => match &model.overview.dry_run {
                DryRunLine::Ok(result) => result.line_stats.len(),
                DryRunLine::Failed(_) => 0,
            },
        }
    }
}
