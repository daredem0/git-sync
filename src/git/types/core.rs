// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Typed data models for core domain concepts.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

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
