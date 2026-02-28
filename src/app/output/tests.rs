//! Output-rendering tests.

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
