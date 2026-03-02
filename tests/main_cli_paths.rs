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

fn rev_parse(repo: &Path, spec: &str) -> String {
    let output = run_command(
        "git",
        &["-C", repo.to_string_lossy().as_ref(), "rev-parse", spec],
        None,
    );
    assert_success(&output, "git rev-parse");
    String::from_utf8(output.stdout)
        .expect("rev-parse stdout should be utf-8")
        .trim()
        .to_string()
}

fn find_incoming_ref_target(receiver_repo: &Path, target_ref: &str) -> Option<(String, String)> {
    let output = run_command(
        "git",
        &[
            "-C",
            receiver_repo.to_string_lossy().as_ref(),
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/sync/incoming",
        ],
        None,
    );
    assert_success(&output, "git for-each-ref incoming namespace");
    let stdout =
        String::from_utf8(output.stdout).expect("incoming ref listing stdout should be utf-8");
    let tail = format!(
        "/{}",
        target_ref.strip_prefix("refs/").unwrap_or(target_ref)
    );
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let Some(ref_name) = parts.next() else {
            continue;
        };
        let Some(object_name) = parts.next() else {
            continue;
        };
        if ref_name.ends_with(&tail) {
            return Some((ref_name.to_string(), object_name.to_string()));
        }
    }
    None
}

fn find_incoming_branch_target(receiver_repo: &Path, target_ref: &str) -> Option<(String, String)> {
    let output = run_command(
        "git",
        &[
            "-C",
            receiver_repo.to_string_lossy().as_ref(),
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads/incoming",
        ],
        None,
    );
    assert_success(&output, "git for-each-ref incoming branch mirror");
    let stdout =
        String::from_utf8(output.stdout).expect("incoming branch listing stdout should be utf-8");
    let tail = format!(
        "/{}",
        target_ref.strip_prefix("refs/").unwrap_or(target_ref)
    );
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let Some(ref_name) = parts.next() else {
            continue;
        };
        let Some(object_name) = parts.next() else {
            continue;
        };
        if ref_name.ends_with(&tail) {
            return Some((ref_name.to_string(), object_name.to_string()));
        }
    }
    None
}

