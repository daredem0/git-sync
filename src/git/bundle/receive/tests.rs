// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for receive bundle integration and rollback behavior.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::test_hooks::{ManualCasMutationBeforeCheck, configure_fault_injection_for_tests};
use super::*;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_fault_injection_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .expect("fault-injection test lock mutex should not be poisoned")
}

fn temp_bare_repo_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-receive-rollback-{suffix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos()
    ))
}

fn commit_from_content(
    repo: &git2::Repository,
    message: &str,
    content: &str,
    parents: &[git2::Oid],
) -> git2::Oid {
    let mut tree_builder = repo.treebuilder(None).expect("must create tree builder");
    let blob = repo.blob(content.as_bytes()).expect("must create blob");
    tree_builder
        .insert("f.txt", blob, 0o100644)
        .expect("must insert tree entry");
    let tree_oid = tree_builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_oid).expect("must resolve tree");

    let parent_commits = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent commit"))
        .collect::<Vec<_>>();
    let parent_refs = parent_commits.iter().collect::<Vec<_>>();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must build sig");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

fn commit_with_gitlink(
    repo: &git2::Repository,
    message: &str,
    file_content: &str,
    gitlink_oid: git2::Oid,
    parents: &[git2::Oid],
) -> git2::Oid {
    let mut tree_builder = repo.treebuilder(None).expect("must create tree builder");
    let blob = repo
        .blob(file_content.as_bytes())
        .expect("must create regular blob");
    tree_builder
        .insert("f.txt", blob, 0o100644)
        .expect("must insert regular file entry");
    tree_builder
        .insert("submodule", gitlink_oid, 0o160000)
        .expect("must insert gitlink tree entry");
    let tree_oid = tree_builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_oid).expect("must resolve tree");

    let parent_commits = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent commit"))
        .collect::<Vec<_>>();
    let parent_refs = parent_commits.iter().collect::<Vec<_>>();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must build sig");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

fn build_two_ref_update_fixture(
    suffix: &str,
) -> (
    PathBuf,
    git2::Repository,
    Vec<PlannedRefUpdate>,
    git2::Oid,
    git2::Oid,
    git2::Oid,
    git2::Oid,
) {
    let repo_path = temp_bare_repo_path(suffix);
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let main_old = commit_from_content(&repo, "main old", "main-old", &[]);
    let main_new = commit_from_content(&repo, "main new", "main-new", &[main_old]);
    let side_old = commit_from_content(&repo, "side old", "side-old", &[main_old]);
    let side_new = commit_from_content(&repo, "side new", "side-new", &[side_old]);

    repo.reference("refs/heads/main", main_old, true, "seed main old")
        .expect("must seed main old ref");
    repo.reference("refs/heads/side", side_old, true, "seed side old")
        .expect("must seed side old ref");

    let updates = vec![
        PlannedRefUpdate {
            ref_name: "refs/heads/main".to_string(),
            expected_old_oid: Some(main_old),
            new_oid: main_new,
        },
        PlannedRefUpdate {
            ref_name: "refs/heads/side".to_string(),
            expected_old_oid: Some(side_old),
            new_oid: side_new,
        },
    ];

    (
        repo_path, repo, updates, main_old, main_new, side_old, side_new,
    )
}

fn sample_inspection_with(
    version: crate::git::BundleVersion,
    prerequisites: Vec<git2::Oid>,
) -> BundleInspection {
    BundleInspection {
        version,
        prerequisites,
        heads: vec![BundleHead {
            oid: git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("must parse synthetic head oid"),
            reference: "refs/heads/main".to_string(),
        }],
    }
}

