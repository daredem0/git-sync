//! Unit tests for models.

use super::fixtures::DiffFixture;
use crate::git::{self, CommitAuditEntry, CommitAuditIdentity};
use crate::ui::types::{
    AuditModel, CommitPagesModel, DryRunLine, OverviewModel, StatusLine, SyntaxHighlighter,
};
use std::path::PathBuf;

/// Builds a sample model with synthetic commits and file stats.
pub(crate) fn sample_model(commit_count: usize, files_per_commit: usize) -> AuditModel {
    let commit_pages = CommitPagesModel::Ok(
        (0..commit_count)
            .map(|i| CommitAuditEntry {
                commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                    .expect("valid oid"),
                subject: format!("commit-{i}"),
                committer: CommitAuditIdentity {
                    name: "Committer".to_string(),
                    email: "committer@example.com".to_string(),
                    time_seconds: 1_700_000_000,
                    offset_minutes: 60,
                },
                author: CommitAuditIdentity {
                    name: "Author".to_string(),
                    email: "author@example.com".to_string(),
                    time_seconds: 1_700_000_001,
                    offset_minutes: 60,
                },
                files: (0..files_per_commit)
                    .map(|n| git::FileLineStat {
                        path: format!("file-{n}.txt"),
                        additions: n + 1,
                        deletions: n,
                    })
                    .collect(),
            })
            .collect(),
    );

    AuditModel {
        overview: OverviewModel {
            repo_path: ".".to_string(),
            bundle_path: "sync.bundle.zip".to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "main".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run: DryRunLine::Failed("not needed for state tests".to_string()),
        },
        commit_pages,
        repo_path: PathBuf::from("."),
        bundle_path: PathBuf::from("sync.bundle.zip"),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}

/// Builds a sample model focused on overview-page rendering.
pub(crate) fn sample_overview_model(dry_run: DryRunLine) -> AuditModel {
    AuditModel {
        overview: OverviewModel {
            repo_path: "/tmp/repo".to_string(),
            bundle_path: "/tmp/sync.bundle.zip".to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "-".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run,
        },
        commit_pages: CommitPagesModel::Ok(vec![CommitAuditEntry {
            commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                .expect("valid oid"),
            subject: "subject".to_string(),
            committer: CommitAuditIdentity {
                name: "Committer".to_string(),
                email: "committer@example.com".to_string(),
                time_seconds: 1_700_000_000,
                offset_minutes: 0,
            },
            author: CommitAuditIdentity {
                name: "Author".to_string(),
                email: "author@example.com".to_string(),
                time_seconds: 1_700_000_001,
                offset_minutes: 0,
            },
            files: vec![git::FileLineStat {
                path: "file.txt".to_string(),
                additions: 2,
                deletions: 1,
            }],
        }]),
        repo_path: PathBuf::from("/tmp/repo"),
        bundle_path: PathBuf::from("/tmp/sync.bundle.zip"),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}

/// Builds an `AuditModel` whose commit page data matches a fixture payload.
pub(crate) fn build_model_from_fixture(fixture: &DiffFixture) -> AuditModel {
    AuditModel {
        overview: OverviewModel {
            repo_path: fixture.receiver_dir.display().to_string(),
            bundle_path: fixture.bundle_archive_path.display().to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "-".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run: DryRunLine::Failed("not needed for ui unit tests".to_string()),
        },
        commit_pages: CommitPagesModel::Ok(fixture.entries.clone()),
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}
