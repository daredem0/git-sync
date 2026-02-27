use crate::ui::types::{AppState, AuditModel, CommitPagesModel};

impl AppState {
    pub(crate) fn new(model: &AuditModel) -> Self {
        let selected_file_indices = match &model.commit_pages {
            CommitPagesModel::Ok(entries) => vec![0; entries.len()],
            CommitPagesModel::Failed(_) => Vec::new(),
        };
        Self {
            page_index: 0,
            selected_file_indices,
            show_help: false,
            action_message: None,
            diff_view: None,
        }
    }

    pub(crate) fn total_pages(&self, model: &AuditModel) -> usize {
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                if entries.is_empty() {
                    1
                } else {
                    1 + entries.len()
                }
            }
            CommitPagesModel::Failed(_) => 2,
        }
    }

    pub(crate) fn next_page(&mut self, model: &AuditModel) {
        let last = self.total_pages(model).saturating_sub(1);
        self.page_index = std::cmp::min(self.page_index + 1, last);
        self.action_message = None;
    }

    pub(crate) fn previous_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
        self.action_message = None;
    }

    pub(crate) fn first_page(&mut self) {
        self.page_index = 0;
        self.action_message = None;
    }

    pub(crate) fn last_page(&mut self, model: &AuditModel) {
        self.page_index = self.total_pages(model).saturating_sub(1);
        self.action_message = None;
    }

    pub(crate) fn move_selection_down(&mut self, model: &AuditModel) {
        let Some((commit_index, file_count)) = self.current_commit_context(model) else {
            return;
        };
        if file_count == 0 {
            return;
        }
        if let Some(selected) = self.selected_file_indices.get_mut(commit_index) {
            *selected = std::cmp::min(*selected + 1, file_count - 1);
        }
    }

    pub(crate) fn move_selection_up(&mut self, model: &AuditModel) {
        let Some((commit_index, _)) = self.current_commit_context(model) else {
            return;
        };
        if let Some(selected) = self.selected_file_indices.get_mut(commit_index) {
            *selected = selected.saturating_sub(1);
        }
    }

    pub(crate) fn selected_file_index(&self, commit_index: usize) -> usize {
        self.selected_file_indices
            .get(commit_index)
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn current_commit_context(&self, model: &AuditModel) -> Option<(usize, usize)> {
        if self.page_index == 0 {
            return None;
        }
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                let commit_index = self.page_index - 1;
                let file_count = entries.get(commit_index)?.files.len();
                Some((commit_index, file_count))
            }
            CommitPagesModel::Failed(_) => None,
        }
    }

    pub(crate) fn is_diff_open(&self) -> bool {
        self.diff_view.is_some()
    }

    pub(crate) fn close_diff(&mut self) {
        self.diff_view = None;
        self.action_message = None;
    }
}
