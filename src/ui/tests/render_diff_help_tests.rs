// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for render diff help behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: rendering behavior for diff page, footer/help text, and help overlay modes.

use super::super::render::{
    help_text_for_mode, render_diff_view, render_footer_text, render_help_overlay, render_page,
};
use super::super::types::{DiffViewState, ExportNotice, MainView, PayloadSubView};
use super::support::*;
use ratatui::text::Line;
use std::path::PathBuf;

// Verifies that footer text switches to diff controls only when a diff view is active.
#[test]
fn render_footer_text_switches_between_page_and_diff_modes() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);

    let overview_footer = render_footer_text(&state);
    assert!(
        !overview_footer.contains("toggle history/payload"),
        "overview footer should not include top-level view-switch hint anymore"
    );
    assert!(
        !overview_footer.contains("1 main | 2 payload | 3 commit"),
        "overview footer should not include direct page shortcut legend"
    );
    assert!(
        overview_footer.contains("Enter open selected head"),
        "overview footer should include commit-page action hints"
    );
    assert!(
        overview_footer.contains("p/P export paudit light/full"),
        "overview footer should include payload-audit export hint"
    );

    state.page_index = 1;
    let commit_footer = render_footer_text(&state);
    assert!(
        !commit_footer.contains("toggle history/payload"),
        "commit footer should not include top-level view switch hint"
    );

    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 10,
        scroll_y: 0,
        scroll_x: 0,
    });
    let diff_footer = render_footer_text(&state);
    assert!(
        diff_footer.contains("PgUp/PgDn"),
        "diff mode footer should include scrolling key hints"
    );

    state.diff_view = None;
    state.page_index = 0;
    state.main_view = super::super::types::MainView::Payload;
    let payload_footer = render_footer_text(&state);
    assert!(
        payload_footer.contains("PgUp/PgDn jump 10"),
        "payload mode footer should include page jump key hints"
    );
    assert!(
        payload_footer.contains("s cycle sort"),
        "payload mode footer should include sort-cycle key hint"
    );
    for line in payload_footer.lines() {
        assert!(
            line.chars().count() <= 110,
            "payload footer line exceeds 110 columns: {line}"
        );
    }

    state.payload_sub_view = PayloadSubView::Entries;
    let entries_footer = render_footer_text(&state);
    assert!(
        entries_footer.contains("e toggle objects/entries"),
        "payload entries footer should include subview toggle hint"
    );
    assert!(
        !entries_footer.contains("s cycle sort"),
        "payload entries footer should not include object-sort hint"
    );
    for line in entries_footer.lines() {
        assert!(
            line.chars().count() <= 110,
            "payload entries footer line exceeds 110 columns: {line}"
        );
    }
}

// Verifies that help text content changes between page mode and diff mode.
#[test]
fn help_text_for_mode_switches_content_by_view() {
    let page_help = help_text_for_mode(false);
    assert!(
        page_help.contains("Hotkeys (Overview)"),
        "page helper should return overview hotkey help when diff mode is inactive"
    );

    let diff_help = help_text_for_mode(true);
    assert!(
        diff_help.contains("Hotkeys (Diff View)"),
        "page helper should return diff hotkey help when diff mode is active"
    );
    assert!(
        diff_help.contains("p: export light payload-audit JSON"),
        "diff-mode help text should include payload-audit export shortcut"
    );
}

// Verifies that rendering diff view prints header metadata and patch container labels.
#[test]
fn render_diff_view_shows_header_and_patch_section() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "main.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1"), Line::from("line 2")],
        max_line_width: 20,
        scroll_y: 0,
        scroll_x: 0,
    });

    let output = render_and_capture_text(140, 30, |frame| {
        render_diff_view(frame, &state);
    });

    assert!(
        output.contains("Diff View"),
        "diff render should include the diff page title"
    );
    assert!(
        output.contains("Patch"),
        "diff render should include patch section title"
    );
    assert!(
        output.contains("main.rs"),
        "diff render should include selected file path in header"
    );
}

// Verifies that rendering help overlay in page mode prints page-navigation hints.
#[test]
fn render_help_overlay_page_mode_renders_page_navigation_hints() {
    let model = sample_model(1, 1);
    let state = super::super::types::AppState::new(&model);
    let output = render_and_capture_text(120, 30, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("Help 1/3 - Hotkeys"),
        "page help overlay should include page indicator and title"
    );
    assert!(
        output.contains("Hotkeys (Overview)"),
        "page help overlay should include overview hotkey hints"
    );
    assert!(
        output.contains("1 Hotkeys | 2 Glossary | 3 Audit Guide"),
        "help overlay should list all available help pages in a persistent header"
    );
}

