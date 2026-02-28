//! Git-domain operations for bundle creation, receive, diffing, and metadata.

mod archive;
mod bundle;
mod context;
mod diff;
mod digest;
mod metadata;
mod types;
mod util;

pub(crate) use bundle::PayloadSession;
pub(crate) use bundle::{
    build_payload_audit_document_for_bundle_input_with_options,
    collect_commit_file_patch_for_bundle_input, collect_head_audit_entries_for_bundle_input,
    collect_payload_audit_for_bundle_input_with_resolve_mode,
    collect_payload_object_detail_for_bundle_input, collect_payload_object_detail_for_session,
    create_bundle, create_bundle_with_options, inspect_bundle, open_payload_session,
    payload_audit_from_session, receive_bundle_input, receive_bundle_input_with_options,
    remove_unarchived_bundle_artifacts,
};
pub(crate) use context::open_context;
pub(crate) use metadata::verify_bundle_metadata_against_repo_input;
pub(crate) use types::{
    BundleHead, BundleInspection, BundleVersion, ChangeStatus, CommitAuditEntry,
    CommitAuditIdentity, CreateBundleOptions, FileLineStat, HeadAuditEntry, OpenContext,
    PackEntryBaseRef, PackEntryKind, PackEntryRecord, PayloadAudit, PayloadAuditDocument,
    PayloadAuditLedgerMode, PayloadObjectDetail, PayloadObjectEntry, PayloadObjectKind,
    PayloadPackVerification, PayloadResolveMode, ReceiveBundleOptions, ReceiveBundleResult,
    ResolutionSource,
};

#[cfg(test)]
pub(crate) use types::{
    CreateBundleResult, PackEntryLedger, PayloadPackProof, PayloadTransportEntry,
};

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
