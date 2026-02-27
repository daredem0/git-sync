//! Unit tests for metadata tests.

use super::support::*;
use super::*;
use std::path::PathBuf;

// Focus: caudit metadata load/parse/integrity checks and verification against repository truth.
// Verifies that bundle metadata validation succeeds when the generated metadata matches the source repository state.
#[test]
fn verify_bundle_metadata_against_repo_accepts_matching_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-matching");
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
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    verify_bundle_metadata_against_repo(&bundle_path, &repo_dir)
        .expect("metadata verification should succeed when metadata and repo state match");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input reads changed-file metadata from a zip-only bundle package.
#[test]
fn collect_changed_files_from_bundle_input_accepts_zip_archive_without_loose_bundle_files() {
    let repo_dir = temp_repo_dir("collect-changes-input-zip");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base content")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip content"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    remove_unarchived_bundle_artifacts(&result).expect("must remove loose bundle artifacts");

    let changes = collect_changed_files_from_bundle_input(&result.archive_path)
        .expect("collect_changed_files_from_bundle_input should read metadata from zip package");
    assert!(
        !changes.is_empty(),
        "zip-contained metadata should provide changed file entries"
    );
    assert!(
        changes.iter().any(|entry| entry.path == "f.txt"),
        "changed file list from zip metadata should include modified file paths"
    );
    assert!(
        changes.iter().any(|entry| entry.path == "new.txt"),
        "changed file list from zip metadata should include added file paths"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification accepts zip-only bundle packages when no loose bundle files remain.
#[test]
fn verify_bundle_metadata_against_repo_input_accepts_zip_archive_without_loose_bundle_files() {
    let repo_dir = temp_repo_dir("verify-caudit-input-zip");
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
    remove_unarchived_bundle_artifacts(&result).expect("must remove loose bundle artifacts");

    verify_bundle_metadata_against_repo_input(&result.archive_path, &repo_dir).expect(
        "metadata verification should succeed when zip-contained bundle metadata matches repo truth",
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that bundle metadata validation rejects tampered metadata content.
#[test]
fn verify_bundle_metadata_against_repo_rejects_tampered_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-tampered");
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
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let metadata_bytes = std::fs::read(&caudit_path).expect("must read created caudit metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&metadata_bytes).expect("metadata should be valid json");
    metadata["range_to_oid"] = serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    std::fs::write(
        &caudit_path,
        serde_json::to_vec_pretty(&metadata).expect("must serialize tampered metadata"),
    )
    .expect("must write tampered metadata");

    let result = verify_bundle_metadata_against_repo(&bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata that does not match repository truth"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects unsupported schema versions.
#[test]
fn verify_bundle_metadata_integrity_rejects_unsupported_schema_version() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-schema", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["schema_version"] = serde_json::json!("999");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject unsupported schema version values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects bundle header version mismatches.
#[test]
fn verify_bundle_metadata_integrity_rejects_header_version_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-header", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["bundle_header_version"] = serde_json::json!("v9");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject mismatched bundle_header_version values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects prerequisite lists that do not match bundle header prerequisites.
#[test]
fn verify_bundle_metadata_integrity_rejects_prerequisite_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-prereq", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["prerequisites"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject prerequisite mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects head entries that do not match the bundle header.
#[test]
fn verify_bundle_metadata_integrity_rejects_heads_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-heads", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["heads"][0]["reference"] = serde_json::json!("refs/heads/other");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject head mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects tip_ref/range_to_oid combinations that do not match any head entry.
#[test]
fn verify_bundle_metadata_integrity_rejects_tip_head_consistency_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-tip", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["tip_ref"] = serde_json::json!("refs/heads/does-not-exist");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject tip_ref/range_to_oid mismatches against head list"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects unsupported patch sidecar format values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_unsupported_format() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-format", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["patch_sidecar"]["format"] = serde_json::json!("unknown-format");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject unsupported patch sidecar formats"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects missing patch sidecar paths.
#[test]
fn verify_bundle_metadata_integrity_rejects_missing_patch_sidecar_path() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-missing", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    let missing_patch_path = repo_dir.join("missing-sidecar.patch");
    metadata["patch_sidecar"]["path"] = serde_json::json!(missing_patch_path.display().to_string());
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject missing patch sidecar paths"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects patch sidecars with mismatched size values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_size_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-patch-size", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    let size = metadata["patch_sidecar"]["size_bytes"]
        .as_u64()
        .expect("patch sidecar size should be present");
    metadata["patch_sidecar"]["size_bytes"] = serde_json::json!(size + 1);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject patch sidecar size mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata integrity validation rejects patch sidecars with mismatched SHA-256 values.
#[test]
fn verify_bundle_metadata_integrity_rejects_patch_sidecar_sha_mismatch() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("integrity-patch-sha", true);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["patch_sidecar"]["sha256"] =
        serde_json::json!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_integrity(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "metadata integrity must reject patch sidecar sha mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input rejects unknown status codes in metadata.
#[test]
fn collect_changed_files_from_bundle_input_rejects_unknown_status_code() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("parse-status", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"][0]["status"] = serde_json::json!("Z");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = collect_changed_files_from_bundle_input(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "changed-file parsing must reject unknown status codes"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that collect_changed_files_from_bundle_input rejects invalid object IDs in metadata.
#[test]
fn collect_changed_files_from_bundle_input_rejects_invalid_oid() {
    let (repo_dir, bundle_result, _, _) = create_linear_bundle_fixture("parse-oid", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"][0]["new_oid"] = serde_json::json!("not-an-oid");
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = collect_changed_files_from_bundle_input(&bundle_result.bundle_path);
    assert!(
        result.is_err(),
        "changed-file parsing must reject invalid oid values"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects ranges whose from/to commits are present but not in ancestor-descendant order.
#[test]
fn verify_bundle_metadata_against_repo_rejects_non_linear_range_in_metadata() {
    let repo_dir = temp_repo_dir("verify-caudit-non-linear-range");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let root_commit_id = commit_from_files(&repo, "root", &[("f.txt", "root")], &[]);
    let base_commit_id = commit_from_files(&repo, "base", &[("f.txt", "base")], &[root_commit_id]);
    let tip_commit_id = commit_from_files(&repo, "tip", &[("f.txt", "tip")], &[base_commit_id]);
    let side_commit_id = commit_from_files(&repo, "side", &[("f.txt", "side")], &[root_commit_id]);
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");

    let caudit_path = PathBuf::from(format!("{}.caudit.json", bundle_path.display()));
    let mut metadata = read_json_value(&caudit_path);
    metadata["range_from_oid"] = serde_json::json!(side_commit_id.to_string());
    write_json_value(&caudit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata where range_from_oid is not an ancestor of range_to_oid"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects mismatched commit_chain values even when commit ids are valid and linear.
#[test]
fn verify_bundle_metadata_against_repo_rejects_commit_chain_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-chain", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["commit_chain"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_result.bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata commit_chain mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that metadata verification rejects mismatched changed_files values even when commit ids are valid and linear.
#[test]
fn verify_bundle_metadata_against_repo_rejects_changed_files_mismatch() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-changes", false);
    let mut metadata = read_json_value(&bundle_result.audit_path);
    metadata["changed_files"] = serde_json::json!([]);
    write_json_value(&bundle_result.audit_path, &metadata);

    let result = verify_bundle_metadata_against_repo(&bundle_result.bundle_path, &repo_dir);
    assert!(
        result.is_err(),
        "verification must reject metadata changed_files mismatches"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that verify_bundle_metadata_against_repo_input supports plain .bundle input paths in addition to zip archives.
#[test]
fn verify_bundle_metadata_against_repo_input_accepts_plain_bundle_input() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("verify-caudit-plain-input", false);

    verify_bundle_metadata_against_repo_input(&bundle_result.bundle_path, &repo_dir)
        .expect("verification should succeed for plain bundle inputs");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that verify_bundle_metadata_integrity_input accepts plain .bundle input paths on the success path.
#[test]
fn verify_bundle_metadata_integrity_input_accepts_plain_bundle_input() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("integrity-plain-input", false);
    verify_bundle_metadata_integrity_input(&bundle_result.bundle_path)
        .expect("integrity verification should succeed for valid plain bundle input");

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that load_bundle_metadata_from_path rejects missing metadata sidecar files.
#[test]
fn load_bundle_metadata_from_path_rejects_missing_path() {
    let missing_path = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-caudit-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = load_bundle_metadata_from_path(&missing_path);
    assert!(
        result.is_err(),
        "metadata loader should reject missing paths"
    );
}

// Verifies that load_bundle_metadata_from_path rejects directory paths.
#[test]
fn load_bundle_metadata_from_path_rejects_directory_path() {
    let metadata_dir = temp_repo_dir("load-metadata-dir");
    std::fs::create_dir_all(&metadata_dir).expect("must create metadata dir");

    let result = load_bundle_metadata_from_path(&metadata_dir);
    assert!(
        result.is_err(),
        "metadata loader should reject directory paths"
    );

    let _ = std::fs::remove_dir_all(metadata_dir);
}

// Verifies that resolve_patch_sidecar_path falls back to metadata sibling directory when explicit path does not exist.
#[test]
fn resolve_patch_sidecar_path_uses_sibling_when_explicit_path_is_missing() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("resolve-sidecar-sibling", true);
    let patch_path = bundle_result
        .patch_audit_path
        .as_ref()
        .expect("patch sidecar path should exist")
        .clone();
    let patch_file_name = patch_path
        .file_name()
        .expect("patch sidecar should have file name")
        .to_string_lossy()
        .to_string();

    let patch_sidecar = CreateBundleAuditPatchSidecar {
        path: format!("missing-dir/{patch_file_name}"),
        format: "unified-diff".to_string(),
        size_bytes: 0,
        sha256: String::new(),
    };
    let resolved_path = resolve_patch_sidecar_path(&bundle_result.audit_path, &patch_sidecar)
        .expect("sibling resolution should succeed");
    assert_eq!(
        resolved_path, patch_path,
        "patch sidecar resolution should fallback to metadata sibling directory"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}
