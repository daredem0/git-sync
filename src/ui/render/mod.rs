// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod commit;
mod commit_table;
mod diff_view;
mod overview;
mod overview_tables;
mod payload;

use crate::ui::types::{AppState, AuditModel, MainView};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub(crate) use commit::render_commit_page;
pub(crate) use diff_view::render_diff_view;
pub(crate) use overview::render_overview_page;
pub(crate) use payload::render_payload_page;

/// Renders the active page (overview, commit, or diff) and optional help overlay.
pub(crate) fn render_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.is_diff_open() {
        render_diff_view(frame, state);
    } else {
        match state.main_view {
            MainView::History => {
                if state.page_index == 0 {
                    render_overview_page(frame, model, state);
                } else {
                    render_commit_page(frame, model, state);
                }
            }
            MainView::Payload => render_payload_page(frame, model, state),
        }
    }

    if state.show_help {
        render_help_overlay(frame, state);
    }
}

/// Renders footer key-hint text, including transient action messages.
pub(crate) fn render_footer_text(state: &AppState) -> String {
    let base = if state.is_diff_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back | ? help | q quit"
    } else if state.is_payload_object_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back to payload list | ? help | q quit"
    } else if state.main_view == MainView::Payload && state.is_payload_entries_view() {
        "j/k or Up/Down select entry | PgUp/PgDn jump 10 | e toggle objects/entries\nEnter open resolved entry detail | v toggle history/payload | ? help | q quit"
    } else if state.main_view == MainView::Payload {
        "j/k or Up/Down select object | PgUp/PgDn jump 10 | s cycle sort | e toggle objects/entries\nEnter open object detail | v toggle history/payload | ? help | q quit"
    } else if state.page_index == 0 {
        "Tab switch heads/would-change focus | j/k or Up/Down move selection\nv toggle history/payload | Enter open selected head | Esc overview/quit | ? help | q quit"
    } else {
        "h/Left prev page | l/Right next page | j/k or Up/Down move selection\nEnter open selected diff | Esc overview/quit | ? help | q quit"
    };
    match &state.action_message {
        Some(message) => format!("{base} | {message}"),
        None => base.to_string(),
    }
}

const HELP_PAGE_COUNT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpContext {
    HistoryOverview,
    HistoryCommit,
    Diff,
    PayloadObjects,
    PayloadEntries,
    PayloadObjectDetail,
}

/// Renders the centered two-page help overlay for the current mode.
pub(crate) fn render_help_overlay(frame: &mut Frame<'_>, state: &AppState) {
    let area = centered_rect(82, 78, frame.area());
    frame.render_widget(Clear, area);
    let context = active_help_context(state);
    let page_index = std::cmp::min(state.help_page_index, HELP_PAGE_COUNT - 1);
    let (page_label, page_text) = if page_index == 0 {
        ("Hotkeys", help_hotkeys_text(context))
    } else {
        ("Context", help_context_text(context))
    };
    let help_text = format!(
        "{page_text}\n\
         \n\
         Page {}/{} | PgUp/PgDn or h/l switch help pages | ? or Esc close | q quit",
        page_index + 1,
        HELP_PAGE_COUNT
    );

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Help {}/{} - {page_label}",
            page_index + 1,
            HELP_PAGE_COUNT
        )))
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

/// Returns contextual key help for page mode or diff mode.
///
/// Kept as a lightweight compatibility helper for tests and diagnostics.
#[cfg(test)]
pub(crate) fn help_text_for_mode(in_diff_view: bool) -> &'static str {
    if in_diff_view {
        help_hotkeys_text(HelpContext::Diff)
    } else {
        help_hotkeys_text(HelpContext::HistoryOverview)
    }
}

fn active_help_context(state: &AppState) -> HelpContext {
    if state.is_diff_open() {
        return HelpContext::Diff;
    }
    if state.is_payload_object_open() {
        return HelpContext::PayloadObjectDetail;
    }
    if state.main_view == MainView::Payload {
        if state.is_payload_entries_view() {
            HelpContext::PayloadEntries
        } else {
            HelpContext::PayloadObjects
        }
    } else if state.page_index == 0 {
        HelpContext::HistoryOverview
    } else {
        HelpContext::HistoryCommit
    }
}

