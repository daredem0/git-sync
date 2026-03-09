// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for history commit-graph view.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::render_footer_text;
use crate::git::HeadAuditEntry;
use crate::ui::types::{AppState, AuditModel, CommitPagesModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SHORT_OID_LEN: usize = 7;

#[derive(Debug, Clone)]
struct GraphNode {
    oid: git2::Oid,
    tree_oid: Option<git2::Oid>,
    parent_oids: Vec<git2::Oid>,
    subject: String,
}

#[derive(Debug, Clone)]
enum GraphRow {
    Commit {
        columns: Vec<char>,
        oid: git2::Oid,
        decorations: Vec<String>,
        tree_oid: Option<git2::Oid>,
        subject: String,
    },
    Transition {
        columns: Vec<char>,
    },
}

impl GraphRow {
    /// Returns commit OID for commit rows; transition rows return `None`.
    fn commit_oid(&self) -> Option<git2::Oid> {
        match self {
            Self::Commit { oid, .. } => Some(*oid),
            Self::Transition { .. } => None,
        }
    }
}

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
            let rows = graph_rows(model, entries);
            let selectable_commit_oids = history_graph_commit_oids(model);
            let selected_commit_index = if selectable_commit_oids.is_empty() {
                0
            } else {
                std::cmp::min(
                    state.history_graph_scroll_y,
                    selectable_commit_oids.len().saturating_sub(1),
                )
            };
            let selected_commit_oid = selectable_commit_oids.get(selected_commit_index).copied();
            let selected_row_index = selected_commit_oid.and_then(|selected_oid| {
                rows.iter()
                    .position(|row| row.commit_oid().is_some_and(|oid| oid == selected_oid))
            });
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

            let row_lines = if rows.is_empty() {
                vec![Line::from(
                    "(no commits available in bundle scope)".to_string(),
                )]
            } else {
                rows.iter()
                    .enumerate()
                    .map(|(index, row)| {
                        render_graph_row(
                            row,
                            selected_row_index.is_some_and(|selected| selected == index),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            Paragraph::new(Text::from(row_lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("git log --oneline --decorate --graph (bundle scope)"),
                )
                .scroll((
                    u16::try_from(selected_row_index.unwrap_or(0).saturating_sub(4))
                        .unwrap_or(u16::MAX),
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

/// Returns graph-selectable commit OIDs in visual order.
pub(crate) fn history_graph_commit_oids(model: &AuditModel) -> Vec<git2::Oid> {
    let CommitPagesModel::Ok(entries) = &model.commit_pages else {
        return Vec::new();
    };
    let mut commits = HashMap::<git2::Oid, GraphNode>::new();
    for head_entry in entries {
        for commit in &head_entry.commits {
            commits
                .entry(commit.commit_id)
                .or_insert_with(|| GraphNode {
                    oid: commit.commit_id,
                    tree_oid: Some(commit.tree_oid),
                    parent_oids: commit.parent_oids.clone(),
                    subject: commit.subject.clone(),
                });
        }
    }
    ordered_commit_ids(entries, &commits)
}

/// Builds graph-formatted rows across all imported heads.
fn graph_rows(model: &AuditModel, entries: &[HeadAuditEntry]) -> Vec<GraphRow> {
    let mut commits = HashMap::<git2::Oid, GraphNode>::new();
    for head_entry in entries {
        for commit in &head_entry.commits {
            commits
                .entry(commit.commit_id)
                .or_insert_with(|| GraphNode {
                    oid: commit.commit_id,
                    tree_oid: Some(commit.tree_oid),
                    parent_oids: commit.parent_oids.clone(),
                    subject: commit.subject.clone(),
                });
        }
    }
    if commits.is_empty() {
        return Vec::new();
    }

    let mut boundary_parent_oids = HashSet::<git2::Oid>::new();
    for commit in commits.values() {
        for parent_oid in &commit.parent_oids {
            if !commits.contains_key(parent_oid) {
                boundary_parent_oids.insert(*parent_oid);
            }
        }
    }
    let boundary_nodes = load_boundary_nodes(&model.repo_path, &boundary_parent_oids);

    let mut candidate_oids = commits.keys().copied().collect::<HashSet<_>>();
    for boundary_oid in boundary_nodes.keys() {
        candidate_oids.insert(*boundary_oid);
    }
    let decorations = collect_decorations(&model.repo_path, entries, &candidate_oids);

    let order = ordered_commit_ids(entries, &commits);
    let mut columns = Vec::<git2::Oid>::new();
    let mut rendered = HashSet::<git2::Oid>::new();
    let mut rows = Vec::new();

    for oid in order {
        if rendered.contains(&oid) {
            continue;
        }
        let Some(commit) = commits.get(&oid) else {
            continue;
        };
        let visible_parents = commit
            .parent_oids
            .iter()
            .copied()
            .filter(|parent_oid| {
                commits.contains_key(parent_oid) || boundary_nodes.contains_key(parent_oid)
            })
            .collect::<Vec<_>>();
        render_graph_node_row(
            commit,
            &decorations,
            &visible_parents,
            &mut columns,
            &mut rows,
            &mut rendered,
        );
    }

    // Render any remaining non-range boundary commits (for example bundle prerequisites).
    while !columns.is_empty() {
        let next_oid = columns[0];
        if rendered.contains(&next_oid) {
            columns.remove(0);
            continue;
        }
        let Some(boundary) = boundary_nodes.get(&next_oid) else {
            columns.remove(0);
            continue;
        };
        render_graph_node_row(
            boundary,
            &decorations,
            &[],
            &mut columns,
            &mut rows,
            &mut rendered,
        );
    }

    rows
}

/// Produces an approximation of `git log` traversal order (tips-first) for displayed commits.
fn ordered_commit_ids(
    entries: &[HeadAuditEntry],
    commits: &HashMap<git2::Oid, GraphNode>,
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

/// Renders one graph node row and optional transition connector rows.
fn render_graph_node_row(
    node: &GraphNode,
    decorations: &HashMap<git2::Oid, Vec<String>>,
    visible_parents: &[git2::Oid],
    columns: &mut Vec<git2::Oid>,
    rows: &mut Vec<GraphRow>,
    rendered: &mut HashSet<git2::Oid>,
) {
    let current_column = match columns.iter().position(|candidate| *candidate == node.oid) {
        Some(index) => index,
        None => {
            columns.insert(0, node.oid);
            0
        }
    };
    let old_len = columns.len();
    rows.push(GraphRow::Commit {
        columns: graph_prefix_columns(old_len, current_column, '*'),
        oid: node.oid,
        decorations: decorations.get(&node.oid).cloned().unwrap_or_default(),
        tree_oid: node.tree_oid,
        subject: node.subject.clone(),
    });
    rendered.insert(node.oid);

    columns.remove(current_column);
    for (offset, parent_oid) in visible_parents.iter().copied().enumerate() {
        columns.insert(current_column + offset, parent_oid);
    }
    dedupe_columns(columns);
    let new_len = columns.len();

    if visible_parents.len() > 1 {
        rows.push(GraphRow::Transition {
            columns: graph_split_transition_columns(
                old_len,
                current_column,
                visible_parents.len(),
                new_len,
            ),
        });
    } else if old_len > new_len && old_len > 1 {
        rows.push(GraphRow::Transition {
            columns: graph_join_transition_columns(old_len, new_len),
        });
    }
}

/// Removes duplicate commit IDs while preserving first-seen column order.
fn dedupe_columns(columns: &mut Vec<git2::Oid>) {
    let mut seen = HashSet::<git2::Oid>::new();
    columns.retain(|oid| seen.insert(*oid));
}

/// Returns abbreviated fixed-width OID text for compact graph rows.
fn short_oid(oid: git2::Oid) -> String {
    oid.to_string().chars().take(SHORT_OID_LEN).collect()
}

/// Builds a git-graph-like prefix columns vector for one node row.
fn graph_prefix_columns(column_count: usize, node_index: usize, node_char: char) -> Vec<char> {
    let mut chars = vec!['|'; column_count];
    if node_index < chars.len() {
        chars[node_index] = node_char;
    }
    chars
}

/// Builds branch-split connector columns (for merge commits with multiple parents).
fn graph_split_transition_columns(
    old_len: usize,
    node_index: usize,
    parent_count: usize,
    new_len: usize,
) -> Vec<char> {
    let max_len = old_len.max(new_len).max(node_index + parent_count);
    let mut chars = vec!['|'; max_len.max(1)];
    for (index, item) in chars.iter_mut().enumerate() {
        if index > node_index && index < node_index + parent_count {
            *item = '\\';
        }
    }
    chars
}

/// Builds branch-join connector columns when active graph columns collapse.
fn graph_join_transition_columns(old_len: usize, new_len: usize) -> Vec<char> {
    let mut chars = vec!['|'; old_len];
    if new_len < old_len {
        chars[new_len] = '/';
        for item in chars.iter_mut().take(old_len).skip(new_len + 1) {
            *item = ' ';
        }
    }
    chars
}

/// Loads commit/tree/subject details for out-of-range boundary parent commits.
fn load_boundary_nodes(
    repo_path: &Path,
    boundary_oids: &HashSet<git2::Oid>,
) -> HashMap<git2::Oid, GraphNode> {
    let mut nodes = HashMap::new();
    let Ok(repo) = git2::Repository::open(repo_path) else {
        for oid in boundary_oids {
            nodes.insert(
                *oid,
                GraphNode {
                    oid: *oid,
                    tree_oid: None,
                    parent_oids: Vec::new(),
                    subject: "<prerequisite commit unavailable>".to_string(),
                },
            );
        }
        return nodes;
    };

    for oid in boundary_oids {
        let node = match repo.find_commit(*oid) {
            Ok(commit) => GraphNode {
                oid: *oid,
                tree_oid: Some(commit.tree_id()),
                parent_oids: Vec::new(),
                subject: commit
                    .summary()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_else(|| "<no subject>".to_string()),
            },
            Err(_) => GraphNode {
                oid: *oid,
                tree_oid: None,
                parent_oids: Vec::new(),
                subject: "<prerequisite commit unavailable>".to_string(),
            },
        };
        nodes.insert(*oid, node);
    }

    nodes
}

/// Collects `--decorate` labels for shown commits from local refs and advertised bundle heads.
fn collect_decorations(
    repo_path: &Path,
    entries: &[HeadAuditEntry],
    candidate_oids: &HashSet<git2::Oid>,
) -> HashMap<git2::Oid, Vec<String>> {
    let mut decorations = HashMap::<git2::Oid, Vec<(u8, String)>>::new();

    // Always include advertised bundle heads even when local repo refs differ.
    for head_entry in entries {
        if !candidate_oids.contains(&head_entry.head.oid) {
            continue;
        }
        if let Some((rank, label)) = decorate_label_for_ref_name(&head_entry.head.reference) {
            decorations
                .entry(head_entry.head.oid)
                .or_default()
                .push((rank, label));
        }
    }

    let Ok(repo) = git2::Repository::open(repo_path) else {
        return finalize_decorations(decorations);
    };

    if let Ok(refs) = repo.references() {
        for reference in refs.flatten() {
            let Some(name) = reference.name() else {
                continue;
            };
            let Some((rank, label)) = decorate_label_for_ref_name(name) else {
                continue;
            };
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            if !candidate_oids.contains(&commit.id()) {
                continue;
            }
            decorations
                .entry(commit.id())
                .or_default()
                .push((rank, label));
        }
    }

    if let Ok(head_ref) = repo.head()
        && let (Some(head_name), Ok(head_commit)) = (head_ref.name(), head_ref.peel_to_commit())
        && candidate_oids.contains(&head_commit.id())
    {
        let head_label = if let Some(branch) = head_name.strip_prefix("refs/heads/") {
            format!("HEAD -> {branch}")
        } else {
            format!("HEAD -> {head_name}")
        };
        decorations
            .entry(head_commit.id())
            .or_default()
            .push((0, head_label));
    }

    finalize_decorations(decorations)
}

/// Converts a full ref name into a compact decoration label.
fn decorate_label_for_ref_name(ref_name: &str) -> Option<(u8, String)> {
    if let Some(name) = ref_name.strip_prefix("refs/heads/") {
        return Some((1, name.to_string()));
    }
    if let Some(name) = ref_name.strip_prefix("refs/tags/") {
        return Some((2, format!("tag: {name}")));
    }
    None
}

/// Sorts and deduplicates decoration labels.
fn finalize_decorations(
    raw: HashMap<git2::Oid, Vec<(u8, String)>>,
) -> HashMap<git2::Oid, Vec<String>> {
    let mut out = HashMap::<git2::Oid, Vec<String>>::new();
    for (oid, mut labels) in raw {
        labels.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        let mut unique = Vec::<String>::new();
        for (_, label) in labels {
            let normalized = label.trim().to_string();
            if normalized.is_empty() {
                continue;
            }
            if unique.last().is_some_and(|last| last == &normalized) {
                continue;
            }
            if unique.iter().any(|existing| existing == &normalized) {
                continue;
            }
            unique.push(normalized);
        }
        out.insert(oid, unique);
    }
    out
}

/// Renders one graph row with git-like color accents.
fn render_graph_row(row: &GraphRow, is_selected: bool) -> Line<'static> {
    let mut line = match row {
        GraphRow::Transition { columns } => Line::from(render_graph_columns_spans(columns)),
        GraphRow::Commit {
            columns,
            oid,
            decorations,
            tree_oid,
            subject,
        } => {
            let mut spans = render_graph_columns_spans(columns);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                short_oid(*oid),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            if !decorations.is_empty() {
                spans.push(Span::raw(" ("));
                for (index, decoration) in decorations.iter().enumerate() {
                    if index > 0 {
                        spans.push(Span::raw(", "));
                    }
                    spans.push(Span::styled(
                        decoration.clone(),
                        decoration_style(decoration),
                    ));
                }
                spans.push(Span::raw(")"));
            }
            spans.push(Span::raw(" | tree "));
            spans.push(Span::styled(
                tree_oid.map(short_oid).unwrap_or_else(|| "-".to_string()),
                Style::default().fg(Color::Blue),
            ));
            spans.push(Span::raw(" | "));
            spans.push(Span::raw(subject.clone()));
            Line::from(spans)
        }
    };
    if is_selected {
        line = line.patch_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

/// Renders graph columns with one separator space and per-column coloring.
fn render_graph_columns_spans(columns: &[char]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, ch) in columns.iter().copied().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        if ch == ' ' {
            spans.push(Span::raw(" "));
            continue;
        }
        spans.push(Span::styled(ch.to_string(), graph_column_style(index, ch)));
    }
    spans
}

/// Chooses a stable color for each graph column.
fn graph_column_style(column_index: usize, ch: char) -> Style {
    let palette = [
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
    ];
    let mut style = Style::default().fg(palette[column_index % palette.len()]);
    if ch == '*' {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Styles decoration labels with category-aware colors similar to git defaults.
fn decoration_style(decoration: &str) -> Style {
    if decoration.starts_with("HEAD -> ") {
        return Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
    }
    if decoration.starts_with("tag: ") {
        return Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
    }
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}
