// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Bundle processing module for payload operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::{
    BundleInspection, MaterializedObjectData, PayloadAudit, PayloadAuditDocument,
    PayloadAuditError, PayloadObjectDetail, PayloadPackVerification, PayloadResolveMode,
};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use super::parse;

mod context;
mod detail;
mod document;
mod input;
mod session;
#[cfg(test)]
mod test_api;
mod verify;

/// Reusable payload session for fast object-detail queries.
#[derive(Debug)]
pub struct PayloadSession {
    inspection: BundleInspection,
    payload: PayloadAudit,
    materialized_store_by_oid: HashMap<git2::Oid, MaterializedObjectData>,
    blob_paths_by_oid: HashMap<git2::Oid, Vec<String>>,
    bundle_path: String,
    bundle_size_bytes: u64,
    bundle_sha256: String,
}

/// Collects transport-entry and pack-object payload audit data for a bundle input with explicit resolve mode.
///
/// # Errors
///
/// Returns an error when the bundle input cannot be parsed/imported or objects
/// cannot be enumerated.
pub fn collect_payload_audit_for_bundle_input_with_resolve_mode(
    bundle_input_path: &Path,
    repo_path: &Path,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadAudit> {
    let session =
        open_payload_session_with_resolve_mode(bundle_input_path, repo_path, resolve_mode)?;
    Ok(payload_audit_from_session(&session))
}

/// Builds a serialized payload-audit JSON document with explicit ledger and resolve modes.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails or object-detail
/// materialization fails.
pub fn build_payload_audit_document_for_bundle_input_with_options(
    bundle_input_path: &Path,
    repo_path: &Path,
    ledger_mode: crate::git::PayloadAuditLedgerMode,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadAuditDocument> {
    let session =
        open_payload_session_with_resolve_mode(bundle_input_path, repo_path, resolve_mode)?;
    payload_audit_document_from_session_with_ledger_mode(&session, ledger_mode)
}

/// Collects detail lines for one selected payload object.
///
/// # Errors
///
/// Returns an error when the object is unavailable in the imported payload.
pub fn collect_payload_object_detail_for_bundle_input(
    bundle_input_path: &Path,
    repo_path: &Path,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    let session = open_payload_session_with_resolve_mode(
        bundle_input_path,
        repo_path,
        PayloadResolveMode::PackOnly,
    )?;
    collect_payload_object_detail_for_session(&session, object_id)
}

/// Opens an imported payload session that can be reused across many detail lookups.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails.
pub fn open_payload_session(bundle_input_path: &Path, repo_path: &Path) -> Result<PayloadSession> {
    open_payload_session_with_resolve_mode(
        bundle_input_path,
        repo_path,
        PayloadResolveMode::PackOnly,
    )
}

/// Opens an imported payload session that can be reused across many detail lookups with explicit resolve mode.
///
/// # Errors
///
/// Returns an error when bundle import/inspection fails.
pub fn open_payload_session_with_resolve_mode(
    bundle_input_path: &Path,
    repo_path: &Path,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadSession> {
    session::open_payload_session_with_resolve_mode_impl(bundle_input_path, repo_path, resolve_mode)
}

/// Returns a payload-audit snapshot captured in the provided session.
pub fn payload_audit_from_session(session: &PayloadSession) -> PayloadAudit {
    session.payload.clone()
}

/// Builds a serialized payload-audit JSON document from a reusable session using a selected ledger mode.
///
/// # Errors
///
/// Returns an error when object detail collection fails for any payload object.
pub fn payload_audit_document_from_session_with_ledger_mode(
    session: &PayloadSession,
    ledger_mode: crate::git::PayloadAuditLedgerMode,
) -> Result<PayloadAuditDocument> {
    document::payload_audit_document_from_session_with_ledger_mode(session, ledger_mode)
}

/// Collects detail lines for one selected payload object from a reusable session.
///
/// # Errors
///
/// Returns an error when the object is unavailable in session object storage.
pub fn collect_payload_object_detail_for_session(
    session: &PayloadSession,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    detail::collect_payload_object_detail_from_store(
        &session.materialized_store_by_oid,
        &session.blob_paths_by_oid,
        object_id,
    )
}

#[cfg(test)]
pub(crate) use test_api::{
    verify_pack_payload_for_bundle_input, verify_pack_payload_for_bundle_input_with_resolve_mode,
};

/// Verifies payload PACK bytes and returns proof + entry ledger truth with optional baseline ODB resolution.
pub fn verify_pack_payload_with_ledger_and_baseline_odb(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    verify::verify_pack_payload(pack_data, baseline_odb).map(verify::into_verification)
}
