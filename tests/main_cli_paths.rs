//! Integration tests for main cli paths.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_git-sync")
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-main-cli-{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn run_command(program: &str, args: &[&str], current_dir: Option<&Path>) -> Output {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    command.output().expect("failed to execute command")
}

fn run_bin(args: &[&str], current_dir: Option<&Path>) -> Output {
    run_command(binary_path(), args, current_dir)
}

fn assert_success(output: &Output, context: &str) {
    if !output.status.success() {
        panic!(
            "{context} failed\nexit={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_failure(output: &Output, context: &str) {
    if output.status.success() {
        panic!(
            "{context} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn output_text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

struct Fixture {
    root: PathBuf,
    source_repo: PathBuf,
    bundle_archive: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn create_fixture() -> Fixture {
    let root = unique_temp_dir("fixture");
    let source_repo = root.join("source");
    fs::create_dir_all(&source_repo).expect("must create fixture source repo dir");

    let init = run_command(
        "git",
        &["init", source_repo.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init, "git init source repo");

    let set_name = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "config",
            "user.name",
            "Test User",
        ],
        None,
    );
    assert_success(&set_name, "git config user.name");

    let set_email = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "config",
            "user.email",
            "test@example.com",
        ],
        None,
    );
    assert_success(&set_email, "git config user.email");

    fs::write(source_repo.join("base.txt"), "base\n").expect("must write base file");
    let add_base = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "add",
            "base.txt",
        ],
        None,
    );
    assert_success(&add_base, "git add base file");
    let commit_base = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "commit",
            "-m",
            "base commit",
        ],
        None,
    );
    assert_success(&commit_base, "git commit base");
    let tag_base = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "tag",
            "sync/base",
        ],
        None,
    );
    assert_success(&tag_base, "git tag sync/base");

    fs::write(source_repo.join("base.txt"), "base modified\n").expect("must modify base file");
    fs::write(source_repo.join("added.txt"), "added\n").expect("must write added file");
    let add_tip = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "add",
            "base.txt",
            "added.txt",
        ],
        None,
    );
    assert_success(&add_tip, "git add tip changes");
    let commit_tip = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "commit",
            "-m",
            "tip commit",
        ],
        None,
    );
    assert_success(&commit_tip, "git commit tip");
    let tag_tip = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "tag",
            "sync/tip",
        ],
        None,
    );
    assert_success(&tag_tip, "git tag sync/tip");

    let bundle_path = root.join("sync.bundle");
    let create_output = run_bin(
        &[
            "create",
            "--repo",
            source_repo.to_string_lossy().as_ref(),
            "--from",
            "sync/base",
            "--to",
            "sync/tip",
            "--output",
            bundle_path.to_string_lossy().as_ref(),
        ],
        None,
    );
    assert_success(&create_output, "create command");

    let bundle_archive = PathBuf::from(format!("{}.zip", bundle_path.display()));
    assert!(
        bundle_archive.exists(),
        "fixture create must produce bundle archive"
    );

    Fixture {
        root,
        source_repo,
        bundle_archive,
    }
}

// Verifies that invoking the binary without a subcommand prints the scaffold/help hint path.
#[test]
fn main_without_subcommand_prints_scaffold_message() {
    let output = run_bin(&[], None);
    assert_success(&output, "running binary without subcommand");
    let text = output_text(&output);
    assert!(
        text.contains("git-sync scaffold is ready."),
        "no-subcommand path should print scaffold status message"
    );
}

// Verifies that --version is available and prints a version line.
#[test]
fn version_flag_prints_version_line() {
    let output = run_bin(&["--version"], None);
    assert_success(&output, "running binary with --version");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let line = stdout.trim();
    assert!(
        line.starts_with("git-sync "),
        "version output should start with binary name and a space"
    );
    let version_part = line
        .split_once(' ')
        .map(|(_, version)| version)
        .unwrap_or_default();
    assert!(
        !version_part.is_empty(),
        "version output should include a non-empty version value"
    );
}

