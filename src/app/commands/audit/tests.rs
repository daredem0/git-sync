// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for app/commands/audit.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::git::create_bundle;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo_dir(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-audit-command-{suffix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ))
}

fn commit_from_files(
    repo: &git2::Repository,
    message: &str,
    files: &[(&str, &str)],
    parents: &[git2::Oid],
) -> git2::Oid {
    let mut tree_builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content) in files {
        let blob_oid = repo
            .blob(content.as_bytes())
            .expect("must create blob object");
        tree_builder
            .insert(*path, blob_oid, 0o100644)
            .expect("must insert tree entry");
    }
    let tree_oid = tree_builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_oid).expect("must resolve tree");
    let parent_commits = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent commit"))
        .collect::<Vec<_>>();
    let parent_refs = parent_commits.iter().collect::<Vec<_>>();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

fn create_audit_fixture(suffix: &str) -> (PathBuf, PathBuf, PathBuf) {
    let repo_dir = temp_repo_dir(suffix);
    std::fs::create_dir_all(&repo_dir).expect("must create fixture repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must initialize fixture repository");

    let base_oid = commit_from_files(&repo, "base", &[("f.txt", "base")], &[]);
    let tip_oid = commit_from_files(
        &repo,
        "tip",
        &[("f.txt", "tip"), ("new.txt", "new")],
        &[base_oid],
    );
    repo.reference("refs/heads/base", base_oid, true, "seed base ref")
        .expect("must seed base ref");
    repo.reference("refs/heads/tip", tip_oid, true, "seed tip ref")
        .expect("must seed tip ref");

    let bundle_path = repo_dir.join("audit.bundle");
    let create_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("must create fixture bundle");

    (
        repo_dir,
        create_result.bundle_path,
        create_result.archive_path,
    )
}

#[test]
fn run_interactive_rejects_non_pack_only_resolve_mode() {
    let result = run_interactive(
        Some(PathBuf::from(".")),
        Some(PathBuf::from("sync.bundle.zip")),
        CliPayloadResolveMode::Baseline,
    );
    let error = result.expect_err("interactive mode should reject baseline resolve mode");
    assert!(
        error
            .to_string()
            .contains("interactive audit currently supports only --resolve pack-only"),
        "error should explain interactive resolve-mode constraint"
    );
}

#[test]
fn run_interactive_with_passes_expected_config_to_runner() {
    let repo_path = PathBuf::from("/tmp/repo");
    let bundle_path = PathBuf::from("/tmp/sync.bundle.zip");
    let mut captured: Option<AppConfig> = None;

    let result = run_interactive_with(
        Some(repo_path.clone()),
        Some(bundle_path.clone()),
        CliPayloadResolveMode::PackOnly,
        |config| {
            captured = Some(config.clone());
            Ok(())
        },
    );

    assert!(
        result.is_ok(),
        "interactive wrapper should return runner success"
    );
    let captured = captured.expect("interactive wrapper should invoke runner");
    assert_eq!(captured.repo_path, repo_path);
    assert_eq!(captured.bundle_path, bundle_path);
    assert_eq!(captured.base_ref, "sync/last");
    assert_eq!(captured.tip_ref, None);
}

#[test]
fn run_verify_metadata_requires_repo_and_bundle_arguments() {
    let missing_repo = run(
        None,
        Some(PathBuf::from("sync.bundle.zip")),
        true,
        None,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        missing_repo
            .expect_err("verify-metadata mode should require repo")
            .to_string()
            .contains("metadata verification requires --repo")
    );

    let missing_bundle = run(
        Some(PathBuf::from(".")),
        None,
        true,
        None,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        missing_bundle
            .expect_err("verify-metadata mode should require bundle")
            .to_string()
            .contains("metadata verification requires --bundle")
    );
}

#[test]
fn run_non_interactive_propagates_target_resolution_error() {
    let result = run_non_interactive(
        None,
        None,
        OutputFormat::Table,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        result
            .expect_err("non-interactive mode should require target input")
            .to_string()
            .contains("payload audit requires both --repo and --bundle"),
        "target resolution error should be preserved"
    );
}

#[test]
fn run_non_interactive_table_succeeds_for_bundle_input() {
    let (repo_dir, bundle_path, _archive_path) = create_audit_fixture("table-mode");

    let result = run_non_interactive(
        Some(repo_dir.clone()),
        Some(bundle_path),
        OutputFormat::Table,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_ok(),
        "table-mode payload audit should succeed for a valid fixture bundle"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

#[test]
fn run_non_interactive_json_supports_full_and_light_profiles() {
    let (repo_dir, bundle_path, _archive_path) = create_audit_fixture("json-profiles");

    let summary_result = run_non_interactive(
        Some(repo_dir.clone()),
        Some(bundle_path.clone()),
        OutputFormat::Json,
        PayloadLedgerMode::Summary,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        summary_result.is_ok(),
        "json summary/full payload audit should succeed for a valid fixture bundle"
    );

    let full_result = run_non_interactive(
        Some(repo_dir.clone()),
        Some(bundle_path.clone()),
        OutputFormat::Json,
        PayloadLedgerMode::Full,
        PayloadDetailMode::Full,
        CliPayloadResolveMode::PackOnly,
    );
    assert!(
        full_result.is_ok(),
        "json full-mode payload audit should succeed for a valid fixture bundle"
    );

    let minimal_result = run_non_interactive(
        Some(repo_dir.clone()),
        Some(bundle_path),
        OutputFormat::Json,
        PayloadLedgerMode::None,
        PayloadDetailMode::Light,
        CliPayloadResolveMode::Baseline,
    );
    assert!(
        minimal_result.is_ok(),
        "json light/none payload audit should succeed for a valid fixture bundle"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}
