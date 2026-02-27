//! Unit tests for archive tests.

use super::support::*;
use super::*;

// Focus: zip archive extraction/writing and removal of temporary unarchived artifacts.
// Verifies that extract_bundle_archive rejects missing archive paths.
#[test]
fn extract_bundle_archive_rejects_missing_path() {
    let missing_archive = std::env::temp_dir().join(format!(
        "git-sync-audit-missing-archive-{}-{}.zip",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));

    let result = extract_bundle_archive(&missing_archive);
    assert!(
        result.is_err(),
        "extract_bundle_archive must reject non-existent archive paths"
    );
}

// Verifies that extract_bundle_archive rejects archive paths that are directories.
#[test]
fn extract_bundle_archive_rejects_directory_path() {
    let archive_dir = temp_repo_dir("extract-archive-dir");
    std::fs::create_dir_all(&archive_dir).expect("must create archive directory");

    let result = extract_bundle_archive(&archive_dir);
    assert!(
        result.is_err(),
        "extract_bundle_archive must reject directory archive paths"
    );

    let _ = std::fs::remove_dir_all(archive_dir);
}

// Verifies that extract_bundle_archive rejects zip archives without any .bundle entry.
#[test]
fn extract_bundle_archive_rejects_zip_without_bundle_entry() {
    let work_dir = temp_repo_dir("extract-archive-no-bundle");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("input.zip");
    write_test_zip(&archive_path, &[("note.txt", b"not a bundle")]);

    let result = extract_bundle_archive(&archive_path);
    assert!(
        result.is_err(),
        "archive extraction must fail when no .bundle entry exists"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that extract_bundle_archive rejects zip archives containing multiple .bundle entries.
#[test]
fn extract_bundle_archive_rejects_zip_with_multiple_bundle_entries() {
    let work_dir = temp_repo_dir("extract-archive-multi-bundle");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("input.zip");
    write_test_zip(
        &archive_path,
        &[
            ("a.bundle", b"# v2 git bundle\n\nPACK"),
            ("b.bundle", b"# v2 git bundle\n\nPACK"),
        ],
    );

    let result = extract_bundle_archive(&archive_path);
    assert!(
        result.is_err(),
        "archive extraction must fail when multiple .bundle entries exist"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that removing unarchived artifacts also removes optional patch sidecar files when present.
#[test]
fn remove_unarchived_bundle_artifacts_removes_optional_patch_sidecar() {
    let (repo_dir, bundle_result, _, _) =
        create_linear_bundle_fixture("remove-artifacts-patch", true);
    let patch_path = bundle_result
        .patch_audit_path
        .as_ref()
        .expect("patch sidecar path should be present when enabled")
        .clone();
    assert!(
        patch_path.exists(),
        "patch sidecar should exist before cleanup"
    );

    remove_unarchived_bundle_artifacts(&bundle_result)
        .expect("cleanup should succeed when patch sidecar exists");
    assert!(
        !bundle_result.bundle_path.exists(),
        "bundle file should be removed by cleanup"
    );
    assert!(
        !bundle_result.audit_path.exists(),
        "audit json should be removed by cleanup"
    );
    assert!(
        !patch_path.exists(),
        "patch sidecar should be removed by cleanup"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that remove_file_if_exists returns an error for paths that exist but are directories.
#[test]
fn remove_file_if_exists_rejects_directory_paths() {
    let dir_path = temp_repo_dir("remove-file-dir");
    std::fs::create_dir_all(&dir_path).expect("must create test directory");

    let result = remove_file_if_exists(&dir_path);
    assert!(
        result.is_err(),
        "remove_file_if_exists should error when given a directory path"
    );

    let _ = std::fs::remove_dir_all(dir_path);
}

// Verifies that write_zip_archive rejects missing archive input files.
#[test]
fn write_zip_archive_rejects_missing_input_file() {
    let work_dir = temp_repo_dir("zip-missing-input");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("out.zip");
    let missing_input = work_dir.join("missing.txt");

    let result = write_zip_archive(&archive_path, &[missing_input]);
    assert!(
        result.is_err(),
        "write_zip_archive should reject missing input paths"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}

// Verifies that write_zip_archive rejects directory inputs in the file list.
#[test]
fn write_zip_archive_rejects_directory_input() {
    let work_dir = temp_repo_dir("zip-directory-input");
    std::fs::create_dir_all(&work_dir).expect("must create work dir");
    let archive_path = work_dir.join("out.zip");
    let input_dir = work_dir.join("dir-input");
    std::fs::create_dir_all(&input_dir).expect("must create directory input");

    let result = write_zip_archive(&archive_path, &[input_dir]);
    assert!(
        result.is_err(),
        "write_zip_archive should reject directory input entries"
    );

    let _ = std::fs::remove_dir_all(work_dir);
}
