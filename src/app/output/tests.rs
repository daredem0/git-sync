// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Tests for non-interactive output formatting behavior.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use super::kind::payload_kind_label;
use super::render_payload_audit_json;
use super::render_payload_audit_table;

fn oid(hex: &str) -> git2::Oid {
    git2::Oid::from_str(hex).expect("must create valid oid")
}

fn sample_payload_for_table() -> crate::git::PayloadAudit {
    crate::git::PayloadAudit {
        bundle_version: crate::git::BundleVersion::V2,
        heads: vec![crate::git::BundleHead {
            oid: oid("1111111111111111111111111111111111111111"),
            reference: "refs/heads/main".to_string(),
        }],
        transport_entries: vec![crate::git::PayloadTransportEntry {
            name: "sync.bundle".to_string(),
            size_bytes: 123,
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        }],
        pack_proof: crate::git::PayloadPackProof::from_entry_counters(
            2,
            4,
            4,
            4,
            3,
            1,
            true,
            false,
            0,
            "sha1".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ),
        entry_ledger: crate::git::PackEntryLedger {
            pack_version: 2,
            declared_entry_count: 4,
            entries: vec![
                crate::git::PackEntryRecord {
                    idx: 0,
                    offset: 12,
                    kind: crate::git::PackEntryKind::Commit,
                    out_size: 120,
                    reconstructed_size: Some(120),
                    base_ref: None,
                    result_oid: Some(oid("2222222222222222222222222222222222222222")),
                    result_kind: Some(crate::git::PayloadObjectKind::Commit),
                    resolved: true,
                    resolved_via: Some(crate::git::ResolutionSource::InPack),
                    note: None,
                },
                crate::git::PackEntryRecord {
                    idx: 1,
                    offset: 44,
                    kind: crate::git::PackEntryKind::Blob,
                    out_size: 40,
                    reconstructed_size: Some(40),
                    base_ref: None,
                    result_oid: Some(oid("3333333333333333333333333333333333333333")),
                    result_kind: Some(crate::git::PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(crate::git::ResolutionSource::InPack),
                    note: None,
                },
                crate::git::PackEntryRecord {
                    idx: 2,
                    offset: 78,
                    kind: crate::git::PackEntryKind::Blob,
                    out_size: 40,
                    reconstructed_size: Some(40),
                    base_ref: None,
                    result_oid: Some(oid("3333333333333333333333333333333333333333")),
                    result_kind: Some(crate::git::PayloadObjectKind::Blob),
                    resolved: true,
                    resolved_via: Some(crate::git::ResolutionSource::InPack),
                    note: None,
                },
                crate::git::PackEntryRecord {
                    idx: 3,
                    offset: 101,
                    kind: crate::git::PackEntryKind::Tree,
                    out_size: 60,
                    reconstructed_size: Some(60),
                    base_ref: None,
                    result_oid: Some(oid("4444444444444444444444444444444444444444")),
                    result_kind: Some(crate::git::PayloadObjectKind::Tree),
                    resolved: true,
                    resolved_via: Some(crate::git::ResolutionSource::InPack),
                    note: None,
                },
            ],
        },
        objects: vec![crate::git::PayloadObjectEntry {
            oid: oid("2222222222222222222222222222222222222222"),
            kind: crate::git::PayloadObjectKind::Commit,
            size_bytes: 120,
            reachable_from_heads: true,
            context_head_index: Some(0),
            context_commit_order: Some(1),
            context_path: None,
        }],
    }
}

// Verifies that non-interactive table output includes entry counters and transfer status.
#[test]
fn audit_table_includes_entry_counts_and_transfer_status() {
    let table = render_payload_audit_table(&sample_payload_for_table());
    assert!(
        table.contains("entries=4/4 materialized=4/4"),
        "table proof header should include parsed/materialized entry counters"
    );
    assert!(
        table.contains("transfer=allowed"),
        "table proof header should include transfer status"
    );
    assert!(
        table.contains("LEDGER summary declared=4 parsed=4 unresolved=0"),
        "table should include concise ledger summary line"
    );
    assert!(
        table.contains("checksum_verified=yes"),
        "table proof header should include checksum verification status"
    );
    assert!(
        table.contains("thin_pack=no"),
        "table proof header should include thin-pack detection status"
    );
    assert!(
        table.contains("baseline_resolutions=0"),
        "table proof header should include baseline resolution counter"
    );
}

// Verifies that payload-kind labels remain stable for all known object kinds.
#[test]
fn payload_kind_label_covers_all_object_kinds() {
    assert_eq!(
        payload_kind_label(crate::git::PayloadObjectKind::Commit),
        "commit"
    );
    assert_eq!(
        payload_kind_label(crate::git::PayloadObjectKind::Tree),
        "tree"
    );
    assert_eq!(
        payload_kind_label(crate::git::PayloadObjectKind::Blob),
        "blob"
    );
    assert_eq!(
        payload_kind_label(crate::git::PayloadObjectKind::Tag),
        "tag"
    );
    assert_eq!(
        payload_kind_label(crate::git::PayloadObjectKind::Unknown),
        "unknown"
    );
}

// Verifies that payload JSON renderer emits pretty JSON for payload-audit documents.
#[test]
fn render_payload_audit_json_outputs_pretty_json_document() {
    let document = crate::git::PayloadAuditDocument {
        schema_version: "1".to_string(),
        tool_version: "0.7.0".to_string(),
        generated_at_unix_secs: 1,
        generated_by_username: "tester".to_string(),
        generated_by_hostname: "host".to_string(),
        bundle_path: "sync.bundle.zip".to_string(),
        bundle_size_bytes: 42,
        bundle_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
        bundle_header_version: "v2".to_string(),
        prerequisites: Vec::new(),
        heads: vec![crate::git::PayloadAuditDocumentHead {
            oid: "1111111111111111111111111111111111111111".to_string(),
            reference: "refs/heads/main".to_string(),
        }],
        transport_entries: vec![crate::git::PayloadAuditDocumentTransportEntry {
            name: "sync.bundle".to_string(),
            size_bytes: 42,
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        }],
        pack_proof: crate::git::PayloadPackProof::from_entry_counters(
            2,
            1,
            1,
            1,
            1,
            0,
            true,
            false,
            0,
            "sha1".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ),
        entry_ledger: crate::git::PayloadAuditDocumentEntryLedger {
            mode: "summary".to_string(),
            declared_entries: 1,
            parsed_entries: 1,
            unresolved_entries: 0,
            first_entries: Vec::new(),
            last_entries: Vec::new(),
            unresolved_entry_rows: Vec::new(),
            entries: Vec::new(),
        },
        pack_summary: crate::git::PayloadAuditPackSummary {
            total_objects: 0,
            reachable_objects: 0,
            unreachable_objects: 0,
            commit_objects: 0,
            tree_objects: 0,
            blob_objects: 0,
            tag_objects: 0,
            unknown_objects: 0,
        },
        pack_objects: Vec::new(),
        object_detail_mode: "full".to_string(),
        object_details: Vec::new(),
    };

    let json = render_payload_audit_json(&document).expect("json rendering should succeed");
    assert!(
        json.contains("\"schema_version\": \"1\""),
        "rendered json should contain schema_version field"
    );
    assert!(
        json.contains('\n'),
        "pretty renderer should include newlines"
    );
}
