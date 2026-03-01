// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for render overview commit behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: rendering behavior for overview and commit pages, including unavailable and out-of-range commit states.

use super::super::render::{render_commit_page, render_overview_page, render_page};
use super::super::types::{CommitPagesModel, PayloadModel, PayloadSubView};
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
            preflight_plan: Vec::new(),
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
        output.contains("Press 1 main | 2 payload | 3 commit"),
        "overview render title should advertise direct page shortcuts"
    );
    assert!(
        !output.contains("Use h/l or left/right"),
        "overview render title should no longer advertise page movement shortcuts"
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
        output.contains("pack proof: OK"),
        "overview render should include pack-proof status in general section"
    );
    assert!(
        output.contains("pack entries parsed: 2/2"),
        "overview render should include parsed/declared entry counts in general section"
    );
    assert!(
        output.contains("pack entries materialized: 2/2"),
        "overview render should include materialized/declared entry counts in general section"
    );
    assert!(
        output.contains("transfer gate: allowed"),
        "overview render should include transfer-gate status in general section"
    );
    assert!(
        output.contains("pack checksum: match"),
        "overview render should include pack checksum match status in general section"
    );
    assert!(
        output.contains("bundle version: v2"),
        "overview render should include payload bundle version in general section"
    );
    assert!(
        output.contains("advertised heads: 1"),
        "overview render should include advertised head count in general section"
    );
    assert!(
        output.contains("transport entries: 2"),
        "overview render should include transport-entry count in general section"
    );
    assert!(
        output.contains("payload objects: 2"),
        "overview render should include payload-object count in general section"
    );
    assert!(
        output.contains("bundle fully reachable from heads: no"),
        "overview render should include bundle-to-history reachability status"
    );
    assert!(
        output.contains("file.txt"),
        "overview render should include rendered file stats rows"
    );
}

// Verifies that overview page surfaces pack-proof failures when processed counts or checksums mismatch.
#[test]
fn render_overview_page_shows_pack_proof_failure_summary() {
    let mut model = sample_overview_model(super::super::types::DryRunLine::Failed(
        "dry-run failed".to_string(),
    ));
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload audit data");
    };
    payload.pack_proof.entries_parsed = 1;
    payload.pack_proof.entries_materialized = 1;
    payload.pack_proof.transfer_allowed = false;
    payload.pack_proof.blocked_reason =
        Some("materialized entries below declared count".to_string());
    payload.pack_proof.trailer_pack_checksum =
        "dddddddddddddddddddddddddddddddddddddddd".to_string();
    let state = super::super::types::AppState::new(&model);

    let output = render_and_capture_text(140, 44, |frame| {
        render_overview_page(frame, &model, &state);
    });

    assert!(
        output.contains("pack proof: FAILED"),
        "overview render should mark pack proof as failed when object counts/checksums mismatch"
    );
    assert!(
        output.contains("pack entries parsed: 1/2"),
        "overview render should show mismatched parsed/declared entry counts"
    );
    assert!(
        output.contains("pack entries materialized: 1/2"),
        "overview render should show mismatched materialized/declared entry counts"
    );
    assert!(
        output.contains("transfer gate: blocked"),
        "overview render should show blocked transfer-gate status when entries are incomplete"
    );
    assert!(
        output.contains("pack checksum: mismatch"),
        "overview render should flag checksum mismatch in general section"
    );
}

// Verifies that overview bundle-integrity section marks full history coverage when all payload objects are reachable from heads.
#[test]
fn render_overview_page_shows_full_bundle_reachability_when_all_objects_are_reachable() {
    let mut model = sample_overview_model(super::super::types::DryRunLine::Failed(
        "dry-run failed".to_string(),
    ));
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload audit data");
    };
    for object in &mut payload.objects {
        object.reachable_from_heads = true;
    }
    let state = super::super::types::AppState::new(&model);

    let output = render_and_capture_text(140, 44, |frame| {
        render_overview_page(frame, &model, &state);
    });

    assert!(
        output.contains("bundle fully reachable from heads: yes"),
        "overview render should show full bundle reachability when all payload objects are in head history"
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
    state.refresh_payload_preview(&model);

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
        output.contains("Pack Preview"),
        "payload page should render preview section for selected pack object"
    );
    assert!(
        output.contains("Pack Objects"),
        "payload page should render object listing section"
    );
    assert!(
        output.contains("pack version:"),
        "payload page should render pack version in top summary"
    );
    assert!(
        output.contains("entries: 2/2"),
        "payload page should render parsed/declared entry counters in top summary"
    );
    assert!(
        output.contains("materialized: 2/2"),
        "payload page should render materialized/declared entry counters in top summary"
    );
    assert!(
        output.contains("computed checksum: cccccccccccccccccccccccccccccccccccccccc"),
        "payload page should render full computed pack checksum"
    );
    assert!(
        output.contains("trailer checksum: cccccccccccccccccccccccccccccccccccccccc"),
        "payload page should render full trailer pack checksum"
    );
}

