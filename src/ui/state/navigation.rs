// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI state transition logic for navigation operations.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::ui::types::{
    AppState, AuditModel, CommitPagesModel, DryRunLine, MainView, OverviewFocus, PayloadSortMode,
    PayloadSubView,
};

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
            overview_focus: OverviewFocus::Heads,
            payload_sub_view: PayloadSubView::Objects,
            page_index: 0,
            selected_head_index: 0,
            selected_change_index: 0,
            selected_file_indices,
            payload_selected_index: 0,
            payload_sort_mode: PayloadSortMode::Canonical,
            show_help: false,
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
