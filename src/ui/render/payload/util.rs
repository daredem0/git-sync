// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload rendering module for util views.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::git::{PackEntryBaseRef, PackEntryKind, PayloadObjectKind};
use ratatui::style::{Color, Style};

/// Returns compact display label for payload object kind.
pub(in crate::ui::render::payload) fn payload_kind_label(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

/// Returns semantic style for payload object kind labels.
pub(in crate::ui::render::payload) fn payload_kind_style(kind: PayloadObjectKind) -> Style {
    match kind {
        PayloadObjectKind::Commit => Style::default().fg(Color::Yellow),
        PayloadObjectKind::Tree => Style::default().fg(Color::White),
        PayloadObjectKind::Blob => Style::default().fg(Color::Blue),
        PayloadObjectKind::Tag => Style::default().fg(Color::Magenta),
        PayloadObjectKind::Unknown => Style::default(),
    }
}

/// Returns compact display label for pack entry kind.
pub(in crate::ui::render::payload) fn payload_entry_kind_label(
    kind: PackEntryKind,
) -> &'static str {
    match kind {
        PackEntryKind::Commit => "commit",
        PackEntryKind::Tree => "tree",
        PackEntryKind::Blob => "blob",
        PackEntryKind::Tag => "tag",
        PackEntryKind::OfsDelta => "ofs-delta",
        PackEntryKind::RefDelta => "ref-delta",
    }
}

/// Returns semantic style for pack-entry kind labels.
pub(in crate::ui::render::payload) fn payload_entry_kind_style(kind: PackEntryKind) -> Style {
    match kind {
        PackEntryKind::Commit => payload_kind_style(PayloadObjectKind::Commit),
        PackEntryKind::Tree => payload_kind_style(PayloadObjectKind::Tree),
        PackEntryKind::Blob => payload_kind_style(PayloadObjectKind::Blob),
        PackEntryKind::Tag => payload_kind_style(PayloadObjectKind::Tag),
        PackEntryKind::OfsDelta | PackEntryKind::RefDelta => Style::default(),
    }
}

/// Returns compact display label for pack entry base references.
pub(in crate::ui::render::payload) fn payload_entry_base_ref_label(
    base_ref: Option<&PackEntryBaseRef>,
) -> String {
    match base_ref {
        Some(PackEntryBaseRef::BaseOffset { distance, .. }) => format!("ofs:{distance}"),
        Some(PackEntryBaseRef::BaseOid(oid)) => format!("oid:{}", short_oid(*oid)),
        None => "-".to_string(),
    }
}

/// Returns shortened digest prefix for compact table output.
pub(in crate::ui::render::payload) fn short_sha256(digest: &str) -> String {
    if digest.len() <= 12 {
        digest.to_string()
    } else {
        digest[..12].to_string()
    }
}

/// Returns shortened object id prefix for compact table output.
pub(in crate::ui::render::payload) fn short_oid(oid: git2::Oid) -> String {
    let full = oid.to_string();
    full[..12].to_string()
}

/// Computes the number of digits needed for line-number gutters.
pub(in crate::ui::render::payload) fn line_number_width(total_lines: usize) -> usize {
    let mut n = total_lines.max(1);
    let mut digits = 1usize;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> git2::Oid {
        git2::Oid::from_str(hex).expect("must parse test oid")
    }

    #[test]
    fn payload_kind_label_covers_all_kinds() {
        assert_eq!(payload_kind_label(PayloadObjectKind::Commit), "commit");
        assert_eq!(payload_kind_label(PayloadObjectKind::Tree), "tree");
        assert_eq!(payload_kind_label(PayloadObjectKind::Blob), "blob");
        assert_eq!(payload_kind_label(PayloadObjectKind::Tag), "tag");
        assert_eq!(payload_kind_label(PayloadObjectKind::Unknown), "unknown");
    }

    #[test]
    fn payload_entry_kind_label_covers_all_pack_entry_kinds() {
        assert_eq!(payload_entry_kind_label(PackEntryKind::Commit), "commit");
        assert_eq!(payload_entry_kind_label(PackEntryKind::Tree), "tree");
        assert_eq!(payload_entry_kind_label(PackEntryKind::Blob), "blob");
        assert_eq!(payload_entry_kind_label(PackEntryKind::Tag), "tag");
        assert_eq!(
            payload_entry_kind_label(PackEntryKind::OfsDelta),
            "ofs-delta"
        );
        assert_eq!(
            payload_entry_kind_label(PackEntryKind::RefDelta),
            "ref-delta"
        );
    }

    #[test]
    fn payload_entry_base_ref_label_formats_all_base_ref_variants() {
        assert_eq!(
            payload_entry_base_ref_label(Some(&PackEntryBaseRef::BaseOffset {
                distance: 42,
                base_offset: Some(7),
            })),
            "ofs:42"
        );

        let oid = oid("1111111111111111111111111111111111111111");
        assert_eq!(
            payload_entry_base_ref_label(Some(&PackEntryBaseRef::BaseOid(oid))),
            "oid:111111111111"
        );
        assert_eq!(payload_entry_base_ref_label(None), "-");
    }

    #[test]
    fn short_sha256_preserves_short_and_truncates_long_digests() {
        assert_eq!(short_sha256("abc"), "abc");
        assert_eq!(
            short_sha256("1234567890abcdef"),
            "1234567890ab",
            "long digests should be truncated to 12 chars"
        );
    }

    #[test]
    fn short_oid_returns_12_char_prefix() {
        let oid = oid("2222222222222222222222222222222222222222");
        assert_eq!(short_oid(oid), "222222222222");
    }

    #[test]
    fn line_number_width_handles_zero_and_multi_digit_counts() {
        assert_eq!(line_number_width(0), 1);
        assert_eq!(line_number_width(9), 1);
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(999), 3);
    }
}
