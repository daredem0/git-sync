// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod commit;
mod commit_table;
mod diff_view;
mod graph;
mod overview;
mod overview_tables;
mod payload;

use crate::ui::types::{AppState, AuditModel, MainView};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub(crate) use commit::render_commit_page;
pub(crate) use diff_view::render_diff_view;
pub(crate) use graph::render_history_graph_page;
pub(crate) use overview::render_overview_page;
pub(crate) use payload::render_payload_page;

/// Renders the active page (overview, commit, or diff) and optional help overlay.
pub(crate) fn render_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.is_diff_open() {
        render_diff_view(frame, state);
    } else {
        match state.main_view {
            MainView::History => {
                if state.is_history_graph_view() {
                    render_history_graph_page(frame, model, state);
                } else if state.page_index == 0 {
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
    if state.is_export_notice_open() {
        render_export_notice_overlay(frame, state);
    }
}

/// Renders footer key-hint text, including transient action messages.
pub(crate) fn render_footer_text(state: &AppState) -> String {
    let base = if state.is_diff_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back | ? help | p/P export paudit minimal/full | q quit"
    } else if state.is_payload_object_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back to payload list | ? help | p/P export paudit minimal/full | q quit"
    } else if state.main_view == MainView::Payload && state.is_payload_entries_view() {
        "j/k or Up/Down select entry | PgUp/PgDn jump 10 | e toggle objects/entries\nEnter open resolved entry detail | ? help | p/P export paudit minimal/full | q quit"
    } else if state.main_view == MainView::Payload {
        "j/k or Up/Down select object | PgUp/PgDn jump 10 | s cycle sort | e toggle objects/entries\nEnter open object detail | ? help | p/P export paudit minimal/full | q quit"
    } else if state.is_history_graph_view() {
        "j/k or Up/Down scroll | PgUp/PgDn fast scroll | 1/2/3/4 jump pages\nEsc return to overview | ? help | p/P export paudit minimal/full | q quit"
    } else if state.page_index == 0 {
        "Tab switch heads/would-change focus | j/k or Up/Down move selection\nEnter open selected head | Esc overview/quit | ? help | p/P export paudit minimal/full | q quit"
    } else {
        "h/Left prev page | l/Right next page | j/k or Up/Down move selection\nEnter open selected diff | Esc overview/quit | ? help | p/P export paudit minimal/full | q quit"
    };
    match &state.action_message {
        Some(message) => format!("{base} | {message}"),
        None => base.to_string(),
    }
}

const HELP_PAGE_COUNT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpContext {
    HistoryOverview,
    HistoryCommit,
    HistoryGraph,
    Diff,
    PayloadObjects,
    PayloadEntries,
    PayloadObjectDetail,
}

/// Renders the centered pageable help overlay for the current mode.
pub(crate) fn render_help_overlay(frame: &mut Frame<'_>, state: &AppState) {
    let area = centered_rect(82, 78, frame.area());
    frame.render_widget(Clear, area);
    let context = active_help_context(state);
    let page_index = std::cmp::min(state.help_page_index, HELP_PAGE_COUNT - 1);
    let page_label = help_page_label(page_index);
    let page_text = match page_index {
        0 => help_hotkeys_text(context),
        1 => help_context_text(context),
        _ => help_audit_text(context),
    };
    let mut lines = vec![help_page_nav_line(page_index), Line::from(String::new())];
    lines.extend(style_help_text(page_text));
    lines.push(Line::from(String::new()));
    lines.push(help_footer_line(page_index));

    let help = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Help {}/{} - {page_label}",
            page_index + 1,
            HELP_PAGE_COUNT
        )))
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

