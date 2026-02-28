//! Git-layer types functionality.

mod core;
mod create;
mod payload_document;
mod payload_ledger;
mod payload_materialized;
mod payload_model;
mod payload_proof;
mod receive;

pub use core::{BundleHead, BundleInspection, BundleVersion, ChangeStatus, OpenContext};
pub use create::{CreateBundleOptions, CreateBundleResult};
pub use payload_document::{
    PayloadAuditDocument, PayloadAuditDocumentEntryLedger, PayloadAuditDocumentHead,
    PayloadAuditDocumentObjectDetail, PayloadAuditDocumentPackEntry,
    PayloadAuditDocumentPackObject, PayloadAuditDocumentTransportEntry, PayloadAuditLedgerMode,
    PayloadAuditPackSummary,
};
pub use payload_ledger::{
    PackEntryBaseRef, PackEntryKind, PackEntryLedger, PackEntryRecord, ResolutionSource,
};
pub use payload_materialized::{
    MaterializedObjectData, MaterializedObjectIndex, MaterializedObjectRecord,
    MaterializedObjectStore, PayloadAuditError, PayloadPackVerification,
};
pub use payload_model::{
    PayloadAudit, PayloadObjectDetail, PayloadObjectEntry, PayloadObjectKind, PayloadResolveMode,
    PayloadTransportEntry,
};
pub use payload_proof::PayloadPackProof;
pub use receive::{
    CommitAuditEntry, CommitAuditIdentity, FileLineStat, HeadAuditEntry, ReceiveBundleOptions,
    ReceiveBundleResult,
};

pub(crate) use core::DiffEntry;
pub(crate) use create::{
    CreateBundleAuditChangedFile, CreateBundleAuditCommit, CreateBundleAuditHead,
    CreateBundleAuditMetadata, CreateBundleAuditPatchSidecar, CreateBundleAuditSignature,
};
