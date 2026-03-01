// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for payload model domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use super::{BundleHead, BundleVersion, PackEntryLedger, PayloadPackProof};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Payload-audit summary used by the payload TUI page.
pub struct PayloadAudit {
    /// Parsed bundle version discovered from the payload.
    pub bundle_version: BundleVersion,
    /// Advertised heads contained in the bundle.
    pub heads: Vec<BundleHead>,
    /// Top-level transport archive entries with integrity metadata.
    pub transport_entries: Vec<PayloadTransportEntry>,
    /// Verifiable PACK-level completeness and integrity metrics.
    pub pack_proof: PayloadPackProof,
    /// Authoritative PACK entry ledger parsed from raw bundle pack bytes.
    pub entry_ledger: PackEntryLedger,
    /// All imported objects collected from the bundle pack payload.
    pub objects: Vec<PayloadObjectEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One transport package entry (zip member or plain bundle file).
pub struct PayloadTransportEntry {
    /// Display name for the transport artifact.
    pub name: String,
    /// Byte size of the artifact.
    pub size_bytes: u64,
    /// SHA-256 digest of the artifact content.
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Object-kind classification for payload object listing and detail view.
pub enum PayloadObjectKind {
    /// Commit object.
    Commit,
    /// Tree object.
    Tree,
    /// Blob object.
    Blob,
    /// Annotated tag object.
    Tag,
    /// Unsupported or unknown object kind.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Resolve strategy for external delta-base dependencies during payload parsing.
pub enum PayloadResolveMode {
    /// Only in-pack data may be used (strict fail-closed).
    PackOnly,
    /// Allow resolving missing ref-delta bases from provided baseline repository ODB.
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One pack object row shown in payload object listing.
pub struct PayloadObjectEntry {
    /// Object id.
    pub oid: git2::Oid,
    /// Object kind.
    pub kind: PayloadObjectKind,
    /// Object size in bytes.
    pub size_bytes: usize,
    /// Whether object is reachable from advertised bundle heads.
    pub reachable_from_heads: bool,
    /// Optional head index where this object is first encountered in context traversal.
    pub context_head_index: Option<usize>,
    /// Optional commit order within the associated head traversal.
    pub context_commit_order: Option<usize>,
    /// Optional tree path context where object is first encountered.
    pub context_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Detailed object payload shown when drilling into a payload object row.
pub struct PayloadObjectDetail {
    /// Object id.
    pub oid: git2::Oid,
    /// Object kind.
    pub kind: PayloadObjectKind,
    /// Object size in bytes.
    pub size_bytes: usize,
    /// Optional path-like hint used for syntax selection of textual blob content.
    pub syntax_path_hint: Option<String>,
    /// Reachable repository paths that reference this blob object.
    pub blob_paths: Vec<String>,
    /// Number of UTF-8 text lines when this object is a textual blob.
    pub text_line_count: Option<usize>,
    /// Pre-rendered textual lines for the object detail view.
    pub lines: Vec<String>,
}
