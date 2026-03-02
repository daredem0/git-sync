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
