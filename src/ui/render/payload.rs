//! TUI-layer payload-page functionality.

use super::render_footer_text;
use crate::git::{PackEntryBaseRef, PackEntryKind, PayloadObjectKind};
use crate::ui::types::{AppState, AuditModel, PayloadModel};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;

/// Renders payload page tables or selected payload-object detail view.
pub(crate) fn render_payload_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.payload_object_view.is_some() {
        render_payload_object_detail(frame, state);
        return;
    }

    let title_text = payload_title_text(model, state);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title =
        Paragraph::new(title_text).block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(title, chunks[0]);

    match &model.payload {
        PayloadModel::Failed(err) => {
            let body = Paragraph::new(format!(
                "Payload data is unavailable.\n\
                 error: {err}\n\
                 \n\
                 Verify the bundle input and retry."
            ))
            .block(Block::default().borders(Borders::ALL).title("Payload"));
            frame.render_widget(body, chunks[1]);
        }
        PayloadModel::Ok(payload) => {
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
                .split(chunks[1]);
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(9), Constraint::Min(8)])
                .split(body_chunks[0]);
            render_transport_entries_table(frame, payload, left_chunks[0]);
            if state.is_payload_entries_view() {
                render_entries_table(frame, payload, state, left_chunks[1]);
            } else {
                render_objects_table(frame, payload, state, left_chunks[1]);
            }
            render_pack_preview(frame, model, state, body_chunks[1]);
        }
    }

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

/// Builds payload-page top summary including pack-proof invariants.
fn payload_title_text(model: &AuditModel, state: &AppState) -> String {
    match &model.payload {
        PayloadModel::Ok(payload) => {
            let proof = &payload.pack_proof;
            let counts_match = proof.entries_declared == proof.entries_parsed;
            let checksums_match = proof.computed_pack_checksum == proof.trailer_pack_checksum;
            let proof_status = if proof.verification_status.eq_ignore_ascii_case("ok")
                && counts_match
                && checksums_match
            {
                "ok"
            } else {
                "failed"
            };
            let transfer_line = if proof.transfer_allowed {
                "transfer: allowed".to_string()
            } else {
                format!(
                    "transfer: blocked ({})",
                    proof
                        .blocked_reason
                        .as_deref()
                        .unwrap_or("entries not fully materialized")
                )
            };
            format!(
                "Payload View\n\
                 status: {proof_status} | pack version: {}\n\
                 entries: {}/{} | materialized: {}/{}\n\
                 unique objects: {} | duplicates: {}\n\
                 {transfer_line} | hash: {} | checksum: {}\n\
                 thin pack: {} | baseline resolutions: {}\n\
                 computed checksum: {}\n\
                 trailer checksum: {}\n\
                 subview: {} (toggle: e)",
                proof.pack_version,
                proof.entries_parsed,
                proof.entries_declared,
                proof.entries_materialized,
                proof.entries_declared,
                proof.unique_objects_materialized,
                proof.duplicate_entry_count_materialized,
                proof.hash_algorithm,
                if proof.checksum_verified {
                    "ok"
                } else {
                    "failed"
                },
                if proof.thin_pack_detected {
                    "yes"
                } else {
                    "no"
                },
                proof.baseline_resolutions_count,
                proof.computed_pack_checksum,
                proof.trailer_pack_checksum,
                state.payload_sub_view_label()
            )
        }
        PayloadModel::Failed(_) => "Payload View\n\
            Transport package entries, selected-object preview, and full pack object listing\n\
            Use j/k to select object rows and Enter to open object detail"
            .to_string(),
    }
}

/// Renders payload transport entry table.
fn render_transport_entries_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    area: Rect,
) {
    let rows: Vec<Row<'_>> = if payload.transport_entries.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no transport entries)"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        payload
            .transport_entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from(entry.name.clone()),
                    Cell::from(entry.size_bytes.to_string()),
                    Cell::from(short_sha256(&entry.sha256)),
                ])
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Min(22),
            Constraint::Length(10),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["ENTRY", "SIZE", "SHA256"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Transport Entries"),
    )
    .column_spacing(1);
    frame.render_widget(table, area);
}

