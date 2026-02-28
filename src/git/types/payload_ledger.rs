//! Authoritative payload PACK ledger types.

use super::PayloadObjectKind;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Authoritative PACK-entry ledger captured while parsing raw pack bytes.
pub struct PackEntryLedger {
    /// PACK format version parsed from header.
    pub pack_version: u32,
    /// Number of entries declared by PACK header.
    pub declared_entry_count: usize,
    /// Parsed entry rows in deterministic stream order.
    pub entries: Vec<PackEntryRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One parsed PACK entry row with parse/materialization status.
pub struct PackEntryRecord {
    /// Zero-based stream order index.
    pub idx: usize,
    /// Byte offset (within PACK payload) where this entry header starts.
    pub offset: usize,
    /// Parsed PACK entry kind.
    pub kind: PackEntryKind,
    /// Declared size from PACK entry header.
    /// For non-delta entries this is object size; for delta entries this is delta-stream size.
    pub out_size: usize,
    /// Reconstructed canonical object size in bytes, when materialized.
    pub reconstructed_size: Option<usize>,
    /// Optional base reference metadata for delta entries.
    pub base_ref: Option<PackEntryBaseRef>,
    /// Canonical object id once materialized.
    pub result_oid: Option<git2::Oid>,
    /// Canonical object kind once materialized.
    pub result_kind: Option<PayloadObjectKind>,
    /// Whether entry materialization succeeded.
    pub resolved: bool,
    /// Materialization source, when resolved.
    pub resolved_via: Option<ResolutionSource>,
    /// Optional note string for unresolved/error context.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// PACK entry kind code from stream headers.
pub enum PackEntryKind {
    /// Full commit object.
    Commit,
    /// Full tree object.
    Tree,
    /// Full blob object.
    Blob,
    /// Full tag object.
    Tag,
    /// Offset delta entry.
    OfsDelta,
    /// Reference delta entry.
    RefDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Base reference metadata for delta entries.
pub enum PackEntryBaseRef {
    /// Backward distance (and resolved absolute offset) for OFS-delta entries.
    BaseOffset {
        /// Encoded backward distance.
        distance: usize,
        /// Resolved absolute base-entry offset, when computable.
        base_offset: Option<usize>,
    },
    /// Base object id for REF-delta entries.
    BaseOid(git2::Oid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Source used to resolve/materialize a PACK entry.
pub enum ResolutionSource {
    /// Fully resolved from in-pack data.
    InPack,
    /// Resolved with baseline/external object database assistance.
    Baseline,
}
