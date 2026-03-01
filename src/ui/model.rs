// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI model construction from repository and payload evidence.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::format::single_line_error;
use super::types::{
    AuditModel, CommitPagesModel, DryRunLine, OverviewModel, PayloadModel, StatusLine,
};
use crate::app::AppConfig;
use crate::git::{self, ReceiveBundleOptions};
use crate::version::APP_VERSION;
use std::path::Path;

/// Builds the full UI model used by overview and commit pages.
///
/// Failures in commit-page collection are captured into `CommitPagesModel` so
/// the overview page remains usable.
pub(crate) fn build_audit_model(config: &AppConfig) -> AuditModel {
    let overview = build_overview_model(config);
    let commit_pages = match git::collect_head_audit_entries_for_bundle_input(
        &config.bundle_path,
        &config.repo_path,
    ) {
        Ok(entries) => CommitPagesModel::Ok(entries),
        Err(err) => CommitPagesModel::Failed(single_line_error(&err)),
    };
    let (payload, payload_session) =
        match git::open_payload_session(&config.bundle_path, &config.repo_path) {
            Ok(session) => {
                let payload = PayloadModel::Ok(Box::new(git::payload_audit_from_session(&session)));
                (payload, Some(session))
            }
            Err(err) => (PayloadModel::Failed(single_line_error(&err)), None),
        };

    AuditModel {
        overview,
        commit_pages,
        payload,
        payload_session,
        repo_path: config.repo_path.clone(),
        bundle_path: config.bundle_path.clone(),
        syntax_highlighter: super::types::SyntaxHighlighter::load(),
    }
}

/// Builds overview status lines from repo and bundle validation checks.
///
/// This eagerly computes metadata verification and dry-run applicability so the
/// overview can show a complete health snapshot.
fn build_overview_model(config: &AppConfig) -> OverviewModel {
    if !config
        .bundle_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        // Non-archive inputs can be pre-validated against local refs for clearer overview state.
        let _ = git::open_context(config);
    }

    let metadata_verification = match git::verify_bundle_metadata_against_repo_input(
        &config.bundle_path,
        &config.repo_path,
    ) {
        Ok(()) => StatusLine::Ok,
        Err(err) => StatusLine::Failed(single_line_error(&err)),
    };

    let dry_run = match git::receive_bundle_input_with_options(
        &config.bundle_path,
        &config.repo_path,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    ) {
        Ok(result) => DryRunLine::Ok(result),
        Err(err) => DryRunLine::Failed(single_line_error(&err)),
    };

    OverviewModel {
        app_version: APP_VERSION.to_string(),
        repo_path: format_repo_display(&config.repo_path),
        bundle_path: config.bundle_path.display().to_string(),
        base_ref: config.base_ref.clone(),
        tip_ref: config.tip_ref.clone().unwrap_or_else(|| "-".to_string()),
        metadata_verification,
        dry_run,
    }
}

/// Formats repository display as `<path> (<repo_name>)` when a remote-derived
/// repository name can be determined.
pub(super) fn format_repo_display(repo_path: &Path) -> String {
    let path = repo_path.display().to_string();
    match derive_repo_name_from_repo(repo_path) {
        Some(name) => format!("{path} ({name})"),
        None => path,
    }
}

/// Attempts to derive a repository name from the configured remotes.
fn derive_repo_name_from_repo(repo_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let remotes = repo.remotes().ok()?;

    let remote_name = if remotes.iter().flatten().any(|name| name == "origin") {
        "origin".to_string()
    } else {
        remotes.iter().flatten().next()?.to_string()
    };

    let remote = repo.find_remote(&remote_name).ok()?;
    let remote_url = remote.url()?;
    derive_repo_name_from_remote_url(remote_url)
}

/// Extracts repo-name tail from common git remote URL forms.
pub(super) fn derive_repo_name_from_remote_url(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches('/');
    let tail = trimmed.rsplit(['/', ':']).next()?;
    if tail.is_empty() {
        return None;
    }
    let name = tail.strip_suffix(".git").unwrap_or(tail);
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}
