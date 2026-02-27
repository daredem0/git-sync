// Focus: rendering behavior for overview and commit pages, including unavailable and out-of-range commit states.

use super::super::render::{render_commit_page, render_overview_page};
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
    let model = super::super::types::AuditModel {
        overview: sample_model(1, 1).overview,
        commit_pages: CommitPagesModel::Failed("metadata load failed".to_string()),
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