fn create_diverged_commit_on_bare_repo(repo: &Path, parent_oid: &str, content: &str) -> String {
    let mut command = Command::new("git");
    command.args([
        "-C",
        repo.to_string_lossy().as_ref(),
        "hash-object",
        "-w",
        "--stdin",
    ]);
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("must spawn git hash-object");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin must be available")
        .write_all(content.as_bytes())
        .expect("must write blob content");
    let output = child.wait_with_output().expect("must wait for hash-object");
    if !output.status.success() {
        panic!(
            "git hash-object failed\nexit={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let blob_oid = String::from_utf8(output.stdout)
        .expect("blob oid stdout should be utf-8")
        .trim()
        .to_string();
    let mktree_input = format!("100644 blob {blob_oid}\tbase.txt\n");
    let mut mktree = Command::new("git");
    mktree.args(["-C", repo.to_string_lossy().as_ref(), "mktree"]);
    mktree
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = mktree.spawn().expect("must spawn git mktree");
    child
        .stdin
        .as_mut()
        .expect("stdin must be available")
        .write_all(mktree_input.as_bytes())
        .expect("must write mktree input");
    let output = child.wait_with_output().expect("must wait for mktree");
    if !output.status.success() {
        panic!(
            "git mktree failed\nexit={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let tree_oid = String::from_utf8(output.stdout)
        .expect("tree oid stdout should be utf-8")
        .trim()
        .to_string();

    let mut commit_tree = Command::new("git");
    commit_tree.args([
        "-C",
        repo.to_string_lossy().as_ref(),
        "commit-tree",
        &tree_oid,
        "-p",
        parent_oid,
    ]);
    commit_tree
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = commit_tree.spawn().expect("must spawn git commit-tree");
    child
        .stdin
        .as_mut()
        .expect("stdin must be available")
        .write_all(b"receiver diverged tip\n")
        .expect("must write commit message");
    let output = child.wait_with_output().expect("must wait for commit-tree");
    if !output.status.success() {
        panic!(
            "git commit-tree failed\nexit={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout)
        .expect("commit oid stdout should be utf-8")
        .trim()
        .to_string()
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
        text.contains("preflight checks:"),
        "dry-run output should include the preflight plan heading"
    );
    assert!(
        text.contains("target_missing"),
        "dry-run output should include the computed per-ref plan status"
    );
    assert!(
        text.contains("changes:"),
        "dry-run output should include per-ref planned actions"
    );
    assert!(
        text.contains("would update"),
        "dry-run output should mark ref updates as would-update actions"
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
        text.contains("already_present"),
        "dry-run output should show already_present status when imported heads are already applied"
    );
    assert!(
        text.contains("(no file content changes)"),
        "dry-run output should print empty-change marker when all heads are already applied"
    );
}

// Verifies that `receive --dry-run --format json` emits a machine-readable preflight plan and line stats.
#[test]
fn receive_dry_run_json_outputs_preflight_plan_and_line_stats() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-dry-run-json");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init dry-run-json receiver");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--dry-run",
            "--format",
            "json",
        ],
        None,
    );
    assert_success(&output, "receive dry-run json");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("dry-run json output should be valid JSON");

    assert_eq!(
        json["bundle_version"],
        serde_json::json!("v2"),
        "dry-run json should include bundle version"
    );
    assert_eq!(
        json["would_import_heads"],
        serde_json::json!(1),
        "fixture should report one imported head"
    );

    let plan = json["preflight_plan"]
        .as_array()
        .expect("preflight_plan should be an array");
    assert_eq!(
        plan.len(),
        1,
        "fixture should emit one preflight row for one exported head"
    );
    assert_eq!(
        plan[0]["status"],
        serde_json::json!("target_missing"),
        "receiver with prerequisite-only state should report target_missing for tip"
    );
    let preserved_incoming_ref = plan[0]["preserved_incoming_ref"]
        .as_str()
        .expect("preflight row should include preserved incoming ref");
    assert!(
        preserved_incoming_ref.starts_with("refs/sync/incoming/"),
        "preflight row should point to safe incoming namespace"
    );

    let line_stats = json["line_stats"]
        .as_array()
        .expect("line_stats should be an array");
    assert!(
        line_stats
            .iter()
            .any(|row| row["path"] == serde_json::json!("added.txt")),
        "dry-run json should include expected changed path rows"
    );
}

// Verifies that `receive --format` is accepted only when `--dry-run` is provided.
#[test]
fn receive_format_requires_dry_run_mode() {
    let output = run_bin(
        &[
            "receive",
            "--repo",
            ".",
            "--bundle",
            "missing.bundle.zip",
            "--format",
            "json",
        ],
        None,
    );
    assert_failure(&output, "receive --format without --dry-run");
    let text = output_text(&output);
    assert!(
        text.contains("receive --format is supported only with --dry-run"),
        "receive --format without dry-run should fail with a clear usage error"
    );
}

// Verifies that `receive --integrate fast-forward-only` succeeds when receiver has only the base prerequisite.
#[test]
fn receive_integrate_fast_forward_only_passes_when_target_can_be_advanced() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-ff-pass");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init ff-pass receiver");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "fast-forward-only",
        ],
        None,
    );
    assert_success(&output, "receive fast-forward-only pass");
    let text = output_text(&output);
    assert!(
        text.contains("preflight checks:"),
        "successful receive should include preflight checks output"
    );
    assert!(
        text.contains("target_missing"),
        "successful receive should include computed preflight status rows"
    );
    assert!(
        text.contains("receive completed successfully."),
        "successful receive should end with a clear success message"
    );
    assert!(
        text.contains("changes:"),
        "successful receive should include per-ref action logs"
    );
    assert!(
        text.contains("updated"),
        "successful receive should include performed update actions"
    );
    assert!(
        text.contains("safety: target refs were updated through a locked ref transaction"),
        "successful receive should describe the applied safety mechanism"
    );

    let source_tip = rev_parse(&fixture.source_repo, "refs/tags/sync/tip^{commit}");
    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, source_tip,
        "fast-forward-only receive should advance target tip ref to bundle tip commit"
    );

    let incoming = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming.is_some(),
        "incoming namespace ref should exist after successful receive"
    );
    let (_, incoming_oid) = incoming.expect("incoming ref should exist");
    assert_eq!(
        incoming_oid, source_tip,
        "incoming namespace ref should point to imported tip commit"
    );
}