// Verifies that rendering help overlay in diff mode prints diff-navigation hints.
#[test]
fn render_help_overlay_diff_mode_renders_diff_navigation_hints() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "main.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![Line::from("line 1")],
        max_line_width: 10,
        scroll_y: 0,
        scroll_x: 0,
    });
    let output = render_and_capture_text(120, 30, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("Hotkeys (Diff View)"),
        "diff help overlay should label diff-view hotkey mode"
    );
    assert!(
        output.contains("Esc: close diff"),
        "diff help overlay should include close-diff hint"
    );
}

// Verifies that help page 2 in overview mode explains integrity-summary fields.
#[test]
fn render_help_overlay_overview_context_page_explains_pack_proof_terms() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.help_page_index = 1;

    let output = render_and_capture_text(140, 35, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("Overview Glossary"),
        "overview context page should render overview glossary heading"
    );
    assert!(
        output.contains("pack proof"),
        "overview context page should explain pack proof"
    );
    assert!(
        output.contains("pack entries parsed"),
        "overview context page should explain parsed-entry ratio"
    );
}

// Verifies that help page 2 in payload entries mode explains PACK-entry terms and delta kinds.
#[test]
fn render_help_overlay_payload_entries_context_page_explains_entry_terms() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.payload_sub_view = PayloadSubView::Entries;
    state.help_page_index = 1;

    let output = render_and_capture_text(140, 35, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("Payload Entries Glossary"),
        "payload entries context page should render entries glossary heading"
    );
    assert!(
        output.contains("ref-delta"),
        "payload entries context page should explain ref-delta entries"
    );
    assert!(
        output.contains("OID"),
        "payload entries context page should explain OID column semantics"
    );
}

// Verifies that help page 3 in overview mode teaches audit checks for non-expert reviewers.
#[test]
fn render_help_overlay_overview_audit_page_teaches_review_focus() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.help_page_index = 2;

    let output = render_and_capture_text(140, 35, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("How to Audit (Overview)"),
        "overview audit page should render audit guidance heading"
    );
    assert!(
        output.contains("pack proof"),
        "overview audit page should emphasize integrity-first checks"
    );
    assert!(
        output.contains("Would Change"),
        "overview audit page should explain what to inspect in would-change rows"
    );
}

// Verifies that help page 3 in payload entries mode explains audit focus for entry-level transport evidence.
#[test]
fn render_help_overlay_payload_entries_audit_page_explains_transport_checks() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = MainView::Payload;
    state.payload_sub_view = PayloadSubView::Entries;
    state.help_page_index = 2;

    let output = render_and_capture_text(140, 35, |frame| {
        render_help_overlay(frame, &state);
    });
    assert!(
        output.contains("How to Audit (Payload Entries)"),
        "payload entries audit page should render audit guidance heading"
    );
    assert!(
        output.contains("RESOLVED"),
        "payload entries audit page should explain resolved-state checks"
    );
    assert!(
        output.contains("HDR_SIZE"),
        "payload entries audit page should explain size sanity checks"
    );
}

// Verifies that export notice overlay shows success text, path, timestamp, and Esc hint.
#[test]
fn render_page_export_notice_overlay_includes_success_details() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.export_notice = Some(ExportNotice {
        path: PathBuf::from("/tmp/20260303T123456Z_repo_sync.bundle.paudit.json"),
        exported_at_human_utc: "2026-03-03 12:34:56 UTC".to_string(),
    });

    let output = render_and_capture_text(140, 35, |frame| {
        render_page(frame, &model, &state);
    });
    assert!(
        output.contains("Payload audit log was successfully exported"),
        "export notice should include success headline"
    );
    assert!(
        output.contains("Path: /tmp/20260303T123456Z_repo_sync.bundle.paudit.json"),
        "export notice should include exported file path"
    );
    assert!(
        output.contains("Date/time: 2026-03-03 12:34:56 UTC"),
        "export notice should include human-readable UTC time"
    );
    assert!(
        output.contains("Press Esc to close this message"),
        "export notice should include Esc close hint"
    );
}
