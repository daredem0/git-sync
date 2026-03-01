// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for model behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::super::model::build_audit_model;
use super::super::model::{derive_repo_name_from_remote_url, format_repo_display};
use super::support::create_diff_fixture;
use crate::app::AppConfig;
use crate::git;
use crate::ui::types::PayloadModel;
use std::fs;
use std::mem::size_of;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after epoch")
        .as_nanos();
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"));
    fs::create_dir_all(&path).expect("must create unique temp directory");
    path
}

// Verifies that HTTPS remote URLs derive the repository tail name without `.git` suffix.
#[test]
fn derive_repo_name_from_remote_url_handles_https_url() {
    let name = derive_repo_name_from_remote_url("https://github.com/daredem0/git-sync.git")
        .expect("https remote should yield repo name");
    assert_eq!(name, "git-sync");
}

// Verifies that SCP-style SSH remote URLs derive the repository tail name.
#[test]
fn derive_repo_name_from_remote_url_handles_scp_style_url() {
    let name = derive_repo_name_from_remote_url("git@github.com:daredem0/git-sync.git")
        .expect("scp-style remote should yield repo name");
    assert_eq!(name, "git-sync");
}

// Verifies that overview repo display appends remote-derived repository name in parentheses.
#[test]
fn format_repo_display_appends_remote_repo_name_when_available() {
    let dir = unique_temp_dir("git-sync-ui-model-remote");
    let repo = git2::Repository::init(&dir).expect("must init temp repo");
    repo.remote("origin", "https://github.com/daredem0/git-sync.git")
        .expect("must configure origin remote");

    let formatted = format_repo_display(&dir);
    let expected_path = dir.display().to_string();
    assert_eq!(formatted, format!("{expected_path} (git-sync)"));

    fs::remove_dir_all(&dir).expect("must clean temp directory");
}

// Verifies that overview repo display falls back to plain path when remotes are unavailable.
#[test]
fn format_repo_display_falls_back_to_path_without_remote_name() {
    let dir = unique_temp_dir("git-sync-ui-model-no-remote");
    let formatted = format_repo_display(&dir);
    assert_eq!(formatted, dir.display().to_string());
    fs::remove_dir_all(&dir).expect("must clean temp directory");
}

// Verifies that overview repo display falls back to plain path when repository exists but has no remotes configured.
#[test]
fn format_repo_display_falls_back_to_path_for_repo_without_remotes() {
    let dir = unique_temp_dir("git-sync-ui-model-repo-no-remotes");
    let _repo = git2::Repository::init(&dir).expect("must init temp repo");

    let formatted = format_repo_display(&dir);
    assert_eq!(formatted, dir.display().to_string());

    fs::remove_dir_all(&dir).expect("must clean temp directory");
}

// Verifies that full audit-model construction still loads payload data for an existing bundle fixture.
#[test]
fn payload_model_build_still_loads_with_existing_bundle_fixture() {
    let fixture = create_diff_fixture();
    let config = AppConfig {
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };

    let model = build_audit_model(&config);
    match model.payload {
        PayloadModel::Ok(payload) => {
            assert!(
                !payload.objects.is_empty(),
                "payload model should contain imported object rows for fixture bundle"
            );
            assert_eq!(
                payload.pack_proof.declared_object_count, payload.pack_proof.processed_object_count,
                "fixture payload model should preserve pack proof declared/processed invariants"
            );
        }
        PayloadModel::Failed(err) => {
            panic!("payload model build should succeed for fixture: {err}")
        }
    }
}

// Verifies that payload model stores heavy payload data behind indirection to keep enum size small.
#[test]
fn payload_model_enum_is_smaller_than_payload_audit_type() {
    assert!(
        size_of::<PayloadModel>() < size_of::<git::PayloadAudit>(),
        "payload model enum should remain compact versus full payload audit data"
    );
}