// Verifies that `receive --integrate fast-forward-only` fails for diverged targets and preserves both target and incoming refs.
#[test]
fn receive_integrate_fast_forward_only_fails_for_diverged_target() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-ff-fail-diverged");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init ff-fail receiver");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let base_oid = rev_parse(&receiver, "refs/tags/sync/base^{commit}");
    let diverged_tip_oid =
        create_diverged_commit_on_bare_repo(&receiver, &base_oid, "receiver-side diverged\n");
    let set_tip = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/tip",
            &diverged_tip_oid,
        ],
        None,
    );
    assert_success(&set_tip, "set diverged tip ref");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "fast-forward-only",
        ],
        None,
    );
    assert_failure(&output, "receive fast-forward-only diverged target");
    let text = output_text(&output);
    assert!(
        text.contains("diverged (non-fast-forward)"),
        "fast-forward-only failure should report non-fast-forward divergence"
    );
    assert!(
        text.contains("target ref:"),
        "fast-forward-only failure should include target ref diagnostics"
    );
    assert!(
        text.contains("target oid:"),
        "fast-forward-only failure should include target oid diagnostics"
    );
    assert!(
        text.contains("incoming oid:"),
        "fast-forward-only failure should include incoming oid diagnostics"
    );
    assert!(
        text.contains("merge-base oid:"),
        "fast-forward-only failure should include merge-base diagnostics"
    );
    assert!(
        text.contains("next-step: merge required; incoming ref preserved at refs/sync/incoming/"),
        "fast-forward-only failure should include merge guidance with preserved incoming ref path"
    );

    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, diverged_tip_oid,
        "failed fast-forward-only receive must keep diverged target tip unchanged"
    );

    let source_tip = rev_parse(&fixture.source_repo, "refs/tags/sync/tip^{commit}");
    let incoming = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming.is_some(),
        "incoming namespace ref should be preserved on fast-forward-only failure"
    );
    let (_, incoming_oid) = incoming.expect("incoming ref should exist");
    assert_eq!(
        incoming_oid, source_tip,
        "incoming namespace ref should still point to bundle tip commit on failure"
    );
}

// Verifies that `receive --check-mergeability` reports merge simulation results for diverged refs
// without updating target refs.
#[test]
fn receive_check_mergeability_reports_diverged_ref_merge_status_without_mutating_receiver() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-mergeability-check");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init mergeability-check receiver");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let base_oid = rev_parse(&receiver, "refs/tags/sync/base^{commit}");
    let diverged_tip_oid =
        create_diverged_commit_on_bare_repo(&receiver, &base_oid, "receiver-side diverged\n");
    let set_tip = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/tip",
            &diverged_tip_oid,
        ],
        None,
    );
    assert_success(&set_tip, "set diverged tip ref");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "fast-forward-only",
            "--check-mergeability",
        ],
        None,
    );
    assert_success(&output, "receive check-mergeability on diverged target");
    let text = output_text(&output);
    assert!(
        text.contains("mergeability checks:"),
        "mergeability mode should render mergeability checks section"
    );
    assert!(
        text.contains("merge context:"),
        "mergeability mode should include compact merge context details"
    );
    assert!(
        text.contains("graph    :"),
        "mergeability mode should include a compact graph-like merge view"
    );
    assert!(
        text.contains("(conflicted)") || text.contains("(clean)") || text.contains("(unknown)"),
        "mergeability checks should include a machine-stable mergeability status"
    );
    assert!(
        text.contains("conflict files:"),
        "mergeability mode should explicitly list conflict file details"
    );
    assert!(
        text.contains("- base.txt"),
        "fixture should report the conflicting file path"
    );
    assert!(
        text.contains("result : mergeability analysis finished; target refs were not updated."),
        "mergeability mode should clearly state that target refs were not updated"
    );
    assert!(
        text.contains("mergeability check completed successfully."),
        "mergeability mode should end with a dedicated success message"
    );

    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, diverged_tip_oid,
        "mergeability mode must not modify the receiver target ref"
    );
    assert!(
        find_incoming_ref_target(&receiver, "refs/tags/sync/tip").is_none(),
        "mergeability mode should not write incoming namespace refs in the real receiver"
    );
}

