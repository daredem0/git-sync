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

// Verifies that verify-metadata mode is non-interactive and requires explicit --repo/--bundle inputs.
#[test]
fn audit_verify_metadata_requires_repo_and_bundle_inputs() {
    let output = run_bin(&["audit", "--verify-metadata"], None);
    assert_failure(&output, "verify-metadata without required inputs");
    let text = output_text(&output);
    assert!(
        text.contains("metadata verification requires --repo"),
        "verify-metadata should fail fast for missing --repo"
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

// Verifies that metadata verification works without --format and prints a plain success message.
#[test]
fn audit_verify_metadata_without_format_outputs_verification_ok() {
    let fixture = create_fixture();
    let output = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--verify-metadata",
        ],
        None,
    );
    assert_success(&output, "audit verify-metadata without format");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert_eq!(
        stdout.trim(),
        "metadata verification passed",
        "verify-metadata mode should print a plain success message"
    );
}

// Verifies that non-interactive audit rejects legacy TSV output in V5 payload-only mode.
#[test]
fn audit_non_interactive_rejects_legacy_tsv_output() {
    let fixture = create_fixture();
    let output = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--format",
            "tsv",
        ],
        None,
    );
    assert_failure(&output, "audit non-interactive tsv output");
}

// Verifies that non-interactive payload table output is stable and uses fixed aligned columns.
#[test]
fn audit_non_interactive_payload_table_output_succeeds() {
    let fixture = create_fixture();
    let output_first = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--format",
            "table",
        ],
        None,
    );
    assert_success(
        &output_first,
        "audit non-interactive payload table first run",
    );
    let stdout_first = String::from_utf8(output_first.stdout).expect("stdout should be utf-8");

    let output_second = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--format",
            "table",
        ],
        None,
    );
    assert_success(
        &output_second,
        "audit non-interactive payload table second run",
    );
    let stdout_second = String::from_utf8(output_second.stdout).expect("stdout should be utf-8");

    assert_eq!(
        stdout_first, stdout_second,
        "payload table output should be stable across repeated runs"
    );

    let lines = stdout_first.lines().collect::<Vec<_>>();
    assert!(
        lines.len() >= 6,
        "payload table output should include proof, transport, and object sections"
    );
    assert!(
        lines[0].starts_with("PACK PROOF status="),
        "first row should be the pack-proof summary"
    );
    assert!(
        lines[1].starts_with("PACK CHECKSUM computed="),
        "second row should be the pack-checksum summary"
    );
    assert!(
        lines.iter().any(|line| *line == "TRANSPORT ENTRIES"),
        "table output should include a transport entry section"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("PACK OBJECTS (bundle ")),
        "table output should include a pack-object section title"
    );

    let pack_header_index = lines
        .iter()
        .position(|line| {
            line.contains("OID")
                && line.contains("TYPE")
                && line.contains("SIZE")
                && line.contains("REACHABLE")
        })
        .expect("payload table output should include object table headers");

    let header = lines[pack_header_index];
    let oid_column = header
        .find("OID")
        .expect("header should include OID column");
    let type_column = header
        .find("TYPE")
        .expect("header should include TYPE column");
    let size_column = header
        .find("SIZE")
        .expect("header should include SIZE column");
    let reachable_column = header
        .find("REACHABLE")
        .expect("header should include REACHABLE column");
    assert!(
        oid_column < type_column && type_column < size_column && size_column < reachable_column,
        "payload table header columns should be rendered left-to-right in fixed order"
    );

    for row in lines.iter().skip(pack_header_index + 1) {
        if row.trim().is_empty() || *row == "(no pack objects)" {
            continue;
        }
        let oid_value = row
            .get(oid_column..type_column)
            .expect("row should contain OID slice")
            .trim();
        let type_value = row
            .get(type_column..size_column)
            .expect("row should contain TYPE slice")
            .trim();
        let size_value = row
            .get(size_column..reachable_column)
            .expect("row should contain SIZE slice")
            .trim();
        let reachable_value = row
            .get(reachable_column..)
            .expect("row should contain REACHABLE slice")
            .trim();

        assert_eq!(
            oid_value.len(),
            40,
            "OID column should contain hex object IDs"
        );
        assert!(
            ["commit", "tree", "blob", "tag", "unknown"].contains(&type_value),
            "TYPE column should use known payload object kind labels"
        );
        assert!(
            size_value.chars().all(|ch| ch.is_ascii_digit()),
            "SIZE column should contain numeric byte size values"
        );
        assert!(
            reachable_value == "yes" || reachable_value == "no",
            "REACHABLE column should contain yes/no markers"
        );
    }
}

