//! PACK verification and materialization for payload audit.
//!
//! Proof invariants enforced at the `VerifiedPayload` boundary:
//! - `entries_parsed == entries_declared`
//! - `entries_materialized == entries_declared`
//! - checksum verification succeeded
//! - transfer gate is allowed
//! - ledger/index counters are internally consistent
//!
//! Any violation returns a fail-closed `PayloadAuditError` and blocks transfer.

mod core;
mod delta;
mod entry;
mod materialized;
mod object;
mod pack;
mod preflight;
mod proof;
mod zlib;

use crate::git::types::PayloadAuditError;

pub(super) fn into_verification(
    value: proof::VerifiedPayload,
) -> crate::git::PayloadPackVerification {
    value.into_verification()
}

pub(super) fn verify_pack_payload(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<proof::VerifiedPayload, PayloadAuditError> {
    let verification = core::verify_pack_payload_impl(pack_data, baseline_odb)?;
    proof::verify_payload_invariants(verification)
}
