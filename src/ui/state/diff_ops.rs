//! TUI-layer diff ops functionality.

use crate::git;
use crate::ui::diff::render_patch_with_syntax;
use crate::ui::format::{is_non_text_patch_unavailable_error, single_line_error};
use crate::ui::types::{AppState, AuditModel, CommitPagesModel, DiffViewState};

impl AppState {
    /// Opens a rendered patch view for the currently selected commit file.
    ///
    /// Non-text files keep page mode open and avoid showing a hard error.
    pub(crate) fn open_selected_diff(&mut self, model: &AuditModel) {
        let Some((commit_index, file_count)) = self.current_commit_context(model) else {
            return;
        };
        if file_count == 0 {
            self.action_message = Some("selected commit has no file content changes".to_string());
            return;
        }

        let CommitPagesModel::Ok(entries) = &model.commit_pages else {
            self.action_message = Some("commit pages are unavailable".to_string());
            return;
        };

        let Some(commit_entry) = entries.get(commit_index) else {
            self.action_message = Some("commit index is out of range".to_string());
            return;
        };

        let file_index = self
            .selected_file_index(commit_index)
            .min(commit_entry.files.len() - 1);
        let file_path = commit_entry.files[file_index].path.clone();

        let patch = git::collect_commit_file_patch_for_bundle_input(
            &model.bundle_path,
            &model.repo_path,
            commit_entry.commit_id,
            &file_path,
        );

        match patch {
            Ok(patch_text) => {
                let rendered =
                    render_patch_with_syntax(&file_path, &patch_text, &model.syntax_highlighter);
                self.diff_view = Some(DiffViewState {
                    commit_index,
                    commit_total: entries.len(),
                    file_index,
                    commit_id: commit_entry.commit_id,
                    commit_subject: commit_entry.subject.clone(),
                    file_path,
                    syntax_name: rendered.syntax_name,
                    lines: rendered.lines,
                    max_line_width: rendered.max_line_width,
                    scroll_y: 0,
                    scroll_x: 0,
                });
                self.action_message = None;
            }
            Err(err) => {
                if is_non_text_patch_unavailable_error(&err) {
                    return;
                }
                self.action_message = Some(format!(
                    "failed to open patch view: {}",
                    single_line_error(&err)
                ));
            }
        }
    }

    /// Scrolls the open diff view down by `step` lines.
    pub(crate) fn scroll_diff_down(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            let last = view.lines.len().saturating_sub(1);
            view.scroll_y = std::cmp::min(view.scroll_y.saturating_add(step), last);
        }
    }

    /// Scrolls the open diff view up by `step` lines.
    pub(crate) fn scroll_diff_up(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_y = view.scroll_y.saturating_sub(step);
        }
    }

    /// Scrolls the open diff view right by `step` columns.
    pub(crate) fn scroll_diff_right(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            let max = view.max_line_width.saturating_sub(1);
            view.scroll_x = std::cmp::min(view.scroll_x.saturating_add(step), max);
        }
    }

    /// Scrolls the open diff view left by `step` columns.
    pub(crate) fn scroll_diff_left(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_x = view.scroll_x.saturating_sub(step);
        }
    }

    /// Resets horizontal and vertical diff scroll offsets to the origin.
    pub(crate) fn reset_diff_scroll(&mut self) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_x = 0;
            view.scroll_y = 0;
        }
    }
}
