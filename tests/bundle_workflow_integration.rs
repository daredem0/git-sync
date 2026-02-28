//! Integration tests for bundle workflow integration.

use std::ffi::OsString;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn bundle_archive_path(bundle_path: &Path) -> PathBuf {
    let mut archive = bundle_path.as_os_str().to_os_string();
    archive.push(".zip");
    PathBuf::from(archive)
}

fn run_checked_command(program: &str, args: &[&str], current_dir: Option<&Path>) -> String {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    let output = command.output().expect("failed to run command");
    if !output.status.success() {
        panic!(
            "command failed: {} {}\nexit: {}\nstdout:\n{}\nstderr:\n{}",
            program,
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).expect("command stdout should be utf-8")
}

fn zip_entry_names(archive_path: &Path) -> Vec<String> {
    let file = File::open(archive_path).expect("zip archive should be readable");
    let mut archive = ZipArchive::new(file).expect("zip archive should be valid");
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .expect("zip entry should be accessible");
        names.push(entry.name().to_string());
    }
    names
}

// Verifies the full end-to-end workflow:
// generate fixture repo, create bundle package, verify metadata,
// receive into a separate receiver repo, and confirm receiver refs resolve to the expected tip commit.
#[test]
fn integration_bundle_create_audit_verify_and_receive_flow() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join("scripts/generate-merge-graph-repo.sh");
    assert!(
        script_path.exists(),
        "fixture script must exist at {}",
        script_path.display()
    );

    let test_root = unique_temp_dir("git-sync-integration");
    fs::create_dir_all(&test_root).expect("must create test root dir");
    let fixture_repo = test_root.join("fixture-repo");
    let bundle_path = test_root.join("sync.bundle");
    let archive_path = bundle_archive_path(&bundle_path);

    let script_arg_owned = script_path.to_string_lossy().into_owned();
    let fixture_arg_owned = fixture_repo.to_string_lossy().into_owned();
    run_checked_command(
        "bash",
        &[&script_arg_owned, &fixture_arg_owned],
        Some(&manifest_dir),
    );
    assert!(
        fixture_repo.join(".git").exists(),
        "fixture repo should be initialized"
    );

    let repo_arg_owned = fixture_repo.to_string_lossy().into_owned();
    let output_arg_owned = bundle_path.to_string_lossy().into_owned();
    run_checked_command(
        "cargo",
        &[
            "run",
            "--quiet",
            "--",
            "create",
            "--repo",
            &repo_arg_owned,
            "--from",
            "sync/base",
            "--to",
            "sync/tip",
            "--output",
            &output_arg_owned,
        ],
        Some(&manifest_dir),
    );

    assert!(
        archive_path.exists(),
        "create command should produce zip package"
    );
    assert!(
        !bundle_path.exists(),
        "create command should remove loose bundle after archiving"
    );
    let mut metadata_path = OsString::from(bundle_path.as_os_str());
    metadata_path.push(".caudit.json");
    assert!(
        !PathBuf::from(&metadata_path).exists(),
        "create command should remove loose metadata after archiving"
    );

    let entries = zip_entry_names(&archive_path);
    assert!(
        entries.iter().any(|entry| entry == "sync.bundle"),
        "zip archive should contain sync.bundle"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry == "sync.bundle.caudit.json"),
        "zip archive should contain sync.bundle.caudit.json"
    );

    let bundle_arg_owned = archive_path.to_string_lossy().into_owned();
    let verify_output = run_checked_command(
        "cargo",
        &[
            "run",
            "--quiet",
            "--",
            "audit",
            "--bundle",
            &bundle_arg_owned,
            "--repo",
            &repo_arg_owned,
            "--verify-metadata",
        ],
        Some(&manifest_dir),
    );
    assert_eq!(
        verify_output.trim(),
        "metadata verification passed",
        "metadata verification should succeed against the source repo"
    );

    let receiver_repo = test_root.join("receiver-repo");
    let receiver_arg_owned = receiver_repo.to_string_lossy().into_owned();
    run_checked_command(
        "git",
        &["init", "--bare", &receiver_arg_owned],
        Some(&manifest_dir),
    );
    run_checked_command(
        "git",
        &[
            "-C",
            &receiver_arg_owned,
            "fetch",
            &repo_arg_owned,
            "refs/tags/sync/base:refs/tags/sync/base",
        ],
        Some(&manifest_dir),
    );

    let receive_output = run_checked_command(
        "cargo",
        &[
            "run",
            "--quiet",
            "--",
            "receive",
            "--repo",
            &receiver_arg_owned,
            "--bundle",
            &bundle_arg_owned,
            "--verify-metadata",
        ],
        Some(&manifest_dir),
    );
    assert!(
        receive_output.contains("bundle received:"),
        "receive command should report successful import"
    );
    assert!(
        receive_output.contains("imported_heads=1"),
        "fixture range should import a single exported head"
    );

    let source_tip_commit = run_checked_command(
        "git",
        &["-C", &repo_arg_owned, "rev-parse", "sync/tip^{commit}"],
        Some(&manifest_dir),
    );
    let receiver_tip_commit = run_checked_command(
        "git",
        &[
            "-C",
            &receiver_arg_owned,
            "rev-parse",
            "refs/tags/sync/tip^{commit}",
        ],
        Some(&manifest_dir),
    );
    assert_eq!(
        source_tip_commit.trim(),
        receiver_tip_commit.trim(),
        "receiver imported tip ref must resolve to the same commit as source sync/tip"
    );

    let _ = fs::remove_dir_all(test_root);
}