#[test]
fn rollback_applied_updates_restores_existing_target_refs() {
    let _test_lock = test_fault_injection_lock();
    let repo_path = temp_bare_repo_path("restore-existing");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let old_oid = commit_from_content(&repo, "old", "old", &[]);
    let new_oid = commit_from_content(&repo, "new", "new", &[old_oid]);
    repo.reference("refs/heads/main", old_oid, true, "seed old")
        .expect("must create old ref");
    repo.reference("refs/heads/main", new_oid, true, "simulate applied update")
        .expect("must update ref to new oid");

    let update = PlannedRefUpdate {
        ref_name: "refs/heads/main".to_string(),
        expected_old_oid: Some(old_oid),
        new_oid,
    };
    let rollback = rollback_applied_updates(&repo, &[update]);

    let current = resolve_reference_target(&repo, "refs/heads/main")
        .expect("must resolve restored ref target");
    assert_eq!(
        current,
        Some(old_oid),
        "rollback should restore old ref target for existing refs"
    );
    assert!(
        rollback
            .restored_refs
            .iter()
            .any(|ref_name| ref_name == "refs/heads/main"),
        "rollback should record restored refs"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn rollback_applied_updates_deletes_newly_created_refs() {
    let _test_lock = test_fault_injection_lock();
    let repo_path = temp_bare_repo_path("delete-created");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let new_oid = commit_from_content(&repo, "new", "new", &[]);
    repo.reference(
        "refs/heads/new-target",
        new_oid,
        true,
        "simulate created ref",
    )
    .expect("must create new ref");

    let update = PlannedRefUpdate {
        ref_name: "refs/heads/new-target".to_string(),
        expected_old_oid: None,
        new_oid,
    };
    let rollback = rollback_applied_updates(&repo, &[update]);

    let current = resolve_reference_target(&repo, "refs/heads/new-target")
        .expect("must resolve deleted ref target");
    assert_eq!(current, None, "rollback should delete newly created refs");
    assert!(
        rollback
            .deleted_refs
            .iter()
            .any(|ref_name| ref_name == "refs/heads/new-target"),
        "rollback should record deleted refs"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn manual_cas_injected_second_update_failure_rolls_back_first_update() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-fail-index");

    let _fault = configure_fault_injection_for_tests(true, Some(1), None, None, false, None, None);
    let result = apply_ref_updates_with_manual_cas(&repo, &updates);
    assert!(
        result.is_err(),
        "injected manual-cas update fault should fail the apply path"
    );
    let err_text = result
        .expect_err("manual-cas apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected manual-cas update failure"),
        "failure diagnostics should include injected manual-cas reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "failed second update should roll back first updated ref to its old target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "failing second update should leave second target at old value"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn temp_bare_repo_from_existing_inherits_unreachable_source_objects() {
    let _test_lock = test_fault_injection_lock();
    let source_repo_path = temp_bare_repo_path("temp-mirror-alternates");
    std::fs::create_dir_all(&source_repo_path).expect("must create source repo path");
    let source_repo = git2::Repository::init_bare(&source_repo_path).expect("must init source");

    let main_oid = commit_from_content(&source_repo, "main", "main", &[]);
    source_repo
        .reference("refs/heads/main", main_oid, true, "seed main ref")
        .expect("must create source main ref");

    // This object is intentionally left unreachable from refs; dry-run mirror must still see it.
    let dangling_blob = source_repo
        .blob(b"unreachable source object")
        .expect("must create dangling blob");
    source_repo
        .find_blob(dangling_blob)
        .expect("source repo must resolve dangling blob");

    let temp_repo = TempBareRepo::from_existing(&source_repo_path)
        .expect("must create temp mirror from source repository");
    let mirror_repo = git2::Repository::open_bare(&temp_repo.path).expect("must open temp mirror");
    mirror_repo
        .find_blob(dangling_blob)
        .expect("dry-run mirror must resolve source-only unreachable blob via alternates");

    drop(temp_repo);
    let _ = std::fs::remove_dir_all(source_repo_path);
}

#[test]
fn manual_cas_injected_first_update_failure_keeps_all_targets_unchanged() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-fail-first-index");

    let _fault = configure_fault_injection_for_tests(true, Some(0), None, None, false, None, None);
    let result = apply_ref_updates_with_manual_cas(&repo, &updates);
    assert!(
        result.is_err(),
        "injected first-update failure should fail manual-cas apply"
    );
    let err_text = result
        .expect_err("manual-cas apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected manual-cas update failure"),
        "diagnostics should include injected update-failure reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "first-update failure should leave main unchanged"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "first-update failure should leave side unchanged"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn manual_cas_injected_precondition_race_rolls_back_applied_updates() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, _side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-precondition-race");

    let side_race = commit_from_content(&repo, "side race", "side-race", &[main_old]);
    let mutation = ManualCasMutationBeforeCheck {
        update_index: 1,
        ref_name: "refs/heads/side".to_string(),
        mutate_to_oid: Some(side_race),
    };
    let _fault =
        configure_fault_injection_for_tests(true, None, Some(mutation), None, false, None, None);
    let result = apply_ref_updates_with_manual_cas(&repo, &updates);
    assert!(
        result.is_err(),
        "injected precondition-race mutation should fail manual-cas apply"
    );
    let err_text = result
        .expect_err("manual-cas apply should fail")
        .to_string();
    assert!(
        err_text.contains("CAS precondition failed"),
        "failure should be reported as CAS precondition violation"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "manual-cas precondition race should roll back previously applied refs"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_race),
        "injected external mutation should remain in place for non-applied update refs"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn manual_cas_injected_deleted_target_precondition_race_rolls_back_applied_updates() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, _side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-precondition-delete");

    let mutation = ManualCasMutationBeforeCheck {
        update_index: 1,
        ref_name: "refs/heads/side".to_string(),
        mutate_to_oid: None,
    };
    let _fault =
        configure_fault_injection_for_tests(true, None, Some(mutation), None, false, None, None);
    let result = apply_ref_updates_with_manual_cas(&repo, &updates);
    assert!(
        result.is_err(),
        "deleted-target precondition race should fail manual-cas apply"
    );
    let err_text = result
        .expect_err("manual-cas apply should fail")
        .to_string();
    assert!(
        err_text.contains("CAS precondition failed"),
        "failure should be reported as CAS precondition violation"
    );
    assert!(
        err_text.contains("expected old target"),
        "diagnostics should include expected/actual target mismatch details"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "precondition failure should roll back already-applied main update"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target, None,
        "deleted-target mutation should persist on non-applied ref"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn manual_cas_injected_rollback_failure_is_reported_explicitly() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, _main_old, main_new, side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-rollback-failure");

    let _fault = configure_fault_injection_for_tests(
        true,
        Some(1),
        None,
        Some("refs/heads/main".to_string()),
        false,
        None,
        None,
    );
    let result = apply_ref_updates_with_manual_cas(&repo, &updates);
    assert!(
        result.is_err(),
        "manual-cas failure with injected rollback failure should return an error"
    );
    let err_text = result
        .expect_err("manual-cas apply should fail")
        .to_string();
    assert!(
        err_text.contains("rollback failures:"),
        "failure diagnostics should include rollback-failures section"
    );
    assert!(
        err_text.contains("injected rollback failure"),
        "failure diagnostics should include injected rollback-failure reason"
    );
    assert!(
        err_text.contains("refs/heads/main"),
        "failure diagnostics should include the ref that failed to roll back"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_new),
        "when rollback fails, first applied update should remain at new target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "second non-applied update should remain at old target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn transaction_injected_lock_ref_first_update_failure_preserves_all_targets() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("transaction-lock-ref-first-failure");

    let _fault = configure_fault_injection_for_tests(false, None, None, None, false, Some(0), None);
    let tx = repo.transaction().expect("must open transaction");
    let result = apply_ref_updates_with_transaction(&repo, tx, &updates);
    assert!(
        result.is_err(),
        "injected first lock-ref failure should fail transactional apply path"
    );
    let err_text = result
        .expect_err("transaction apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected transaction lock-ref failure"),
        "diagnostics should include injected lock-ref failure reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "first lock-ref failure should preserve main target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "first lock-ref failure should preserve side target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn transaction_injected_commit_failure_reports_and_preserves_targets() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, _side_old, _side_new) =
        build_two_ref_update_fixture("transaction-commit-failure");

    let _fault = configure_fault_injection_for_tests(false, None, None, None, true, None, None);
    let tx = repo.transaction().expect("must open transaction");
    let result = apply_ref_updates_with_transaction(&repo, tx, &[updates[0].clone()]);
    assert!(
        result.is_err(),
        "injected transaction commit failure should fail transactional apply path"
    );
    let err_text = result
        .expect_err("transaction apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected transaction commit failure"),
        "failure diagnostics should include injected transaction failure reason"
    );
    assert!(
        err_text.contains("unable to apply receive target updates (ref_transaction)"),
        "failure diagnostics should include transaction backend context"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "injected transaction failure should preserve target refs"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn transaction_injected_set_target_first_update_failure_preserves_all_targets() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("transaction-set-target-first-failure");

    let _fault = configure_fault_injection_for_tests(false, None, None, None, false, None, Some(0));
    let tx = repo.transaction().expect("must open transaction");
    let result = apply_ref_updates_with_transaction(&repo, tx, &updates);
    assert!(
        result.is_err(),
        "injected first set-target failure should fail transactional apply path"
    );
    let err_text = result
        .expect_err("transaction apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected transaction set-target failure"),
        "diagnostics should include injected set-target failure reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "first set-target failure should preserve main target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "first set-target failure should preserve side target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn transaction_injected_lock_ref_failure_preserves_all_targets() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("transaction-lock-ref-failure");

    let _fault = configure_fault_injection_for_tests(false, None, None, None, false, Some(1), None);
    let tx = repo.transaction().expect("must open transaction");
    let result = apply_ref_updates_with_transaction(&repo, tx, &updates);
    assert!(
        result.is_err(),
        "injected transaction lock-ref failure should fail transactional apply path"
    );
    let err_text = result
        .expect_err("transaction apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected transaction lock-ref failure"),
        "diagnostics should include injected lock-ref failure reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "lock-ref failure should preserve main target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "lock-ref failure should preserve side target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn transaction_injected_set_target_failure_preserves_all_targets() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("transaction-set-target-failure");

    let _fault = configure_fault_injection_for_tests(false, None, None, None, false, None, Some(1));
    let tx = repo.transaction().expect("must open transaction");
    let result = apply_ref_updates_with_transaction(&repo, tx, &updates);
    assert!(
        result.is_err(),
        "injected transaction set-target failure should fail transactional apply path"
    );
    let err_text = result
        .expect_err("transaction apply should fail")
        .to_string();
    assert!(
        err_text.contains("injected transaction set-target failure"),
        "diagnostics should include injected set-target failure reason"
    );

    let main_target =
        resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
    assert_eq!(
        main_target,
        Some(main_old),
        "set-target failure should preserve main target"
    );
    let side_target =
        resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
    assert_eq!(
        side_target,
        Some(side_old),
        "set-target failure should preserve side target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn manual_cas_fault_stress_repeated_failures_keep_refs_consistent() {
    let _test_lock = test_fault_injection_lock();
    let (repo_path, repo, updates, main_old, _main_new, side_old, _side_new) =
        build_two_ref_update_fixture("manual-cas-fault-stress");

    for attempt in 0..32 {
        repo.reference("refs/heads/main", main_old, true, "reset main to old")
            .expect("must reset main before stress attempt");
        repo.reference("refs/heads/side", side_old, true, "reset side to old")
            .expect("must reset side before stress attempt");

        let _fault = if attempt % 2 == 0 {
            configure_fault_injection_for_tests(true, Some(1), None, None, false, None, None)
        } else {
            let injected_side = commit_from_content(
                &repo,
                "side race injected",
                &format!("side-race-{attempt}"),
                &[main_old],
            );
            let mutation = ManualCasMutationBeforeCheck {
                update_index: 1,
                ref_name: "refs/heads/side".to_string(),
                mutate_to_oid: Some(injected_side),
            };
            configure_fault_injection_for_tests(true, None, Some(mutation), None, false, None, None)
        };

        let result = apply_ref_updates_with_manual_cas(&repo, &updates);
        assert!(
            result.is_err(),
            "stress attempt should fail due injected fault (attempt={attempt})"
        );

        let main_target =
            resolve_reference_target(&repo, "refs/heads/main").expect("must resolve main");
        assert_eq!(
            main_target,
            Some(main_old),
            "stress attempt should preserve or restore main target (attempt={attempt})"
        );

        let side_target =
            resolve_reference_target(&repo, "refs/heads/side").expect("must resolve side");
        if attempt % 2 == 0 {
            assert_eq!(
                side_target,
                Some(side_old),
                "update-fault attempts should preserve side old target (attempt={attempt})"
            );
        } else {
            assert_ne!(
                side_target,
                Some(side_old),
                "race-injection attempts should leave externally mutated side target (attempt={attempt})"
            );
        }
    }

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn missing_objects_indexer_error_enables_fetch_import_fallback() {
    let error = anyhow::anyhow!("packfile is missing 17 objects; class=Indexer(15)");
    assert!(
        should_try_fetch_import_fallback(&error),
        "missing-object indexer failure should trigger fetch fallback"
    );
}

#[test]
fn fetch_staging_ref_name_uses_deterministic_sync_namespace() {
    let refs_head = fetch_staging_ref_name("abcd1234ef00", "refs/heads/main");
    assert_eq!(
        refs_head, "refs/sync/fetch-staging/abcd1234ef00/heads/main",
        "fetch staging refs should be derived from refs/* suffixes"
    );

    let head = fetch_staging_ref_name("abcd1234ef00", "HEAD");
    assert_eq!(
        head, "refs/sync/fetch-staging/abcd1234ef00/HEAD",
        "HEAD inputs should still be represented in staging namespace"
    );
}

#[test]
fn bundle_fetch_remote_candidates_include_path_and_file_url_for_local_bundle() {
    let repo_path = temp_bare_repo_path("bundle-fetch-remote-url");
    std::fs::create_dir_all(&repo_path).expect("must create temp path for URL normalization");
    let bundle_path = repo_path.join("example.bundle");
    std::fs::write(&bundle_path, b"placeholder").expect("must write temporary bundle file");

    let remotes = bundle_fetch_remote_candidates(&bundle_path)
        .expect("local bundle paths should produce fallback URL candidates");
    assert_eq!(
        remotes.len(),
        2,
        "local bundle fallback should try both plain path and file:// URL forms"
    );
    let remote = remotes[1].clone();
    assert!(
        remote.starts_with("file://"),
        "local fallback remote URL should use file:// scheme"
    );
    assert!(
        remote.ends_with("example.bundle"),
        "normalized URL should retain bundle filename"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn bundle_fetch_remote_candidates_keep_existing_schemes() {
    let remotes =
        bundle_fetch_remote_candidates(std::path::Path::new("https://example.com/sync.bundle"))
            .expect("scheme-based input should be accepted unchanged");
    assert_eq!(
        remotes.len(),
        1,
        "pre-schemed inputs should produce one candidate"
    );
    let remote = remotes[0].clone();
    assert_eq!(
        remote, "https://example.com/sync.bundle",
        "pre-schemed bundle URL should remain unchanged"
    );
}

#[test]
fn connectivity_validation_accepts_simple_head_history() {
    let repo_path = temp_bare_repo_path("connectivity-validation-ok");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let root = commit_from_content(&repo, "root", "root", &[]);
    let tip = commit_from_content(&repo, "tip", "tip", &[root]);
    let heads = vec![BundleHead {
        oid: tip,
        reference: "refs/heads/main".to_string(),
    }];
    validate_import_connectivity_for_heads(
        &repo,
        &heads,
        &[],
        ImportPath::CompatIndexerVerifyFalse,
    )
    .expect("connectivity validation should accept complete reachable history");

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn connectivity_validation_rejects_missing_head_commit() {
    let repo_path = temp_bare_repo_path("connectivity-validation-missing-head");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let missing_head = git2::Oid::from_str("1111111111111111111111111111111111111111")
        .expect("must parse fixed missing OID");
    let heads = vec![BundleHead {
        oid: missing_head,
        reference: "refs/heads/main".to_string(),
    }];
    let error =
        validate_import_connectivity_for_heads(&repo, &heads, &[], ImportPath::CompatFetchFallback)
            .expect_err("missing head should fail connectivity validation");
    let text = error.to_string();
    assert!(
        text.contains("post-import connectivity check")
            && (text.contains("failed to push head") || text.contains("missing commit")),
        "connectivity diagnostics should explain missing head failure: {text}"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn connectivity_validation_accepts_missing_gitlink_target_object() {
    let repo_path = temp_bare_repo_path("connectivity-validation-gitlink-external");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let root = commit_from_content(&repo, "root", "root", &[]);
    let external_gitlink_target = git2::Oid::from_str("f2c4a90927bd468dedde0aa1bd4894e84df9458f")
        .expect("must parse synthetic gitlink oid");
    let tip = commit_with_gitlink(
        &repo,
        "tip-with-gitlink",
        "tip",
        external_gitlink_target,
        &[root],
    );
    let heads = vec![BundleHead {
        oid: tip,
        reference: "refs/heads/main".to_string(),
    }];

    assert!(
        repo.find_object(external_gitlink_target, None).is_err(),
        "fixture should keep gitlink target oid absent from receiver odb"
    );

    validate_import_connectivity_for_heads(
        &repo,
        &heads,
        &[],
        ImportPath::CompatIndexerVerifyFalse,
    )
    .expect("gitlink entries may reference external submodule commits and must not fail connectivity");

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn import_path_labels_and_fallback_flags_are_stable() {
    assert_eq!(ImportPath::StrictIndexer.label(), "strict-indexer");
    assert_eq!(
        ImportPath::CompatIndexerVerifyFalse.label(),
        "compat-indexer-verify-false"
    );
    assert_eq!(
        ImportPath::CompatFetchFallback.label(),
        "compat-fetch-fallback"
    );
    assert!(
        !ImportPath::StrictIndexer.is_compatibility_fallback(),
        "strict indexer path must not be treated as compatibility fallback"
    );
    assert!(
        ImportPath::CompatIndexerVerifyFalse.is_compatibility_fallback()
            && ImportPath::CompatFetchFallback.is_compatibility_fallback(),
        "compatibility fallback import paths must be flagged accordingly"
    );
}

#[test]
fn verbose_indexer_diagnostics_returns_plain_error_when_disabled() {
    let repo_path = temp_bare_repo_path("verbose-diagnostics-disabled");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");
    let inspection = sample_inspection_with(crate::git::BundleVersion::V2, Vec::new());

    let err = with_verbose_indexer_diagnostics(
        "indexer failed",
        false,
        &repo,
        std::path::Path::new("/tmp/sync.bundle"),
        &inspection,
        0,
        "indexer initialization (verify=true)",
    );
    assert_eq!(
        err.to_string(),
        "indexer failed",
        "non-verbose mode should preserve original error text without context expansion"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn verbose_indexer_diagnostics_reports_receiver_context_and_alternates() {
    let repo_path = temp_bare_repo_path("verbose-diagnostics-enabled");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");
    let objects_dir = repo.path().join("objects");
    let info_dir = objects_dir.join("info");
    std::fs::create_dir_all(&info_dir).expect("must create objects/info directory");

    let relative_alt = objects_dir.join("relative-alt");
    let absolute_alt = repo_path.join("absolute-alt");
    std::fs::create_dir_all(&relative_alt).expect("must create relative alternate dir");
    std::fs::create_dir_all(&absolute_alt).expect("must create absolute alternate dir");
    let alternates_content = format!(
        "# comment\n\nrelative-alt\n{}\n",
        absolute_alt.to_string_lossy()
    );
    std::fs::write(info_dir.join("alternates"), alternates_content.as_bytes())
        .expect("must write alternates file");

    let prerequisites = (0..9usize)
        .map(|idx| {
            git2::Oid::from_str(&format!("{idx:040x}"))
                .expect("must build synthetic prerequisite oid")
        })
        .collect::<Vec<_>>();
    let inspection = sample_inspection_with(crate::git::BundleVersion::V3, prerequisites);

    let err = with_verbose_indexer_diagnostics(
        "packfile is missing objects",
        true,
        &repo,
        std::path::Path::new("/tmp/sync.bundle"),
        &inspection,
        1234,
        "pack write (verify=true)",
    );
    let text = err.to_string();
    assert!(
        text.contains("unable to import bundle pack during pack write (verify=true)")
            && text.contains("verbose diagnostics:"),
        "verbose diagnostics should include stage and expanded context"
    );
    assert!(
        text.contains("bundle version: v3")
            && text.contains("receiver bare repo: true")
            && text.contains("object format: sha1"),
        "verbose diagnostics should include receiver/bundle metadata details"
    );
    assert!(
        text.contains("alternates entries:")
            && text.contains(relative_alt.to_string_lossy().as_ref())
            && text.contains(absolute_alt.to_string_lossy().as_ref()),
        "verbose diagnostics should include resolved alternates entries"
    );
    assert!(
        text.contains("missing prerequisite objects in receiver odb: 9/9")
            && text.contains(", ..."),
        "verbose diagnostics should include missing-prerequisite count and truncation marker"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn format_merge_policy_diagnostics_covers_clean_unknown_and_missing_checks() {
    let target_a = git2::Oid::from_str("1111111111111111111111111111111111111111")
        .expect("must parse target oid A");
    let target_b = git2::Oid::from_str("2222222222222222222222222222222222222222")
        .expect("must parse target oid B");
    let target_c = git2::Oid::from_str("3333333333333333333333333333333333333333")
        .expect("must parse target oid C");
    let incoming_a = git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .expect("must parse incoming oid A");
    let incoming_b = git2::Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .expect("must parse incoming oid B");
    let incoming_c = git2::Oid::from_str("cccccccccccccccccccccccccccccccccccccccc")
        .expect("must parse incoming oid C");

    let row_a = ReceivePlanEntry {
        target_ref: "refs/heads/a".to_string(),
        target_oid: Some(target_a),
        target_commit_oid: Some(target_a),
        incoming_oid: incoming_a,
        merge_base_oid: None,
        preserved_incoming_ref: "refs/sync/incoming/id/heads/a".to_string(),
        status: ReceivePlanStatus::DivergedMergeRequired,
    };
    let row_b = ReceivePlanEntry {
        target_ref: "refs/heads/b".to_string(),
        target_oid: Some(target_b),
        target_commit_oid: Some(target_b),
        incoming_oid: incoming_b,
        merge_base_oid: None,
        preserved_incoming_ref: "refs/sync/incoming/id/heads/b".to_string(),
        status: ReceivePlanStatus::DivergedMergeRequired,
    };
    let row_c = ReceivePlanEntry {
        target_ref: "refs/heads/c".to_string(),
        target_oid: Some(target_c),
        target_commit_oid: Some(target_c),
        incoming_oid: incoming_c,
        merge_base_oid: None,
        preserved_incoming_ref: "refs/sync/incoming/id/heads/c".to_string(),
        status: ReceivePlanStatus::DivergedMergeRequired,
    };
    let checks = vec![
        ReceiveMergeabilityCheck {
            target_ref: row_a.target_ref.clone(),
            target_oid: row_a.target_oid,
            target_summary: None,
            incoming_oid: row_a.incoming_oid,
            incoming_summary: None,
            merge_base_oid: row_a.merge_base_oid,
            merge_base_summary: None,
            status: ReceiveMergeabilityStatus::Unknown,
            detail: Some("simulation failed".to_string()),
            conflict_paths: vec!["src/lib.rs".to_string()],
        },
        ReceiveMergeabilityCheck {
            target_ref: row_b.target_ref.clone(),
            target_oid: row_b.target_oid,
            target_summary: None,
            incoming_oid: row_b.incoming_oid,
            incoming_summary: None,
            merge_base_oid: row_b.merge_base_oid,
            merge_base_summary: None,
            status: ReceiveMergeabilityStatus::Clean,
            detail: Some("clean".to_string()),
            conflict_paths: Vec::new(),
        },
    ];

    let diagnostics = format_merge_policy_diagnostics(&[&row_a, &row_b, &row_c], &checks);
    assert!(
        diagnostics.contains("reason: mergeability check failed")
            && diagnostics.contains("reason: mergeability precheck did not pass")
            && diagnostics.contains("reason: mergeability check missing"),
        "merge-policy diagnostics should cover unknown, clean-but-blocked, and missing-check reasons"
    );
    assert!(
        diagnostics.contains("conflict files:")
            && diagnostics.contains("src/lib.rs")
            && diagnostics.contains("detail: simulation failed"),
        "merge-policy diagnostics should include conflict paths and optional detail lines"
    );
}

#[test]
fn resolve_reference_target_resolves_symbolic_references() {
    let repo_path = temp_bare_repo_path("resolve-symbolic-ref");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");
    let main_oid = commit_from_content(&repo, "main", "main", &[]);
    repo.reference("refs/heads/main", main_oid, true, "seed main ref")
        .expect("must seed main ref");
    repo.reference_symbolic(
        "refs/heads/current",
        "refs/heads/main",
        true,
        "seed symbolic ref",
    )
    .expect("must seed symbolic ref");

    let resolved = resolve_reference_target(&repo, "refs/heads/current")
        .expect("symbolic ref resolution should succeed");
    assert_eq!(
        resolved,
        Some(main_oid),
        "symbolic reference should resolve to direct head target"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn compute_receive_plan_peels_annotated_tag_targets_for_commit_comparison() {
    let repo_path = temp_bare_repo_path("compute-plan-annotated-tag");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let base_oid = commit_from_content(&repo, "base", "base", &[]);
    let tip_oid = commit_from_content(&repo, "tip", "tip", &[base_oid]);
    let tip_object = repo
        .find_object(tip_oid, Some(git2::ObjectType::Commit))
        .expect("must resolve tip commit object");
    let signature = git2::Signature::now("Test User", "test@example.com")
        .expect("must construct test signature");
    let tag_oid = repo
        .tag("sync/tip", &tip_object, &signature, "annotated tip", false)
        .expect("must create annotated tag");

    let incoming_refs = vec![IncomingHeadRef {
        target_ref: "refs/tags/sync/tip".to_string(),
        incoming_ref: "refs/sync/incoming/test/tags/sync/tip".to_string(),
        incoming_oid: tip_oid,
    }];

    let plan = compute_receive_plan(&repo, &incoming_refs)
        .expect("annotated-tag target should be peeled for preflight commit checks");
    assert_eq!(plan.len(), 1, "fixture should yield one preflight plan row");
    let row = &plan[0];
    assert_eq!(
        row.status,
        ReceivePlanStatus::AlreadyPresent,
        "annotated-tag refs that peel to incoming commit should be treated as already present"
    );
    assert_eq!(
        row.target_oid,
        Some(tag_oid),
        "plan should preserve raw tag-object oid for safe CAS update checks"
    );
    assert_eq!(
        row.target_commit_oid,
        Some(tip_oid),
        "plan should also capture peeled commit oid for ancestry checks"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn bundle_fetch_remote_candidates_resolve_relative_paths_from_current_dir() {
    let repo_path = temp_bare_repo_path("bundle-fetch-relative-path");
    std::fs::create_dir_all(&repo_path).expect("must create temp directory");
    let bundle_path = repo_path.join("relative.bundle");
    std::fs::write(&bundle_path, b"placeholder").expect("must write relative bundle fixture");

    let previous_dir = std::env::current_dir().expect("must read current dir");
    std::env::set_current_dir(&repo_path).expect("must switch into temp fixture directory");
    let candidates = bundle_fetch_remote_candidates(std::path::Path::new("relative.bundle"))
        .expect("relative bundle input should resolve against current directory");
    std::env::set_current_dir(previous_dir).expect("must restore previous current dir");

    assert_eq!(
        candidates.len(),
        2,
        "relative bundle input should still produce path and file:// candidates"
    );
    assert!(
        candidates[0].ends_with("relative.bundle") && candidates[1].starts_with("file://"),
        "relative bundle candidates should include resolved absolute path and file:// URL"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn fetch_fallback_requires_at_least_one_head() {
    let repo_path = temp_bare_repo_path("fetch-fallback-empty-heads");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");

    let error = import_bundle_pack_with_libgit2_fetch(
        &repo,
        std::path::Path::new("/tmp/missing.bundle"),
        &[],
        "bundle-id",
    )
    .expect_err("fetch fallback should reject empty advertised-head set");
    assert!(
        error
            .to_string()
            .contains("bundle fetch fallback requires at least one advertised head"),
        "error should explain empty-head fallback precondition"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn fetch_fallback_reports_candidate_failures_for_missing_bundle_path() {
    let repo_path = temp_bare_repo_path("fetch-fallback-missing-bundle");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");
    let heads = vec![BundleHead {
        oid: git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("must parse synthetic oid"),
        reference: "refs/heads/main".to_string(),
    }];

    let missing_bundle = repo_path.join("missing.bundle");
    let error = import_bundle_pack_with_libgit2_fetch(&repo, &missing_bundle, &heads, "bundle-id")
        .expect_err("missing bundle path should fail libgit2 fetch fallback");
    let text = error.to_string();
    assert!(
        text.contains("libgit2 bundle fetch fallback failed for all URL candidates")
            && text.contains("fetch failed"),
        "fallback diagnostics should summarize per-candidate fetch failures"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn cleanup_temporary_fetch_refs_deletes_existing_and_ignores_missing_refs() {
    let repo_path = temp_bare_repo_path("cleanup-fetch-staging-refs");
    std::fs::create_dir_all(&repo_path).expect("must create repo path");
    let repo = git2::Repository::init_bare(&repo_path).expect("must init bare repo");
    let oid = commit_from_content(&repo, "staging", "staging", &[]);
    repo.reference(
        "refs/sync/fetch-staging/test/heads/main",
        oid,
        true,
        "seed temporary staging ref",
    )
    .expect("must create staging ref");

    let refs = [
        "refs/sync/fetch-staging/test/heads/main".to_string(),
        "refs/sync/fetch-staging/test/heads/missing".to_string(),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    cleanup_temporary_fetch_refs(&repo, &refs)
        .expect("cleanup should delete existing refs and ignore missing ones");

    assert_eq!(
        resolve_reference_target(&repo, "refs/sync/fetch-staging/test/heads/main")
            .expect("staging ref lookup should succeed"),
        None,
        "cleanup should delete present staging refs"
    );

    let _ = std::fs::remove_dir_all(repo_path);
}
