// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Tests for receive behavior and invariants.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::support::*;
use super::*;
use std::path::PathBuf;

fn commit_from_entries(
    repo: &git2::Repository,
    message: &str,
    entries: &[(&str, &[u8], i32)],
    parent_oids: &[git2::Oid],
) -> git2::Oid {
    let mut builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content, mode) in entries {
        let blob_id = repo.blob(content).expect("must create blob object");
        builder
            .insert(*path, blob_id, *mode)
            .expect("must insert tree entry");
    }
    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must resolve written tree");

    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent commit"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create sig");

    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

fn create_symlink_bundle_fixture(
    suffix: &str,
) -> (PathBuf, CreateBundleResult, git2::Oid, git2::Oid) {
    let repo_dir = temp_repo_dir(suffix);
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_entries(
        &source_repo,
        "base commit",
        &[("f.txt", b"base content\n", 0o100644)],
        &[],
    );
    let tip_commit_id = commit_from_entries(
        &source_repo,
        "tip commit adds symlink",
        &[
            ("f.txt", b"base content\n", 0o100644),
            ("link-to-f", b"f.txt", 0o120000),
        ],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed for symlink fixture");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose artifacts");

    (repo_dir, bundle_result, base_commit_id, tip_commit_id)
}

fn create_binary_bundle_fixture(
    suffix: &str,
) -> (PathBuf, CreateBundleResult, git2::Oid, git2::Oid) {
    let repo_dir = temp_repo_dir(suffix);
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_entries(
        &source_repo,
        "base commit",
        &[("payload.bin", b"\x00\x01\x02\x03\x04", 0o100644)],
        &[],
    );
    let tip_commit_id = commit_from_entries(
        &source_repo,
        "tip commit modifies binary",
        &[("payload.bin", b"\x00\x01\x02\x03\x05", 0o100644)],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed for binary fixture");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose artifacts");

    (repo_dir, bundle_result, base_commit_id, tip_commit_id)
}

fn find_incoming_head_ref_target(
    repo: &git2::Repository,
    head_reference: &str,
) -> Option<(String, git2::Oid)> {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    let expected_tail = format!("/{suffix}");
    let mut refs = repo.references().ok()?;
    while let Some(reference_result) = refs.next() {
        let reference = reference_result.ok()?;
        let Some(name) = reference.name() else {
            continue;
        };
        if !name.starts_with("refs/sync/incoming/") {
            continue;
        }
        if !name.ends_with(&expected_tail) {
            continue;
        }
        if let Some(target) = reference.target() {
            return Some((name.to_string(), target));
        }
    }
    None
}

fn find_incoming_head_branch_target(
    repo: &git2::Repository,
    head_reference: &str,
) -> Option<(String, git2::Oid)> {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    let expected_tail = format!("/{suffix}");
    let mut refs = repo.references().ok()?;
    while let Some(reference_result) = refs.next() {
        let reference = reference_result.ok()?;
        let Some(name) = reference.name() else {
            continue;
        };
        if !name.starts_with("refs/heads/incoming/") {
            continue;
        }
        if !name.ends_with(&expected_tail) {
            continue;
        }
        if let Some(target) = reference.target() {
            return Some((name.to_string(), target));
        }
    }
    None
}