// Verifies that interactive audit rejects --verify-metadata when no --format is provided.
#[test]
fn audit_interactive_rejects_verify_metadata_flag() {
    let output = run_bin(&["audit", "--verify-metadata"], None);
    assert_failure(&output, "interactive audit with verify-metadata");
    let text = output_text(&output);
    assert!(
        text.contains("interactive audit does not accept --verify-metadata"),
        "interactive audit should explain verify-metadata/format constraint"
    );
}

// Verifies that interactive audit rejects --from/--to range arguments when no --format is provided.
#[test]
fn audit_interactive_rejects_from_to_flags() {
    let output = run_bin(&["audit", "--from", "HEAD~1", "--to", "HEAD"], None);
    assert_failure(&output, "interactive audit with from/to");
    let text = output_text(&output);
    assert!(
        text.contains("interactive audit does not accept --from/--to"),
        "interactive audit should reject from/to args in TUI mode"
    );
}

// Verifies that interactive audit requires --repo when entering TUI mode.
#[test]
fn audit_interactive_requires_repo_argument() {
    let output = run_bin(&["audit", "--bundle", "sync.bundle.zip"], None);
    assert_failure(&output, "interactive audit without repo");
    let text = output_text(&output);
    assert!(
        text.contains("interactive audit requires --repo"),
        "interactive audit should fail fast when repo is missing"
    );
}

// Verifies that interactive audit requires --bundle when entering TUI mode.
#[test]
fn audit_interactive_requires_bundle_argument() {
    let output = run_bin(&["audit", "--repo", "."], None);
    assert_failure(&output, "interactive audit without bundle");
    let text = output_text(&output);
    assert!(
        text.contains("interactive audit requires --bundle"),
        "interactive audit should fail fast when bundle is missing"
    );
}

// Verifies that metadata verification in JSON format prints the expected success JSON document.
#[test]
fn audit_verify_metadata_json_outputs_verification_ok() {
    let fixture = create_fixture();
    let output = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--verify-metadata",
            "--format",
            "json",
        ],
        None,
    );
    assert_success(&output, "audit verify-metadata json");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert_eq!(
        stdout.trim(),
        "{\"verification\":\"ok\"}",
        "verification JSON mode should print deterministic OK payload"
    );
}

// Verifies that receive --dry-run prints non-empty file-change table when receiver only has prerequisites.
#[test]
fn receive_dry_run_prints_would_change_table_for_pending_import() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-pending");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init pending receiver");

    let fetch_base = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "fetch",
            fixture.source_repo.to_string_lossy().as_ref(),
            "refs/tags/sync/base:refs/tags/sync/base",
        ],
        None,
    );
    assert_success(&fetch_base, "fetch prerequisite base");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--dry-run",
        ],
        None,
    );
    assert_success(&output, "receive dry-run pending import");
    let text = output_text(&output);
    assert!(
        text.contains("would change (per-file line diff summary):"),
        "dry-run output should include would-change heading"
    );
    assert!(
        text.contains("PATH") && text.contains("+LINES") && text.contains("-LINES"),
        "dry-run output should include line-stat table headers"
    );
    assert!(
        text.contains("added.txt"),
        "dry-run output should include changed file rows when import is pending"
    );
}

// Verifies that receive --dry-run prints the empty-change marker when all bundle heads are already present.
#[test]
fn receive_dry_run_prints_no_changes_when_head_already_applied() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-applied");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init applied receiver");

    let fetch_base_and_tip = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "fetch",
            fixture.source_repo.to_string_lossy().as_ref(),
            "refs/tags/sync/base:refs/tags/sync/base",
            "refs/tags/sync/tip:refs/tags/sync/tip",
        ],
        None,
    );
    assert_success(&fetch_base_and_tip, "fetch base+tip");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--dry-run",
        ],
        None,
    );
    assert_success(&output, "receive dry-run already applied");
    let text = output_text(&output);
    assert!(
        text.contains("bundle can be applied without conflicts"),
        "dry-run output should include applicability summary"
    );
    assert!(
        text.contains("(no file content changes)"),
        "dry-run output should print empty-change marker when all heads are already applied"
    );
}
