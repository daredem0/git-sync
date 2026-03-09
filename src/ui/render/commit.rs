// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI rendering module for commit views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::commit_table::render_commit_files_table;
use super::render_footer_text;
use crate::ui::format::{format_git_timestamp, format_identity};
use crate::ui::types::{AppState, AuditModel, CommitPagesModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Renders a commit detail page with identity metadata and changed files table.
pub(crate) fn render_commit_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    match &model.commit_pages {
        CommitPagesModel::Failed(err) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(12),
                    Constraint::Min(10),
                    Constraint::Length(2),
                ])
                .split(frame.area());
            let page_label = format!("page {}/{}", state.page_index + 1, state.total_pages(model));
            let message = Paragraph::new(format!(
                "Commit page data is unavailable ({})\nerror: {}\n\
                 The overview page is still usable for package-level auditing.",
                page_label, err
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Commit Pages Unavailable"),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(message, chunks[0]);
            frame.render_widget(
                Paragraph::new("No commit list can be rendered for this package.").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Changed Files"),
                ),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(render_footer_text(state))
                    .style(Style::default().add_modifier(Modifier::ITALIC)),
                chunks[2],
            );
        }
        CommitPagesModel::Ok(entries) => {
            let fallback_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(12),
                    Constraint::Min(10),
                    Constraint::Length(2),
                ])
                .split(frame.area());
            if entries.is_empty() {
                frame.render_widget(
                    Paragraph::new("No heads are available for commit-page rendering.")
                        .block(Block::default().borders(Borders::ALL).title("Commit")),
                    fallback_chunks[0],
                );
                frame.render_widget(
                    Paragraph::new(render_footer_text(state))
                        .style(Style::default().add_modifier(Modifier::ITALIC)),
                    fallback_chunks[2],
                );
                return;
            }

            let selected_head_index = std::cmp::min(state.selected_head_index, entries.len() - 1);
            let head_entry = &entries[selected_head_index];
            let commit_index = state.page_index.saturating_sub(1);
            let Some(entry) = head_entry.commits.get(commit_index) else {
                frame.render_widget(
                    Paragraph::new("Page index is out of bounds for commit entries.")
                        .block(Block::default().borders(Borders::ALL).title("Commit")),
                    fallback_chunks[0],
                );
                frame.render_widget(
                    Paragraph::new(render_footer_text(state))
                        .style(Style::default().add_modifier(Modifier::ITALIC)),
                    fallback_chunks[2],
                );
                return;
            };

            let header_text = format!(
                "Press 1 main | 2 payload | 3 commit | 4 graph\nHEAD {}/{}: {}\nCommit {}/{}: {}\nTree: {}\n{}\nCommitter: {} | {}\nAuthor: {} | {}\nChanged files: {}\n\n{}",
                selected_head_index + 1,
                entries.len(),
                head_entry.head.reference,
                commit_index + 1,
                head_entry.commits.len(),
                entry.commit_id,
                entry.tree_oid,
                format_parent_line(entry),
                format_identity(&entry.committer),
                format_git_timestamp(entry.committer.time_seconds, entry.committer.offset_minutes,),
                format_identity(&entry.author),
                format_git_timestamp(entry.author.time_seconds, entry.author.offset_minutes),
                entry.files.len(),
                entry.message
            );
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(commit_detail_header_height(&header_text)),
                    Constraint::Min(10),
                    Constraint::Length(2),
                ])
                .split(frame.area());

            let header = Paragraph::new(header_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Commit Detail"),
                )
                .wrap(Wrap { trim: false });
            frame.render_widget(header, chunks[0]);

            render_commit_files_table(frame, entry, commit_index, state, chunks[1]);
            frame.render_widget(
                Paragraph::new(render_footer_text(state))
                    .style(Style::default().add_modifier(Modifier::ITALIC)),
                chunks[2],
            );
        }
    }
}

fn format_parent_line(entry: &crate::git::CommitAuditEntry) -> String {
    match entry.parent_oids.as_slice() {
        [] => "Parent: -".to_string(),
        [parent] => format!("Parent: {parent}"),
        parents => format!(
            "Parents: {}",
            parents
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn commit_detail_header_height(header_text: &str) -> u16 {
    let text_lines = header_text.lines().count().max(10);
    let boxed_lines = text_lines.saturating_add(2);
    u16::try_from(boxed_lines).unwrap_or(u16::MAX)
}
