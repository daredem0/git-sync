//! TUI-layer model functionality.

use super::format::single_line_error;
use super::types::{AuditModel, CommitPagesModel, DryRunLine, OverviewModel, StatusLine};
use crate::app::AppConfig;
use crate::git::{self, ReceiveBundleOptions};
use crate::version::APP_VERSION;

/// Builds the full UI model used by overview and commit pages.
///
/// Failures in commit-page collection are captured into `CommitPagesModel` so
/// the overview page remains usable.
pub(crate) fn build_audit_model(config: &AppConfig) -> AuditModel {
    let overview = build_overview_model(config);
    let commit_pages = match git::collect_commit_audit_entries_for_bundle_input(
        &config.bundle_path,
        &config.repo_path,
    ) {
        Ok(entries) => CommitPagesModel::Ok(entries),
        Err(err) => CommitPagesModel::Failed(single_line_error(&err)),
    };

    AuditModel {
        overview,
        commit_pages,
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
        repo_path: config.repo_path.display().to_string(),
        bundle_path: config.bundle_path.display().to_string(),
        base_ref: config.base_ref.clone(),
        tip_ref: config.tip_ref.clone().unwrap_or_else(|| "-".to_string()),
        metadata_verification,
        dry_run,
    }
}
