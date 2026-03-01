//! PACK entry parsing/materialization logic.

use crate::git::types::{
    PackEntryBaseRef, PackEntryKind, PackEntryLedger, PackEntryRecord, PayloadAuditError,
    ResolutionSource,
};
use anyhow::anyhow;
use std::collections::HashMap;

use super::delta::apply_git_delta;
use super::object::{
    ParsedPackObject, load_parsed_object_from_odb, object_oid_for_content,
    pack_entry_kind_to_payload_kind,
};
use super::pack::{parse_ofs_delta_base_distance, parse_pack_entry_header};
use super::zlib::decompress_zlib_stream;

pub(super) struct EntryProcessOutcome {
    pub(super) next_offset: usize,
}

pub(super) struct EntryProcessingState<'a> {
    pub(super) ledger: &'a mut PackEntryLedger,
    pub(super) objects_by_offset: &'a mut HashMap<usize, ParsedPackObject>,
    pub(super) objects_by_oid: &'a mut HashMap<git2::Oid, ParsedPackObject>,
    pub(super) thin_pack_detected: &'a mut bool,
    pub(super) baseline_resolutions_count: &'a mut usize,
}

pub(super) fn process_next_entry(
    pack_data: &[u8],
    trailer_offset: usize,
    baseline_odb: Option<&git2::Odb<'_>>,
    offset: usize,
    processed_object_count: usize,
    mut state: EntryProcessingState<'_>,
) -> std::result::Result<EntryProcessOutcome, PayloadAuditError> {
    let entry_offset = offset;
    let (entry_kind, expected_header_size, header_end) = parse_pack_entry_header(pack_data, offset)
        .map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    let mut entry_record = PackEntryRecord {
        idx: processed_object_count,
        offset: entry_offset,
        kind: entry_kind,
        out_size: expected_header_size,
        reconstructed_size: None,
        base_ref: None,
        result_oid: None,
        result_kind: None,
        resolved: false,
        resolved_via: None,
        note: None,
    };

    let mut data_offset = header_end;
    let mut entry_resolved_via = ResolutionSource::InPack;

    let parsed = match entry_kind {
        PackEntryKind::Commit | PackEntryKind::Tree | PackEntryKind::Blob | PackEntryKind::Tag => {
            process_full_object(
                pack_data,
                trailer_offset,
                processed_object_count,
                entry_kind,
                expected_header_size,
                data_offset,
                state.ledger,
                &mut entry_record,
            )
            .map(|(parsed, next_offset)| {
                data_offset = next_offset;
                parsed
            })?
        }
        PackEntryKind::OfsDelta => process_ofs_delta(
            pack_data,
            trailer_offset,
            processed_object_count,
            entry_offset,
            expected_header_size,
            data_offset,
            &mut state,
            &mut entry_record,
        )
        .map(|(parsed, next_offset)| {
            data_offset = next_offset;
            parsed
        })?,
        PackEntryKind::RefDelta => process_ref_delta(
            pack_data,
            trailer_offset,
            baseline_odb,
            processed_object_count,
            expected_header_size,
            data_offset,
            &mut state,
            &mut entry_record,
            &mut entry_resolved_via,
        )
        .map(|(parsed, next_offset)| {
            data_offset = next_offset;
            parsed
        })?,
    };

    let object_oid =
        object_oid_for_content(parsed.kind, &parsed.content).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;
    entry_record.result_oid = Some(object_oid);
    entry_record.result_kind = Some(parsed.kind);
    entry_record.resolved = true;
    entry_record.resolved_via = Some(entry_resolved_via);
    state.ledger.entries.push(entry_record);
    state.objects_by_offset.insert(entry_offset, parsed.clone());
    state.objects_by_oid.insert(object_oid, parsed);

    Ok(EntryProcessOutcome {
        next_offset: data_offset,
    })
}

#[allow(clippy::too_many_arguments)]
fn process_full_object(
    pack_data: &[u8],
    trailer_offset: usize,
    processed_object_count: usize,
    entry_kind: PackEntryKind,
    expected_object_size: usize,
    data_offset: usize,
    ledger: &PackEntryLedger,
    entry_record: &mut PackEntryRecord,
) -> std::result::Result<(ParsedPackObject, usize), PayloadAuditError> {
    let (consumed, content) = decompress_zlib_stream(&pack_data[data_offset..trailer_offset])
        .map_err(|err| {
            anyhow!(
                "failed to decompress pack object #{} ({:?}) at offset {}: {err}",
                processed_object_count + 1,
                entry_kind,
                data_offset
            )
        })
        .map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger.clone()),
        })?;

    let next_offset = data_offset
        .checked_add(consumed)
        .ok_or_else(|| PayloadAuditError {
            reason: "pack entry offset overflow".to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger.clone()),
        })?;

    if content.len() != expected_object_size {
        return Err(PayloadAuditError {
            reason: format!(
                "pack object size mismatch at object {}: header={}, actual={}",
                processed_object_count + 1,
                expected_object_size,
                content.len()
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger.clone()),
        });
    }

    entry_record.reconstructed_size = Some(content.len());
    Ok((
        ParsedPackObject {
            kind: pack_entry_kind_to_payload_kind(entry_kind),
            content,
        },
        next_offset,
    ))
}

