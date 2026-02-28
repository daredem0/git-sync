//! Git-layer types functionality.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supported git bundle header versions.
pub enum BundleVersion {
    /// Classic v2 bundle header.
    V2,
    /// Newer v3 bundle header.
    V3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A head reference advertised by a bundle.
pub struct BundleHead {
    /// Target commit object ID for the head reference.
    pub oid: git2::Oid,
    /// Fully-qualified reference name, e.g. `refs/heads/main`.
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Parsed header-level metadata for a bundle file.
pub struct BundleInspection {
    /// Parsed bundle header version.
    pub version: BundleVersion,
    /// Bundle prerequisite commits required by the receiver.
    pub prerequisites: Vec<git2::Oid>,
    /// Heads carried by the bundle payload.
    pub heads: Vec<BundleHead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Validated repository/bundle context for opening the TUI.
pub struct OpenContext {
    /// Resolved commit for the configured base reference.
    pub base_commit_id: git2::Oid,
    /// Optional resolved tip commit when a tip reference is configured.
    pub tip_commit_id: Option<git2::Oid>,
    /// Bundle version discovered from the inspected bundle input.
    pub bundle_version: BundleVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// File-level status classification used across diff and metadata outputs.
pub enum ChangeStatus {
    /// File exists only on the new side.
    Added,
    /// File contents or metadata changed.
    Modified,
    /// File exists only on the old side.
    Deleted,
    /// File path changed.
    Renamed,
    /// File was copied.
    Copied,
    /// File kind/mode changed (for example regular file to symlink).
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Output paths and commit metadata produced by bundle creation.
pub struct CreateBundleResult {
    /// Range start commit encoded as bundle prerequisite.
    pub from_commit_id: git2::Oid,
    /// Range end commit encoded as bundle head target.
    pub to_commit_id: git2::Oid,
    /// Head reference name recorded in the bundle header.
    pub tip_ref_name: String,
    /// Generated raw bundle path.
    pub bundle_path: PathBuf,
    /// Generated metadata sidecar path.
    pub audit_path: PathBuf,
    /// Optional unified-diff sidecar path.
    pub patch_audit_path: Option<PathBuf>,
    /// Final packaged archive path that contains produced artifacts.
    pub archive_path: PathBuf,
}

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
    /// Entry-ledger export section (summary or full rows).
    pub entry_ledger: PayloadAuditDocumentEntryLedger,
    /// Aggregate object-count summary by type/reachability.
    pub pack_summary: PayloadAuditPackSummary,
    /// Per-object listing from payload object enumeration.
    pub pack_objects: Vec<PayloadAuditDocumentPackObject>,
    /// Per-object textual detail content for deep review/export.
    pub object_details: Vec<PayloadAuditDocumentObjectDetail>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Output mode for serialized entry-ledger section in payload-audit JSON.
pub enum PayloadAuditLedgerMode {
    /// Emit bounded first/last/unresolved subsets only.
    Summary,
    /// Emit all parsed entry rows.
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized entry-ledger section in payload-audit document.
pub struct PayloadAuditDocumentEntryLedger {
    /// Export mode label (`summary` or `full`).
    pub mode: String,
    /// Number of entries declared by PACK header.
    pub declared_entries: usize,
    /// Number of entries parsed into the in-memory ledger.
    pub parsed_entries: usize,
    /// Number of unresolved entries in parsed ledger.
    pub unresolved_entries: usize,
    /// First-K parsed rows (summary mode).
    pub first_entries: Vec<PayloadAuditDocumentPackEntry>,
    /// Last-K parsed rows (summary mode).
    pub last_entries: Vec<PayloadAuditDocumentPackEntry>,
    /// Unresolved rows (summary and full mode).
    pub unresolved_entry_rows: Vec<PayloadAuditDocumentPackEntry>,
    /// Full parsed rows (full mode).
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
/// Pack-level proof metrics emitted for pre-transfer completeness auditing.
pub struct PayloadPackProof {
    /// Explicit verification result for PACK completeness/integrity checks.
    pub verification_status: String,
    /// PACK format version parsed from pack header.
    pub pack_version: u32,
    /// Number of objects declared by PACK header.
    pub declared_object_count: usize,
    /// Number of objects fully processed by parser/verifier.
    pub processed_object_count: usize,
    /// Number of PACK entries declared by pack header.
    pub entries_declared: usize,
    /// Number of PACK entries parsed from raw stream.
    pub entries_parsed: usize,
    /// Number of entries with full object bytes materialized.
    pub entries_materialized: usize,
    /// Number of unique materialized object IDs.
    pub unique_objects_materialized: usize,
    /// Duplicate count among materialized entries (`entries_materialized - unique_objects_materialized`).
    pub duplicate_entry_count_materialized: usize,
    /// Whether transfer is allowed by entry-materialization gate.
    pub transfer_allowed: bool,
    /// Optional blocked-transfer reason when gate is closed.
    pub blocked_reason: Option<String>,
    /// Hash algorithm used for pack trailer/object IDs.
    pub hash_algorithm: String,
    /// SHA-1 of all pack bytes except trailer (computed locally).
    pub computed_pack_checksum: String,
    /// SHA-1 trailer checksum embedded in PACK payload.
    pub trailer_pack_checksum: String,
}

impl PayloadPackProof {
    /// Builds proof counters and deterministic transfer-gate status from entry metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn from_entry_counters(
        pack_version: u32,
        entries_declared: usize,
        entries_parsed: usize,
        entries_materialized: usize,
        unique_objects_materialized: usize,
        duplicate_entry_count_materialized: usize,
        hash_algorithm: String,
        computed_pack_checksum: String,
        trailer_pack_checksum: String,
    ) -> Self {
        let transfer_allowed = entries_materialized == entries_declared;
        let blocked_reason = if transfer_allowed {
            None
        } else {
            Some(format!(
                "materialized entries below declared count: materialized={}, declared={}",
                entries_materialized, entries_declared
            ))
        };

        Self {
            verification_status: if transfer_allowed {
                "ok".to_string()
            } else {
                "blocked".to_string()
            },
            pack_version,
            declared_object_count: entries_declared,
            processed_object_count: entries_parsed,
            entries_declared,
            entries_parsed,
            entries_materialized,
            unique_objects_materialized,
            duplicate_entry_count_materialized,
            transfer_allowed,
            blocked_reason,
            hash_algorithm,
            computed_pack_checksum,
            trailer_pack_checksum,
        }
    }
}

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
    /// Declared output size from PACK entry header.
    pub out_size: usize,
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
#[allow(dead_code)]
pub enum ResolutionSource {
    /// Fully resolved from in-pack data.
    InPack,
    /// Resolved with baseline/external object database assistance.
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structured result of PACK-proof verification with entry-level ledger truth.
pub struct PayloadPackVerification {
    /// Summary proof counters/checksums.
    pub proof: PayloadPackProof,
    /// Authoritative parsed PACK entry ledger.
    pub ledger: PackEntryLedger,
    /// Deduplicated materialized objects derived from ledger result rows.
    pub materialized_index: MaterializedObjectIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Deduplicated object index derived from materialized ledger entries.
pub struct MaterializedObjectIndex {
    /// Unique object rows in deterministic order.
    pub objects: Vec<MaterializedObjectRecord>,
    /// Number of materialized ledger entries (before deduplication).
    pub materialized_entry_count: usize,
    /// Number of unique materialized objects (after deduplication).
    pub unique_object_count: usize,
    /// Materialized duplicate-entry count (`materialized_entry_count - unique_object_count`).
    pub duplicate_entry_count_materialized: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One unique materialized object row derived from PACK ledger entries.
pub struct MaterializedObjectRecord {
    /// Canonical object id.
    pub oid: git2::Oid,
    /// Canonical object kind.
    pub kind: PayloadObjectKind,
    /// Uncompressed object size in bytes.
    pub size_bytes: usize,
    /// First ledger entry index where this object was observed.
    pub first_entry_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structured fail-closed error for payload PACK verification.
pub struct PayloadAuditError {
    /// Human-readable reason string.
    pub reason: String,
    /// Optional zero-based index of the blocked entry.
    pub blocked_entry_idx: Option<usize>,
    /// Optional partial ledger captured before failure.
    pub ledger_partial: Option<PackEntryLedger>,
}

impl std::fmt::Display for PayloadAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.blocked_entry_idx, &self.ledger_partial) {
            (Some(idx), Some(ledger)) => write!(
                f,
                "{} (blocked_entry_idx={}, entries_parsed={}, entries_declared={})",
                self.reason,
                idx,
                ledger.entries.len(),
                ledger.declared_entry_count
            ),
            (Some(idx), None) => write!(f, "{} (blocked_entry_idx={})", self.reason, idx),
            (None, Some(ledger)) => write!(
                f,
                "{} (entries_parsed={}, entries_declared={})",
                self.reason,
                ledger.entries.len(),
                ledger.declared_entry_count
            ),
            (None, None) => f.write_str(&self.reason),
        }
    }
}

impl std::error::Error for PayloadAuditError {}

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

#[derive(Debug, Clone, PartialEq, Eq)]
/// Result of receiving a bundle or running receive in dry-run mode.
pub struct ReceiveBundleResult {
    /// Detected version of the imported bundle.
    pub bundle_version: BundleVersion,
    /// Heads that were (or would be) imported.
    pub imported_heads: Vec<BundleHead>,
    /// Whether the import can be applied cleanly.
    pub can_apply_without_conflicts: bool,
    /// Per-file additions/deletions produced during dry-run analysis.
    pub line_stats: Vec<FileLineStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Compact per-file line-delta summary.
pub struct FileLineStat {
    /// Repository-relative file path.
    pub path: String,
    /// Number of added lines.
    pub additions: usize,
    /// Number of deleted lines.
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Commit-level entry used by commit pages in the TUI.
pub struct CommitAuditEntry {
    /// Commit object ID.
    pub commit_id: git2::Oid,
    /// First line of commit message.
    pub subject: String,
    /// Committer identity/time metadata.
    pub committer: CommitAuditIdentity,
    /// Author identity/time metadata.
    pub author: CommitAuditIdentity,
    /// Changed files summary for this commit.
    pub files: Vec<FileLineStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Head-scoped commit and line-stat entries used by multi-head UI views.
pub struct HeadAuditEntry {
    /// Bundle head this entry represents.
    pub head: BundleHead,
    /// Per-head dry-run file summary.
    pub line_stats: Vec<FileLineStat>,
    /// Commit entries reachable from this head.
    pub commits: Vec<CommitAuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Canonicalized git identity and timestamp offset information.
pub struct CommitAuditIdentity {
    /// Identity name component.
    pub name: String,
    /// Identity email component.
    pub email: String,
    /// Unix timestamp in seconds.
    pub time_seconds: i64,
    /// UTC offset in minutes.
    pub offset_minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Options controlling bundle creation behavior.
pub struct CreateBundleOptions {
    /// When true, emit a `.caudit.patch` unified-diff sidecar.
    pub include_patch_sidecar: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Options controlling receive/verification behavior.
pub struct ReceiveBundleOptions {
    /// When true, validate metadata integrity before import.
    pub verify_metadata: bool,
    /// When true, apply into a temporary mirror and report would-change stats.
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditMetadata {
    pub(crate) schema_version: String,
    pub(crate) tool_version: String,
    pub(crate) generated_at_unix_secs: u64,
    pub(crate) generated_by_username: String,
    pub(crate) generated_by_hostname: String,
    pub(crate) bundle_path: String,
    pub(crate) bundle_size_bytes: u64,
    pub(crate) bundle_sha256: String,
    pub(crate) bundle_header_version: String,
    pub(crate) prerequisites: Vec<String>,
    pub(crate) heads: Vec<CreateBundleAuditHead>,
    pub(crate) range_from_oid: String,
    pub(crate) range_to_oid: String,
    pub(crate) tip_ref: String,
    pub(crate) commit_chain: Vec<CreateBundleAuditCommit>,
    pub(crate) changed_files: Vec<CreateBundleAuditChangedFile>,
    pub(crate) patch_sidecar: Option<CreateBundleAuditPatchSidecar>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditHead {
    pub(crate) oid: String,
    pub(crate) reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditCommit {
    pub(crate) oid: String,
    pub(crate) tree_oid: String,
    pub(crate) parent_oids: Vec<String>,
    pub(crate) subject: String,
    pub(crate) author: CreateBundleAuditSignature,
    pub(crate) committer: CreateBundleAuditSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditSignature {
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) time_seconds: i64,
    pub(crate) offset_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditChangedFile {
    pub(crate) status: String,
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
    pub(crate) old_oid: Option<String>,
    pub(crate) new_oid: Option<String>,
    pub(crate) old_mode: Option<String>,
    pub(crate) new_mode: Option<String>,
    pub(crate) is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CreateBundleAuditPatchSidecar {
    pub(crate) path: String,
    pub(crate) format: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffEntry {
    pub(crate) status: ChangeStatus,
    pub(crate) path: String,
    pub(crate) old_path: Option<String>,
    pub(crate) old_oid: Option<git2::Oid>,
    pub(crate) new_oid: Option<git2::Oid>,
    pub(crate) old_mode: Option<u32>,
    pub(crate) new_mode: Option<u32>,
    pub(crate) is_binary: bool,
}
