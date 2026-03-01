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
        render_help_overlay(frame, state.is_diff_open());
    }
}

/// Renders footer key-hint text, including transient action messages.
pub(crate) fn render_footer_text(state: &AppState) -> String {
    let base = if state.is_diff_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back | ? help | q quit"
    } else if state.is_payload_object_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset\nEsc back to payload list | ? help | q quit"
    } else if state.main_view == MainView::Payload && state.is_payload_entries_view() {
        "j/k or Up/Down select entry | PgUp/PgDn jump 10 | e toggle objects/entries\nv toggle history/payload | ? help | q quit"
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

/// Renders the centered keymap help overlay for the current mode.
pub(crate) fn render_help_overlay(frame: &mut Frame<'_>, in_diff_view: bool) {
    let area = centered_rect(75, 45, frame.area());
    frame.render_widget(Clear, area);
    let help_text = help_text_for_mode(in_diff_view);

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Keymap"))
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

/// Returns contextual key help for page mode or diff mode.
pub(crate) fn help_text_for_mode(in_diff_view: bool) -> &'static str {
    if in_diff_view {
        "Navigation (Diff View)\n\
         - j / Down: scroll down\n\
         - k / Up: scroll up\n\
         - h / Left: horizontal scroll left\n\
         - l / Right: horizontal scroll right\n\
         - PgUp / PgDn: fast vertical scroll\n\
         - Home: reset scroll\n\
         - Esc: close diff and return to commit page\n\
         - ?: toggle this help\n\
         - q: quit"
    } else {
        "Navigation (Page View)\n\
         - h / Left: previous page\n\
         - l / Right: next page\n\
         - j / Down: move head selection on overview, file selection on commit pages\n\
         - k / Up: move head selection on overview, file selection on commit pages\n\
         - PgUp / PgDn: in payload view, jump object selection by 10 rows\n\
         - s: in payload objects view, cycle object-list sort mode\n\
         - e: in payload view, toggle Objects/Entries subview\n\
         - Tab: in overview, toggle focus between Heads and Would Change tables\n\
         - v: toggle History/Payload main view (main page only)\n\
         - 1: switch to History main view\n\
         - 2: switch to Payload main view\n\
         - 3: open first commit detail page for selected head\n\
         - g: first page\n\
         - G: last page\n\
         - Enter: open selected head (overview) or selected file diff (commit page)\n\
         - Esc: return to overview or quit from overview\n\
         - ?: toggle this help\n\
         - q: quit"
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