#[allow(clippy::too_many_arguments)]
fn process_ofs_delta(
    pack_data: &[u8],
    trailer_offset: usize,
    processed_object_count: usize,
    entry_offset: usize,
    expected_delta_stream_len: usize,
    data_offset: usize,
    state: &mut EntryProcessingState<'_>,
    entry_record: &mut PackEntryRecord,
) -> std::result::Result<(ParsedPackObject, usize), PayloadAuditError> {
    let (base_offset_distance, consumed) = parse_ofs_delta_base_distance(pack_data, data_offset)
        .map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    let delta_offset = data_offset
        .checked_add(consumed)
        .ok_or_else(|| PayloadAuditError {
            reason: "ofs-delta offset overflow".to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    let base_entry_offset = entry_offset
        .checked_sub(base_offset_distance)
        .ok_or_else(|| PayloadAuditError {
            reason: "ofs-delta base offset underflow".to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    entry_record.base_ref = Some(PackEntryBaseRef::BaseOffset {
        distance: base_offset_distance,
        base_offset: Some(base_entry_offset),
    });

    let Some(base) = state.objects_by_offset.get(&base_entry_offset) else {
        let reason = format!(
            "ofs-delta references unresolved base at distance {} (external dependency/thin pack)",
            base_offset_distance
        );
        entry_record.note = Some(reason.clone());
        state.ledger.entries.push(entry_record.clone());
        return Err(PayloadAuditError {
            reason,
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        });
    };

    let (delta_consumed, delta_bytes) =
        decompress_zlib_stream(&pack_data[delta_offset..trailer_offset])
            .map_err(|err| {
                anyhow!(
                    "failed to decompress ofs-delta object #{} at offset {}: {err}",
                    processed_object_count + 1,
                    delta_offset
                )
            })
            .map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(state.ledger.clone()),
            })?;

    let next_offset =
        delta_offset
            .checked_add(delta_consumed)
            .ok_or_else(|| PayloadAuditError {
                reason: "ofs-delta data offset overflow".to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(state.ledger.clone()),
            })?;

    if delta_bytes.len() != expected_delta_stream_len {
        return Err(PayloadAuditError {
            reason: format!(
                "ofs-delta stream size mismatch at object {}: header={}, actual={}",
                processed_object_count + 1,
                expected_delta_stream_len,
                delta_bytes.len()
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        });
    }

    let content =
        apply_git_delta(&base.content, &delta_bytes).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    entry_record.reconstructed_size = Some(content.len());
    Ok((
        ParsedPackObject {
            kind: base.kind,
            content,
        },
        next_offset,
    ))
}

#[allow(clippy::too_many_arguments)]
fn process_ref_delta(
    pack_data: &[u8],
    trailer_offset: usize,
    baseline_odb: Option<&git2::Odb<'_>>,
    processed_object_count: usize,
    expected_delta_stream_len: usize,
    data_offset: usize,
    state: &mut EntryProcessingState<'_>,
    entry_record: &mut PackEntryRecord,
    entry_resolved_via: &mut ResolutionSource,
) -> std::result::Result<(ParsedPackObject, usize), PayloadAuditError> {
    if trailer_offset.saturating_sub(data_offset) < 20 {
        entry_record.note = Some("ref-delta entry is missing base object id bytes".to_string());
        state.ledger.entries.push(entry_record.clone());
        return Err(PayloadAuditError {
            reason: "ref-delta entry is missing base object id bytes".to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        });
    }

    let base_oid =
        git2::Oid::from_bytes(&pack_data[data_offset..data_offset + 20]).map_err(|err| {
            PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(state.ledger.clone()),
            }
        })?;
    entry_record.base_ref = Some(PackEntryBaseRef::BaseOid(base_oid));
    let delta_offset = data_offset + 20;

    let in_pack_base = state.objects_by_oid.get(&base_oid).cloned();
    let baseline_base = if in_pack_base.is_none() {
        baseline_odb.and_then(|odb| load_parsed_object_from_odb(odb, base_oid).ok())
    } else {
        None
    };
    let Some(base) = in_pack_base.or(baseline_base) else {
        let reason = format!(
            "ref-delta references unresolved base object {} (external dependency/thin pack)",
            base_oid
        );
        entry_record.note = Some(reason.clone());
        state.ledger.entries.push(entry_record.clone());
        return Err(PayloadAuditError {
            reason,
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        });
    };

    if state.objects_by_oid.contains_key(&base_oid) {
        *entry_resolved_via = ResolutionSource::InPack;
    } else {
        *state.thin_pack_detected = true;
        *state.baseline_resolutions_count += 1;
        *entry_resolved_via = ResolutionSource::Baseline;
    }

    let (delta_consumed, delta_bytes) =
        decompress_zlib_stream(&pack_data[delta_offset..trailer_offset])
            .map_err(|err| {
                anyhow!(
                    "failed to decompress ref-delta object #{} at offset {}: {err}",
                    processed_object_count + 1,
                    delta_offset
                )
            })
            .map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(state.ledger.clone()),
            })?;

    let next_offset =
        delta_offset
            .checked_add(delta_consumed)
            .ok_or_else(|| PayloadAuditError {
                reason: "ref-delta data offset overflow".to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(state.ledger.clone()),
            })?;

    if delta_bytes.len() != expected_delta_stream_len {
        return Err(PayloadAuditError {
            reason: format!(
                "ref-delta stream size mismatch at object {}: header={}, actual={}",
                processed_object_count + 1,
                expected_delta_stream_len,
                delta_bytes.len()
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        });
    }

    let content =
        apply_git_delta(&base.content, &delta_bytes).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(state.ledger.clone()),
        })?;

    entry_record.reconstructed_size = Some(content.len());
    Ok((
        ParsedPackObject {
            kind: base.kind,
            content,
        },
        next_offset,
    ))
}

#[cfg(test)]
mod tests {
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
}
