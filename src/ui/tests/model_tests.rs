//! Unit tests for model tests.
//!
//! Focus: overview-model repository display helpers and remote-name derivation.

use super::super::model::{derive_repo_name_from_remote_url, format_repo_display};
use std::fs;
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
    let name =
        derive_repo_name_from_remote_url("https://github.com/daredem0/git-sync.git")
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
