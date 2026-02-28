//! Unit tests for payload tests.

use super::support::*;
use super::*;

// Verifies that payload audit collection includes transport entries and imported pack-object rows.
#[test]
fn collect_payload_audit_for_bundle_input_includes_transport_entries_and_objects() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-list", false);

    let payload = collect_payload_audit_for_bundle_input(&bundle_result.archive_path, &repo_dir)
        .expect("must collect payload audit for archive input");
    assert!(
        payload
            .transport_entries
            .iter()
            .any(|entry| entry.name.ends_with(".bundle")),
        "payload transport listing should include packaged .bundle entry"
    );
    assert!(
        payload
            .transport_entries
            .iter()
            .any(|entry| entry.name.ends_with(".caudit.json")),
        "payload transport listing should include packaged metadata sidecar"
    );
    assert!(
        !payload.objects.is_empty(),
        "payload object list should include imported pack objects"
    );
    assert!(
        payload
            .objects
            .iter()
            .any(|entry| matches!(entry.kind, PayloadObjectKind::Commit)),
        "payload object list should include commit objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload object detail collection returns readable lines for a selected object.
#[test]
fn collect_payload_object_detail_for_bundle_input_returns_detail_lines() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-detail", false);
    let payload = collect_payload_audit_for_bundle_input(&bundle_result.archive_path, &repo_dir)
        .expect("must collect payload audit for archive input");
    let target = payload
        .objects
        .iter()
        .find(|entry| matches!(entry.kind, PayloadObjectKind::Commit))
        .expect("fixture payload should contain at least one commit object");

    let detail = collect_payload_object_detail_for_bundle_input(
        &bundle_result.archive_path,
        &repo_dir,
        target.oid,
    )
    .expect("must collect payload object detail for selected object");
    assert!(
        !detail.lines.is_empty(),
        "object detail should contain non-empty textual lines"
    );
    assert_eq!(
        detail.oid, target.oid,
        "detail payload should be returned for requested object id"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that blob detail includes blob-path metadata and text line counts for preview rendering.
#[test]
fn collect_payload_object_detail_for_text_blob_includes_paths_and_line_count() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-blob-metadata", false);
    let payload = collect_payload_audit_for_bundle_input(&bundle_result.archive_path, &repo_dir)
        .expect("must collect payload audit for archive input");
    let blob_target = payload
        .objects
        .iter()
        .find(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("fixture payload should contain at least one blob object");

    let detail = collect_payload_object_detail_for_bundle_input(
        &bundle_result.archive_path,
        &repo_dir,
        blob_target.oid,
    )
    .expect("must collect payload object detail for selected blob");
    assert!(
        !detail.blob_paths.is_empty(),
        "blob object detail should include at least one reachable blob path"
    );
    assert!(
        detail.text_line_count.is_some(),
        "textual blob detail should include a text line count"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that blob path scanning is capped to a bounded number of discovered paths.
#[test]
fn collect_payload_object_detail_caps_blob_path_scan() {
    let repo_dir = temp_repo_dir("payload-blob-path-limit");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base", &[("base.txt", "base")], &[]);
    let mut tip_files: Vec<(String, String)> = Vec::new();
    tip_files.push(("base.txt".to_string(), "base".to_string()));
    for index in 0..32usize {
        tip_files.push((format!("path-{index}.txt"), "shared-content\n".to_string()));
    }
    let tip_file_refs: Vec<(&str, &str)> = tip_files
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let tip_commit_id = commit_from_files(&repo, "tip", &tip_file_refs, &[base_commit_id]);
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    let shared_blob_oid = repo
        .blob(b"shared-content\n")
        .expect("must create shared blob oid");

    let detail = collect_payload_object_detail_for_bundle_input(
        &result.archive_path,
        &repo_dir,
        shared_blob_oid,
    )
    .expect("must collect payload detail for shared blob");
    assert!(
        detail.blob_paths.len() <= 12,
        "blob path scan should be capped for preview/detail responsiveness"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that an opened payload session can serve detail queries after the input bundle is removed.
#[test]
fn open_payload_session_allows_detail_lookup_without_bundle_file() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-session-reuse", false);
    let session = open_payload_session(&bundle_result.archive_path, &repo_dir)
        .expect("must open payload session from bundle input");
    let payload = payload_audit_from_session(&session);
    let target = payload
        .objects
        .iter()
        .find(|entry| matches!(entry.kind, PayloadObjectKind::Commit))
        .expect("fixture payload should contain commit object");

    std::fs::remove_file(&bundle_result.archive_path).expect("must remove bundle archive input");
    let detail = collect_payload_object_detail_for_session(&session, target.oid)
        .expect("session-backed detail lookup should not require bundle input path");
    assert_eq!(
        detail.oid, target.oid,
        "session-backed detail lookup should return requested object detail"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload-audit schema file exists and declares phase-2 required top-level fields.
#[test]
fn payload_audit_schema_declares_phase2_required_fields() {
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("sync.bundle.paudit.schema.json");
    let schema_bytes = std::fs::read(&schema_path).expect("must read payload-audit schema file");
    let schema_json: serde_json::Value =
        serde_json::from_slice(&schema_bytes).expect("schema file must parse as valid json");

    let required = schema_json
        .get("required")
        .and_then(|value| value.as_array())
        .expect("schema must define required top-level field list");

    for field in [
        "schema_version",
        "tool_version",
        "generated_at_unix_secs",
        "generated_by_username",
        "generated_by_hostname",
        "bundle_path",
        "bundle_size_bytes",
        "bundle_sha256",
        "bundle_header_version",
        "prerequisites",
        "heads",
        "transport_entries",
        "pack_summary",
        "pack_objects",
        "object_details",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "schema required field list must include '{field}'"
        );
    }
}

// Verifies that payload-audit JSON document builder emits required metadata and consistent summary counters.
#[test]
fn build_payload_audit_document_for_bundle_input_emits_phase2_shape() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-shape", false);

    let document =
        build_payload_audit_document_for_bundle_input(&bundle_result.archive_path, &repo_dir)
            .expect("must build payload-audit document");

    assert_eq!(
        document.schema_version, "1",
        "payload-audit document must advertise schema version 1"
    );
    assert!(
        !document.tool_version.trim().is_empty(),
        "payload-audit document must include non-empty tool_version"
    );
    assert!(
        !document.bundle_path.trim().is_empty(),
        "payload-audit document must include non-empty bundle_path"
    );
    assert!(
        document.bundle_size_bytes > 0,
        "payload-audit document must include positive bundle_size_bytes"
    );
    assert_eq!(
        document.bundle_sha256.len(),
        64,
        "payload-audit document must include 64-char bundle SHA-256 digest"
    );
    assert!(
        !document.heads.is_empty(),
        "payload-audit document must include at least one advertised head"
    );
    assert!(
        !document.pack_objects.is_empty(),
        "payload-audit document must include at least one pack-object row"
    );
    assert_eq!(
        document.pack_summary.total_objects,
        document.pack_objects.len(),
        "pack_summary.total_objects must match pack_objects length"
    );
    assert_eq!(
        document.object_details.len(),
        document.pack_objects.len(),
        "payload-audit object_details should be available for all pack objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload-audit document sections used for auditing are deterministic across repeated builds.
#[test]
fn build_payload_audit_document_for_bundle_input_is_deterministic_for_payload_sections() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-deterministic", false);

    let first =
        build_payload_audit_document_for_bundle_input(&bundle_result.archive_path, &repo_dir)
            .expect("first payload-audit document build must succeed");
    let second =
        build_payload_audit_document_for_bundle_input(&bundle_result.archive_path, &repo_dir)
            .expect("second payload-audit document build must succeed");

    assert_eq!(
        first.transport_entries, second.transport_entries,
        "transport entry rows must be deterministic across repeated builds"
    );
    assert_eq!(
        first.pack_summary, second.pack_summary,
        "pack summary counters must be deterministic across repeated builds"
    );
    assert_eq!(
        first.pack_objects, second.pack_objects,
        "pack object rows must be deterministic across repeated builds"
    );
    assert_eq!(
        first.object_details, second.object_details,
        "object detail rows must be deterministic across repeated builds"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload-audit document transport entries are emitted in deterministic sorted name order.
#[test]
fn build_payload_audit_document_for_bundle_input_sorts_transport_entries_by_name() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-transport-order", false);

    let document =
        build_payload_audit_document_for_bundle_input(&bundle_result.archive_path, &repo_dir)
            .expect("payload-audit document build must succeed");
    let names = document
        .transport_entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    let mut sorted_names = names.clone();
    sorted_names.sort();

    assert_eq!(
        names, sorted_names,
        "transport entry names must be sorted for deterministic output"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}
