use super::support::*;
use super::*;
use std::path::PathBuf;

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
