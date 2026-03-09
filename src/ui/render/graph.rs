// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for history commit-graph view.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::render_footer_text;
use crate::git::{CommitAuditEntry, HeadAuditEntry};
use crate::ui::types::{AppState, AuditModel, CommitPagesModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::{HashMap, HashSet};

/// Renders a scrollable commit graph derived from bundle commit entries.
pub(crate) fn render_history_graph_page(
    frame: &mut Frame<'_>,
    model: &AuditModel,
    state: &AppState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let body = match &model.commit_pages {
        CommitPagesModel::Ok(entries) => {
            let rows = graph_rows(entries);
            let title = Paragraph::new(format!(
                "Commit Graph\n\
                 Press 1 main | 2 payload | 3 commit | 4 graph\n\
                 rows: {}\n\
                 source: bundle-derived commit entries",
                rows.len()
            ))
            .block(Block::default().borders(Borders::ALL).title("git-sync"))
            .wrap(Wrap { trim: false });
            frame.render_widget(title, chunks[0]);

            let row_text = if rows.is_empty() {
                "(no commits available in bundle scope)".to_string()
            } else {
                rows.join("\n")
            };
            Paragraph::new(Text::from(row_text))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("git log --oneline --decorate --graph (bundle scope)"),
                )
                .scroll((
                    u16::try_from(state.history_graph_scroll_y).unwrap_or(u16::MAX),
                    0,
                ))
        }
        CommitPagesModel::Failed(err) => {
            let title = Paragraph::new(
                "Commit Graph\n\
                 Press 1 main | 2 payload | 3 commit | 4 graph\n\
                 rows: 0\n\
                 source: unavailable",
            )
            .block(Block::default().borders(Borders::ALL).title("git-sync"))
            .wrap(Wrap { trim: false });
            frame.render_widget(title, chunks[0]);

            Paragraph::new(format!(
                "Commit graph data is unavailable.\n\
                 error: {err}\n\
                 \n\
                 The overview and payload pages remain available."
            ))
            .block(Block::default().borders(Borders::ALL).title("Commit Graph"))
            .wrap(Wrap { trim: false })
        }
    };
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

/// Builds graph-formatted rows across all imported heads.
fn graph_rows(entries: &[HeadAuditEntry]) -> Vec<String> {
    let mut commits = HashMap::<git2::Oid, &CommitAuditEntry>::new();
    let mut decorations = HashMap::<git2::Oid, Vec<String>>::new();
    for head_entry in entries {
        decorations
            .entry(head_entry.head.oid)
            .or_default()
            .push(head_entry.head.reference.clone());
        for commit in &head_entry.commits {
            commits.entry(commit.commit_id).or_insert(commit);
        }
    }
    if commits.is_empty() {
        return Vec::new();
    }

    let order = ordered_commit_ids(entries, &commits);
    let mut columns = Vec::<git2::Oid>::new();
    let mut rows = Vec::new();

    for oid in order {
        let Some(commit) = commits.get(&oid).copied() else {
            continue;
        };
        let current_column = match columns.iter().position(|candidate| *candidate == oid) {
            Some(index) => index,
            None => {
                columns.insert(0, oid);
                0
            }
        };

        let prefix = columns
            .iter()
            .enumerate()
            .map(|(index, _)| if index == current_column { "*" } else { "|" })
            .collect::<Vec<_>>()
            .join(" ");

        rows.push(format!(
            "{prefix} commit {} | tree {}{} | {}",
            short_oid(commit.commit_id),
            short_oid(commit.tree_oid),
            decorate_suffix(decorations.get(&commit.commit_id)),
            commit.subject
        ));

        columns.remove(current_column);
        let in_scope_parents = commit
            .parent_oids
            .iter()
            .copied()
            .filter(|parent_oid| commits.contains_key(parent_oid))
            .collect::<Vec<_>>();
        if let Some(first_parent) = in_scope_parents.first().copied() {
            columns.insert(current_column, first_parent);
            for (offset, parent_oid) in in_scope_parents.iter().copied().enumerate().skip(1) {
                columns.insert(current_column + offset, parent_oid);
            }
        }
        dedupe_columns(&mut columns);
    }

    rows
}

/// Produces an approximation of `git log` traversal order (tips-first) for displayed commits.
fn ordered_commit_ids(
    entries: &[HeadAuditEntry],
    commits: &HashMap<git2::Oid, &CommitAuditEntry>,
) -> Vec<git2::Oid> {
    let mut stack = Vec::<git2::Oid>::new();
    for head_entry in entries {
        if commits.contains_key(&head_entry.head.oid) {
            stack.push(head_entry.head.oid);
        } else if let Some(commit) = head_entry.commits.last() {
            stack.push(commit.commit_id);
        }
    }

    let mut seen = HashSet::<git2::Oid>::new();
    let mut ordered = Vec::<git2::Oid>::new();
    while let Some(oid) = stack.pop() {
        if !seen.insert(oid) {
            continue;
        }
        let Some(commit) = commits.get(&oid) else {
            continue;
        };
        ordered.push(oid);
        for parent_oid in &commit.parent_oids {
            if commits.contains_key(parent_oid) {
                stack.push(*parent_oid);
            }
        }
    }

    for head_entry in entries {
        for commit in head_entry.commits.iter().rev() {
            if seen.insert(commit.commit_id) {
                ordered.push(commit.commit_id);
            }
        }
    }

    ordered
}

/// Removes duplicate commit IDs while preserving first-seen column order.
fn dedupe_columns(columns: &mut Vec<git2::Oid>) {
    let mut seen = HashSet::<git2::Oid>::new();
    columns.retain(|oid| seen.insert(*oid));
}

/// Returns abbreviated fixed-width OID text for compact graph rows.
fn short_oid(oid: git2::Oid) -> String {
    oid.to_string().chars().take(12).collect()
}

/// Formats `--decorate`-style suffix for refs that point to this commit.
fn decorate_suffix(refs: Option<&Vec<String>>) -> String {
    match refs {
        Some(refs) if !refs.is_empty() => format!(" ({})", refs.join(", ")),
        _ => String::new(),
    }
}
