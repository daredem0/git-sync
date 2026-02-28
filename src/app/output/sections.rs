//! Table section rendering helpers.

use crate::git::PayloadAudit;

use super::kind::payload_kind_label;
use super::layout::{
    OID_HEADER, REACHABLE_HEADER, SIZE_HEADER, TRANSPORT_NAME_HEADER, TRANSPORT_SHA_HEADER,
    TRANSPORT_SIZE_HEADER, TYPE_HEADER, TableWidths,
};

pub(super) fn append_pack_proof_section(out: &mut String, payload: &PayloadAudit) {
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
}

pub(super) fn append_transport_section(out: &mut String, payload: &PayloadAudit, w: &TableWidths) {
    out.push('\n');
    out.push_str("TRANSPORT ENTRIES\n");
    out.push_str(&format!(
        "{:<transport_name$}  {:>transport_size$}  {}\n",
        TRANSPORT_NAME_HEADER,
        TRANSPORT_SIZE_HEADER,
        TRANSPORT_SHA_HEADER,
        transport_name = w.transport_name,
        transport_size = w.transport_size,
    ));

    for entry in &payload.transport_entries {
        out.push_str(&format!(
            "{:<transport_name$}  {:>transport_size$}  {}\n",
            entry.name,
            entry.size_bytes,
            entry.sha256,
            transport_name = w.transport_name,
            transport_size = w.transport_size,
        ));
    }

    if payload.transport_entries.is_empty() {
        out.push_str("(no transport entries)\n");
    }
}

pub(super) fn append_objects_section(out: &mut String, payload: &PayloadAudit, w: &TableWidths) {
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
        "{:<oid$}  {:<kind$}  {:>size$}  {:<reachable$}\n",
        OID_HEADER,
        TYPE_HEADER,
        SIZE_HEADER,
        REACHABLE_HEADER,
        oid = w.oid,
        kind = w.kind,
        size = w.size,
        reachable = w.reachable,
    ));

    for object in &payload.objects {
        out.push_str(&format!(
            "{:<oid$}  {:<kind$}  {:>size$}  {:<reachable$}\n",
            object.oid,
            payload_kind_label(object.kind),
            object.size_bytes,
            if object.reachable_from_heads {
                "yes"
            } else {
                "no"
            },
            oid = w.oid,
            kind = w.kind,
            size = w.size,
            reachable = w.reachable,
        ));
    }

    if payload.objects.is_empty() {
        out.push_str("(no pack objects)\n");
    }
}