// Verifies that non-interactive audit JSON includes the phase-2 payload document contract fields.
#[test]
fn audit_non_interactive_payload_json_output_succeeds() {
    let fixture = create_fixture();
    let output = run_bin(
        &[
            "audit",
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--repo",
            fixture.source_repo.to_string_lossy().as_ref(),
            "--format",
            "json",
        ],
        None,
    );
    assert_success(&output, "audit non-interactive payload json");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("payload json output should parse as valid json");
    assert!(
        value.get("schema_version").is_some(),
        "payload json output must include schema_version"
    );
    assert!(
        value.get("tool_version").is_some(),
        "payload json output must include tool_version"
    );
    assert!(
        value.get("generated_at_unix_secs").is_some(),
        "payload json output must include generated_at_unix_secs"
    );
    assert!(
        value.get("generated_by_username").is_some(),
        "payload json output must include generated_by_username"
    );
    assert!(
        value.get("generated_by_hostname").is_some(),
        "payload json output must include generated_by_hostname"
    );
    assert!(
        value.get("bundle_path").is_some(),
        "payload json output must include bundle_path"
    );
    assert!(
        value.get("bundle_size_bytes").is_some(),
        "payload json output must include bundle_size_bytes"
    );
    assert!(
        value.get("bundle_sha256").is_some(),
        "payload json output must include bundle_sha256"
    );
    assert!(
        value.get("bundle_header_version").is_some(),
        "payload json output must include bundle_header_version"
    );
    assert!(
        value.get("prerequisites").is_some(),
        "payload json output must include prerequisites"
    );
    assert!(
        value.get("heads").is_some(),
        "payload json output must include heads"
    );
    assert!(
        value.get("transport_entries").is_some(),
        "payload json output should include transport_entries section"
    );
    assert!(
        value.get("pack_proof").is_some(),
        "payload json output should include pack_proof section"
    );
    assert!(
        value.get("pack_summary").is_some(),
        "payload json output should include pack_summary section"
    );
    assert!(
        value.get("pack_objects").is_some(),
        "payload json output should include pack_objects section"
    );
    assert!(
        value.get("object_details").is_some(),
        "payload json output should include object_details section"
    );
    let pack_objects = value["pack_objects"]
        .as_array()
        .expect("pack_objects should be a JSON array");
    let object_details = value["object_details"]
        .as_array()
        .expect("object_details should be a JSON array");
    assert!(
        !pack_objects.is_empty(),
        "payload json output should include at least one pack object row"
    );
    assert_eq!(
        pack_objects.len(),
        object_details.len(),
        "payload json output should include one object_details row per pack object"
    );
    assert!(
        object_details
            .iter()
            .all(|detail| detail.get("lines").is_some() && detail["lines"].is_array()),
        "payload json object_details rows should include textual lines arrays"
    );
    assert_eq!(
        value["pack_proof"]["declared_object_count"], value["pack_proof"]["processed_object_count"],
        "pack proof should report equal declared and processed object counts"
    );
    assert_eq!(
        value["pack_proof"]["verification_status"],
        serde_json::json!("ok"),
        "payload json should include explicit pack verification status"
    );
    assert_eq!(
        value["schema_version"],
        serde_json::json!("1"),
        "payload json schema_version must be set to 1"
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
