use super::format::single_line_error;
use super::types::{AuditModel, CommitPagesModel, DryRunLine, OverviewModel, StatusLine};
use crate::app::AppConfig;
use crate::git::{self, ReceiveBundleOptions};

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

fn build_overview_model(config: &AppConfig) -> OverviewModel {
    if !config
        .bundle_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
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
        repo_path: config.repo_path.display().to_string(),
        bundle_path: config.bundle_path.display().to_string(),
        base_ref: config.base_ref.clone(),
        tip_ref: config.tip_ref.clone().unwrap_or_else(|| "-".to_string()),
        metadata_verification,
        dry_run,
    }
}