/// Renders selected pack object preview (commit/tree/blob/tag) on payload page.
fn render_pack_preview(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState, area: Rect) {
    let lines: Vec<Line<'_>> = match &model.payload {
        PayloadModel::Failed(_) => vec![
            Line::from("Payload data is unavailable."),
            Line::from("Preview cannot be rendered."),
        ],
        PayloadModel::Ok(payload) if state.is_payload_entries_view() => {
            if let Some(entry) = state.payload_selected_entry(payload) {
                let raw_lines = vec![
                    format!("entry #{}", entry.idx + 1),
                    format!("offset: {}", entry.offset),
                    format!("kind: {}", payload_entry_kind_label(entry.kind)),
                    format!("header size: {}", entry.out_size),
                    format!(
                        "reconstructed size: {}",
                        entry
                            .reconstructed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ),
                    format!(
                        "base: {}",
                        payload_entry_base_ref_label(entry.base_ref.as_ref())
                    ),
                    format!("resolved: {}", if entry.resolved { "yes" } else { "no" }),
                    format!(
                        "resolved via: {}",
                        match entry.resolved_via {
                            Some(crate::git::ResolutionSource::InPack) => "in-pack",
                            Some(crate::git::ResolutionSource::Baseline) => "baseline",
                            None => "-",
                        }
                    ),
                    format!(
                        "oid: {}",
                        entry
                            .result_oid
                            .map(|oid| oid.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ),
                    format!("note: {}", entry.note.as_deref().unwrap_or("-")),
                ];
                render_preview_lines_to_area(raw_lines, area, None, None, &model.syntax_highlighter)
            } else {
                vec![
                    Line::from("No entry selected."),
                    Line::from("Use j/k to select a ledger entry row."),
                ]
            }
        }
        PayloadModel::Ok(payload) => {
            let selected_object = {
                let sorted = state.payload_sorted_objects(payload);
                sorted
                    .get(std::cmp::min(
                        state.payload_selected_index,
                        sorted.len().saturating_sub(1),
                    ))
                    .copied()
            };
            match &state.payload_preview {
                Some(preview) => {
                    let mut raw_lines = vec![format!(
                        "selected: {} ({})",
                        preview.oid,
                        payload_kind_label(preview.kind)
                    )];
                    if let Some(entry) = selected_object {
                        raw_lines.push(format!(
                            "reachable from heads: {}",
                            if entry.reachable_from_heads {
                                "yes"
                            } else {
                                "no"
                            }
                        ));
                        raw_lines.push(format!(
                            "context head: {}",
                            entry
                                .context_head_index
                                .map(|index| format!("#{}", index + 1))
                                .unwrap_or_else(|| "-".to_string())
                        ));
                        raw_lines.push(format!(
                            "context commit order: {}",
                            entry
                                .context_commit_order
                                .map(|order| order.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        ));
                        raw_lines.push(format!(
                            "context path: {}",
                            entry.context_path.as_deref().unwrap_or("-")
                        ));
                    }
                    raw_lines.push(String::new());
                    let prefix_len = raw_lines.len();
                    raw_lines.extend(preview.lines.iter().cloned());
                    let syntax_start = preview.syntax_start_index.map(|index| index + prefix_len);
                    render_preview_lines_to_area(
                        raw_lines,
                        area,
                        preview.syntax_path_hint.as_deref(),
                        syntax_start,
                        &model.syntax_highlighter,
                    )
                }
                None => vec![
                    Line::from("No preview loaded."),
                    Line::from("Use j/k to select a pack object row."),
                    Line::from("Enter opens full object detail."),
                ],
            }
        }
    };

    let preview = Paragraph::new(ratatui::text::Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title("Pack Preview"));
    frame.render_widget(preview, area);
}

/// Renders payload entry-ledger table with selected-row highlight.
fn render_entries_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    state: &AppState,
    area: Rect,
) {
    let entries = &payload.entry_ledger.entries;
    let rows: Vec<Row<'_>> = if entries.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no entries)"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from((entry.idx + 1).to_string()),
                    Cell::from(entry.offset.to_string()),
                    Cell::from(payload_entry_kind_label(entry.kind)),
                    Cell::from(entry.out_size.to_string()),
                    Cell::from(
                        entry
                            .reconstructed_size
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    Cell::from(payload_entry_base_ref_label(entry.base_ref.as_ref())),
                    Cell::from(
                        entry
                            .result_oid
                            .map(short_oid)
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    Cell::from(if entry.resolved {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }),
                ])
            })
            .collect()
    };
    let title = format!(
        "Pack Entries ({} parsed / {} declared)",
        entries.len(),
        payload.entry_ledger.declared_entry_count
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec![
            "#",
            "OFFSET",
            "KIND",
            "HDR_SIZE",
            "RECON_SIZE",
            "BASE",
            "OID",
            "RESOLVED",
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(Block::default().borders(Borders::ALL).title(title))
    .column_spacing(1);

    let mut table_state = TableState::default();
    if !entries.is_empty() {
        table_state.select(Some(std::cmp::min(
            state.payload_selected_index,
            entries.len() - 1,
        )));
    }
    frame.render_stateful_widget(table, area, &mut table_state);
}

/// Clips plain preview lines to panel height and highlights only visible lines.
fn render_preview_lines_to_area(
    lines: Vec<String>,
    area: Rect,
    syntax_path_hint: Option<&str>,
    syntax_start_index: Option<usize>,
    highlighter: &crate::ui::types::SyntaxHighlighter,
) -> Vec<Line<'static>> {
    let max_rows = usize::from(area.height.saturating_sub(2));
    if max_rows == 0 {
        return Vec::new();
    }

    if lines.len() <= max_rows {
        return render_visible_plain_lines(
            &lines,
            syntax_path_hint,
            syntax_start_index,
            highlighter,
        );
    }
    if max_rows == 1 {
        return vec![Line::from(format!("... ({} more lines)", lines.len()))];
    }

    let shown = max_rows - 1;
    let hidden = lines.len().saturating_sub(shown);
    let mut clipped = render_visible_plain_lines(
        &lines[..shown],
        syntax_path_hint,
        syntax_start_index,
        highlighter,
    );
    clipped.push(Line::from(format!("... ({} more lines)", hidden)));
    clipped
}

