// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Test-only adapter helpers for payload verification.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Keeps production payload APIs focused while exposing deterministic test entry points.

use super::input;
use super::parse;
use super::verify_pack_payload_with_ledger_and_baseline_odb;
use crate::git::types::{PayloadAuditError, PayloadPackVerification};
use std::path::Path;

/// Verifies pack proof + ledger directly from a bundle or packaged zip input.
pub(crate) fn verify_pack_payload_for_bundle_input(
    bundle_input_path: &Path,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    verify_pack_payload_for_bundle_input_with_resolve_mode(bundle_input_path, None)
}

/// Verifies pack proof + ledger from bundle input with optional baseline resolve repository.
pub(crate) fn verify_pack_payload_for_bundle_input_with_resolve_mode(
    bundle_input_path: &Path,
    baseline_repo_path: Option<&Path>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    let baseline_repo = baseline_repo_path.and_then(|path| git2::Repository::open(path).ok());
    let baseline_odb = baseline_repo.as_ref().and_then(|repo| repo.odb().ok());
    let loaded = input::load_payload_input(bundle_input_path).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    let parsed_bundle =
        parse::parse_bundle_payload(&loaded.bundle_bytes).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    verify_pack_payload_with_ledger_and_baseline_odb(parsed_bundle.pack_data, baseline_odb.as_ref())
}
