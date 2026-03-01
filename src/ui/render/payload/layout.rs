// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload rendering module for layout views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub(super) struct PayloadPageLayout {
    pub(super) title: Rect,
    pub(super) body: Rect,
    pub(super) footer: Rect,
}

pub(super) struct PayloadBodyLayout {
    pub(super) transport_entries: Rect,
    pub(super) left_table: Rect,
    pub(super) preview: Rect,
}

pub(super) struct PayloadDetailLayout {
    pub(super) header: Rect,
    pub(super) content: Rect,
    pub(super) footer: Rect,
}

pub(super) fn split_payload_page(area: Rect) -> PayloadPageLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);
    PayloadPageLayout {
        title: chunks[0],
        body: chunks[1],
        footer: chunks[2],
    }
}

pub(super) fn split_payload_body(area: Rect) -> PayloadBodyLayout {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(area);
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(8)])
        .split(body_chunks[0]);
    PayloadBodyLayout {
        transport_entries: left_chunks[0],
        left_table: left_chunks[1],
        preview: body_chunks[1],
    }
}

pub(super) fn split_payload_detail(area: Rect) -> PayloadDetailLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);
    PayloadDetailLayout {
        header: chunks[0],
        content: chunks[1],
        footer: chunks[2],
    }
}