/// Renders a centered export-success notice overlay.
fn render_export_notice_overlay(frame: &mut Frame<'_>, state: &AppState) {
    let Some(notice) = state.export_notice.as_ref() else {
        return;
    };

    let area = centered_rect(82, 44, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from("Payload audit log was successfully exported."),
        Line::from(String::new()),
        Line::from(format!("Path: {}", notice.path.display())),
        Line::from(format!("Date/time: {}", notice.exported_at_human_utc)),
        Line::from(String::new()),
        Line::from("Press Esc to close this message."),
    ];

    let notice = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("paudit Export"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(notice, area);
}

#[cfg(test)]
pub(crate) use test_api::help_text_for_mode;
#[cfg(test)]
mod test_api;

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
    } else if state.is_history_graph_view() {
        HelpContext::HistoryGraph
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
             - 1 / 2 / 3 / 4: direct jump to overview, payload, commit pages, or commit graph\n\
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
             - 1 / 2 / 3 / 4: direct jump to overview, payload, commit pages, or commit graph\n\
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
             - ?: open/close help overlay\n\
             - Esc: return to overview\n\
             - q: quit"
        }
        HelpContext::HistoryGraph => {
            "Hotkeys (Commit Graph)\n\
             - j/k or Up/Down: vertical scroll through graph rows\n\
             - PgUp/PgDn: fast vertical scroll\n\
             - 1 / 2 / 3 / 4: direct jump to overview, payload, commit pages, or commit graph\n\
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
             - 1 / 2 / 3 / 4: direct jump to overview, payload, commit pages, or commit graph\n\
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
             - 1 / 2 / 3 / 4: direct jump to overview, payload, commit pages, or commit graph\n\
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
             - p: export minimal payload-audit JSON (light details, no ledger or pack-object rows)\n\
             - P: export full payload-audit JSON (.paudit) file\n\
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
        HelpContext::HistoryGraph => {
            "Commit Graph Glossary\n\
             - graph columns: branch/merge shape approximation derived from parent links.\n\
             - commit: abbreviated commit object id for each displayed row.\n\
             - tree: abbreviated tree object id referenced by that commit.\n\
             - decorations: refs whose head OID equals the shown commit.\n\
             - subject: first commit-message line for quick review context."
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

fn help_audit_text(context: HelpContext) -> &'static str {
    match context {
        HelpContext::HistoryOverview => {
            "How to Audit (Overview)\n\
             - Start with green checks only: metadata verification, pack proof, transfer gate, and pack checksum should all pass.\n\
             - Verify the ratios: pack entries parsed and materialized should be N/N (no missing rows).\n\
             - Reachability must be complete: \"bundle fully reachable from heads\" should be yes.\n\
             - Scan the Heads To Import table: confirm refs and OIDs match what you expect to receive.\n\
             - Scan Would Change: unexpected sensitive paths, huge +LINES/-LINES spikes, or many unrelated files are red flags.\n\
             - If any integrity line is red, stop and request a rebuilt bundle before moving forward."
        }
        HelpContext::HistoryCommit => {
            "How to Audit (Commit Page)\n\
             - Work commit by commit, not file by file across commits.\n\
             - Check commit subject/position for a coherent story (feature/fix scope should be narrow and explainable).\n\
             - Open changed files that affect security, build scripts, deployment, auth, crypto, and external interfaces first.\n\
             - Treat wide-scoped commits touching many unrelated directories as suspicious until justified.\n\
             - Use this page to decide where deeper diff review is needed; then open file diffs with Enter."
        }
        HelpContext::HistoryGraph => {
            "How to Audit (Commit Graph)\n\
             - Validate topology first: inspect branch/merge shape before reading individual diffs.\n\
             - Confirm ref decorations point where expected (especially release and integration refs).\n\
             - Spot unexpected merge commits or side branches, then jump to commit pages for details.\n\
             - Use tree identifiers to quickly detect commits that re-root to unexpected trees.\n\
             - Treat unexplained topology changes as a stop signal before approving transfer."
        }
        HelpContext::Diff => {
            "How to Audit (Diff)\n\
             - Read hunk by hunk and ask: what behavior changes for users, data, auth, and network boundaries?\n\
             - Prioritize dangerous classes: credential handling, shell execution, path handling, serialization, and permissions.\n\
             - Look for hidden risk in small edits: condition flips, removed checks, default value changes, and silent error handling.\n\
             - Validate deletions as much as additions; removed checks can be as risky as new code.\n\
             - If intent is unclear, block transfer until commit message and code intent are aligned."
        }
        HelpContext::PayloadObjects => {
            "How to Audit (Payload Objects)\n\
             - Confirm object mix is plausible for the claimed update (commit/tree/blob/tag distribution).\n\
             - Every listed object should be reachable unless explicitly justified.\n\
             - Use object detail on high-risk blobs (scripts/config/security-sensitive files) to inspect real content.\n\
             - Unexpected large blobs or many binary blobs deserve extra scrutiny before approval.\n\
             - This view helps verify \"what is inside\" independent of branch names."
        }
        HelpContext::PayloadEntries => {
            "How to Audit (Payload Entries)\n\
             - Treat this as transport-level evidence: each row is one PACK entry that must resolve cleanly.\n\
             - RESOLVED should be yes for all entries that matter to transfer; unresolved rows are a stop signal.\n\
             - Compare HDR_SIZE and RECON_SIZE for sanity; extreme mismatches can indicate unusual delta expansion.\n\
             - Review delta rows (ofs-delta/ref-delta): they are normal, but unresolved or inconsistent bases are not.\n\
             - BASE and OID fields should look consistent and complete for resolved rows."
        }
        HelpContext::PayloadObjectDetail => {
            "How to Audit (Object Detail)\n\
             - This is the closest view to ground truth content; read it as the exact object payload.\n\
             - For blobs, verify file intent, sensitive data handling, and dangerous operations directly in content.\n\
             - For non-text/binary blobs, require external validation of provenance and expected purpose.\n\
             - Cross-check suspicious objects back to commit/diff context before approving transfer.\n\
             - If object content cannot be explained, block and request clarification or re-export."
        }
    }
}

fn style_help_text(text: &str) -> Vec<Line<'static>> {
    text.lines().map(style_help_line).collect()
}