fn help_hotkeys_text(context: HelpContext) -> &'static str {
    match context {
        HelpContext::HistoryOverview => {
            "Hotkeys (Overview)\n\
             - Tab: switch focus between Heads and Would Change tables\n\
             - j/k or Up/Down: move selected row in the focused table\n\
             - Enter: open selected head and move into commit pages\n\
             - v: toggle main view (History/Payload) on main page\n\
             - 1 / 2 / 3: direct jump to main overview, payload, or first commit page\n\
             - ?: open/close help overlay\n\
             - Esc: quit (from overview)\n\
             - q: quit"
        }
        HelpContext::HistoryCommit => {
            "Hotkeys (Commit Page)\n\
             - h/l or Left/Right: previous/next commit page\n\
             - j/k or Up/Down: move selected changed file\n\
             - Enter: open diff for selected file\n\
             - g / G: jump to first or last commit page for selected head\n\
             - 1 / 2 / 3: direct jump to main overview, payload, or first commit page\n\
             - ?: open/close help overlay\n\
             - Esc: return to overview\n\
             - q: quit"
        }
        HelpContext::Diff => {
            "Hotkeys (Diff View)\n\
             - j/k or Up/Down: vertical scroll\n\
             - h/l or Left/Right: horizontal scroll\n\
             - PgUp/PgDn: fast vertical scroll\n\
             - Home: reset diff scroll to origin\n\
             - ?: open/close help overlay\n\
             - Esc: close diff and return to commit page\n\
             - q: quit"
        }
        HelpContext::PayloadObjects => {
            "Hotkeys (Payload Objects)\n\
             - j/k or Up/Down: move selected object row\n\
             - PgUp/PgDn: jump object selection by 10 rows\n\
             - s: cycle object sorting (canonical/context)\n\
             - e: toggle Objects/Entries subview\n\
             - Enter: open selected object detail\n\
             - v: toggle main view (History/Payload) on main page\n\
             - 1 / 2 / 3: direct jump to main overview, payload, or first commit page\n\
             - ?: open/close help overlay\n\
             - Esc: quit (from payload main page)\n\
             - q: quit"
        }
        HelpContext::PayloadEntries => {
            "Hotkeys (Payload Entries)\n\
             - j/k or Up/Down: move selected entry row\n\
             - PgUp/PgDn: jump entry selection by 10 rows\n\
             - e: toggle Objects/Entries subview\n\
             - Enter: open detail for selected resolved entry\n\
             - v: toggle main view (History/Payload) on main page\n\
             - 1 / 2 / 3: direct jump to main overview, payload, or first commit page\n\
             - ?: open/close help overlay\n\
             - Esc: quit (from payload main page)\n\
             - q: quit"
        }
        HelpContext::PayloadObjectDetail => {
            "Hotkeys (Payload Object Detail)\n\
             - j/k or Up/Down: vertical scroll\n\
             - h/l or Left/Right: horizontal scroll\n\
             - PgUp/PgDn: fast vertical scroll\n\
             - Home: reset object-detail scroll to origin\n\
             - ?: open/close help overlay\n\
             - Esc: close object detail and return to payload list\n\
             - q: quit"
        }
    }
}

fn help_context_text(context: HelpContext) -> &'static str {
    match context {
        HelpContext::HistoryOverview => {
            "Overview Glossary\n\
             - metadata verification: sidecar metadata integrity check result for this repo.\n\
             - dry-run applicability: whether receive can apply refs locally without conflicts.\n\
             - pack proof: overall payload verification result (status plus invariant checks).\n\
             - pack entries parsed: parsed PACK entries versus declared entry count (N/N expected).\n\
             - pack entries materialized: entries fully reconstructed into canonical git objects (N/N expected).\n\
             - transfer gate: pass/fail decision used to allow or block transfer.\n\
             - pack checksum: computed PACK checksum versus trailer checksum.\n\
             - bundle fully reachable from heads: whether every payload object is reachable from advertised heads.\n\
             - thin pack detected: indicates whether the bundle depended on external base objects.\n\
             - baseline resolutions: number of delta bases resolved from local baseline objects.\n\
             - +LINES / -LINES: per-file added/deleted line counts from dry-run impact analysis."
        }
        HelpContext::HistoryCommit => {
            "Commit Page Glossary\n\
             - commit header fields: selected commit id, subject, and position in the selected head.\n\
             - changed-files table: files touched by the selected commit, shown as review targets.\n\
             - selected file row: Enter opens unified patch/diff rendering for that file.\n\
             - selection state: table position is remembered per commit while paging."
        }
        HelpContext::Diff => {
            "Diff Glossary\n\
             - unified diff: patch format with context and +/- change lines.\n\
             - hunk header (@@ ... @@): old/new line anchors for each changed block.\n\
             - additions/deletions/context: + added lines, - removed lines, and unchanged context lines.\n\
             - syntax name: language lexer selected for diff readability when known."
        }
        HelpContext::PayloadObjects => {
            "Payload Objects Glossary\n\
             - OID: canonical object id (SHA-1 in this project) for the reconstructed git object.\n\
             - TYPE: object kind (commit, tree, blob, tag).\n\
             - SIZE: canonical object size in bytes.\n\
             - REACHABLE: whether object is reachable from advertised heads in the bundle.\n\
             - commit: points to a tree and parent commits; records author/message.\n\
             - tree: directory-like mapping from names to blobs/trees.\n\
             - blob: file content bytes.\n\
             - tag: annotated object that references another object.\n\
             - entries/materialized/duplicates: PACK-entry counters and deduplication summary from proofing."
        }
        HelpContext::PayloadEntries => {
            "Payload Entries Glossary\n\
             - #: 1-based entry order in the PACK stream.\n\
             - OFFSET: byte offset of the entry in the PACK payload.\n\
             - KIND: encoded entry type (commit/tree/blob/tag/ofs-delta/ref-delta).\n\
             - HDR_SIZE: object size declared in the PACK entry header.\n\
             - RECON_SIZE: reconstructed canonical object size after delta apply.\n\
             - BASE: delta base reference; ofs:<distance> for ofs-delta, oid:<prefix> for ref-delta.\n\
             - OID: reconstructed canonical object id when resolution succeeds.\n\
             - RESOLVED: whether this entry could be materialized into a canonical object.\n\
             - ref-delta: delta entry referencing a base object by object id.\n\
             - ofs-delta: delta entry referencing a base object by backward offset in the PACK stream."
        }
        HelpContext::PayloadObjectDetail => {
            "Payload Object Detail Glossary\n\
             - detail header: selected object id, kind, detected syntax, and preview metadata.\n\
             - text preview: decoded text lines for blob objects when available.\n\
             - binary preview: placeholder metadata when textual decoding is not possible.\n\
             - line-number gutter: stable anchor for discussing exact lines during review."
        }
    }
}

/// Computes a centered popup rectangle using percentage-based constraints.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