/// Renders visible plain preview lines with optional syntax highlighting.
fn render_visible_plain_lines(
    lines: &[String],
    syntax_path_hint: Option<&str>,
    syntax_start_index: Option<usize>,
    highlighter: &crate::ui::types::SyntaxHighlighter,
) -> Vec<Line<'static>> {
    let (Some(path_hint), Some(start_index)) = (syntax_path_hint, syntax_start_index) else {
        return lines
            .iter()
            .map(|line| Line::from(line.to_string()))
            .collect::<Vec<_>>();
    };
    let line_no_width = line_number_width(lines.len().saturating_sub(start_index));

    let (syntax, _syntax_name) = highlighter.resolve_syntax_for_path(path_hint);
    let mut syntax_state = HighlightLines::new(syntax, &highlighter.theme);
    let mut rendered = Vec::with_capacity(lines.len());

    for (index, raw_line) in lines.iter().enumerate() {
        if index < start_index {
            rendered.push(Line::from(raw_line.to_string()));
            continue;
        }
        let line_no = index - start_index + 1;

        let mut highlight_input = String::with_capacity(raw_line.len() + 1);
        highlight_input.push_str(raw_line);
        highlight_input.push('\n');
        let spans = match syntax_state.highlight_line(&highlight_input, &highlighter.syntax_set) {
            Ok(regions) if !regions.is_empty() => {
                let last = regions.len() - 1;
                let mut spans = Vec::new();
                for (region_index, (style, segment)) in regions.into_iter().enumerate() {
                    let text = if region_index == last {
                        segment.strip_suffix('\n').unwrap_or(segment)
                    } else {
                        segment
                    };
                    if text.is_empty() {
                        continue;
                    }
                    spans.push(ratatui::text::Span::styled(
                        text.to_string(),
                        syntect_style_to_ratatui(style),
                    ));
                }
                if spans.is_empty() {
                    vec![ratatui::text::Span::raw(String::new())]
                } else {
                    spans
                }
            }
            _ => vec![ratatui::text::Span::raw(raw_line.to_string())],
        };
        rendered.push(numbered_styled_line(line_no, line_no_width, spans));
    }

    rendered
}

