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
