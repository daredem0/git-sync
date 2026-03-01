// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/entry.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::git::types::PayloadObjectKind;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write as _;

fn empty_ledger() -> PackEntryLedger {
    PackEntryLedger {
        pack_version: 2,
        declared_entry_count: 1,
        entries: Vec::new(),
    }
}

fn encode_pack_entry_header(kind: PackEntryKind, size: usize) -> Vec<u8> {
    let kind_code = match kind {
        PackEntryKind::Commit => 1u8,
        PackEntryKind::Tree => 2u8,
        PackEntryKind::Blob => 3u8,
        PackEntryKind::Tag => 4u8,
        PackEntryKind::OfsDelta => 6u8,
        PackEntryKind::RefDelta => 7u8,
    };
    let mut out = Vec::new();
    let mut remaining = size >> 4;
    let mut first = (kind_code << 4) | ((size & 0x0f) as u8);
    if remaining != 0 {
        first |= 0x80;
    }
    out.push(first);
    while remaining != 0 {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }
    out
}

fn encode_delta_varint(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_literal_delta(base_size: usize, target_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_delta_varint(base_size, &mut out);
    encode_delta_varint(target_bytes.len(), &mut out);
    out.push(target_bytes.len() as u8);
    out.extend_from_slice(target_bytes);
    out
}

fn zlib_compress(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .expect("must write zlib input bytes");
    encoder.finish().expect("must finalize zlib encoding")
}

fn base_blob() -> (git2::Oid, ParsedPackObject) {
    let base = ParsedPackObject {
        kind: PayloadObjectKind::Blob,
        content: b"base\n".to_vec(),
    };
    let oid = super::object_oid_for_content(base.kind, &base.content)
        .expect("must hash base object content");
    (oid, base)
}

#[test]
fn process_next_entry_rejects_ref_delta_missing_base_oid_bytes() {
    let pack = encode_pack_entry_header(PackEntryKind::RefDelta, 1);
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("missing ref-delta base oid bytes should fail");
    assert!(
        error
            .reason
            .contains("ref-delta entry is missing base object id bytes"),
        "error should explicitly report missing base object id bytes"
    );
}

#[test]
fn process_next_entry_rejects_unresolved_ref_delta_base() {
    let unresolved_base = git2::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .expect("must parse unresolved base oid");
    let mut pack = encode_pack_entry_header(PackEntryKind::RefDelta, 1);
    pack.extend_from_slice(unresolved_base.as_bytes());
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("unresolved ref-delta base should fail closed");
    assert!(
        error
            .reason
            .contains("ref-delta references unresolved base object"),
        "error should report unresolved ref-delta base object"
    );
}

#[test]
fn process_next_entry_rejects_full_object_zlib_decode_failure() {
    let mut pack = encode_pack_entry_header(PackEntryKind::Blob, 1);
    pack.extend_from_slice(&[0xff, 0x00, 0x00]);
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("invalid zlib data should fail full-object decode");
    assert!(
        error.reason.contains("failed to decompress pack object"),
        "error should include full-object zlib decode context"
    );
}

#[test]
fn process_next_entry_rejects_full_object_size_mismatch() {
    let mut pack = encode_pack_entry_header(PackEntryKind::Blob, 7);
    pack.extend_from_slice(&zlib_compress(b"abc"));
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("mismatched full-object size should fail");
    assert!(
        error.reason.contains("pack object size mismatch"),
        "error should report full-object size mismatch"
    );
}

#[test]
fn process_next_entry_rejects_unresolved_ofs_delta_base() {
    let mut pack = vec![0u8];
    pack.extend_from_slice(&encode_pack_entry_header(PackEntryKind::OfsDelta, 1));
    pack.push(1);
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        1,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("unresolved ofs-delta base should fail closed");
    assert!(
        error
            .reason
            .contains("ofs-delta references unresolved base"),
        "error should report unresolved ofs-delta base"
    );
}

#[test]
fn process_next_entry_rejects_ofs_delta_zlib_decode_failure() {
    let (_base_oid, base) = base_blob();
    let mut pack = encode_pack_entry_header(PackEntryKind::OfsDelta, 1);
    pack.push(0);
    pack.extend_from_slice(&[0xff, 0x00, 0x00]);
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    by_offset.insert(0usize, base);
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("invalid zlib data should fail ofs-delta decode");
    assert!(
        error
            .reason
            .contains("failed to decompress ofs-delta object"),
        "error should include ofs-delta zlib decode context"
    );
}

#[test]
fn process_next_entry_rejects_ofs_delta_size_mismatch() {
    let (_base_oid, base) = base_blob();
    let delta = encode_literal_delta(base.content.len(), b"x\n");
    let mut pack = encode_pack_entry_header(PackEntryKind::OfsDelta, delta.len() + 1);
    pack.push(0);
    pack.extend_from_slice(&zlib_compress(&delta));
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    by_offset.insert(0usize, base);
    let mut by_oid = HashMap::new();
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("ofs-delta stream-size mismatch should fail");
    assert!(
        error.reason.contains("ofs-delta stream size mismatch"),
        "error should report ofs-delta stream-size mismatch"
    );
}

#[test]
fn process_next_entry_rejects_ref_delta_zlib_decode_failure() {
    let (base_oid, base) = base_blob();
    let mut pack = encode_pack_entry_header(PackEntryKind::RefDelta, 1);
    pack.extend_from_slice(base_oid.as_bytes());
    pack.extend_from_slice(&[0xff, 0x00, 0x00]);
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    by_oid.insert(base_oid, base);
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("invalid zlib data should fail ref-delta decode");
    assert!(
        error
            .reason
            .contains("failed to decompress ref-delta object"),
        "error should include ref-delta zlib decode context"
    );
}

#[test]
fn process_next_entry_rejects_ref_delta_size_mismatch() {
    let (base_oid, base) = base_blob();
    let delta = encode_literal_delta(base.content.len(), b"x\n");
    let mut pack = encode_pack_entry_header(PackEntryKind::RefDelta, delta.len() + 1);
    pack.extend_from_slice(base_oid.as_bytes());
    pack.extend_from_slice(&zlib_compress(&delta));
    let trailer_offset = pack.len();
    let mut ledger = empty_ledger();
    let mut by_offset = HashMap::new();
    let mut by_oid = HashMap::new();
    by_oid.insert(base_oid, base);
    let mut thin_pack_detected = false;
    let mut baseline_resolutions = 0usize;

    let error = process_next_entry(
        &pack,
        trailer_offset,
        None,
        0,
        0,
        EntryProcessingState {
            ledger: &mut ledger,
            objects_by_offset: &mut by_offset,
            objects_by_oid: &mut by_oid,
            thin_pack_detected: &mut thin_pack_detected,
            baseline_resolutions_count: &mut baseline_resolutions,
        },
    )
    .err()
    .expect("ref-delta stream-size mismatch should fail");
    assert!(
        error.reason.contains("ref-delta stream size mismatch"),
        "error should report ref-delta stream-size mismatch"
    );
}
