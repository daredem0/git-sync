//! Unit tests for payload tests.

use super::support::*;
use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write as _;

// Verifies that PACK proof parsing succeeds for a normal generated bundle and reports a non-zero declared count.
#[test]
fn verify_pack_payload_parses_declared_count_for_normal_bundle() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-proof-normal-count", false);

    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.bundle_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
    .expect("must collect payload audit for plain bundle input");
    assert!(
        payload.pack_proof.declared_object_count > 0,
        "pack proof should report a non-zero declared object count for generated bundle"
    );
    assert_eq!(
        payload.pack_proof.declared_object_count, payload.pack_proof.processed_object_count,
        "pack proof should process exactly the declared number of objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that PACK proof parsing accepts a syntactic ref-delta entry and reaches unresolved-base fail-closed semantics.
#[test]
fn verify_pack_payload_parses_declared_count_for_delta_bundle() {
    let repo_dir = temp_repo_dir("payload-proof-delta-count");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&1u32.to_be_bytes());
    // Entry header: type=REF_DELTA (7), size=0, no continuation.
    pack_prefix.push(0x70);
    // Missing base OID triggers unresolved-external-base fail-closed path after parsing.
    pack_prefix.extend_from_slice(&[0x11; 20]);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);
    let bundle_path = repo_dir.join("delta.bundle");
    std::fs::write(&bundle_path, bundle_bytes).expect("must write synthetic delta bundle");

    let result = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_err(),
        "synthetic delta bundle with unresolved external base must fail closed"
    );
    let error_text = format!(
        "{:#}",
        result.expect_err("synthetic delta bundle should fail payload audit")
    );
    assert!(
        error_text.contains("unresolved base") || error_text.contains("thin pack"),
        "error should indicate unresolved external delta dependency after declared-count parse"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that PACK proof validation rejects bundles with tampered trailer checksums.
#[test]
fn verify_pack_payload_validates_trailer_checksum() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-proof-checksum", false);

    let mut bytes = std::fs::read(&bundle_result.bundle_path).expect("must read fixture bundle");
    let last = bytes
        .len()
        .checked_sub(1)
        .expect("fixture bundle must contain at least one byte");
    bytes[last] ^= 0x01;
    let tampered_bundle_path = repo_dir.join("tampered-trailer.bundle");
    std::fs::write(&tampered_bundle_path, bytes).expect("must write tampered bundle");

    let result = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &tampered_bundle_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_err(),
        "payload audit should fail when pack trailer checksum is tampered"
    );
    let error_text = format!(
        "{:#}",
        result.expect_err("tampered trailer bundle should fail payload audit")
    );
    assert!(
        error_text.contains("pack trailer checksum mismatch"),
        "error should explicitly report pack trailer checksum mismatch"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that pack-entry ledger length matches declared entry count and uses deterministic index ordering.
#[test]
fn pack_ledger_contains_exactly_declared_entry_count() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("pack-ledger-count", false);

    let verification = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("pack verification with ledger should succeed for generated bundle");
    assert_eq!(
        verification.ledger.declared_entry_count, verification.proof.declared_object_count,
        "ledger declared count should match proof declared count"
    );
    assert_eq!(
        verification.ledger.entries.len(),
        verification.ledger.declared_entry_count,
        "ledger entries length should match declared entry count"
    );
    for (index, entry) in verification.ledger.entries.iter().enumerate() {
        assert_eq!(
            entry.idx, index,
            "ledger index should be deterministic and match stream order"
        );
    }
    for pair in verification.ledger.entries.windows(2) {
        assert!(
            pair[0].offset < pair[1].offset,
            "ledger offsets should be strictly increasing by stream order"
        );
    }

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that ref-delta ledger rows record base object id metadata when unresolved external base is encountered.
#[test]
fn pack_ledger_records_ref_delta_base_oid() {
    let repo_dir = temp_repo_dir("pack-ledger-ref-base-oid");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let bundle_path = write_synthetic_ref_delta_bundle(&repo_dir, "ref-delta.bundle");
    let error = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect_err("verification should fail for unresolved ref-delta base");
    let ledger = error
        .ledger_partial
        .expect("failure should include partial ledger context");
    assert_eq!(
        ledger.entries.len(),
        1,
        "unresolved first entry should still be captured in partial ledger"
    );
    let record = &ledger.entries[0];
    assert_eq!(
        record.kind,
        PackEntryKind::RefDelta,
        "partial ledger row kind should classify ref-delta entry"
    );
    match &record.base_ref {
        Some(PackEntryBaseRef::BaseOid(oid)) => {
            assert_eq!(
                *oid,
                git2::Oid::from_bytes(&[0x11; 20]).expect("must construct expected base oid"),
                "ledger should preserve ref-delta base object id metadata"
            );
        }
        other => panic!("expected BaseOid metadata for ref-delta entry, got: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that ofs-delta ledger rows record backward distance and resolved absolute base offset metadata.
#[test]
fn pack_ledger_records_ofs_delta_base_offset() {
    let repo_dir = temp_repo_dir("pack-ledger-ofs-base-offset");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let bundle_path = write_synthetic_ofs_delta_bundle(&repo_dir, "ofs-delta.bundle")
        .expect("must write synthetic ofs-delta bundle");
    let verification = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect("ofs-delta bundle with in-pack base should verify successfully");
    assert_eq!(
        verification.ledger.entries.len(),
        2,
        "ofs-delta fixture should produce two ledger rows"
    );
    let first_offset = verification.ledger.entries[0].offset;
    let ofs_record = &verification.ledger.entries[1];
    assert_eq!(
        ofs_record.kind,
        PackEntryKind::OfsDelta,
        "second fixture row should classify as ofs-delta"
    );
    match &ofs_record.base_ref {
        Some(PackEntryBaseRef::BaseOffset {
            distance,
            base_offset,
        }) => {
            assert!(
                *distance > 0,
                "ofs-delta base distance should be a positive backward distance"
            );
            assert_eq!(
                *base_offset,
                Some(first_offset),
                "ofs-delta base offset should resolve to first entry offset"
            );
        }
        other => panic!("expected BaseOffset metadata for ofs-delta entry, got: {other:?}"),
    }
}

// Verifies that unresolved external-base failures mark blocked row as unresolved with explanatory note before fail-closed return.
#[test]
fn pack_ledger_marks_unresolved_external_base_before_fail() {
    let repo_dir = temp_repo_dir("pack-ledger-unresolved-mark");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let bundle_path = write_synthetic_ref_delta_bundle(&repo_dir, "unresolved.bundle");
    let error = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect_err("verification should fail for unresolved external ref-delta base");
    assert_eq!(
        error.blocked_entry_idx,
        Some(0),
        "blocked entry index should point to unresolved first entry"
    );
    let ledger = error
        .ledger_partial
        .expect("failure should include partial ledger data");
    let record = ledger
        .entries
        .first()
        .expect("partial ledger should include unresolved row");
    assert!(
        !record.resolved,
        "unresolved external base row must be marked as unresolved"
    );
    let note = record
        .note
        .as_ref()
        .expect("unresolved row should include explanatory note");
    assert!(
        note.contains("unresolved base") || note.contains("thin pack"),
        "unresolved row note should mention external/unresolved base dependency"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that parse failures after at least one successful entry return partial-ledger context and blocked entry index.
#[test]
fn pack_parse_failure_returns_partial_ledger_context() {
    let repo_dir = temp_repo_dir("pack-ledger-partial-context");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let bundle_path = write_synthetic_truncated_second_entry_bundle(&repo_dir, "truncated.bundle")
        .expect("must write synthetic truncated bundle");
    let error = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect_err("truncated second entry should fail verification");
    assert_eq!(
        error.blocked_entry_idx,
        Some(1),
        "blocked entry should point to second entry parse failure"
    );
    let ledger = error
        .ledger_partial
        .expect("parse failure should include partial ledger context");
    assert_eq!(
        ledger.entries.len(),
        1,
        "partial ledger should preserve first successfully parsed entry"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that materialized object index rows are derived from ledger result OIDs (deduplicated) rather than repository ODB enumeration.
#[test]
fn materialized_index_is_built_from_ledger_result_oids() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("materialized-index-from-ledger", false);

    let verification = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("pack verification should succeed for fixture bundle");
    let expected_unique_oids = verification
        .ledger
        .entries
        .iter()
        .filter_map(|entry| entry.result_oid)
        .collect::<std::collections::BTreeSet<_>>();
    let actual_unique_oids = verification
        .materialized_index
        .objects
        .iter()
        .map(|entry| entry.oid)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual_unique_oids, expected_unique_oids,
        "materialized index should be built from unique ledger result OIDs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that materialized object index derivation is deterministic across repeated verification runs.
#[test]
fn materialized_index_is_deterministic_across_runs() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("materialized-index-deterministic", false);

    let first = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("first pack verification should succeed");
    let second = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("second pack verification should succeed");
    assert_eq!(
        first.materialized_index, second.materialized_index,
        "materialized index should be deterministic across repeated verification runs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload object rows remain stable for existing bundle fixture after switching to ledger-derived materialized index.
#[test]
fn payload_objects_view_remains_stable_for_existing_fixture() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-objects-stable-after-phase2", false);

    let first = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
    .expect("first payload collection should succeed");
    let second = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
    .expect("second payload collection should succeed");
    assert_eq!(
        first.objects, second.objects,
        "payload object rows should remain stable across repeated runs"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that proof/materialization counters are derived from entry ledger counts and not influenced by reachability enrichment.
#[test]
fn proof_counters_are_independent_of_reachability() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("proof-counters-independent-reachability", false);

    let verification = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("pack verification should succeed for fixture bundle");
    assert_eq!(
        verification.materialized_index.materialized_entry_count,
        verification.proof.declared_object_count,
        "materialized-entry count should come from ledger proof parsing, not reachability enrichment"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that transfer gate is open when all declared entries are parsed and materialized.
#[test]
fn transfer_allowed_true_when_materialized_equals_declared() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("transfer-gate-allowed", false);

    let verification = verify_pack_payload_for_bundle_input(&bundle_result.bundle_path)
        .expect("pack verification should succeed for fixture bundle");
    assert_eq!(
        verification.proof.entries_declared, verification.proof.entries_parsed,
        "successful pack proof should parse all declared entries"
    );
    assert_eq!(
        verification.proof.entries_declared, verification.proof.entries_materialized,
        "successful pack proof should materialize all declared entries"
    );
    assert!(
        verification.proof.transfer_allowed,
        "transfer gate should be open when materialized entry count equals declared count"
    );
    assert!(
        verification.proof.blocked_reason.is_none(),
        "blocked reason must be absent when transfer gate is open"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that transfer gate is blocked when materialized entries are fewer than declared entries.
#[test]
fn transfer_allowed_false_when_materialized_less_than_declared() {
    let proof = PayloadPackProof::from_entry_counters(
        2,
        2,
        2,
        1,
        1,
        0,
        "sha1".to_string(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
    );
    assert!(
        !proof.transfer_allowed,
        "transfer gate should be blocked when materialized entries are below declared count"
    );
    assert!(
        proof.blocked_reason.is_some(),
        "blocked transfer should include an explanatory blocked reason"
    );
}

// Verifies that duplicate-entry signal is reported deterministically from materialized ledger rows.
#[test]
fn duplicate_entry_count_materialized_is_reported_deterministically() {
    let repo_dir = temp_repo_dir("materialized-index-duplicate-count");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let bundle_path = write_synthetic_duplicate_blob_bundle(&repo_dir, "duplicate-blob.bundle")
        .expect("must write duplicate-blob synthetic bundle");
    let first = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect("first verification should succeed for duplicate-entry pack");
    let second = verify_pack_payload_for_bundle_input(&bundle_path)
        .expect("second verification should succeed for duplicate-entry pack");
    assert_eq!(
        first.materialized_index.duplicate_entry_count_materialized, 1,
        "duplicate-entry pack should report one duplicate materialized entry"
    );
    assert_eq!(
        first.materialized_index.duplicate_entry_count_materialized,
        second.materialized_index.duplicate_entry_count_materialized,
        "duplicate materialized-entry count should be deterministic across runs"
    );
    assert_eq!(
        first.materialized_index.unique_object_count, 1,
        "duplicate-entry pack should collapse to one unique materialized object"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload audit collection includes transport entries and imported pack-object rows.
#[test]
fn collect_payload_audit_for_bundle_input_includes_transport_entries_and_objects() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-list", false);

    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
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

// Verifies that large text blobs remain materialized via object-detail export even when UI preview is partial.
#[test]
fn large_blob_is_materialized_via_export_even_if_preview_is_partial() {
    let repo_dir = temp_repo_dir("payload-large-blob-materialized");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base", &[("base.txt", "base")], &[]);
    let large_content = (0..220usize)
        .map(|index| format!("line-{index:04} some large textual payload content"))
        .collect::<Vec<_>>()
        .join("\n");
    let tip_commit_id = commit_from_files(
        &repo,
        "tip",
        &[("big.txt", &large_content), ("base.txt", "base")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed");
    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
    .expect("must collect payload audit from bundle archive");
    assert!(
        payload.pack_proof.transfer_allowed,
        "large textual blob should still be fully materialized for transfer-gate completeness"
    );
    assert_eq!(
        payload.pack_proof.entries_materialized, payload.pack_proof.entries_declared,
        "large textual blob should not reduce materialized-entry completeness"
    );

    let blob_target = payload
        .objects
        .iter()
        .find(|entry| matches!(entry.kind, PayloadObjectKind::Blob))
        .expect("payload should include blob object");
    let detail = collect_payload_object_detail_for_bundle_input(
        &bundle_result.archive_path,
        &repo_dir,
        blob_target.oid,
    )
    .expect("must be able to export/read full blob detail");
    assert!(
        detail.lines.len() >= 200,
        "full object-detail export should include full large-blob content lines"
    );
    assert!(
        detail.text_line_count.unwrap_or(0) >= 200,
        "text line count should report full large-blob line count"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload object detail collection returns readable lines for a selected object.
#[test]
fn collect_payload_object_detail_for_bundle_input_returns_detail_lines() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-detail", false);
    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
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
    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
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

// Verifies that payload-audit schema requires phase-5 transfer-gate counters and entry-ledger section.
#[test]
fn paudit_schema_requires_transfer_gate_and_entry_counters() {
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
        "pack_proof",
        "entry_ledger",
        "pack_summary",
        "pack_objects",
        "object_details",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "schema required field list must include '{field}'"
        );
    }

    let pack_proof_required = schema_json["properties"]["pack_proof"]["required"]
        .as_array()
        .expect("pack_proof schema must define required field list");
    assert!(
        pack_proof_required
            .iter()
            .any(|value| value.as_str() == Some("verification_status")),
        "pack_proof required field list must include verification_status"
    );
    for field in [
        "entries_declared",
        "entries_parsed",
        "entries_materialized",
        "unique_objects_materialized",
        "duplicate_entry_count_materialized",
        "transfer_allowed",
        "blocked_reason",
    ] {
        assert!(
            pack_proof_required
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "pack_proof required field list must include '{field}'"
        );
    }

    let entry_ledger_required = schema_json["properties"]["entry_ledger"]["required"]
        .as_array()
        .expect("entry_ledger schema must define required field list");
    for field in [
        "mode",
        "declared_entries",
        "parsed_entries",
        "unresolved_entries",
        "first_entries",
        "last_entries",
        "unresolved_entry_rows",
        "entries",
    ] {
        assert!(
            entry_ledger_required
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "entry_ledger required field list must include '{field}'"
        );
    }
}

// Verifies that payload-audit JSON document builder emits required metadata and consistent summary counters.
#[test]
fn build_payload_audit_document_for_bundle_input_emits_phase2_shape() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-shape", false);

    let document = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Summary,
        PayloadResolveMode::PackOnly,
    )
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
        document.pack_proof.declared_object_count, document.pack_proof.processed_object_count,
        "pack proof declared and processed object counts must match"
    );
    assert_eq!(
        document.pack_proof.verification_status, "ok",
        "pack proof should emit explicit verification status"
    );
    assert_eq!(
        document.object_details.len(),
        document.pack_objects.len(),
        "payload-audit object_details should be available for all pack objects"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that summary-mode payload JSON emits only ledger subset rows and omits full entry list.
#[test]
fn audit_json_summary_mode_includes_required_ledger_subset() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-summary-ledger", false);

    let document = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Summary,
        PayloadResolveMode::PackOnly,
    )
    .expect("must build payload-audit summary-ledger document");

    assert_eq!(
        document.entry_ledger.mode, "summary",
        "summary mode should encode entry_ledger.mode as 'summary'"
    );
    assert!(
        document.entry_ledger.entries.is_empty(),
        "summary mode should not emit full ledger entries array"
    );
    assert!(
        !document.entry_ledger.first_entries.is_empty()
            || !document.entry_ledger.last_entries.is_empty(),
        "summary mode should emit at least one first/last ledger subset row"
    );
    assert_eq!(
        document.entry_ledger.parsed_entries, document.pack_proof.entries_parsed,
        "entry ledger parsed counter should match pack proof parsed counter"
    );
    assert_eq!(
        document.entry_ledger.declared_entries, document.pack_proof.entries_declared,
        "entry ledger declared counter should match pack proof declared counter"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that full-mode payload JSON emits all parsed ledger rows in deterministic index order.
#[test]
fn audit_json_full_mode_includes_all_entry_rows() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-full-ledger", false);

    let document = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Full,
        PayloadResolveMode::PackOnly,
    )
    .expect("must build payload-audit full-ledger document");

    assert_eq!(
        document.entry_ledger.mode, "full",
        "full mode should encode entry_ledger.mode as 'full'"
    );
    assert_eq!(
        document.entry_ledger.entries.len(),
        document.pack_proof.entries_parsed,
        "full mode should include all parsed ledger rows"
    );
    let expected_indices = (0..document.entry_ledger.entries.len()).collect::<Vec<_>>();
    let actual_indices = document
        .entry_ledger
        .entries
        .iter()
        .map(|entry| entry.idx)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_indices, expected_indices,
        "full-mode ledger rows should retain deterministic stream index ordering"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload-audit document sections used for auditing are deterministic across repeated builds.
#[test]
fn build_payload_audit_document_for_bundle_input_is_deterministic_for_payload_sections() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-audit-document-deterministic", false);

    let first = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Summary,
        PayloadResolveMode::PackOnly,
    )
    .expect("first payload-audit document build must succeed");
    let second = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Summary,
        PayloadResolveMode::PackOnly,
    )
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

    let document = build_payload_audit_document_for_bundle_input_with_options(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadAuditLedgerMode::Summary,
        PayloadResolveMode::PackOnly,
    )
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

// Verifies that payload audit fails closed when PACK declared object count does not match processable objects.
#[test]
fn collect_payload_audit_for_bundle_input_rejects_pack_count_mismatch() {
    let (repo_dir, bundle_result, _base_commit_id, _tip_commit_id) =
        create_linear_bundle_fixture("payload-pack-count-mismatch", false);

    let original_bytes =
        std::fs::read(&bundle_result.bundle_path).expect("must read original bundle bytes");
    let pack_offset = original_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .expect("bundle must contain PACK payload");
    let mut tampered = original_bytes.clone();
    let declared_count = u32::from_be_bytes([
        tampered[pack_offset + 8],
        tampered[pack_offset + 9],
        tampered[pack_offset + 10],
        tampered[pack_offset + 11],
    ]);
    let tampered_count = declared_count
        .checked_add(1)
        .expect("declared count increment should not overflow");
    tampered[pack_offset + 8..pack_offset + 12].copy_from_slice(&tampered_count.to_be_bytes());
    let tampered_path = repo_dir.join("tampered-count.bundle");
    std::fs::write(&tampered_path, tampered).expect("must write tampered bundle");

    let result = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &tampered_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_err(),
        "payload audit must reject bundles where declared PACK object count mismatches processed entries"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload audit rejects ref-delta entries whose base object is not present in PACK payload.
#[test]
fn collect_payload_audit_for_bundle_input_rejects_unresolved_ref_delta_base() {
    let repo_dir = temp_repo_dir("payload-unresolved-ref-delta");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let _repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&1u32.to_be_bytes());
    // Entry header: type=REF_DELTA (7), size=0, no continuation.
    pack_prefix.push(0x70);
    // Missing base OID (not present in this pack).
    pack_prefix.extend_from_slice(&[0x11; 20]);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);

    let bundle_path = repo_dir.join("unresolved-ref-delta.bundle");
    std::fs::write(&bundle_path, bundle_bytes).expect("must write synthetic bundle");

    let result = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_err(),
        "payload audit must reject unresolved ref-delta bases to enforce fail-closed behavior"
    );
    let error_text = format!(
        "{:#}",
        result.expect_err("result should be an error for unresolved ref-delta base")
    );
    assert!(
        error_text.contains("unresolved base") || error_text.contains("thin pack"),
        "error should explain unresolved external delta dependency"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that baseline-assisted resolve mode materializes external ref-delta entries when the base object exists.
#[test]
fn baseline_resolution_materializes_external_ref_delta_when_base_exists() {
    let repo_dir = temp_repo_dir("payload-baseline-resolve-success");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let base_bytes = b"base\n";
    let target_bytes = b"target\n";
    let base_oid = repo
        .blob(base_bytes)
        .expect("must write baseline base blob");
    let bundle_path = write_synthetic_external_ref_delta_bundle(
        &repo_dir,
        "external-ref-delta.bundle",
        base_oid,
        base_bytes,
        target_bytes,
    )
    .expect("must write synthetic external ref-delta bundle");

    let verification =
        verify_pack_payload_for_bundle_input_with_resolve_mode(&bundle_path, Some(&repo_dir))
            .expect("baseline resolve mode should materialize external ref-delta base");
    assert!(
        verification.proof.transfer_allowed,
        "baseline-assisted resolution should keep transfer gate open when base exists"
    );
    assert_eq!(
        verification.proof.entries_materialized, verification.proof.entries_declared,
        "baseline-assisted resolution should materialize all declared entries"
    );
    assert_eq!(
        verification.ledger.entries[0].resolved_via,
        Some(ResolutionSource::Baseline),
        "resolved entry should be marked as baseline-resolved"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that pack-only mode keeps external ref-delta bases unresolved and fails closed.
#[test]
fn resolve_mode_pack_only_keeps_external_base_unresolved() {
    let repo_dir = temp_repo_dir("payload-pack-only-unresolved");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let base_bytes = b"base\n";
    let target_bytes = b"target\n";
    let base_oid = repo
        .blob(base_bytes)
        .expect("must write baseline base blob");
    let bundle_path = write_synthetic_external_ref_delta_bundle(
        &repo_dir,
        "external-ref-delta-pack-only.bundle",
        base_oid,
        base_bytes,
        target_bytes,
    )
    .expect("must write synthetic external ref-delta bundle");

    let error = verify_pack_payload_for_bundle_input_with_resolve_mode(&bundle_path, None)
        .expect_err("pack-only mode should fail for external ref-delta base");
    assert_eq!(
        error.blocked_entry_idx,
        Some(0),
        "pack-only unresolved external base should block on first entry"
    );
    let partial = error
        .ledger_partial
        .expect("pack-only unresolved error should include partial ledger");
    assert!(
        !partial.entries[0].resolved,
        "pack-only unresolved row should remain unresolved"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that strict mode blocks transfer when unresolved external entries remain.
#[test]
fn strict_mode_blocks_when_unresolved_entries_remain() {
    let repo_dir = temp_repo_dir("payload-strict-unresolved-block");
    std::fs::create_dir_all(&repo_dir).expect("must create repo directory");
    let repo = git2::Repository::init(&repo_dir).expect("must init git repository");

    let base_bytes = b"base\n";
    let target_bytes = b"target\n";
    let base_oid = repo
        .blob(base_bytes)
        .expect("must write baseline base blob");
    let bundle_path = write_synthetic_external_ref_delta_bundle(
        &repo_dir,
        "external-ref-delta-strict.bundle",
        base_oid,
        base_bytes,
        target_bytes,
    )
    .expect("must write synthetic external ref-delta bundle");

    let result = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    );
    assert!(
        result.is_err(),
        "strict pack-only mode should block payload audit when unresolved entries remain"
    );
    let error_text = format!(
        "{:#}",
        result.expect_err("strict unresolved mode should return an error")
    );
    assert!(
        error_text.contains("unresolved base") || error_text.contains("thin pack"),
        "strict unresolved error should mention external/unresolved base dependency"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

// Verifies that payload audit remains robust when commit trees are omitted from pack and only available via prerequisites.
#[test]
fn collect_payload_audit_for_bundle_input_skips_missing_prerequisite_tree_context() {
    let repo_dir = temp_repo_dir("payload-missing-prerequisite-tree");
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base", &[("f.txt", "same")], &[]);
    // Same file set/content as base produces a commit whose tree can be reused from prerequisite.
    let tip_commit_id = commit_from_files(
        &repo,
        "tip-same-tree",
        &[("f.txt", "same")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("same-tree.bundle");
    let bundle_result = create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
        .expect("create_bundle should succeed for same-tree range");

    let payload = collect_payload_audit_for_bundle_input_with_resolve_mode(
        &bundle_result.archive_path,
        &repo_dir,
        PayloadResolveMode::PackOnly,
    )
    .expect("payload audit should not fail when prerequisite tree context is missing from pack");
    assert!(
        !payload.objects.is_empty(),
        "payload audit should still provide materialized object rows"
    );

    let _ = std::fs::remove_dir_all(repo_dir);
}

fn sha1_bytes(bytes: &[u8]) -> [u8; 20] {
    let mut ctx = std::mem::MaybeUninit::<openssl_sys::SHA_CTX>::uninit();
    let init_ok = unsafe { openssl_sys::SHA1_Init(ctx.as_mut_ptr()) } == 1;
    assert!(init_ok, "sha1 init should succeed in test helper");
    let mut ctx = unsafe { ctx.assume_init() };
    let update_ok =
        unsafe { openssl_sys::SHA1_Update(&mut ctx, bytes.as_ptr().cast(), bytes.len()) } == 1;
    assert!(update_ok, "sha1 update should succeed in test helper");
    let mut digest = [0u8; 20];
    let final_ok = unsafe { openssl_sys::SHA1_Final(digest.as_mut_ptr(), &mut ctx) } == 1;
    assert!(final_ok, "sha1 final should succeed in test helper");
    digest
}

fn write_synthetic_ref_delta_bundle(
    repo_dir: &std::path::Path,
    file_name: &str,
) -> std::path::PathBuf {
    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&1u32.to_be_bytes());
    pack_prefix.push(0x70);
    pack_prefix.extend_from_slice(&[0x11; 20]);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);
    let bundle_path = repo_dir.join(file_name);
    std::fs::write(&bundle_path, bundle_bytes).expect("must write synthetic ref-delta bundle");
    bundle_path
}

fn write_synthetic_external_ref_delta_bundle(
    repo_dir: &std::path::Path,
    file_name: &str,
    base_oid: git2::Oid,
    base_bytes: &[u8],
    target_bytes: &[u8],
) -> anyhow::Result<std::path::PathBuf> {
    let mut pack_body = Vec::new();
    let mut entry = encode_pack_entry_header(PackEntryKind::RefDelta, target_bytes.len());
    entry.extend_from_slice(base_oid.as_bytes());
    let delta_bytes = encode_literal_delta(base_bytes.len(), target_bytes)?;
    entry.extend_from_slice(&zlib_compress(&delta_bytes)?);
    pack_body.extend_from_slice(&entry);

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&1u32.to_be_bytes());
    pack_prefix.extend_from_slice(&pack_body);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);

    let bundle_path = repo_dir.join(file_name);
    std::fs::write(&bundle_path, bundle_bytes)?;
    Ok(bundle_path)
}

fn write_synthetic_ofs_delta_bundle(
    repo_dir: &std::path::Path,
    file_name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let base_blob = b"base\n";
    let target_blob = b"x\n";

    let mut pack_body = Vec::new();
    let mut first_entry = encode_pack_entry_header(PackEntryKind::Blob, base_blob.len());
    first_entry.extend_from_slice(&zlib_compress(base_blob)?);
    pack_body.extend_from_slice(&first_entry);

    let second_entry_offset = 12 + pack_body.len();
    let base_entry_offset = 12usize;
    let distance = second_entry_offset
        .checked_sub(base_entry_offset)
        .ok_or_else(|| anyhow::anyhow!("ofs-delta distance underflow"))?;
    let mut second_entry = encode_pack_entry_header(PackEntryKind::OfsDelta, target_blob.len());
    second_entry.extend_from_slice(&encode_ofs_delta_distance(distance));
    let delta_bytes = encode_literal_delta(base_blob.len(), target_blob)?;
    second_entry.extend_from_slice(&zlib_compress(&delta_bytes)?);
    pack_body.extend_from_slice(&second_entry);

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&pack_body);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);

    let bundle_path = repo_dir.join(file_name);
    std::fs::write(&bundle_path, bundle_bytes)?;
    Ok(bundle_path)
}

fn write_synthetic_truncated_second_entry_bundle(
    repo_dir: &std::path::Path,
    file_name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let first_blob = b"ok\n";
    let mut pack_body = Vec::new();
    let mut first_entry = encode_pack_entry_header(PackEntryKind::Blob, first_blob.len());
    first_entry.extend_from_slice(&zlib_compress(first_blob)?);
    pack_body.extend_from_slice(&first_entry);
    // Second entry header (blob size=1), but intentionally omit zlib stream bytes.
    pack_body.extend_from_slice(&encode_pack_entry_header(PackEntryKind::Blob, 1));

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&pack_body);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);

    let bundle_path = repo_dir.join(file_name);
    std::fs::write(&bundle_path, bundle_bytes)?;
    Ok(bundle_path)
}

fn encode_pack_entry_header(kind: PackEntryKind, size: usize) -> Vec<u8> {
    let kind_code = match kind {
        PackEntryKind::Commit => 1u8,
        PackEntryKind::Tree => 2u8,
        PackEntryKind::Blob => 3u8,
        PackEntryKind::Tag => 4u8,
        PackEntryKind::OfsDelta => 6u8,
        PackEntryKind::RefDelta => 7u8,
    };
    let mut out = Vec::new();
    let mut remaining = size >> 4;
    let mut first = (kind_code << 4) | ((size & 0x0f) as u8);
    if remaining != 0 {
        first |= 0x80;
    }
    out.push(first);
    while remaining != 0 {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }
    out
}

fn encode_ofs_delta_distance(distance: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut n = distance;
    bytes.push((n & 0x7f) as u8);
    n >>= 7;
    while n != 0 {
        n = n.saturating_sub(1);
        bytes.push(((n & 0x7f) as u8) | 0x80);
        n >>= 7;
    }
    bytes.reverse();
    bytes
}

fn encode_literal_delta(base_size: usize, target_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    if target_bytes.is_empty() {
        anyhow::bail!("target bytes must be non-empty for literal delta encoding");
    }
    if target_bytes.len() > 0x7f {
        anyhow::bail!("target bytes must fit one delta literal opcode byte");
    }

    let mut out = Vec::new();
    encode_delta_varint(base_size, &mut out);
    encode_delta_varint(target_bytes.len(), &mut out);
    out.push(target_bytes.len() as u8);
    out.extend_from_slice(target_bytes);
    Ok(out)
}

fn encode_delta_varint(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn zlib_compress(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    let out = encoder.finish()?;
    Ok(out)
}

fn write_synthetic_duplicate_blob_bundle(
    repo_dir: &std::path::Path,
    file_name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let blob = b"same-content\n";
    let mut first = encode_pack_entry_header(PackEntryKind::Blob, blob.len());
    first.extend_from_slice(&zlib_compress(blob)?);
    let mut second = encode_pack_entry_header(PackEntryKind::Blob, blob.len());
    second.extend_from_slice(&zlib_compress(blob)?);

    let mut pack_prefix = Vec::new();
    pack_prefix.extend_from_slice(b"PACK");
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&2u32.to_be_bytes());
    pack_prefix.extend_from_slice(&first);
    pack_prefix.extend_from_slice(&second);
    let trailer = sha1_bytes(&pack_prefix);
    let mut pack_bytes = pack_prefix;
    pack_bytes.extend_from_slice(&trailer);

    let mut bundle_bytes = Vec::new();
    bundle_bytes.extend_from_slice(b"# v2 git bundle\n");
    bundle_bytes.extend_from_slice(b"1111111111111111111111111111111111111111 refs/heads/main\n\n");
    bundle_bytes.extend_from_slice(&pack_bytes);

    let bundle_path = repo_dir.join(file_name);
    std::fs::write(&bundle_path, bundle_bytes)?;
    Ok(bundle_path)
}
