// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload audit module for document operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::{
    PackEntryBaseRef, PackEntryKind, PackEntryLedger, PackEntryRecord, PayloadAuditDocument,
    PayloadAuditDocumentEntryLedger, PayloadAuditDocumentHead, PayloadAuditDocumentObjectDetail,
    PayloadAuditDocumentPackEntry, PayloadAuditDocumentPackObject,
    PayloadAuditDocumentTransportEntry, PayloadAuditLedgerMode, PayloadAuditObjectDetailMode,
    PayloadAuditPackSummary, PayloadObjectKind, ResolutionSource,
};
use crate::git::util::{
    bundle_version_code, current_hostname, current_unix_timestamp_secs, current_username,
};
use anyhow::Result;

use super::PayloadSession;

const LEDGER_SUMMARY_EDGE_COUNT: usize = 20;

/// Builds a serialized payload-audit JSON document from a reusable session using a selected ledger mode.
///
/// # Errors
///
/// Returns an error when object detail collection fails for any payload object.
pub(super) fn payload_audit_document_from_session_with_ledger_and_detail_mode(
    session: &PayloadSession,
    ledger_mode: PayloadAuditLedgerMode,
    detail_mode: PayloadAuditObjectDetailMode,
) -> Result<PayloadAuditDocument> {
    let mut pack_objects = Vec::<PayloadAuditDocumentPackObject>::new();
    let mut object_details = Vec::<PayloadAuditDocumentObjectDetail>::new();
    let emit_pack_object_rows = !matches!(detail_mode, PayloadAuditObjectDetailMode::Light)
        || !matches!(ledger_mode, PayloadAuditLedgerMode::None);

    let mut reachable_objects = 0usize;
    let mut commit_objects = 0usize;
    let mut tree_objects = 0usize;
    let mut blob_objects = 0usize;
    let mut tag_objects = 0usize;
    let mut unknown_objects = 0usize;

    for object in &session.payload.objects {
        if object.reachable_from_heads {
            reachable_objects += 1;
        }

        match object.kind {
            PayloadObjectKind::Commit => commit_objects += 1,
            PayloadObjectKind::Tree => tree_objects += 1,
            PayloadObjectKind::Blob => blob_objects += 1,
            PayloadObjectKind::Tag => tag_objects += 1,
            PayloadObjectKind::Unknown => unknown_objects += 1,
        }

        if emit_pack_object_rows {
            pack_objects.push(PayloadAuditDocumentPackObject {
                oid: object.oid.to_string(),
                kind: payload_kind_code(object.kind).to_string(),
                size_bytes: object.size_bytes,
                reachable_from_heads: object.reachable_from_heads,
                context_head_index: object.context_head_index,
                context_commit_order: object.context_commit_order,
                context_path: object.context_path.clone(),
            });
        }

        if matches!(detail_mode, PayloadAuditObjectDetailMode::Full) {
            let detail = super::collect_payload_object_detail_for_session(session, object.oid)?;
            object_details.push(PayloadAuditDocumentObjectDetail {
                oid: detail.oid.to_string(),
                kind: payload_kind_code(detail.kind).to_string(),
                size_bytes: detail.size_bytes,
                syntax_path_hint: detail.syntax_path_hint,
                blob_paths: detail.blob_paths,
                text_line_count: detail.text_line_count,
                lines: detail.lines,
            });
        }
    }

    let total_objects = session.payload.objects.len();
    let summary = PayloadAuditPackSummary {
        total_objects,
        reachable_objects,
        unreachable_objects: total_objects.saturating_sub(reachable_objects),
        commit_objects,
        tree_objects,
        blob_objects,
        tag_objects,
        unknown_objects,
    };

    Ok(PayloadAuditDocument {
        schema_version: "1".to_string(),
        tool_version: crate::version::APP_VERSION.to_string(),
        generated_at_unix_secs: current_unix_timestamp_secs()?,
        generated_by_username: current_username(),
        generated_by_hostname: current_hostname(),
        bundle_path: session.bundle_path.clone(),
        bundle_size_bytes: session.bundle_size_bytes,
        bundle_sha256: session.bundle_sha256.clone(),
        bundle_header_version: bundle_version_code(session.inspection.version).to_string(),
        prerequisites: session
            .inspection
            .prerequisites
            .iter()
            .map(|oid| oid.to_string())
            .collect(),
        heads: session
            .inspection
            .heads
            .iter()
            .map(|head| PayloadAuditDocumentHead {
                oid: head.oid.to_string(),
                reference: head.reference.clone(),
            })
            .collect(),
        transport_entries: session
            .payload
            .transport_entries
            .iter()
            .map(|entry| PayloadAuditDocumentTransportEntry {
                name: entry.name.clone(),
                size_bytes: entry.size_bytes,
                sha256: entry.sha256.clone(),
            })
            .collect(),
        pack_proof: session.payload.pack_proof.clone(),
        entry_ledger: build_document_entry_ledger(&session.payload.entry_ledger, ledger_mode),
        pack_summary: summary,
        pack_objects,
        object_detail_mode: payload_object_detail_mode_code(detail_mode).to_string(),
        object_details,
    })
}