// Verifies that fast-forward-only validates all planned head updates before mutating target refs.
// If any head is diverged, target refs are left untouched (all-or-none target integration),
// while incoming namespace refs are still preserved for manual follow-up.
#[test]
fn receive_integrate_fast_forward_only_rejects_mixed_plan_without_partial_target_updates() {
    let root = unique_temp_dir("receive-ff-all-or-none");
    let source_repo = root.join("source");
    fs::create_dir_all(&source_repo).expect("must create source repo dir");

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
    assert_success(&add_base, "git add base");
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
    assert_success(&tag_base, "git tag base");
    let base_oid = rev_parse(&source_repo, "refs/tags/sync/base^{commit}");

    fs::write(source_repo.join("base.txt"), "tip branch\n").expect("must write tip branch change");
    let add_tip = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "add",
            "base.txt",
        ],
        None,
    );
    assert_success(&add_tip, "git add tip");
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
    assert_success(&tag_tip, "git tag tip");
    let tip_oid = rev_parse(&source_repo, "refs/tags/sync/tip^{commit}");

    let checkout_side = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "checkout",
            "-b",
            "side",
            &base_oid,
        ],
        None,
    );
    assert_success(&checkout_side, "git checkout side from base");
    fs::write(source_repo.join("side.txt"), "side branch\n").expect("must write side branch file");
    let add_side = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "add",
            "side.txt",
        ],
        None,
    );
    assert_success(&add_side, "git add side");
    let commit_side = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "commit",
            "-m",
            "side commit",
        ],
        None,
    );
    assert_success(&commit_side, "git commit side");
    let tag_side = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "tag",
            "sync/side",
        ],
        None,
    );
    assert_success(&tag_side, "git tag side");
    let side_oid = rev_parse(&source_repo, "refs/tags/sync/side^{commit}");

    let bundle_path = root.join("multi-head.bundle");
    let create_bundle = run_command(
        "git",
        &[
            "-C",
            source_repo.to_string_lossy().as_ref(),
            "bundle",
            "create",
            bundle_path.to_string_lossy().as_ref(),
            "^refs/tags/sync/base",
            "refs/tags/sync/tip",
            "refs/tags/sync/side",
        ],
        None,
    );
    assert_success(&create_bundle, "git bundle create multi-head");

    let receiver = root.join("receiver");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "git init bare receiver");
    let fetch_base = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "fetch",
            source_repo.to_string_lossy().as_ref(),
            "refs/tags/sync/base:refs/tags/sync/base",
        ],
        None,
    );
    assert_success(&fetch_base, "git fetch base into receiver");
    let set_side_to_base = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/side",
            &base_oid,
        ],
        None,
    );
    assert_success(&set_side_to_base, "seed side target at base");

    let diverged_tip_oid =
        create_diverged_commit_on_bare_repo(&receiver, &base_oid, "receiver diverged tip\n");
    let set_tip = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/tip",
            &diverged_tip_oid,
        ],
        None,
    );
    assert_success(&set_tip, "seed diverged tip target");

    let receive = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            bundle_path.to_string_lossy().as_ref(),
            "--integrate",
            "fast-forward-only",
        ],
        None,
    );
    assert_failure(&receive, "receive mixed-plan fast-forward-only");
    let text = output_text(&receive);
    assert!(
        text.contains("diverged (non-fast-forward)"),
        "mixed-plan failure should include divergence reason"
    );

    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, diverged_tip_oid,
        "diverged tip must remain unchanged after failure"
    );
    let receiver_side = rev_parse(&receiver, "refs/tags/sync/side^{commit}");
    assert_eq!(
        receiver_side, base_oid,
        "fast-forwardable side ref must remain unchanged when plan validation fails"
    );

    let incoming_tip = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming_tip.is_some(),
        "incoming namespace should preserve tip head even on failure"
    );
    let (_, incoming_tip_oid) = incoming_tip.expect("incoming tip namespace ref should exist");
    assert_eq!(
        incoming_tip_oid, tip_oid,
        "incoming tip namespace ref should point to source tip commit"
    );

    let incoming_side = find_incoming_ref_target(&receiver, "refs/tags/sync/side");
    assert!(
        incoming_side.is_some(),
        "incoming namespace should preserve side head even on failure"
    );
    let (_, incoming_side_oid) = incoming_side.expect("incoming side namespace ref should exist");
    assert_eq!(
        incoming_side_oid, side_oid,
        "incoming side namespace ref should point to source side commit"
    );

    let _ = fs::remove_dir_all(root);
}