fn help_page_label(page_index: usize) -> &'static str {
    match page_index {
        0 => "Hotkeys",
        1 => "Glossary",
        _ => "Audit Guide",
    }
}

fn help_page_nav_line(page_index: usize) -> Line<'static> {
    let active_style = help_key_style();
    let inactive_style = Style::default().fg(Color::Gray);
    let labels = ["Hotkeys", "Glossary", "Audit Guide"];
    let mut spans = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", inactive_style));
        }
        let style = if index == page_index {
            active_style
        } else {
            inactive_style
        };
        spans.push(Span::styled(format!("{} {}", index + 1, label), style));
    }
    Line::from(spans)
}

fn style_help_line(line: &str) -> Line<'static> {
    if line.trim().is_empty() {
        return Line::from(String::new());
    }

    let rules = help_highlight_rules();
    let mut spans = Vec::<Span<'static>>::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        let mut next_match: Option<(usize, usize, Style)> = None;
        for (pattern, style) in &rules {
            if let Some(found) = line[cursor..].find(pattern) {
                let start = cursor + found;
                let len = pattern.len();
                match next_match {
                    None => next_match = Some((start, len, *style)),
                    Some((best_start, best_len, _))
                        if start < best_start || (start == best_start && len > best_len) =>
                    {
                        next_match = Some((start, len, *style));
                    }
                    _ => {}
                }
            }
        }

        if let Some((start, len, style)) = next_match {
            if start > cursor {
                spans.push(Span::raw(line[cursor..start].to_string()));
            }
            spans.push(Span::styled(line[start..start + len].to_string(), style));
            cursor = start + len;
        } else {
            spans.push(Span::raw(line[cursor..].to_string()));
            break;
        }
    }

    Line::from(spans)
}

fn help_footer_line(page_index: usize) -> Line<'static> {
    let key_style = help_key_style();
    Line::from(vec![
        Span::raw(format!("Page {}/{} | ", page_index + 1, HELP_PAGE_COUNT)),
        Span::styled("PgUp/PgDn", key_style),
        Span::raw(" or "),
        Span::styled("h/l", key_style),
        Span::raw(" switch pages | "),
        Span::styled("?/Esc", key_style),
        Span::raw(" close | "),
        Span::styled("q", key_style),
        Span::raw(" quit"),
    ])
}

fn help_highlight_rules() -> Vec<(&'static str, Style)> {
    vec![
        ("ref-delta", help_delta_style()),
        ("ofs-delta", help_delta_style()),
        ("RECON_SIZE", help_delta_style()),
        ("HDR_SIZE", help_delta_style()),
        ("materialized", help_ok_style()),
        ("RESOLVED", help_ok_style()),
        ("reachable", help_ok_style()),
        ("checksum", help_ok_style()),
        ("pack proof", help_ok_style()),
        ("OID", help_oid_style()),
        ("oid", help_oid_style()),
        ("commit", help_commit_style()),
        ("tree", help_tree_style()),
        ("blob", help_blob_style()),
        ("tag", help_tag_style()),
        ("PgUp/PgDn", help_key_style()),
        ("Enter", help_key_style()),
        ("Esc", help_key_style()),
        ("Tab", help_key_style()),
        ("p:", help_key_style()),
        ("P:", help_key_style()),
        ("h/l", help_key_style()),
        ("j/k", help_key_style()),
        ("1 / 2 / 3 / 4", help_key_style()),
        ("+LINES", help_ok_style()),
        ("-LINES", help_warn_style()),
    ]
}

fn help_key_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn help_commit_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn help_tree_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn help_blob_style() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::BOLD)
}

fn help_tag_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD)
}

fn help_delta_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn help_oid_style() -> Style {
    Style::default().fg(Color::Yellow)
}

fn help_ok_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

fn help_warn_style() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
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
