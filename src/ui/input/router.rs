// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Keyboard input handling for router behavior.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::ui::types::{AppState, MainView};
use crossterm::event::KeyCode;

const DIFF_SCROLL_VERTICAL_STEP: usize = 1;
const DIFF_SCROLL_HORIZONTAL_STEP: usize = 2;
const DIFF_SCROLL_PAGE_STEP: usize = 20;
const PAYLOAD_SELECT_PAGE_STEP: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyAction {
    GoHistoryOverview,
    GoPayloadOverview,
    GoCommitDetailPage,
    ExportPayloadAuditJsonFull,
    ExportPayloadAuditJsonLight,
    Quit,
    Escape,
    ToggleHelp,
    ToggleMainView,
    ToggleOverviewFocus,
    HistoryNextPage,
    HistoryPreviousPage,
    HistoryMoveSelectionDown,
    HistoryMoveSelectionUp,
    HistoryFirstPage,
    HistoryLastPage,
    HistoryOpenSelection,
    PayloadMoveSelectionDown(usize),
    PayloadMoveSelectionUp(usize),
    PayloadCycleSort,
    PayloadToggleSubview,
    PayloadOpenSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HelpAction {
    NextPage,
    PreviousPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffAction {
    ScrollDown(usize),
    ScrollUp(usize),
    ScrollRight(usize),
    ScrollLeft(usize),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PayloadObjectAction {
    ScrollDown(usize),
    ScrollUp(usize),
    ScrollRight(usize),
    ScrollLeft(usize),
    Reset,
}

pub(super) fn global_action(code: KeyCode) -> Option<KeyAction> {
    primary_navigation_action(code).or(match code {
        KeyCode::Char('q') => Some(KeyAction::Quit),
        KeyCode::Char('p') => Some(KeyAction::ExportPayloadAuditJsonLight),
        KeyCode::Char('P') => Some(KeyAction::ExportPayloadAuditJsonFull),
        KeyCode::Esc => Some(KeyAction::Escape),
        KeyCode::Char('?') => Some(KeyAction::ToggleHelp),
        _ => None,
    })
}

pub(super) fn action_for_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    if let Some(action) = primary_navigation_action(code) {
        return Some(action);
    }

    let on_main_page = state.page_index == 0;
    let on_history_overview = on_main_page && state.main_view == MainView::History;
    match code {
        KeyCode::Char('v') if on_main_page => return Some(KeyAction::ToggleMainView),
        KeyCode::Tab if on_history_overview => return Some(KeyAction::ToggleOverviewFocus),
        _ => {}
    }

    if state.main_view == MainView::Payload {
        return action_for_payload_page_key(state, code);
    }
    action_for_history_page_key(state, code)
}

pub(super) fn action_for_help_key(code: KeyCode) -> Option<HelpAction> {
    match code {
        KeyCode::Down
        | KeyCode::Char('j')
        | KeyCode::Right
        | KeyCode::Char('l')
        | KeyCode::PageDown
        | KeyCode::Tab => Some(HelpAction::NextPage),
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Left | KeyCode::Char('h') | KeyCode::PageUp => {
            Some(HelpAction::PreviousPage)
        }
        _ => None,
    }
}

pub(super) fn action_for_diff_key(code: KeyCode) -> Option<DiffAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            Some(DiffAction::ScrollDown(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Up | KeyCode::Char('k') => Some(DiffAction::ScrollUp(DIFF_SCROLL_VERTICAL_STEP)),
        KeyCode::Right | KeyCode::Char('l') => {
            Some(DiffAction::ScrollRight(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::Left | KeyCode::Char('h') => {
            Some(DiffAction::ScrollLeft(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::PageDown => Some(DiffAction::ScrollDown(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::PageUp => Some(DiffAction::ScrollUp(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::Home => Some(DiffAction::Reset),
        _ => None,
    }
}

pub(super) fn action_for_payload_object_key(code: KeyCode) -> Option<PayloadObjectAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            Some(PayloadObjectAction::ScrollDown(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Up | KeyCode::Char('k') => {
            Some(PayloadObjectAction::ScrollUp(DIFF_SCROLL_VERTICAL_STEP))
        }
        KeyCode::Right | KeyCode::Char('l') => Some(PayloadObjectAction::ScrollRight(
            DIFF_SCROLL_HORIZONTAL_STEP,
        )),
        KeyCode::Left | KeyCode::Char('h') => {
            Some(PayloadObjectAction::ScrollLeft(DIFF_SCROLL_HORIZONTAL_STEP))
        }
        KeyCode::PageDown => Some(PayloadObjectAction::ScrollDown(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::PageUp => Some(PayloadObjectAction::ScrollUp(DIFF_SCROLL_PAGE_STEP)),
        KeyCode::Home => Some(PayloadObjectAction::Reset),
        _ => None,
    }
}

fn primary_navigation_action(code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Char('1') => Some(KeyAction::GoHistoryOverview),
        KeyCode::Char('2') => Some(KeyAction::GoPayloadOverview),
        KeyCode::Char('3') => Some(KeyAction::GoCommitDetailPage),
        _ => None,
    }
}

fn action_for_payload_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Down | KeyCode::Char('j') => Some(KeyAction::PayloadMoveSelectionDown(1)),
        KeyCode::Up | KeyCode::Char('k') => Some(KeyAction::PayloadMoveSelectionUp(1)),
        KeyCode::PageDown => Some(KeyAction::PayloadMoveSelectionDown(
            PAYLOAD_SELECT_PAGE_STEP,
        )),
        KeyCode::PageUp => Some(KeyAction::PayloadMoveSelectionUp(PAYLOAD_SELECT_PAGE_STEP)),
        KeyCode::Char('s') if state.is_payload_objects_view() => Some(KeyAction::PayloadCycleSort),
        KeyCode::Char('e') => Some(KeyAction::PayloadToggleSubview),
        KeyCode::Enter => Some(KeyAction::PayloadOpenSelection),
        _ => None,
    }
}

fn action_for_history_page_key(state: &AppState, code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Right | KeyCode::Char('l') if state.page_index > 0 => {
            Some(KeyAction::HistoryNextPage)
        }
        KeyCode::Left | KeyCode::Char('h') if state.page_index > 1 => {
            Some(KeyAction::HistoryPreviousPage)
        }
        KeyCode::Down | KeyCode::Char('j') => Some(KeyAction::HistoryMoveSelectionDown),
        KeyCode::Up | KeyCode::Char('k') => Some(KeyAction::HistoryMoveSelectionUp),
        KeyCode::Char('g') => Some(KeyAction::HistoryFirstPage),
        KeyCode::Char('G') => Some(KeyAction::HistoryLastPage),
        KeyCode::Enter => Some(KeyAction::HistoryOpenSelection),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