// Verifies that payload top summary uses entry-based proof counters instead of object-count wording.
#[test]
fn payload_summary_uses_entry_based_counters_not_odb_count() {
    let mut model = sample_model(1, 1);
    let PayloadModel::Ok(payload) = &mut model.payload else {
        panic!("fixture model must include payload audit data");
    };
    payload.pack_proof.entries_declared = 7;
    payload.pack_proof.entries_parsed = 7;
    payload.pack_proof.entries_materialized = 6;
    payload.pack_proof.unique_objects_materialized = 5;
    payload.pack_proof.duplicate_entry_count_materialized = 1;
    payload.pack_proof.transfer_allowed = false;
    payload.pack_proof.blocked_reason = Some("1 unresolved entry".to_string());

    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;

    let output = render_and_capture_text(160, 44, |frame| {
        render_page(frame, &model, &state);
    });
    assert!(
        output.contains("entries: 7/7"),
        "payload summary should render parsed/declared entry counters"
    );
    assert!(
        output.contains("materialized: 6/7"),
        "payload summary should render materialized/declared entry counters"
    );
    assert!(
        output.contains("unique objects: 5"),
        "payload summary should render unique-materialized-object counter"
    );
    assert!(
        output.contains("duplicates: 1"),
        "payload summary should render duplicate materialized-entry counter"
    );
    assert!(
        !output.contains("objects: 7/7"),
        "payload summary should avoid legacy parsed/declared object-counter wording"
    );
}

// Verifies that payload entries subview renders the raw ledger table headers.
#[test]
fn entries_table_renders_expected_headers() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;
    state.payload_sub_view = PayloadSubView::Entries;

    let output = render_and_capture_text(160, 44, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("OFFSET"),
        "entries subview should render OFFSET table header"
    );
    assert!(
        output.contains("KIND"),
        "entries subview should render KIND table header"
    );
    assert!(
        output.contains("HDR_SIZE"),
        "entries subview should render HDR_SIZE table header"
    );
    assert!(
        output.contains("RECON"),
        "entries subview should render reconstructed-size table header"
    );
    assert!(
        output.contains("BASE"),
        "entries subview should render BASE table header"
    );
    assert!(
        output.contains("RESOLVED"),
        "entries subview should render RESOLVED table header"
    );
}

// Verifies that entries subview preview includes reconstructed object preview when entry is resolved.
#[test]
fn entries_subview_shows_materialized_object_preview_for_resolved_entry() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;
    state.payload_sub_view = PayloadSubView::Entries;
    let super::super::types::PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit");
    };
    state.payload_selected_index = payload
        .entry_ledger
        .entries
        .iter()
        .position(|entry| matches!(entry.result_kind, Some(crate::git::PayloadObjectKind::Blob)))
        .expect("fixture payload should include at least one resolved blob entry");
    state.refresh_payload_preview(&model);

    let output = render_and_capture_text(160, 44, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("materialized object:"),
        "entries preview should include materialized object header for resolved rows"
    );
    assert!(
        output.contains("content preview:"),
        "entries preview should include reconstructed object content section"
    );
}

// Verifies that payload blob selection renders blob-specific preview metadata.
#[test]
fn render_page_in_payload_view_shows_blob_preview_metadata() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;
    let super::super::types::PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit");
    };
    state.payload_selected_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, crate::git::PayloadObjectKind::Blob))
        .expect("fixture payload should include blob object");
    state.refresh_payload_preview(&model);

    let output = render_and_capture_text(160, 44, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("text lines:"),
        "blob preview should report textual line count"
    );
    assert!(
        output.contains("blob paths:"),
        "blob preview should list reachable paths for the selected blob object"
    );
    assert!(
        !output.contains("1 │ selected:"),
        "preview metadata/header lines should not receive line-number gutters"
    );
    assert!(
        output.contains("1 │ fn value()"),
        "preview should line-number actual text content lines"
    );
    assert!(
        output.contains("reachable from heads:"),
        "payload preview should include selected object reachability context"
    );
    assert!(
        output.contains("context head:"),
        "payload preview should include selected object head context"
    );
    assert!(
        output.contains("context commit order:"),
        "payload preview should include selected object commit-order context"
    );
}

// Verifies that truncated payload preview appends a dynamic overflow marker at the bottom.
#[test]
fn render_page_in_payload_view_shows_dynamic_preview_overflow_marker() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.main_view = super::super::types::MainView::Payload;
    let super::super::types::PayloadModel::Ok(payload) = &model.payload else {
        panic!("fixture model must include payload audit");
    };
    state.payload_selected_index = payload
        .objects
        .iter()
        .position(|entry| matches!(entry.kind, crate::git::PayloadObjectKind::Blob))
        .expect("fixture payload should include blob object");
    state.refresh_payload_preview(&model);

    let output = render_and_capture_text(140, 16, |frame| {
        render_page(frame, &model, &state);
    });

    assert!(
        output.contains("... ("),
        "small preview area should show overflow marker with hidden line count"
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
    assert!(
        output.contains("1 │"),
        "payload object detail should render line-number gutters"
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
        payload_session: sample.payload_session,
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