// Focus: receive workflow correctness, prerequisite handling, idempotency, and ref-application checks.
// Verifies that receive_bundle_input imports a zip-packaged bundle and updates exported head refs when prerequisites exist.
#[test]
fn receive_bundle_input_imports_zip_bundle_and_updates_heads_when_prerequisite_exists() {
    let repo_dir = temp_repo_dir("receive-bundle-success");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-success-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("receive_bundle_input should succeed when prerequisites are present");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "receive should import exactly one exported head for this bundle"
    );
    assert_eq!(
        receive_result.imported_heads[0].reference, "refs/heads/tip",
        "receive should update the exported tip ref"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "receive should point exported tip ref at the imported tip commit"
    );
    assert_eq!(
        receive_result.preflight_plan.len(),
        1,
        "single-head fixture should produce one preflight plan row"
    );
    assert_eq!(
        receive_result.preflight_plan[0].status,
        ReceivePlanStatus::TargetMissing,
        "first receive should classify missing target refs as target_missing"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist after receive");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "receiver tip ref should match imported tip commit"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receiving the same bundle package twice is idempotent and does not create additional packfiles.
#[test]
fn receive_bundle_input_is_idempotent_when_same_package_is_applied_twice() {
    let repo_dir = temp_repo_dir("receive-bundle-idempotent");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-idempotent-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let first_receive = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("first receive should succeed");
    assert_eq!(
        first_receive.imported_heads.len(),
        1,
        "fixture range should import exactly one head"
    );

    let pack_dir = receiver_dir.join("objects").join("pack");
    let mut pack_entries_after_first = std::fs::read_dir(&pack_dir)
        .expect("receiver pack dir should exist")
        .map(|entry| {
            entry
                .expect("pack dir entry should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    pack_entries_after_first.sort();

    let second_receive = receive_bundle_input(&bundle_result.archive_path, &receiver_dir)
        .expect("second receive should also succeed");
    assert_eq!(
        second_receive.imported_heads, first_receive.imported_heads,
        "idempotent receive should report the same imported heads"
    );
    assert_eq!(
        second_receive.preflight_plan.len(),
        1,
        "single-head fixture should produce one preflight row"
    );
    assert_eq!(
        second_receive.preflight_plan[0].status,
        ReceivePlanStatus::AlreadyPresent,
        "second receive should classify unchanged refs as already_present"
    );

    let mut pack_entries_after_second = std::fs::read_dir(&pack_dir)
        .expect("receiver pack dir should still exist")
        .map(|entry| {
            entry
                .expect("pack dir entry should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    pack_entries_after_second.sort();
    assert_eq!(
        pack_entries_after_second, pack_entries_after_first,
        "second receive should not add new pack files when package is already applied"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "tip ref should remain pinned to the imported tip commit after repeated receive"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that fast-forward-only integration fails for diverged targets, keeps target refs unchanged,
// and still preserves incoming heads under refs/sync/incoming/<bundle-id>/...
#[test]
fn receive_fast_forward_only_rejects_diverged_target_and_preserves_incoming_namespace_refs() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-fast-forward-only-diverged", false);
    let receiver_dir = temp_repo_dir("receive-fast-forward-only-diverged-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");

    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let diverged_tip_oid = commit_from_files(
        &receiver_repo,
        "receiver diverged tip",
        &[
            ("f.txt", "receiver-side change"),
            ("local.txt", "local only"),
        ],
        &[base_commit_id],
    );
    receiver_repo
        .reference(
            "refs/heads/tip",
            diverged_tip_oid,
            true,
            "seed diverged target",
        )
        .expect("must seed diverged receiver tip ref");

    let receive_result = receive_bundle_input_with_options_and_policy(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: false,
        },
        ReceiveIntegratePolicy::FastForwardOnly,
    );
    assert!(
        receive_result.is_err(),
        "fast-forward-only receive must fail when target ref has diverged"
    );
    let error_text = receive_result
        .expect_err("result must be error")
        .to_string();
    assert!(
        error_text.contains("diverged (non-fast-forward)"),
        "failure diagnostics should include non-fast-forward reason"
    );
    assert!(
        error_text.contains("next-step: merge required"),
        "failure diagnostics should include merge-required guidance"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("target tip ref should still exist");
    assert_eq!(
        tip_ref.target(),
        Some(diverged_tip_oid),
        "diverged target tip must remain unchanged after failed fast-forward-only receive"
    );

    let incoming_ref = find_incoming_head_ref_target(&receiver_repo, "refs/heads/tip")
        .expect("incoming namespace ref for tip should be created even on divergence");
    assert_eq!(
        incoming_ref.1, tip_commit_id,
        "incoming namespace ref must point at imported bundle head oid"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that create-refs-only integration never updates target refs and still writes incoming namespace refs.
#[test]
fn receive_create_refs_only_preserves_target_ref_even_when_fast_forward_is_possible() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-create-refs-only", false);
    let receiver_dir = temp_repo_dir("receive-create-refs-only-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");

    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    receiver_repo
        .reference(
            "refs/heads/tip",
            base_commit_id,
            true,
            "seed receiver tip at base",
        )
        .expect("must create receiver tip reference");

    let receive_result = receive_bundle_input_with_options_and_policy(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: false,
        },
        ReceiveIntegratePolicy::CreateRefsOnly,
    )
    .expect("create-refs-only receive should succeed");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should report one imported head"
    );
    assert_eq!(
        receive_result.preflight_plan.len(),
        1,
        "single-head fixture should produce one preflight row"
    );
    assert_eq!(
        receive_result.preflight_plan[0].status,
        ReceivePlanStatus::FastForwardOk,
        "create-refs-only should still classify this target as fast_forward_ok"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("target tip ref should still exist");
    assert_eq!(
        tip_ref.target(),
        Some(base_commit_id),
        "create-refs-only policy must not update existing target refs"
    );

    let incoming_ref = find_incoming_head_ref_target(&receiver_repo, "refs/heads/tip")
        .expect("incoming namespace ref for tip should be created");
    assert_eq!(
        incoming_ref.1, tip_commit_id,
        "incoming namespace ref must point at imported bundle head oid"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that incoming_as_branches additionally mirrors imported heads under refs/heads/incoming/<bundle-id>/...
#[test]
fn receive_incoming_as_branches_writes_branch_mirrors() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-incoming-as-branches", false);
    let receiver_dir = temp_repo_dir("receive-incoming-as-branches-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");

    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    receiver_repo
        .reference(
            "refs/heads/tip",
            base_commit_id,
            true,
            "seed receiver tip at base",
        )
        .expect("must create receiver tip reference");

    let receive_result = receive_bundle_input_with_options_policy_and_branch_mirror(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: false,
        },
        ReceiveIntegratePolicy::CreateRefsOnly,
        true,
    )
    .expect("receive with incoming_as_branches should succeed");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should report one imported head"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let incoming_namespace = find_incoming_head_ref_target(&receiver_repo, "refs/heads/tip")
        .expect("incoming namespace ref for tip should be created");
    assert_eq!(
        incoming_namespace.1, tip_commit_id,
        "incoming namespace ref must point at imported bundle head oid"
    );

    let incoming_branch = find_incoming_head_branch_target(&receiver_repo, "refs/heads/tip")
        .expect("incoming branch mirror ref for tip should be created");
    assert_eq!(
        incoming_branch.1, tip_commit_id,
        "incoming branch mirror ref must point at imported bundle head oid"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input fails when the receiver lacks prerequisite objects required by the bundle pack.
#[test]
fn receive_bundle_input_fails_when_prerequisite_is_missing() {
    let repo_dir = temp_repo_dir("receive-bundle-missing-prereq");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&bundle_result).expect("must remove loose bundle artifacts");

    let receiver_dir = temp_repo_dir("receive-bundle-missing-prereq-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let receive_result = receive_bundle_input(&bundle_result.archive_path, &receiver_dir);
    assert!(
        receive_result.is_err(),
        "receive should fail when receiver does not have prerequisite commit history"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed receive should not create tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input_with_options rejects imports when metadata verification is enabled and sidecar content is tampered.
#[test]
fn receive_bundle_input_with_options_rejects_tampered_metadata_when_verification_enabled() {
    let repo_dir = temp_repo_dir("receive-bundle-verify-metadata-fail");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read generated metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["bundle_sha256"] =
        serde_json::Value::String("00000000000000000000000000000000".to_string());
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let receiver_dir = temp_repo_dir("receive-bundle-verify-metadata-fail-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input_with_options(
        &bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: true,
            dry_run: false,
        },
    );
    assert!(
        receive_result.is_err(),
        "receive with verify_metadata=true must reject tampered metadata"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed receive should not update tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input_with_options can import when metadata verification is disabled even if metadata sidecar was tampered.
#[test]
fn receive_bundle_input_with_options_allows_tampered_metadata_when_verification_disabled() {
    let repo_dir = temp_repo_dir("receive-bundle-verify-metadata-disabled");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read generated metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["bundle_sha256"] =
        serde_json::Value::String("ffffffffffffffffffffffffffffffff".to_string());
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let receiver_dir = temp_repo_dir("receive-bundle-verify-metadata-disabled-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input_with_options(
        &bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: false,
        },
    )
    .expect("receive with verify_metadata=false should not block on tampered metadata");

    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "receive should import one head from this bundle range"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "imported head must point to source tip commit"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist after receive");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "receiver tip ref should match source tip commit"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive_bundle_input rejects bundles that do not declare any head entries.
#[test]
fn receive_bundle_input_rejects_bundle_without_heads() {
    let work_dir = temp_repo_dir("receive-no-heads");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let bundle_path = work_dir.join("empty-heads.bundle");
    std::fs::write(&bundle_path, b"# v2 git bundle\n\nPACK").expect("must write bundle file");

    let receiver_dir = temp_repo_dir("receive-no-heads-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    let result = receive_bundle_input(&bundle_path, &receiver_dir);
    assert!(
        result.is_err(),
        "receive must reject bundle payloads without any head entries"
    );

    let _ = std::fs::remove_dir_all(work_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that is_head_already_applied resolves symbolic refs (such as HEAD) before comparing target OIDs.
#[test]
fn is_head_already_applied_resolves_symbolic_reference_targets() {
    let repo_dir = temp_repo_dir("receive-symbolic-head");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "commit", &[("f.txt", "content")], &[]);
    repo.reference("refs/heads/main", commit_id, true, "set main")
        .expect("must create main ref");
    repo.set_head("refs/heads/main")
        .expect("must set HEAD symbolic ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_id,
            reference: "HEAD".to_string(),
        },
    )
    .expect("symbolic reference resolution should not fail");
    assert!(
        applied,
        "symbolic HEAD should resolve to refs/heads/main and match commit id"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that is_head_already_applied returns false when symbolic refs cannot be resolved.
#[test]
fn is_head_already_applied_returns_false_for_broken_symbolic_reference() {
    let repo_dir = temp_repo_dir("receive-broken-symbolic");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let commit_id = commit_from_files(&repo, "commit", &[("f.txt", "content")], &[]);
    repo.reference_symbolic(
        "refs/heads/broken-symbolic",
        "refs/heads/does-not-exist",
        true,
        "create broken symbolic ref",
    )
    .expect("must create broken symbolic ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_id,
            reference: "refs/heads/broken-symbolic".to_string(),
        },
    )
    .expect("broken symbolic ref lookup should not return hard error");
    assert!(
        !applied,
        "broken symbolic refs should not be treated as already applied"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that receive with verify_metadata=true supports plain .bundle input paths (non-zip) on the success path.
#[test]
fn receive_bundle_input_with_options_accepts_plain_bundle_with_verification() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-plain-verify", false);
    let receiver_dir = temp_repo_dir("receive-plain-verify-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    let receive_result = receive_bundle_input_with_options(
        &bundle_result.bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: true,
            dry_run: false,
        },
    )
    .expect("receive should succeed with plain bundle input when metadata is valid");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should import exactly one head"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let base_ref = receiver_repo
        .find_reference("refs/heads/base")
        .expect("base prerequisite ref should exist");
    assert_eq!(
        base_ref.target(),
        Some(base_commit_id),
        "base ref should match"
    );
    let tip_ref = receiver_repo
        .find_reference("refs/heads/tip")
        .expect("tip ref should exist");
    assert_eq!(
        tip_ref.target(),
        Some(tip_commit_id),
        "tip ref should match"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that dry-run receive reports imported heads but does not write refs or packfiles.
#[test]
fn receive_bundle_input_with_options_dry_run_does_not_modify_receiver_repo() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-dry-run", false);
    let receiver_dir = temp_repo_dir("receive-dry-run-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    let pack_dir = receiver_dir.join("objects").join("pack");
    let mut pack_entries_before = match std::fs::read_dir(&pack_dir) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("pack dir entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("must list receiver pack dir entries before dry-run: {err}"),
    };
    pack_entries_before.sort();

    let receive_result = receive_bundle_input_with_options(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    )
    .expect("dry-run receive should succeed");
    assert!(
        receive_result.can_apply_without_conflicts,
        "dry-run should confirm package can be applied without conflicts"
    );
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture bundle should still report one imported head during dry-run"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "dry-run should report the same head oid that would be imported"
    );
    assert!(
        receive_result
            .line_stats
            .iter()
            .any(|stat| stat.path == "f.txt" && stat.additions > 0 && stat.deletions > 0),
        "dry-run should include modified file line stats"
    );
    assert!(
        receive_result
            .line_stats
            .iter()
            .any(|stat| stat.path == "new.txt" && stat.additions > 0),
        "dry-run should include added file line stats"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let base_ref = receiver_repo
        .find_reference("refs/heads/base")
        .expect("base prerequisite ref should exist");
    assert_eq!(
        base_ref.target(),
        Some(base_commit_id),
        "base prerequisite ref should remain unchanged"
    );
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "dry-run must not create or update tip refs"
    );

    let mut pack_entries_after = match std::fs::read_dir(&pack_dir) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("pack dir entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("must list receiver pack dir entries after dry-run: {err}"),
    };
    pack_entries_after.sort();
    assert_eq!(
        pack_entries_after, pack_entries_before,
        "dry-run must not add new packfiles to receiver object database"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that dry-run receive still enforces metadata verification when requested.
#[test]
fn receive_bundle_input_with_options_dry_run_rejects_tampered_metadata_when_verification_enabled() {
    let repo_dir = temp_repo_dir("receive-dry-run-verify-metadata-fail");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let source_repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(
        &source_repo,
        "base commit",
        &[("f.txt", "base content")],
        &[],
    );
    let tip_commit_id = commit_from_files(
        &source_repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "extra")],
        &[base_commit_id],
    );
    source_repo
        .reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    source_repo
        .reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read generated metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["bundle_sha256"] =
        serde_json::Value::String("00000000000000000000000000000000".to_string());
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let receiver_dir = temp_repo_dir("receive-dry-run-verify-metadata-fail-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite into receiver");

    let receive_result = receive_bundle_input_with_options(
        &bundle_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: true,
            dry_run: true,
        },
    );
    assert!(
        receive_result.is_err(),
        "dry-run receive with verify_metadata=true must reject tampered metadata"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    assert!(
        receiver_repo.find_reference("refs/heads/tip").is_err(),
        "failed dry-run must not create tip ref"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive rejects bundles whose header head OID is missing after pack import.
#[test]
fn receive_bundle_input_rejects_when_head_oid_is_missing_after_import() {
    let (repo_dir, bundle_result, base_commit_id, _) =
        create_linear_bundle_fixture("receive-missing-head-commit", false);
    let bundle_bytes =
        std::fs::read(&bundle_result.bundle_path).expect("must read generated bundle bytes");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain pack payload");
    let pack_data = &bundle_bytes[pack_offset..];

    let fake_head_oid = "ffffffffffffffffffffffffffffffffffffffff";
    let tampered_header =
        format!("# v2 git bundle\n-{base_commit_id}\n{fake_head_oid} refs/heads/tip\n\n");
    let mut tampered_bytes = tampered_header.into_bytes();
    tampered_bytes.extend_from_slice(pack_data);
    std::fs::write(&bundle_result.bundle_path, tampered_bytes)
        .expect("must write tampered bundle header");

    let receiver_dir = temp_repo_dir("receive-missing-head-commit-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let receive_result = receive_bundle_input(&bundle_result.bundle_path, &receiver_dir);
    assert!(
        receive_result.is_err(),
        "receive should fail when declared head oid is missing after pack import"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive accepts valid bundles even when the header text contains "PACK" in a ref name.
#[test]
fn receive_bundle_input_accepts_bundle_when_header_contains_pack_text() {
    let (repo_dir, bundle_result, base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-header-pack-text", false);
    let bundle_path = bundle_result.bundle_path.clone();
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read generated bundle bytes");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain pack payload");
    let header_text = String::from_utf8(bundle_bytes[..pack_offset].to_vec())
        .expect("bundle header bytes should be utf-8");
    let rewritten_header_text = header_text.replace(" refs/heads/tip\n", " refs/heads/PACK-tip\n");
    assert_ne!(
        rewritten_header_text, header_text,
        "fixture header rewrite should modify the tip reference line"
    );
    let mut rewritten_bytes = rewritten_header_text.into_bytes();
    rewritten_bytes.extend_from_slice(&bundle_bytes[pack_offset..]);
    std::fs::write(&bundle_path, rewritten_bytes).expect("must write rewritten header bundle");

    let receiver_dir = temp_repo_dir("receive-header-pack-text-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let receive_result = receive_bundle_input(&bundle_path, &receiver_dir)
        .expect("receive should succeed for valid bundle with PACK text in header");
    assert_eq!(
        receive_result.imported_heads.len(),
        1,
        "fixture range should import exactly one head"
    );
    assert_eq!(
        receive_result.imported_heads[0].reference, "refs/heads/PACK-tip",
        "imported reference should preserve rewritten header reference name"
    );
    assert_eq!(
        receive_result.imported_heads[0].oid, tip_commit_id,
        "imported reference target should remain the original tip commit"
    );

    let receiver_repo = git2::Repository::open_bare(&receiver_dir).expect("must open receiver");
    let base_ref = receiver_repo
        .find_reference("refs/heads/base")
        .expect("base prerequisite ref should exist");
    assert_eq!(
        base_ref.target(),
        Some(base_commit_id),
        "base ref should remain the fetched prerequisite commit"
    );
    let pack_tip_ref = receiver_repo
        .find_reference("refs/heads/PACK-tip")
        .expect("rewritten PACK-tip ref should exist after receive");
    assert_eq!(
        pack_tip_ref.target(),
        Some(tip_commit_id),
        "rewritten PACK-tip ref target should match imported tip commit"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that receive rejects bundles where bytes appear between the header terminator and the PACK payload.
#[test]
fn receive_bundle_input_rejects_bundle_with_gap_before_pack_payload() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("receive-gap-before-pack", false);
    let bundle_path = bundle_result.bundle_path.clone();
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read generated bundle bytes");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain pack payload");
    let mut tampered_bytes = bundle_bytes[..pack_offset].to_vec();
    tampered_bytes.extend_from_slice(b"GARBAGE-BETWEEN-HEADER-AND-PAYLOAD\n");
    tampered_bytes.extend_from_slice(&bundle_bytes[pack_offset..]);
    std::fs::write(&bundle_path, tampered_bytes).expect("must write tampered bundle bytes");

    let receiver_dir = temp_repo_dir("receive-gap-before-pack-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let receive_result = receive_bundle_input(&bundle_path, &receiver_dir);
    assert!(
        receive_result.is_err(),
        "receive must reject bundles where PACK does not start immediately after header terminator"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that is_head_already_applied returns false when the current ref target OID differs from requested head OID.
#[test]
fn is_head_already_applied_returns_false_when_ref_target_differs() {
    let repo_dir = temp_repo_dir("head-applied-target-differs");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init repo");

    let commit_a = commit_from_files(&repo, "A", &[("f.txt", "a")], &[]);
    let commit_b = commit_from_files(&repo, "B", &[("f.txt", "b")], &[commit_a]);
    repo.reference("refs/heads/main", commit_a, true, "set main")
        .expect("must create main ref");

    let applied = is_head_already_applied(
        &repo,
        &BundleHead {
            oid: commit_b,
            reference: "refs/heads/main".to_string(),
        },
    )
    .expect("ref comparison should not fail");
    assert!(
        !applied,
        "head should not be treated as applied when target oid differs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that head-scoped commit-audit entries expose author/committer identity fields needed by the commit-detail TUI page.
#[test]
fn collect_commit_audit_entries_includes_author_and_committer_identity() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-audit-identities", false);

    let receiver_dir = temp_repo_dir("receive-commit-audit-identities-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let head_entries =
        collect_head_audit_entries_for_bundle_input(&bundle_result.archive_path, &receiver_dir)
            .expect("must collect head-scoped commit-audit entries from package");
    assert_eq!(
        head_entries.len(),
        1,
        "fixture range should produce one head entry"
    );
    let entry = &head_entries[0].commits[0];
    assert_eq!(
        entry.commit_id, tip_commit_id,
        "entry commit id should match the tip commit in this fixture range"
    );
    assert_eq!(
        entry.author.name, "Test User",
        "author identity should expose the commit author name"
    );
    assert_eq!(
        entry.author.email, "test@example.com",
        "author identity should expose the commit author email"
    );
    assert_eq!(
        entry.committer.name, "Test User",
        "committer identity should expose the commit committer name"
    );
    assert_eq!(
        entry.committer.email, "test@example.com",
        "committer identity should expose the commit committer email"
    );
    assert!(
        entry.author.time_seconds > 0 && entry.committer.time_seconds > 0,
        "identity timestamps should contain valid unix timestamps"
    );
    assert_eq!(
        entry.author.offset_minutes, entry.committer.offset_minutes,
        "fixture uses the same timezone offset for author and committer"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that head-audit collection returns head-scoped line stats and commits for bundled history.
#[test]
fn collect_head_audit_entries_for_bundle_input_returns_head_scoped_entries() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-head-audit-entries", false);

    let receiver_dir = temp_repo_dir("receive-head-audit-entries-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let entries =
        collect_head_audit_entries_for_bundle_input(&bundle_result.archive_path, &receiver_dir)
            .expect("must collect head-audit entries from package");
    assert_eq!(
        entries.len(),
        1,
        "single-tip fixture should yield one head entry"
    );

    let head_entry = &entries[0];
    assert_eq!(
        head_entry.head.reference, "refs/heads/tip",
        "head entry should expose the imported tip reference"
    );
    assert_eq!(
        head_entry.commits.len(),
        1,
        "fixture base..tip range should contain one commit for this head"
    );
    assert_eq!(
        head_entry.commits[0].commit_id, tip_commit_id,
        "head commit entry should target the bundled tip commit"
    );
    assert!(
        head_entry
            .line_stats
            .iter()
            .any(|stat| stat.path == "f.txt"),
        "head line stats should include the changed file path from base..tip"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that head-scoped commit-audit page data can be collected from a plain bundle even when the metadata sidecar is absent.
#[test]
fn collect_commit_audit_entries_for_plain_bundle_succeeds_without_metadata_sidecar() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-audit-no-sidecar", false);
    let metadata_path = PathBuf::from(format!(
        "{}.caudit.json",
        bundle_result.bundle_path.display()
    ));
    std::fs::remove_file(&metadata_path).expect("must remove metadata sidecar");

    let receiver_dir = temp_repo_dir("receive-commit-audit-no-sidecar-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let head_entries = collect_head_audit_entries_for_bundle_input(
        &bundle_result.bundle_path,
        &receiver_dir,
    )
    .expect(
        "must collect head-scoped commit-audit entries from plain bundle without metadata sidecar",
    );
    assert_eq!(
        head_entries.len(),
        1,
        "fixture range should produce one head entry"
    );
    assert_eq!(
        head_entries[0].commits[0].commit_id, tip_commit_id,
        "entry commit id should match the tip commit in this fixture range"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that a per-file patch can be collected for a changed file in a commit inside a bundle package.
#[test]
fn collect_commit_file_patch_for_bundle_input_returns_patch_for_changed_file() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-file-patch", false);

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let patch = collect_commit_file_patch_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
        tip_commit_id,
        "f.txt",
    )
    .expect("must collect patch for changed file");
    assert!(
        patch.contains("diff --git a/f.txt b/f.txt"),
        "patch should include diff header for the selected path"
    );
    assert!(
        patch.contains("--- a/f.txt") && patch.contains("+++ b/f.txt"),
        "patch should include file headers for the selected path"
    );
    assert!(
        patch.contains("@@"),
        "patch should include at least one hunk for modified file content"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that per-file patch collection from a plain bundle does not depend on a metadata sidecar file.
#[test]
fn collect_commit_file_patch_for_plain_bundle_succeeds_without_metadata_sidecar() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-file-patch-no-sidecar", false);
    let metadata_path = PathBuf::from(format!(
        "{}.caudit.json",
        bundle_result.bundle_path.display()
    ));
    std::fs::remove_file(&metadata_path).expect("must remove metadata sidecar");

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-no-sidecar-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let patch = collect_commit_file_patch_for_bundle_input(
        &bundle_result.bundle_path,
        &receiver_dir,
        tip_commit_id,
        "f.txt",
    )
    .expect("must collect patch for changed file from plain bundle without metadata sidecar");
    assert!(
        patch.contains("diff --git a/f.txt b/f.txt"),
        "patch should include diff header for selected path"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that per-file patch collection fails when the requested path is not changed by the selected commit.
#[test]
fn collect_commit_file_patch_for_bundle_input_rejects_missing_path() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_linear_bundle_fixture("receive-commit-file-patch-missing", false);

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-missing-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let result = collect_commit_file_patch_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
        tip_commit_id,
        "does-not-exist.txt",
    );
    assert!(
        result.is_err(),
        "collecting a patch for an unchanged path must fail"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that dry-run line statistics ignore non-text entries (for example symlinks) by reporting 0/0 line deltas.
#[test]
fn receive_bundle_input_with_options_dry_run_reports_zero_line_stats_for_symlink_changes() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_symlink_bundle_fixture("receive-dry-run-symlink-zero-stats");

    let receiver_dir = temp_repo_dir("receive-dry-run-symlink-zero-stats-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let dry_run = receive_bundle_input_with_options(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    )
    .expect("dry-run receive should succeed for symlink fixture");

    let symlink_stat = dry_run
        .line_stats
        .iter()
        .find(|stat| stat.path == "link-to-f")
        .expect("dry-run line stats should include symlink path");
    assert_eq!(
        symlink_stat.additions, 0,
        "non-text symlink changes must not report textual additions"
    );
    assert_eq!(
        symlink_stat.deletions, 0,
        "non-text symlink changes must not report textual deletions"
    );
    assert_eq!(
        dry_run.preflight_plan.len(),
        1,
        "single-head fixture should produce one preflight row"
    );
    assert_eq!(
        dry_run.preflight_plan[0].status,
        ReceivePlanStatus::TargetMissing,
        "dry-run should classify missing target ref as target_missing"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that dry-run line statistics ignore binary content deltas by reporting 0/0 line counts.
#[test]
fn receive_bundle_input_with_options_dry_run_reports_zero_line_stats_for_binary_changes() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_binary_bundle_fixture("receive-dry-run-binary-zero-stats");

    let receiver_dir = temp_repo_dir("receive-dry-run-binary-zero-stats-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let dry_run = receive_bundle_input_with_options(
        &bundle_result.archive_path,
        &receiver_dir,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    )
    .expect("dry-run receive should succeed for binary fixture");

    let binary_stat = dry_run
        .line_stats
        .iter()
        .find(|stat| stat.path == "payload.bin")
        .expect("dry-run line stats should include binary path");
    assert_eq!(
        binary_stat.additions, 0,
        "binary changes must not report textual additions"
    );
    assert_eq!(
        binary_stat.deletions, 0,
        "binary changes must not report textual deletions"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that collecting a per-file patch for a non-text entry (for example symlink) returns a clean rejection error.
#[test]
fn collect_commit_file_patch_for_bundle_input_rejects_non_text_path() {
    let (repo_dir, bundle_result, _base_commit_id, tip_commit_id) =
        create_symlink_bundle_fixture("receive-commit-file-patch-symlink");

    let receiver_dir = temp_repo_dir("receive-commit-file-patch-symlink-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver bare repo");
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch prerequisite base history");

    let result = collect_commit_file_patch_for_bundle_input(
        &bundle_result.archive_path,
        &receiver_dir,
        tip_commit_id,
        "link-to-f",
    );
    let err = result.expect_err("non-text path patch extraction must fail");
    assert!(
        err.to_string()
            .contains("textual diff unavailable for non-text path"),
        "error should explain that textual patch extraction is unavailable for non-text paths"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}