// Verifies that `receive --integrate create-refs-only` succeeds and does not update target refs.
#[test]
fn receive_integrate_create_refs_only_passes_without_updating_target_ref() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-create-refs-pass");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init create-refs-only pass receiver");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let base_oid = rev_parse(&receiver, "refs/tags/sync/base^{commit}");
    let pin_tip_to_base = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/tip",
            &base_oid,
        ],
        None,
    );
    assert_success(&pin_tip_to_base, "pin tip ref to base");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "create-refs-only",
        ],
        None,
    );
    assert_success(&output, "receive create-refs-only pass");
    let text = output_text(&output);
    assert!(
        text.contains("changes:"),
        "create-refs-only receive should include per-ref action logs"
    );
    assert!(
        text.contains("keep") || text.contains("kept"),
        "create-refs-only receive should log skipped target updates"
    );

    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, base_oid,
        "create-refs-only receive must not update existing target tip ref"
    );

    let source_tip = rev_parse(&fixture.source_repo, "refs/tags/sync/tip^{commit}");
    let incoming = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming.is_some(),
        "create-refs-only should still write incoming namespace refs"
    );
    let (_, incoming_oid) = incoming.expect("incoming ref should exist");
    assert_eq!(
        incoming_oid, source_tip,
        "incoming namespace ref should point to bundle tip commit"
    );
}

// Verifies that `receive --integrate create-refs-only` succeeds for diverged targets
// (no target integration requested) and preserves incoming refs.
#[test]
fn receive_integrate_create_refs_only_passes_for_diverged_target() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-create-refs-diverged-pass");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(
        &init_receiver,
        "init create-refs-only diverged-pass receiver",
    );

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let base_oid = rev_parse(&receiver, "refs/tags/sync/base^{commit}");
    let diverged_tip_oid =
        create_diverged_commit_on_bare_repo(&receiver, &base_oid, "receiver-side diverged\n");
    let set_tip = run_command(
        "git",
        &[
            "-C",
            receiver.to_string_lossy().as_ref(),
            "update-ref",
            "refs/tags/sync/tip",
            &diverged_tip_oid,
        ],
        None,
    );
    assert_success(&set_tip, "set diverged tip ref");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "create-refs-only",
        ],
        None,
    );
    assert_success(
        &output,
        "receive create-refs-only should succeed on diverged target without integrating target refs",
    );
    let text = output_text(&output);
    assert!(
        text.contains("bundle received:"),
        "successful create-refs-only receive should report normal success summary"
    );

    let receiver_tip = rev_parse(&receiver, "refs/tags/sync/tip^{commit}");
    assert_eq!(
        receiver_tip, diverged_tip_oid,
        "create-refs-only receive must keep diverged target tip unchanged"
    );

    let source_tip = rev_parse(&fixture.source_repo, "refs/tags/sync/tip^{commit}");
    let incoming = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming.is_some(),
        "incoming namespace ref should be created on create-refs-only diverged success"
    );
    let (_, incoming_oid) = incoming.expect("incoming ref should exist");
    assert_eq!(
        incoming_oid, source_tip,
        "incoming namespace ref should point to bundle tip commit"
    );
}

