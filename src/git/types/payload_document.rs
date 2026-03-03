// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for payload document domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use serde::{Deserialize, Serialize};

use super::PayloadPackProof;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized non-interactive payload-audit document (`audit --format json`).
pub struct PayloadAuditDocument {
    /// Schema version for payload-audit JSON.
    pub schema_version: String,
    /// Tool version that produced this document.
    pub tool_version: String,
    /// Generation timestamp in UNIX seconds.
    pub generated_at_unix_secs: u64,
    /// Local username on the auditing host.
    pub generated_by_username: String,
    /// Local hostname on the auditing host.
    pub generated_by_hostname: String,
    /// Audited raw bundle file path/name.
    pub bundle_path: String,
    /// Audited raw bundle byte size.
    pub bundle_size_bytes: u64,
    /// Audited raw bundle SHA-256 digest.
    pub bundle_sha256: String,
    /// Parsed bundle header version (`v2`/`v3`).
    pub bundle_header_version: String,
    /// Bundle prerequisite object ids.
    pub prerequisites: Vec<String>,
    /// Advertised bundle heads.
    pub heads: Vec<PayloadAuditDocumentHead>,
    /// All transport package entries hashed for audit.
    pub transport_entries: Vec<PayloadAuditDocumentTransportEntry>,
    /// PACK-level completeness and integrity proof metrics.
    pub pack_proof: PayloadPackProof,
    /// Entry-ledger export section (`none`, `summary`, or `full` rows).
    pub entry_ledger: PayloadAuditDocumentEntryLedger,
    /// Aggregate object-count summary by type/reachability.
    pub pack_summary: PayloadAuditPackSummary,
    /// Per-object listing from payload object enumeration.
    pub pack_objects: Vec<PayloadAuditDocumentPackObject>,
    /// Object-detail export mode (`full` or `light`).
    pub object_detail_mode: String,
    /// Per-object textual detail content for deep review/export.
    pub object_details: Vec<PayloadAuditDocumentObjectDetail>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Output mode for serialized entry-ledger section in payload-audit JSON.
pub enum PayloadAuditLedgerMode {
    /// Omit all entry-ledger rows while retaining entry counters.
    None,
    /// Emit bounded first/last/unresolved subsets only.
    Summary,
    /// Emit all parsed entry rows.
    Full,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Output mode for serialized object-details section in payload-audit JSON.
pub enum PayloadAuditObjectDetailMode {
    /// Omit object detail content lines.
    Light,
    /// Emit full object detail content for all pack objects.
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized entry-ledger section in payload-audit document.
pub struct PayloadAuditDocumentEntryLedger {
    /// Export mode label (`none`, `summary`, or `full`).
    pub mode: String,
    /// Number of entries declared by PACK header.
    pub declared_entries: usize,
    /// Number of entries parsed into the in-memory ledger.
    pub parsed_entries: usize,
    /// Number of unresolved entries in parsed ledger.
    pub unresolved_entries: usize,
    /// First-K parsed rows (summary mode; empty in none mode).
    pub first_entries: Vec<PayloadAuditDocumentPackEntry>,
    /// Last-K parsed rows (summary mode; empty in none mode).
    pub last_entries: Vec<PayloadAuditDocumentPackEntry>,
    /// Unresolved rows (summary/full mode; empty in none mode).
    pub unresolved_entry_rows: Vec<PayloadAuditDocumentPackEntry>,
    /// Full parsed rows (full mode; empty in none/summary modes).
    pub entries: Vec<PayloadAuditDocumentPackEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized one-row entry in payload entry-ledger section.
pub struct PayloadAuditDocumentPackEntry {
    /// Zero-based stream index.
    pub idx: usize,
    /// Byte offset where this entry begins within PACK payload.
    pub offset: usize,
    /// Entry kind label.
    pub kind: String,
    /// Declared output size.
    pub out_size: usize,
    /// Reconstructed canonical object size in bytes, when materialized.
    pub reconstructed_size: Option<usize>,
    /// Optional base reference label for delta entries.
    pub base: Option<String>,
    /// Optional canonical object ID when resolved.
    pub result_oid: Option<String>,
    /// Optional canonical object kind when resolved.
    pub result_kind: Option<String>,
    /// Whether this entry is resolved/materialized.
    pub resolved: bool,
    /// Optional resolution source label (`in-pack`/`baseline`).
    pub resolved_via: Option<String>,
    /// Optional row note.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized head entry in payload-audit document.
pub struct PayloadAuditDocumentHead {
    /// Head tip object id.
    pub oid: String,
    /// Head reference name.
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized transport-entry hash row in payload-audit document.
pub struct PayloadAuditDocumentTransportEntry {
    /// Transport entry name (zip member or raw bundle file name).
    pub name: String,
    /// Byte size of the entry.
    pub size_bytes: u64,
    /// SHA-256 digest of entry bytes.
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Aggregate pack summary counters in payload-audit document.
pub struct PayloadAuditPackSummary {
    /// Total objects enumerated.
    pub total_objects: usize,
    /// Objects reachable from advertised heads.
    pub reachable_objects: usize,
    /// Objects not reachable from advertised heads.
    pub unreachable_objects: usize,
    /// Commit object count.
    pub commit_objects: usize,
    /// Tree object count.
    pub tree_objects: usize,
    /// Blob object count.
    pub blob_objects: usize,
    /// Tag object count.
    pub tag_objects: usize,
    /// Unknown object count.
    pub unknown_objects: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized per-object row in payload-audit document.
pub struct PayloadAuditDocumentPackObject {
    /// Object id.
    pub oid: String,
    /// Object kind.
    pub kind: String,
    /// Uncompressed object size in bytes.
    pub size_bytes: usize,
    /// Reachability marker from advertised heads.
    pub reachable_from_heads: bool,
    /// Optional context head index for first-seen association.
    pub context_head_index: Option<usize>,
    /// Optional context commit order for first-seen association.
    pub context_commit_order: Option<usize>,
    /// Optional context path for first-seen association.
    pub context_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized per-object textual detail in payload-audit document.
pub struct PayloadAuditDocumentObjectDetail {
    /// Object id.
    pub oid: String,
    /// Object kind.
    pub kind: String,
    /// Uncompressed object size in bytes.
    pub size_bytes: usize,
    /// Optional syntax hint path for text rendering.
    pub syntax_path_hint: Option<String>,
    /// Reachable blob paths for blob objects.
    pub blob_paths: Vec<String>,
    /// Optional UTF-8 line count for text blobs.
    pub text_line_count: Option<usize>,
    /// Full textual representation/content lines.
    pub lines: Vec<String>,
}
