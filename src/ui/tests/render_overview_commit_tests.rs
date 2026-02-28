//! Unit tests for render overview commit tests.

// Focus: rendering behavior for overview and commit pages, including unavailable and out-of-range commit states.

use super::super::render::{render_commit_page, render_overview_page, render_page};
use super::super::types::CommitPagesModel;
use super::support::*;
use crate::git::{self, BundleVersion};
use std::path::PathBuf;

// Verifies that rendering overview page with successful dry-run shows import and change sections.
#[test]
fn render_overview_page_with_dry_run_ok_shows_summary_sections() {
    let model = sample_overview_model(super::super::types::DryRunLine::Ok(
        git::ReceiveBundleResult {
            bundle_version: BundleVersion::V2,
            imported_heads: vec![git::BundleHead {
                oid: git2::Oid::from_str("2222222222222222222222222222222222222222")
                    .expect("valid oid"),
                reference: "refs/heads/main".to_string(),
            }],
            can_apply_without_conflicts: true,
            line_stats: vec![git::FileLineStat {
                path: "file.txt".to_string(),
                additions: 2,
                deletions: 1,
            }],
        },
    ));
    let state = super::super::types::AppState::new(&model);

    let output = render_and_capture_text(140, 40, |frame| {
        render_overview_page(frame, &model, &state);
    });

    assert!(
        output.contains("Heads To Import"),
        "overview render should include heads section in dry-run success path"
    );
    assert!(
        output.contains("Would Change"),
        "overview render should include would-change section in dry-run success path"
    );
    assert!(
        output.contains("tool version: test-version"),
        "overview render should include embedded tool version in general section"
    );
    assert!(
        output.contains("metadata verification: OK"),
        "overview render should include metadata verification status in general section"
    );
    assert!(
        output.contains("dry-run applicability: bundle can be applied without conflicts"),
        "overview render should include dry-run applicability status in general section"
    );
    assert!(
        output.contains("file.txt"),
        "overview render should include rendered file stats rows"
    );
}

// Verifies that rendering overview page with dry-run failure shows a user-facing failure explanation.
#[test]
fn render_overview_page_with_dry_run_failed_shows_error_text() {
    let model = sample_overview_model(super::super::types::DryRunLine::Failed(
        "dry-run failed".to_string(),
    ));
    let state = super::super::types::AppState::new(&model);

    let output = render_and_capture_text(140, 40, |frame| {
        render_overview_page(frame, &model, &state);
    });

    assert!(
        output.contains("Dry-run failed"),
        "overview render should explain when dry-run data is unavailable"
    );
}

// Verifies that overview would-change table follows the currently selected head on the heads table.
#[test]
fn render_overview_page_renders_selected_head_would_change_rows() {
    let model = sample_multi_head_model(&[1, 1]);
    let mut state = super::super::types::AppState::new(&model);
    state.selected_head_index = 1;

    let output = render_and_capture_text(140, 40, |frame| {
        render_overview_page(frame, &model, &state);
    });

    assert!(
        output.contains("head-2-file-1.txt"),
        "selected head file rows should be shown in would-change table"
    );
    assert!(
        !output.contains("head-1-file-1.txt"),
        "unselected head file rows should not be shown in would-change table"
    );
}

// Verifies that selecting payload view renders payload page instead of history overview/commit pages.
#[test]
fn render_page_in_payload_view_shows_payload_screen() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;

    let output = render_and_capture_text(140, 40, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("Payload View"),
        "payload mode should render a dedicated payload page title"
    );
    assert!(
        output.contains("Transport Entries"),
        "payload page should render transport entry table section"
    );
    assert!(
        output.contains("Pack Objects"),
        "payload page should render object listing section"
    );
}

// Verifies that payload object drill-down renders object detail content after opening a selected payload row.
#[test]
fn render_page_in_payload_object_detail_mode_shows_object_content() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;
    state.open_selected_payload_object(&model);
    assert!(
        state.payload_object_view.is_some(),
        "precondition: payload object detail should be open"
    );

    let output = render_and_capture_text(140, 40, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("Payload Object Detail"),
        "payload object detail render should include dedicated title"
    );
    assert!(
        output.contains("Object Content"),
        "payload object detail render should include object content section"
    );
}

// Verifies that rendering commit page in normal mode shows commit metadata and changed-file table.
#[test]
fn render_commit_page_shows_commit_detail_and_changed_files() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;

    let output = render_and_capture_text(140, 40, |frame| {
        render_commit_page(frame, &model, &state);
    });

    assert!(
        output.contains("Commit Detail"),
        "commit page render should include the detail block title"
    );
    assert!(
        output.contains("Changed Files"),
        "commit page render should include changed-file table"
    );
    assert!(
        output.contains("file-0.txt"),
        "commit page render should include the selected commit file list"
    );
}

// Verifies that rendering commit page handles commit-page-load failures without panicking.
#[test]
fn render_commit_page_failed_mode_shows_unavailable_message() {
    let sample = sample_model(1, 1);
    let model = super::super::types::AuditModel {
        overview: sample.overview,
        commit_pages: CommitPagesModel::Failed("metadata load failed".to_string()),
        payload: sample.payload,
        repo_path: PathBuf::from("."),
        bundle_path: PathBuf::from("sync.bundle.zip"),
        syntax_highlighter: super::super::types::SyntaxHighlighter::load(),
    };
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 1;

    let output = render_and_capture_text(140, 30, |frame| {
        render_commit_page(frame, &model, &state);
    });

    assert!(
        output.contains("Commit Pages Unavailable"),
        "commit page render should show unavailable state title"
    );
}

// Verifies that out-of-bounds commit page indices render fallback text instead of panicking.
#[test]
fn render_commit_page_out_of_bounds_shows_fallback_message() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.page_index = 5;

    let output = render_and_capture_text(120, 30, |frame| {
        render_commit_page(frame, &model, &state);
    });

    assert!(
        output.contains("out of bounds"),
        "commit page render should gracefully handle out-of-range page indices"
    );
}
