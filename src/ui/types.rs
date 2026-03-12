// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Shared UI types for view and state models.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::git;
use ratatui::text::Line;
use std::collections::HashMap;
use std::path::PathBuf;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

#[derive(Debug)]
pub(crate) struct AuditModel {
    pub(crate) overview: OverviewModel,
    pub(crate) commit_pages: CommitPagesModel,
    pub(crate) payload: PayloadModel,
    pub(crate) payload_session: Option<git::PayloadSession>,
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
pub(crate) enum PayloadModel {
    Ok(Box<git::PayloadAudit>),
    Failed(String),
}

#[derive(Debug)]
pub(crate) struct OverviewModel {
    pub(crate) app_version: String,
    pub(crate) repo_path: String,
    pub(crate) bundle_path: String,
    pub(crate) bundle_range_from: String,
    pub(crate) bundle_range_to: String,
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
    pub(crate) history_view_mode: HistoryViewMode,
    pub(crate) history_commit_return_to_graph: bool,
    pub(crate) overview_focus: OverviewFocus,
    pub(crate) payload_sub_view: PayloadSubView,
    pub(crate) payload_sort_mode: PayloadSortMode,
    pub(crate) page_index: usize,
    pub(crate) history_graph_scroll_y: usize,
    pub(crate) selected_head_index: usize,
    pub(crate) selected_change_index: usize,
    pub(crate) selected_file_indices: Vec<Vec<usize>>,
    pub(crate) payload_selected_index: usize,
    pub(crate) show_help: bool,
    pub(crate) help_page_index: usize,
    pub(crate) export_notice: Option<ExportNotice>,
    pub(crate) action_message: Option<String>,
    pub(crate) payload_detail_cache: HashMap<git2::Oid, git::PayloadObjectDetail>,
    pub(crate) payload_preview_cache: HashMap<git2::Oid, PayloadPreviewState>,
    pub(crate) payload_preview: Option<PayloadPreviewState>,
    pub(crate) payload_object_view: Option<PayloadObjectViewState>,
    pub(crate) diff_view: Option<DiffViewState>,
    pub(crate) full_redraw_requested: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ExportNotice {
    pub(crate) path: PathBuf,
    pub(crate) exported_at_human_utc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainView {
    History,
    Payload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryViewMode {
    CommitPages,
    Graph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewFocus {
    Heads,
    WouldChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadSubView {
    Objects,
    Entries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadSortMode {
    Canonical,
    Context,
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

#[derive(Debug, Clone)]
pub(crate) struct PayloadObjectViewState {
    pub(crate) oid: git2::Oid,
    pub(crate) kind: git::PayloadObjectKind,
    pub(crate) syntax_name: String,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) max_line_width: usize,
    pub(crate) scroll_y: usize,
    pub(crate) scroll_x: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PayloadPreviewState {
    pub(crate) oid: git2::Oid,
    pub(crate) kind: git::PayloadObjectKind,
    pub(crate) lines: Vec<String>,
    pub(crate) syntax_path_hint: Option<String>,
    pub(crate) syntax_start_index: Option<usize>,
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
