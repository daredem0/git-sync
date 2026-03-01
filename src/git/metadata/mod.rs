// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Metadata domain module wiring and exports.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

mod collect;
mod load;
mod patch;
mod verify;

#[cfg(test)]
pub(crate) use verify::verify_bundle_metadata_against_repo;
pub use verify::verify_bundle_metadata_against_repo_input;

pub(crate) use collect::{collect_changed_files_for_metadata, collect_commit_chain_for_metadata};
#[cfg(test)]
pub(crate) use load::load_bundle_metadata_from_path;
pub(crate) use patch::write_patch_sidecar;
#[cfg(test)]
pub(crate) use verify::verify_bundle_metadata_integrity;
pub(crate) use verify::verify_bundle_metadata_integrity_input;
