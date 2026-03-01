// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Output formatting logic for kind views.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

/// Returns stable label for payload object kind.
pub(super) fn payload_kind_label(kind: crate::git::PayloadObjectKind) -> &'static str {
    match kind {
        crate::git::PayloadObjectKind::Commit => "commit",
        crate::git::PayloadObjectKind::Tree => "tree",
        crate::git::PayloadObjectKind::Blob => "blob",
        crate::git::PayloadObjectKind::Tag => "tag",
        crate::git::PayloadObjectKind::Unknown => "unknown",
    }
}
