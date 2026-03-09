// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Tests for bundle create behavior and invariants.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::support::*;
use super::*;
use std::path::PathBuf;

// Focus: bundle creation contract, generated metadata, and optional patch sidecar behavior.
// Verifies that create_bundle writes a v2 bundle with prerequisite and tip lines followed by PACK data.
#[test]
fn create_bundle_writes_valid_bundle_header_and_pack_data() {
    let repo_dir = temp_repo_dir("create-bundle-file");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed for a linear range");

    assert_eq!(
        result.from_commit_id, base_commit_id,
        "from commit in result should match resolved base ref"
    );
    assert_eq!(
        result.to_commit_id, tip_commit_id,
        "to commit in result should match resolved tip ref"
    );
    assert_eq!(
        result.tip_ref_name, "refs/heads/tip",
        "tip ref name should preserve the resolved tip reference when available"
    );

    let bytes = std::fs::read(&bundle_path).expect("must read created bundle");
    assert!(
        bytes.starts_with(b"# v2 git bundle\n"),
        "bundle should start with the v2 bundle signature line"
    );

    let header_preview = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    assert!(
        header_preview.contains(&format!("-{base_commit_id}")),
        "bundle header should contain prerequisite commit line"
    );
    assert!(
        header_preview.contains(&format!("{tip_commit_id} refs/heads/tip")),
        "bundle header should contain tip commit to ref mapping"
    );
    assert!(
        bytes.windows(4).any(|w| w == b"PACK"),
        "bundle should contain a packfile payload"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle rejects ranges where the end commit does not descend from the start commit.
#[test]
fn create_bundle_fails_when_to_commit_is_not_descendant_of_from_commit() {
    let repo_dir = temp_repo_dir("create-bundle-not-descendant");
    std::fs::create_dir_all(&repo_dir).expect("must create repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repo");

    let root_commit_id = commit_from_files(&repo, "root commit", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(
        &repo,
        "base branch commit",
        &[("f.txt", "base branch")],
        &[root_commit_id],
    );
    let tip_commit_id = commit_from_files(
        &repo,
        "diverged tip commit",
        &[("f.txt", "tip branch")],
        &[root_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path);
    assert!(
        result.is_err(),
        "create_bundle must reject non-linear ranges for deterministic incremental export"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that a created bundle can be fetched into another repo when prerequisite commits are present.
#[test]
fn create_bundle_can_be_fetched_when_prerequisite_is_present() {
    use std::io::Write as _;

    let repo_dir = temp_repo_dir("create-bundle-fetch");
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
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let receiver_dir = temp_repo_dir("create-bundle-fetch-receiver");
    std::fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
    let receiver_repo =
        git2::Repository::init_bare(&receiver_dir).expect("must init receiver repo");

    // First, seed prerequisite history into receiver (simulates receiver already having base).
    let mut source_remote = receiver_repo
        .remote_anonymous(repo_dir.to_str().expect("repo path should be utf-8"))
        .expect("must create source remote");
    source_remote
        .fetch(&["refs/heads/base:refs/heads/base"], None, None)
        .expect("must fetch base prerequisite");

    // Then apply bundle pack payload into receiver object database via Indexer.
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read created bundle");
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain PACK payload");
    let pack_data = &bundle_bytes[pack_offset..];

    let odb = receiver_repo.odb().expect("must open receiver odb");
    let mut indexer = git2::Indexer::new(
        Some(&odb),
        receiver_repo.path().join("objects").join("pack").as_path(),
        0o644,
        true,
    )
    .expect("must create indexer");
    indexer
        .write_all(pack_data)
        .expect("must write pack payload into indexer");
    indexer.commit().expect("must finalize indexed pack");

    let imported_tip = receiver_repo
        .find_commit(tip_commit_id)
        .expect("tip commit from bundle pack should be present after indexing");
    assert_eq!(
        imported_tip.id(),
        tip_commit_id,
        "imported tip commit should match original tip commit id"
    );
    assert_eq!(
        result.tip_ref_name, "refs/heads/tip",
        "result metadata should preserve exported tip ref name"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
    let _ = std::fs::remove_dir_all(receiver_dir);
}

// Verifies that create_bundle writes a .caudit.json sidecar with core audit identity fields.
#[test]
fn create_bundle_writes_caudit_metadata_file_with_core_identity_fields() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-core");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let expected_caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    assert_eq!(
        result.audit_path, expected_caudit_path,
        "create_bundle should return the generated .caudit.json path"
    );
    assert!(
        result.audit_path.exists(),
        "create_bundle should write a .caudit.json metadata file"
    );

    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");

    assert_eq!(
        metadata["schema_version"],
        serde_json::json!("1"),
        "schema version must match the initial metadata contract"
    );
    assert_eq!(
        metadata["range_from_oid"],
        serde_json::json!(base_commit_id.to_string()),
        "metadata must record the resolved from commit id"
    );
    assert_eq!(
        metadata["range_to_oid"],
        serde_json::json!(tip_commit_id.to_string()),
        "metadata must record the resolved to commit id"
    );
    assert_eq!(
        metadata["tip_ref"],
        serde_json::json!("refs/heads/tip"),
        "metadata must preserve the exported tip reference name"
    );
    assert_eq!(
        metadata["bundle_path"],
        serde_json::json!(bundle_path.display().to_string()),
        "metadata must include the bundle path used during creation"
    );
    assert_eq!(
        metadata["bundle_header_version"],
        serde_json::json!("v2"),
        "metadata should report the bundle format version"
    );
    let generated_by_username = metadata["generated_by_username"]
        .as_str()
        .expect("metadata should include generated_by_username as a string");
    assert!(
        !generated_by_username.is_empty(),
        "generated_by_username should not be empty"
    );
    let generated_by_hostname = metadata["generated_by_hostname"]
        .as_str()
        .expect("metadata should include generated_by_hostname as a string");
    assert!(
        !generated_by_hostname.is_empty(),
        "generated_by_hostname should not be empty"
    );
    let bundle_bytes = std::fs::read(&bundle_path).expect("must read generated bundle bytes");
    let expected_bundle_sha256 = sha256_hex(&bundle_bytes).expect("must hash bundle bytes");
    assert_eq!(
        metadata["bundle_size_bytes"],
        serde_json::json!(bundle_bytes.len() as u64),
        "metadata must report the exact bundle byte length"
    );
    let bundle_sha256 = metadata["bundle_sha256"]
        .as_str()
        .expect("bundle_sha256 should be present as a string");
    assert_eq!(
        bundle_sha256, expected_bundle_sha256,
        "metadata bundle_sha256 must match the actual bundle file content digest"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle writes a .zip archive containing at least the bundle and metadata files.
#[test]
fn create_bundle_writes_archive_with_bundle_and_metadata_entries() {
    let repo_dir = temp_repo_dir("create-bundle-archive");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let expected_archive_path = PathBuf::from(format!("{}.zip", bundle_path.display()));
    assert_eq!(
        result.archive_path, expected_archive_path,
        "create_bundle should return a deterministic archive path next to the bundle"
    );
    assert!(
        result.archive_path.exists(),
        "create_bundle should write a .zip archive"
    );

    let archive_bytes =
        std::fs::read(&result.archive_path).expect("must read generated archive bytes");
    assert!(
        archive_bytes.starts_with(b"PK\x03\x04"),
        "archive should use ZIP local-header signature"
    );
    let archive_text = String::from_utf8_lossy(&archive_bytes);
    assert!(
        archive_text.contains("range.bundle"),
        "archive should contain the bundle file entry name"
    );
    assert!(
        archive_text.contains("range.bundle.caudit.json"),
        "archive should contain the metadata file entry name"
    );
    assert!(
        !archive_text.contains("range.bundle.caudit.patch"),
        "default archive should not include patch sidecar entry when patches are disabled"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle metadata stays compact by omitting inline per-file patch text by default.
#[test]
fn create_bundle_caudit_omits_inline_patch_details_by_default() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-patch");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "other")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");

    let changed_files = metadata["changed_files"]
        .as_array()
        .expect("changed_files should be serialized as an array");
    let modified_f_txt = changed_files
        .iter()
        .find(|entry| {
            entry["status"] == serde_json::json!("M") && entry["path"] == serde_json::json!("f.txt")
        })
        .expect("changed_files should include f.txt as a modified entry");

    assert_eq!(
        modified_f_txt["is_binary"],
        serde_json::json!(false),
        "text file changes should be marked as non-binary"
    );
    assert!(
        modified_f_txt.get("patch").is_none(),
        "compact metadata should not embed full unified patch text per changed file"
    );
    assert!(
        metadata["patch_sidecar"].is_null(),
        "compact metadata should not include a patch sidecar descriptor unless explicitly requested"
    );
    assert!(
        result.patch_audit_path.is_none(),
        "create_bundle result should not expose a patch sidecar path by default"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that create_bundle can optionally write a patch sidecar and reference it from metadata.
#[test]
fn create_bundle_with_patch_sidecar_writes_and_references_sidecar() {
    let repo_dir = temp_repo_dir("create-bundle-caudit-sidecar");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("g.txt", "other")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle_with_options(
        &repo_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
        CreateBundleOptions {
            include_patch_sidecar: true,
            assume_present_revs: Vec::new(),
        },
    )
    .expect("create_bundle_with_options should succeed with patch sidecar enabled");

    let patch_path = result
        .patch_audit_path
        .clone()
        .expect("patch sidecar path should be returned when enabled");
    assert!(
        patch_path.exists(),
        "patch sidecar should be written to disk"
    );

    let patch_bytes = std::fs::read(&patch_path).expect("must read patch sidecar bytes");
    let patch_text = String::from_utf8_lossy(&patch_bytes);
    assert!(
        patch_text.contains("base content"),
        "patch sidecar should include previous text content"
    );
    assert!(
        patch_text.contains("tip content"),
        "patch sidecar should include updated text content"
    );

    let metadata_bytes =
        std::fs::read(&result.audit_path).expect("must read generated .caudit metadata file");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    let sidecar = metadata["patch_sidecar"]
        .as_object()
        .expect("metadata should include a patch_sidecar descriptor");
    let path_from_metadata = sidecar
        .get("path")
        .and_then(|value| value.as_str())
        .expect("patch_sidecar.path must be present");
    let sha_from_metadata = sidecar
        .get("sha256")
        .and_then(|value| value.as_str())
        .expect("patch_sidecar.sha256 must be present");
    assert_eq!(
        path_from_metadata,
        patch_path.display().to_string(),
        "metadata should reference the exact patch sidecar path"
    );
    assert_eq!(
        sha_from_metadata,
        sha256_hex(&patch_bytes).expect("must hash patch sidecar"),
        "metadata sidecar sha256 should match patch sidecar bytes"
    );
    assert!(
        result.archive_path.exists(),
        "archive path should be generated when patch sidecar is enabled"
    );
    let archive_bytes =
        std::fs::read(&result.archive_path).expect("must read generated archive bytes");
    let archive_text = String::from_utf8_lossy(&archive_bytes);
    assert!(
        archive_text.contains("range.bundle.caudit.patch"),
        "archive should include patch sidecar entry when patch generation is enabled"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that assume-present commits reachable from tip are emitted as additional bundle prerequisites.
#[test]
fn create_bundle_with_assume_present_adds_reachable_prerequisite() {
    let repo_dir = temp_repo_dir("create-bundle-assume-present-reachable");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let root_commit_id = commit_from_files(&repo, "root", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(&repo, "base", &[("f.txt", "base")], &[root_commit_id]);
    let main_commit_id = commit_from_files(&repo, "main", &[("f.txt", "main")], &[base_commit_id]);
    let side_commit_id =
        commit_from_files(&repo, "side", &[("side.txt", "side")], &[root_commit_id]);
    let tip_commit_id = commit_from_files(
        &repo,
        "merge tip",
        &[("f.txt", "tip"), ("side.txt", "side")],
        &[main_commit_id, side_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/side", side_commit_id, true, "create side ref")
        .expect("must create side ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle_with_options(
        &repo_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
        CreateBundleOptions {
            include_patch_sidecar: false,
            assume_present_revs: vec!["refs/heads/side".to_string()],
        },
    )
    .expect("create_bundle_with_options should succeed with reachable assume-present ref");

    let inspection = inspect_bundle(&bundle_path).expect("must inspect created bundle");
    assert_eq!(
        result.from_commit_id, base_commit_id,
        "from commit id should remain anchored to --from revision"
    );
    assert_eq!(
        result.to_commit_id, tip_commit_id,
        "to commit id should remain anchored to --to revision"
    );
    assert!(
        inspection.prerequisites.contains(&base_commit_id),
        "bundle prerequisites should include --from commit id"
    );
    assert!(
        inspection.prerequisites.contains(&side_commit_id),
        "bundle prerequisites should include reachable assume-present commit id"
    );
    assert_eq!(
        inspection.prerequisites.len(),
        2,
        "bundle should emit one prerequisite for --from plus one for reachable assume-present"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that assume-present commits not reachable from tip are ignored and do not become prerequisites.
#[test]
fn create_bundle_with_assume_present_ignores_non_reachable_prerequisite() {
    let repo_dir = temp_repo_dir("create-bundle-assume-present-non-reachable");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let root_commit_id = commit_from_files(&repo, "root", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(&repo, "base", &[("f.txt", "base")], &[root_commit_id]);
    let tip_commit_id = commit_from_files(&repo, "tip", &[("f.txt", "tip")], &[base_commit_id]);
    let unrelated_commit_id = commit_from_files(
        &repo,
        "unrelated",
        &[("other.txt", "other")],
        &[root_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference(
        "refs/heads/unrelated",
        unrelated_commit_id,
        true,
        "create unrelated ref",
    )
    .expect("must create unrelated ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle_with_options(
        &repo_dir,
        "refs/heads/base",
        "refs/heads/tip",
        &bundle_path,
        CreateBundleOptions {
            include_patch_sidecar: false,
            assume_present_revs: vec!["refs/heads/unrelated".to_string()],
        },
    )
    .expect("create_bundle_with_options should succeed with non-reachable assume-present ref");

    let inspection = inspect_bundle(&bundle_path).expect("must inspect created bundle");
    assert_eq!(
        inspection.prerequisites,
        vec![base_commit_id],
        "only --from prerequisite should remain when assume-present commit is not reachable from tip"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}
