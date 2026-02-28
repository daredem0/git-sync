//! TUI-layer types functionality.

use crate::git;
use ratatui::text::Line;
use std::path::PathBuf;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

#[derive(Debug)]
pub(crate) struct AuditModel {
    pub(crate) overview: OverviewModel,
    pub(crate) commit_pages: CommitPagesModel,
    pub(crate) repo_path: PathBuf,
    pub(crate) bundle_path: PathBuf,
    pub(crate) syntax_highlighter: SyntaxHighlighter,
}

#[derive(Debug)]
pub(crate) enum CommitPagesModel {
    Ok(Vec<git::HeadAuditEntry>),
    Failed(String),
}

#[derive(Debug)]
pub(crate) struct OverviewModel {
    pub(crate) app_version: String,
    pub(crate) repo_path: String,
    pub(crate) bundle_path: String,
    pub(crate) base_ref: String,
    pub(crate) tip_ref: String,
    pub(crate) metadata_verification: StatusLine,
    pub(crate) dry_run: DryRunLine,
}

#[derive(Debug)]
pub(crate) enum StatusLine {
    Ok,
    Failed(String),
}

#[derive(Debug)]
pub(crate) enum DryRunLine {
    Ok(git::ReceiveBundleResult),
    Failed(String),
}

#[derive(Debug)]
pub(crate) struct AppState {
    pub(crate) main_view: MainView,
    pub(crate) page_index: usize,
    pub(crate) selected_head_index: usize,
    pub(crate) selected_file_indices: Vec<Vec<usize>>,
    pub(crate) show_help: bool,
    pub(crate) action_message: Option<String>,
    pub(crate) diff_view: Option<DiffViewState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainView {
    History,
    Payload,
}

#[derive(Debug, Clone)]
pub(crate) struct DiffViewState {
    pub(crate) commit_index: usize,
    pub(crate) commit_total: usize,
    pub(crate) file_index: usize,
    pub(crate) commit_id: git2::Oid,
    pub(crate) commit_subject: String,
    pub(crate) file_path: String,
    pub(crate) syntax_name: String,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) max_line_width: usize,
    pub(crate) scroll_y: usize,
    pub(crate) scroll_x: usize,
}

#[derive(Debug)]
pub(crate) struct RenderedDiff {
    pub(crate) syntax_name: String,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) max_line_width: usize,
}

#[derive(Debug)]
pub(crate) struct SyntaxHighlighter {
    pub(crate) syntax_set: SyntaxSet,
    pub(crate) theme: Theme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchLineKind {
    Header,
    Hunk,
    Added,
    Deleted,
    Context,
    Other,
}
