//! Unit tests for models.

use super::fixtures::DiffFixture;
use crate::git::{self, BundleVersion, CommitAuditEntry, CommitAuditIdentity};
use crate::ui::types::{
    AuditModel, CommitPagesModel, DryRunLine, OverviewModel, StatusLine, SyntaxHighlighter,
};
use std::path::PathBuf;

fn oid_from_u64(value: u64) -> git2::Oid {
    git2::Oid::from_str(&format!("{value:040x}")).expect("must create valid oid")
}

/// Builds a sample model with synthetic commits and file stats.
pub(crate) fn sample_model(commit_count: usize, files_per_commit: usize) -> AuditModel {
    let commits: Vec<CommitAuditEntry> = (0..commit_count)
        .map(|i| CommitAuditEntry {
            commit_id: oid_from_u64(1 + i as u64),
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
        .collect();

    let line_stats = (0..files_per_commit)
        .map(|n| git::FileLineStat {
            path: format!("file-{n}.txt"),
            additions: n + 1,
            deletions: n,
        })
        .collect();

    let commit_pages = CommitPagesModel::Ok(vec![git::HeadAuditEntry {
        head: git::BundleHead {
            oid: oid_from_u64(999),
            reference: "refs/heads/main".to_string(),
        },
        line_stats,
        commits,
    }]);

    AuditModel {
        overview: OverviewModel {
            app_version: "test-version".to_string(),
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

/// Builds a synthetic model with multiple heads for selected-head navigation tests.
pub(crate) fn sample_multi_head_model(commit_counts: &[usize]) -> AuditModel {
    let head_entries: Vec<git::HeadAuditEntry> = commit_counts
        .iter()
        .enumerate()
        .map(|(head_idx, &commit_count)| {
            let commits = (0..commit_count)
                .map(|commit_idx| CommitAuditEntry {
                    commit_id: oid_from_u64(10_000 + (head_idx * 100 + commit_idx) as u64),
                    subject: format!("head-{}-commit-{}", head_idx + 1, commit_idx + 1),
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
                        path: format!("head-{}-file-{}.txt", head_idx + 1, commit_idx + 1),
                        additions: commit_idx + 1,
                        deletions: commit_idx,
                    }],
                })
                .collect::<Vec<_>>();

            let line_stats = (0..commit_count)
                .map(|commit_idx| git::FileLineStat {
                    path: format!("head-{}-file-{}.txt", head_idx + 1, commit_idx + 1),
                    additions: commit_idx + 1,
                    deletions: commit_idx,
                })
                .collect();

            git::HeadAuditEntry {
                head: git::BundleHead {
                    oid: oid_from_u64(20_000 + head_idx as u64),
                    reference: format!("refs/heads/head-{}", head_idx + 1),
                },
                line_stats,
                commits,
            }
        })
        .collect();

    let imported_heads = head_entries
        .iter()
        .map(|entry| entry.head.clone())
        .collect::<Vec<_>>();
    let aggregated_line_stats = head_entries
        .iter()
        .flat_map(|entry| entry.line_stats.clone())
        .collect::<Vec<_>>();

    AuditModel {
        overview: OverviewModel {
            app_version: "test-version".to_string(),
            repo_path: "/tmp/repo".to_string(),
            bundle_path: "/tmp/sync.bundle.zip".to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "-".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run: DryRunLine::Ok(git::ReceiveBundleResult {
                bundle_version: BundleVersion::V2,
                imported_heads,
                can_apply_without_conflicts: true,
                line_stats: aggregated_line_stats,
            }),
        },
        commit_pages: CommitPagesModel::Ok(head_entries),
        repo_path: PathBuf::from("/tmp/repo"),
        bundle_path: PathBuf::from("/tmp/sync.bundle.zip"),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}

/// Builds a sample model focused on overview-page rendering.
pub(crate) fn sample_overview_model(dry_run: DryRunLine) -> AuditModel {
    AuditModel {
        overview: OverviewModel {
            app_version: "test-version".to_string(),
            repo_path: "/tmp/repo".to_string(),
            bundle_path: "/tmp/sync.bundle.zip".to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "-".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run,
        },
        commit_pages: CommitPagesModel::Ok(vec![git::HeadAuditEntry {
            head: git::BundleHead {
                oid: oid_from_u64(555),
                reference: "refs/heads/main".to_string(),
            },
            line_stats: vec![git::FileLineStat {
                path: "file.txt".to_string(),
                additions: 2,
                deletions: 1,
            }],
            commits: vec![CommitAuditEntry {
                commit_id: oid_from_u64(556),
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
            }],
        }]),
        repo_path: PathBuf::from("/tmp/repo"),
        bundle_path: PathBuf::from("/tmp/sync.bundle.zip"),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}

/// Builds an `AuditModel` whose commit page data matches a fixture payload.
pub(crate) fn build_model_from_fixture(fixture: &DiffFixture) -> AuditModel {
    let head_oid = fixture
        .entries
        .last()
        .map(|entry| entry.commit_id)
        .unwrap_or_else(|| oid_from_u64(777));
    let line_stats = fixture
        .entries
        .first()
        .map(|entry| entry.files.clone())
        .unwrap_or_default();

    AuditModel {
        overview: OverviewModel {
            app_version: "test-version".to_string(),
            repo_path: fixture.receiver_dir.display().to_string(),
            bundle_path: fixture.bundle_archive_path.display().to_string(),
            base_ref: "sync/last".to_string(),
            tip_ref: "-".to_string(),
            metadata_verification: StatusLine::Ok,
            dry_run: DryRunLine::Failed("not needed for ui unit tests".to_string()),
        },
        commit_pages: CommitPagesModel::Ok(vec![git::HeadAuditEntry {
            head: git::BundleHead {
                oid: head_oid,
                reference: "refs/heads/tip".to_string(),
            },
            line_stats,
            commits: fixture.entries.clone(),
        }]),
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}