/// Builds serialized entry-ledger export section according to requested mode.
fn build_document_entry_ledger(
    ledger: &PackEntryLedger,
    mode: PayloadAuditLedgerMode,
) -> PayloadAuditDocumentEntryLedger {
    let parsed_entries = ledger.entries.len();
    let declared_entries = ledger.declared_entry_count;
    let unresolved_entries = ledger
        .entries
        .iter()
        .filter(|entry| !entry.resolved)
        .count();
    let (first_entries, last_entries, unresolved_entry_rows, entries) = match mode {
        PayloadAuditLedgerMode::None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        PayloadAuditLedgerMode::Summary => {
            let unresolved_entry_rows = ledger
                .entries
                .iter()
                .filter(|entry| !entry.resolved)
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            let first_entries = ledger
                .entries
                .iter()
                .take(LEDGER_SUMMARY_EDGE_COUNT)
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            let last_entries = ledger
                .entries
                .iter()
                .skip(parsed_entries.saturating_sub(LEDGER_SUMMARY_EDGE_COUNT))
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            (
                first_entries,
                last_entries,
                unresolved_entry_rows,
                Vec::new(),
            )
        }
        PayloadAuditLedgerMode::Full => {
            let unresolved_entry_rows = ledger
                .entries
                .iter()
                .filter(|entry| !entry.resolved)
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            let first_entries = ledger
                .entries
                .iter()
                .take(LEDGER_SUMMARY_EDGE_COUNT)
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            let last_entries = ledger
                .entries
                .iter()
                .skip(parsed_entries.saturating_sub(LEDGER_SUMMARY_EDGE_COUNT))
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            let entries = ledger
                .entries
                .iter()
                .map(document_pack_entry_row)
                .collect::<Vec<_>>();
            (first_entries, last_entries, unresolved_entry_rows, entries)
        }
    };

    PayloadAuditDocumentEntryLedger {
        mode: payload_ledger_mode_code(mode).to_string(),
        declared_entries,
        parsed_entries,
        unresolved_entries,
        first_entries,
        last_entries,
        unresolved_entry_rows,
        entries,
    }
}

/// Converts one in-memory ledger row into serialized payload-audit document row.
fn document_pack_entry_row(entry: &PackEntryRecord) -> PayloadAuditDocumentPackEntry {
    PayloadAuditDocumentPackEntry {
        idx: entry.idx,
        offset: entry.offset,
        kind: payload_entry_kind_code(entry.kind).to_string(),
        out_size: entry.out_size,
        reconstructed_size: entry.reconstructed_size,
        base: entry.base_ref.as_ref().map(payload_entry_base_ref_code),
        result_oid: entry.result_oid.map(|oid| oid.to_string()),
        result_kind: entry
            .result_kind
            .map(|kind| payload_kind_code(kind).to_string()),
        resolved: entry.resolved,
        resolved_via: entry
            .resolved_via
            .map(|value| resolution_source_code(value).to_string()),
        note: entry.note.clone(),
    }
}

/// Returns stable string code for payload object kinds.
fn payload_kind_code(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

/// Returns stable string code for payload ledger export mode.
fn payload_ledger_mode_code(mode: PayloadAuditLedgerMode) -> &'static str {
    match mode {
        PayloadAuditLedgerMode::None => "none",
        PayloadAuditLedgerMode::Summary => "summary",
        PayloadAuditLedgerMode::Full => "full",
    }
}

/// Returns stable string code for payload object-detail export mode.
fn payload_object_detail_mode_code(mode: PayloadAuditObjectDetailMode) -> &'static str {
    match mode {
        PayloadAuditObjectDetailMode::Light => "light",
        PayloadAuditObjectDetailMode::Full => "full",
    }
}

/// Returns stable string code for pack entry kinds.
fn payload_entry_kind_code(kind: PackEntryKind) -> &'static str {
    match kind {
        PackEntryKind::Commit => "commit",
        PackEntryKind::Tree => "tree",
        PackEntryKind::Blob => "blob",
        PackEntryKind::Tag => "tag",
        PackEntryKind::OfsDelta => "ofs-delta",
        PackEntryKind::RefDelta => "ref-delta",
    }
}

/// Returns a compact code for pack entry base references.
fn payload_entry_base_ref_code(base_ref: &PackEntryBaseRef) -> String {
    match base_ref {
        PackEntryBaseRef::BaseOffset {
            distance,
            base_offset,
        } => match base_offset {
            Some(offset) => format!("ofs:{distance}@{offset}"),
            None => format!("ofs:{distance}"),
        },
        PackEntryBaseRef::BaseOid(oid) => format!("oid:{oid}"),
    }
}

/// Returns stable string code for pack-entry resolution source.
fn resolution_source_code(source: ResolutionSource) -> &'static str {
    match source {
        ResolutionSource::InPack => "in-pack",
        ResolutionSource::Baseline => "baseline",
    }
}
