//! Shared payload kind labels for non-interactive output.

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
