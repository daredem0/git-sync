//! Non-interactive CLI output rendering helpers.

use anyhow::Result;

/// Renders non-interactive payload audit as a human-readable aligned table.
pub fn render_payload_audit_table(payload: &crate::git::PayloadAudit) -> String {
    let oid_header = "OID";
    let type_header = "TYPE";
    let size_header = "SIZE";
    let reachable_header = "REACHABLE";

    let oid_width = std::cmp::max(
        oid_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.oid.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let type_width = std::cmp::max(
        type_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| payload_kind_label(entry.kind).len())
            .max()
            .unwrap_or(0),
    );
    let size_width = std::cmp::max(
        size_header.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let reachable_width = std::cmp::max(reachable_header.len(), 9);
    let transport_name_header = "NAME";
    let transport_size_header = "SIZE";
    let transport_sha_header = "SHA256";
    let transport_name_width = std::cmp::max(
        transport_name_header.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.name.len())
            .max()
            .unwrap_or(0),
    );
    let transport_size_width = std::cmp::max(
        transport_size_header.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );

    let mut out = String::new();
    let transfer_status = if payload.pack_proof.transfer_allowed {
        "allowed".to_string()
    } else {
        format!(
            "blocked ({})",
            payload
                .pack_proof
                .blocked_reason
                .as_deref()
                .unwrap_or("entries not fully materialized")
        )
    };
    out.push_str(&format!(
        "PACK PROOF status={} version={} entries={}/{} materialized={}/{} transfer={} hash={} checksum_verified={} thin_pack={} baseline_resolutions={}\n",
        payload.pack_proof.verification_status,
        payload.pack_proof.pack_version,
        payload.pack_proof.entries_parsed,
        payload.pack_proof.entries_declared,
        payload.pack_proof.entries_materialized,
        payload.pack_proof.entries_declared,
        transfer_status,
        payload.pack_proof.hash_algorithm,
        if payload.pack_proof.checksum_verified {
            "yes"
        } else {
            "no"
        },
        if payload.pack_proof.thin_pack_detected {
            "yes"
        } else {
            "no"
        },
        payload.pack_proof.baseline_resolutions_count
    ));
    out.push_str(&format!(
        "PACK CHECKSUM computed={} trailer={}\n",
        payload.pack_proof.computed_pack_checksum, payload.pack_proof.trailer_pack_checksum
    ));
    let unresolved_entries = payload
        .entry_ledger
        .entries
        .iter()
        .filter(|entry| !entry.resolved)
        .count();
    out.push_str(&format!(
        "LEDGER summary declared={} parsed={} unresolved={}\n",
        payload.entry_ledger.declared_entry_count,
        payload.entry_ledger.entries.len(),
        unresolved_entries
    ));
    out.push('\n');
    out.push_str("TRANSPORT ENTRIES\n");
    out.push_str(&format!(
        "{:<transport_name_width$}  {:>transport_size_width$}  {}\n",
        transport_name_header, transport_size_header, transport_sha_header
    ));
    for entry in &payload.transport_entries {
        out.push_str(&format!(
            "{:<transport_name_width$}  {:>transport_size_width$}  {}\n",
            entry.name, entry.size_bytes, entry.sha256
        ));
    }
    if payload.transport_entries.is_empty() {
        out.push_str("(no transport entries)\n");
    }
    out.push('\n');
    out.push_str(&format!(
        "PACK OBJECTS (bundle {}, heads={})\n",
        match payload.bundle_version {
            crate::git::BundleVersion::V2 => "v2",
            crate::git::BundleVersion::V3 => "v3",
        },
        payload.heads.len()
    ));
    out.push_str(&format!(
        "{:<oid_width$}  {:<type_width$}  {:>size_width$}  {:<reachable_width$}\n",
        oid_header, type_header, size_header, reachable_header
    ));

    for object in &payload.objects {
        out.push_str(&format!(
            "{:<oid_width$}  {:<type_width$}  {:>size_width$}  {:<reachable_width$}\n",
            object.oid,
            payload_kind_label(object.kind),
            object.size_bytes,
            if object.reachable_from_heads {
                "yes"
            } else {
                "no"
            }
        ));
    }

    if payload.objects.is_empty() {
        out.push_str("(no pack objects)\n");
    }

    out
}

/// Renders non-interactive payload audit document as pretty-printed JSON.
pub fn render_payload_audit_json(document: &crate::git::PayloadAuditDocument) -> Result<String> {
    Ok(serde_json::to_string_pretty(document)?)
}

fn payload_kind_label(kind: crate::git::PayloadObjectKind) -> &'static str {
    match kind {
        crate::git::PayloadObjectKind::Commit => "commit",
        crate::git::PayloadObjectKind::Tree => "tree",
        crate::git::PayloadObjectKind::Blob => "blob",
        crate::git::PayloadObjectKind::Tag => "tag",
        crate::git::PayloadObjectKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
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
}
