// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/render/payload/util.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

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
