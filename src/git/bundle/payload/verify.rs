// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Top-level PACK payload verification orchestration.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

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