// Verifies that `receive --incoming-as-branches` mirrors incoming heads under refs/heads/incoming/<bundle-id>/...
#[test]
fn receive_incoming_as_branches_creates_branch_mirror_refs() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-incoming-branches");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init receiver for incoming-as-branches");

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
    assert_success(&fetch_base, "fetch base prerequisite");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "create-refs-only",
            "--incoming-as-branches",
        ],
        None,
    );
    assert_success(&output, "receive with incoming-as-branches");

    let source_tip = rev_parse(&fixture.source_repo, "refs/tags/sync/tip^{commit}");

    let incoming_namespace = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming_namespace.is_some(),
        "safe incoming namespace refs should still be created"
    );
    let (_, namespace_oid) = incoming_namespace.expect("incoming namespace ref should exist");
    assert_eq!(
        namespace_oid, source_tip,
        "incoming namespace ref should point to imported tip commit"
    );

    let incoming_branch = find_incoming_branch_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming_branch.is_some(),
        "incoming branch mirror ref should be created when flag is enabled"
    );
    let (_, branch_oid) = incoming_branch.expect("incoming branch ref should exist");
    assert_eq!(
        branch_oid, source_tip,
        "incoming branch mirror ref should point to imported tip commit"
    );
}

// Verifies that `receive --integrate create-refs-only` still fails when repository prerequisites are missing.
#[test]
fn receive_integrate_create_refs_only_fails_without_prerequisite_history() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-create-refs-fail-prereq");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(&init_receiver, "init create-refs-only fail receiver");

    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            fixture.bundle_archive.to_string_lossy().as_ref(),
            "--integrate",
            "create-refs-only",
        ],
        None,
    );
    assert_failure(&output, "receive create-refs-only without prerequisites");

    let incoming = find_incoming_ref_target(&receiver, "refs/tags/sync/tip");
    assert!(
        incoming.is_none(),
        "failed import without prerequisites should not create incoming namespace refs"
    );
}

// Verifies that `receive --integrate create-refs-only` fails for a missing bundle path (different failure cause).
#[test]
fn receive_integrate_create_refs_only_fails_for_missing_bundle_path() {
    let fixture = create_fixture();
    let receiver = fixture.root.join("receiver-create-refs-fail-missing-path");
    let init_receiver = run_command(
        "git",
        &["init", "--bare", receiver.to_string_lossy().as_ref()],
        None,
    );
    assert_success(
        &init_receiver,
        "init create-refs-only missing-path receiver",
    );

    let missing_bundle = fixture.root.join("does-not-exist.bundle.zip");
    let output = run_bin(
        &[
            "receive",
            "--repo",
            receiver.to_string_lossy().as_ref(),
            "--bundle",
            missing_bundle.to_string_lossy().as_ref(),
            "--integrate",
            "create-refs-only",
        ],
        None,
    );
    assert_failure(&output, "receive create-refs-only with missing bundle path");
    let text = output_text(&output);
    assert!(
        text.contains("No such file")
            || text.contains("not found")
            || text.contains("does not exist"),
        "missing bundle path failure should mention missing file path"
    );
}
