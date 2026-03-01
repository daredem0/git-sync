// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! PACK payload verification step for object.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::digest::sha1_hex;
use crate::git::types::{PackEntryKind, PayloadObjectKind};
use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub(super) struct ParsedPackObject {
    pub(super) kind: PayloadObjectKind,
    pub(super) content: Vec<u8>,
}

/// Returns the canonical object id for `(kind, content)` as SHA-1.
pub(super) fn object_oid_for_content(kind: PayloadObjectKind, content: &[u8]) -> Result<git2::Oid> {
    let type_name = match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => bail!("cannot hash unknown pack object kind"),
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(format!("{type_name} {}\0", content.len()).as_bytes());
    bytes.extend_from_slice(content);
    let digest_hex = sha1_hex(&bytes)?;
    Ok(git2::Oid::from_str(&digest_hex)?)
}

/// Loads one baseline object by OID for external ref-delta base resolution.
pub(super) fn load_parsed_object_from_odb(
    odb: &git2::Odb<'_>,
    oid: git2::Oid,
) -> Result<ParsedPackObject> {
    let object = odb.read(oid)?;
    let kind = payload_kind_from_git(object.kind());
    if matches!(kind, PayloadObjectKind::Unknown) {
        bail!("baseline base object has unsupported type for delta resolution: {oid}");
    }
    Ok(ParsedPackObject {
        kind,
        content: object.data().to_vec(),
    })
}

/// Converts one pack entry kind into payload object kind.
pub(super) fn pack_entry_kind_to_payload_kind(kind: PackEntryKind) -> PayloadObjectKind {
    match kind {
        PackEntryKind::Commit => PayloadObjectKind::Commit,
        PackEntryKind::Tree => PayloadObjectKind::Tree,
        PackEntryKind::Blob => PayloadObjectKind::Blob,
        PackEntryKind::Tag => PayloadObjectKind::Tag,
        PackEntryKind::OfsDelta | PackEntryKind::RefDelta => PayloadObjectKind::Unknown,
    }
}

fn payload_kind_from_git(kind: git2::ObjectType) -> PayloadObjectKind {
    match kind {
        git2::ObjectType::Commit => PayloadObjectKind::Commit,
        git2::ObjectType::Tree => PayloadObjectKind::Tree,
        git2::ObjectType::Blob => PayloadObjectKind::Blob,
        git2::ObjectType::Tag => PayloadObjectKind::Tag,
        _ => PayloadObjectKind::Unknown,
    }
}

#[cfg(test)]
mod tests;