/// Converts a syntect style span into an equivalent ratatui style.
fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut result = Style::default().fg(ratatui::style::Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        result = result.add_modifier(ratatui::style::Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        result = result.add_modifier(ratatui::style::Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        result = result.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }

    result
}

/// Renders payload object table with selected-row highlight.
fn render_objects_table(
    frame: &mut Frame<'_>,
    payload: &crate::git::PayloadAudit,
    state: &AppState,
    area: Rect,
) {
    let sorted = state.payload_sorted_objects(payload);
    let rows: Vec<Row<'_>> = if sorted.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no objects)"),
            Cell::from("-"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        sorted
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Cell::from(short_oid(entry.oid)),
                    Cell::from(payload_kind_label(entry.kind)),
                    Cell::from(entry.size_bytes.to_string()),
                    Cell::from(if entry.reachable_from_heads {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }),
                ])
            })
            .collect()
    };

    let title = format!(
        "Pack Objects ({} total, {} heads, sort: {})",
        payload.objects.len(),
        payload.heads.len(),
        state.payload_sort_mode_label()
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(11),
        ],
    )
    .header(
        Row::new(vec!["OID", "TYPE", "SIZE", "REACHABLE"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(Block::default().borders(Borders::ALL).title(title))
    .column_spacing(1);

    let mut table_state = TableState::default();
    if !sorted.is_empty() {
        table_state.select(Some(std::cmp::min(
            state.payload_selected_index,
            sorted.len() - 1,
        )));
    }
    frame.render_stateful_widget(table, area, &mut table_state);
}

/// Renders selected payload object detail with scroll offsets.
fn render_payload_object_detail(frame: &mut Frame<'_>, state: &AppState) {
    let Some(view) = &state.payload_object_view else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let header = Paragraph::new(format!(
        "Payload Object Detail\n\
         oid: {}\n\
         type: {}\n\
         syntax: {}",
        view.oid,
        payload_kind_label(view.kind),
        view.syntax_name
    ))
    .block(Block::default().borders(Borders::ALL).title("git-sync"));
    frame.render_widget(header, chunks[0]);

    let detail_text = ratatui::text::Text::from(numbered_lines(&view.lines));
    let detail = Paragraph::new(detail_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Object Content"),
        )
        .scroll((
            u16::try_from(view.scroll_y).unwrap_or(u16::MAX),
            u16::try_from(view.scroll_x).unwrap_or(u16::MAX),
        ));
    frame.render_widget(detail, chunks[1]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

/// Prefixes a styled line with a line-number gutter.
fn numbered_styled_line(
    line_no: usize,
    width: usize,
    content: Vec<Span<'static>>,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(content.len() + 1);
    spans.push(Span::styled(
        format!("{line_no:>width$} │ "),
        Style::default().fg(Color::DarkGray),
    ));
    spans.extend(content);
    Line::from(spans)
}

/// Adds line-number gutters to rendered payload object detail lines.
fn numbered_lines(lines: &[Line<'static>]) -> Vec<Line<'static>> {
    let width = line_number_width(lines.len());
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::styled(
                format!("{:>width$} │ ", index + 1),
                Style::default().fg(Color::DarkGray),
            ));
            spans.extend(line.spans.clone());
            Line::from(spans)
        })
        .collect()
}

/// Computes the number of digits needed for line-number gutters.
fn line_number_width(total_lines: usize) -> usize {
    let mut n = total_lines.max(1);
    let mut digits = 1usize;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

/// Returns compact display label for payload object kind.
fn payload_kind_label(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

/// Returns compact display label for pack entry kind.
fn payload_entry_kind_label(kind: PackEntryKind) -> &'static str {
    match kind {
        PackEntryKind::Commit => "commit",
        PackEntryKind::Tree => "tree",
        PackEntryKind::Blob => "blob",
        PackEntryKind::Tag => "tag",
        PackEntryKind::OfsDelta => "ofs-delta",
        PackEntryKind::RefDelta => "ref-delta",
    }
}

/// Returns compact display label for pack entry base references.
fn payload_entry_base_ref_label(base_ref: Option<&PackEntryBaseRef>) -> String {
    match base_ref {
        Some(PackEntryBaseRef::BaseOffset { distance, .. }) => format!("ofs:{distance}"),
        Some(PackEntryBaseRef::BaseOid(oid)) => format!("oid:{}", short_oid(*oid)),
        None => "-".to_string(),
    }
}

/// Returns shortened digest prefix for compact table output.
fn short_sha256(digest: &str) -> String {
    if digest.len() <= 12 {
        digest.to_string()
    } else {
        digest[..12].to_string()
    }
}

/// Returns shortened object id prefix for compact table output.
fn short_oid(oid: git2::Oid) -> String {
    let full = oid.to_string();
    full[..12].to_string()
}
