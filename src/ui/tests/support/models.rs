//! Unit tests for models.

use super::fixtures::DiffFixture;
use crate::git::{self, BundleVersion, CommitAuditEntry, CommitAuditIdentity};
use crate::ui::types::{
    AuditModel, CommitPagesModel, DryRunLine, OverviewModel, PayloadModel, StatusLine,
    SyntaxHighlighter,
};
use std::path::PathBuf;

fn oid_from_u64(value: u64) -> git2::Oid {
    git2::Oid::from_str(&format!("{value:040x}")).expect("must create valid oid")
}

fn sample_payload_audit() -> git::PayloadAudit {
    git::PayloadAudit {
        bundle_version: BundleVersion::V2,
        heads: vec![git::BundleHead {
            oid: oid_from_u64(42),
            reference: "refs/heads/main".to_string(),
        }],
        transport_entries: vec![
            git::PayloadTransportEntry {
                name: "sync.bundle".to_string(),
                size_bytes: 1234,
                sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            },
            git::PayloadTransportEntry {
                name: "sync.bundle.caudit.json".to_string(),
                size_bytes: 456,
                sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            },
        ],
        pack_proof: git::PayloadPackProof {
            verification_status: "ok".to_string(),
            pack_version: 2,
            declared_object_count: 2,
            processed_object_count: 2,
            hash_algorithm: "sha1".to_string(),
            computed_pack_checksum: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
            trailer_pack_checksum: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
        },
        entry_ledger: git::PackEntryLedger {
            pack_version: 2,
            declared_entry_count: 2,
            entries: vec![
                git::PackEntryRecord {
                    idx: 0,
                    offset: 12,
                    kind: git::PackEntryKind::Commit,
                    out_size: 180,
                    base_ref: None,
                    result_oid: Some(oid_from_u64(1000)),
                    result_kind: Some(git::PayloadObjectKind::Commit),
                    resolved: true,
                    resolved_via: Some(git::ResolutionSource::InPack),
                    note: None,
                },
                git::PackEntryRecord {
                    idx: 1,
                    offset: 64,
                    kind: git::PackEntryKind::Blob,
                    out_size: 42,
                    base_ref: None,
                    result_oid: Some(oid_from_u64(1001)),
                    result_kind: Some(git::PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(git::ResolutionSource::InPack),
                    note: None,
                },
            ],
        },
        objects: vec![
            git::PayloadObjectEntry {
                oid: oid_from_u64(1000),
                kind: git::PayloadObjectKind::Commit,
                size_bytes: 180,
                reachable_from_heads: true,
                context_head_index: Some(0),
                context_commit_order: Some(1),
                context_path: None,
            },
            git::PayloadObjectEntry {
                oid: oid_from_u64(1001),
                kind: git::PayloadObjectKind::Blob,
                size_bytes: 42,
                reachable_from_heads: false,
                context_head_index: None,
                context_commit_order: None,
                context_path: None,
            },
        ],
    }
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
        payload: PayloadModel::Ok(sample_payload_audit()),
        payload_session: None,
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
        payload: PayloadModel::Ok(sample_payload_audit()),
        payload_session: None,
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
        payload: PayloadModel::Ok(sample_payload_audit()),
        payload_session: None,
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
    let payload_session =
        git::open_payload_session(&fixture.bundle_archive_path, &fixture.receiver_dir)
            .expect("must open payload session for fixture bundle");
    let payload = git::payload_audit_from_session(&payload_session);

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
        payload: PayloadModel::Ok(payload),
        payload_session: Some(payload_session),
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
}
