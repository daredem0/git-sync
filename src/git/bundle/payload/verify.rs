//! PACK verification and materialization helpers for payload audit.

use crate::git::types::{
    MaterializedObjectData, MaterializedObjectIndex, MaterializedObjectRecord,
    MaterializedObjectStore, PackEntryBaseRef, PackEntryKind, PackEntryLedger, PackEntryRecord,
    PayloadAuditError, PayloadObjectKind, PayloadPackProof, PayloadPackVerification,
    ResolutionSource,
};
use anyhow::{Result, anyhow, bail};
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::HashMap;

const MAX_BLOB_STORE_BYTES: usize = 4 * 1024 * 1024;
const LARGE_BLOB_PREVIEW_BYTES: usize = 8192;

#[derive(Debug, Clone)]
struct ParsedPackObject {
    kind: PayloadObjectKind,
    content: Vec<u8>,
}

pub(super) fn verify_pack_payload_impl(
    pack_data: &[u8],
    baseline_odb: Option<&git2::Odb<'_>>,
) -> std::result::Result<PayloadPackVerification, PayloadAuditError> {
    if pack_data.len() < 32 {
        return Err(PayloadAuditError {
            reason: "pack payload is too small".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }
    if &pack_data[..4] != b"PACK" {
        return Err(PayloadAuditError {
            reason: "pack payload does not start with PACK header".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    let pack_version = read_be_u32(pack_data, 4).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })?;
    if pack_version != 2 && pack_version != 3 {
        return Err(PayloadAuditError {
            reason: format!("unsupported pack version: {pack_version}"),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }
    let declared_object_count = read_be_u32(pack_data, 8).map_err(|err| PayloadAuditError {
        reason: err.to_string(),
        blocked_entry_idx: None,
        ledger_partial: None,
    })? as usize;

    let trailer_offset = pack_data
        .len()
        .checked_sub(20)
        .ok_or_else(|| PayloadAuditError {
            reason: "pack payload missing trailer checksum".to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    let computed_checksum =
        sha1_hex(&pack_data[..trailer_offset]).map_err(|err| PayloadAuditError {
            reason: err.to_string(),
            blocked_entry_idx: None,
            ledger_partial: None,
        })?;
    let trailer_checksum = hex_encode(&pack_data[trailer_offset..]);
    if computed_checksum != trailer_checksum {
        return Err(PayloadAuditError {
            reason: format!(
                "pack trailer checksum mismatch: computed={}, trailer={}",
                computed_checksum, trailer_checksum
            ),
            blocked_entry_idx: None,
            ledger_partial: None,
        });
    }

    let mut ledger = PackEntryLedger {
        pack_version,
        declared_entry_count: declared_object_count,
        entries: Vec::with_capacity(declared_object_count),
    };
    let mut offset = 12usize;
    let mut processed_object_count = 0usize;
    let mut thin_pack_detected = false;
    let mut baseline_resolutions_count = 0usize;
    let mut objects_by_offset = HashMap::<usize, ParsedPackObject>::new();
    let mut objects_by_oid = HashMap::<git2::Oid, ParsedPackObject>::new();

    while processed_object_count < declared_object_count {
        if offset >= trailer_offset {
            return Err(PayloadAuditError {
                reason: format!(
                    "pack ended before declared object count was processed: declared={}, processed={}",
                    declared_object_count, processed_object_count
                ),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger),
            });
        }

        let entry_offset = offset;
        let (entry_kind, expected_header_size, header_end) =
            parse_pack_entry_header(pack_data, offset).map_err(|err| PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger.clone()),
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
        offset = header_end;

        let mut entry_resolved_via = ResolutionSource::InPack;
        let parsed = match entry_kind {
            PackEntryKind::Commit
            | PackEntryKind::Tree
            | PackEntryKind::Blob
            | PackEntryKind::Tag => {
                let expected_object_size = expected_header_size;
                let (consumed, content) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress pack object #{} ({:?}) at offset {}: {err}",
                                processed_object_count + 1,
                                entry_kind,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
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
                        ledger_partial: Some(ledger),
                    });
                }
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: pack_entry_kind_to_payload_kind(entry_kind),
                    content,
                }
            }
            PackEntryKind::OfsDelta => {
                let (base_offset_distance, consumed) =
                    parse_ofs_delta_base_distance(pack_data, offset).map_err(|err| {
                        PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        }
                    })?;
                offset = offset
                    .checked_add(consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ofs-delta offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let base_entry_offset =
                    entry_offset
                        .checked_sub(base_offset_distance)
                        .ok_or_else(|| PayloadAuditError {
                            reason: "ofs-delta base offset underflow".to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                entry_record.base_ref = Some(PackEntryBaseRef::BaseOffset {
                    distance: base_offset_distance,
                    base_offset: Some(base_entry_offset),
                });
                let Some(base) = objects_by_offset.get(&base_entry_offset) else {
                    let reason = format!(
                        "ofs-delta references unresolved base at distance {} (external dependency/thin pack)",
                        base_offset_distance
                    );
                    entry_record.note = Some(reason.clone());
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason,
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                };
                let (delta_consumed, delta_bytes) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress ofs-delta object #{} at offset {}: {err}",
                                processed_object_count + 1,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
                    .checked_add(delta_consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ofs-delta data offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let expected_delta_stream_len = expected_header_size;
                // PACK delta headers encode delta-stream byte length for ofs/ref-delta entries.
                // Spec: https://git-scm.com/docs/pack-format (Size encoding section).
                if delta_bytes.len() != expected_delta_stream_len {
                    return Err(PayloadAuditError {
                        reason: format!(
                            "ofs-delta stream size mismatch at object {}: header={}, actual={}",
                            processed_object_count + 1,
                            expected_delta_stream_len,
                            delta_bytes.len()
                        ),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let content = apply_git_delta(&base.content, &delta_bytes).map_err(|err| {
                    PayloadAuditError {
                        reason: err.to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    }
                })?;
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: base.kind,
                    content,
                }
            }
            PackEntryKind::RefDelta => {
                if trailer_offset.saturating_sub(offset) < 20 {
                    entry_record.note =
                        Some("ref-delta entry is missing base object id bytes".to_string());
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason: "ref-delta entry is missing base object id bytes".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let base_oid =
                    git2::Oid::from_bytes(&pack_data[offset..offset + 20]).map_err(|err| {
                        PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        }
                    })?;
                entry_record.base_ref = Some(PackEntryBaseRef::BaseOid(base_oid));
                offset += 20;

                let in_pack_base = objects_by_oid.get(&base_oid).cloned();
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
                    ledger.entries.push(entry_record);
                    return Err(PayloadAuditError {
                        reason,
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                };
                if objects_by_oid.contains_key(&base_oid) {
                    entry_resolved_via = ResolutionSource::InPack;
                } else {
                    thin_pack_detected = true;
                    baseline_resolutions_count += 1;
                    entry_resolved_via = ResolutionSource::Baseline;
                }
                let (delta_consumed, delta_bytes) =
                    decompress_zlib_stream(&pack_data[offset..trailer_offset])
                        .map_err(|err| {
                            anyhow!(
                                "failed to decompress ref-delta object #{} at offset {}: {err}",
                                processed_object_count + 1,
                                offset
                            )
                        })
                        .map_err(|err| PayloadAuditError {
                            reason: err.to_string(),
                            blocked_entry_idx: Some(processed_object_count),
                            ledger_partial: Some(ledger.clone()),
                        })?;
                offset = offset
                    .checked_add(delta_consumed)
                    .ok_or_else(|| PayloadAuditError {
                        reason: "ref-delta data offset overflow".to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    })?;
                let expected_delta_stream_len = expected_header_size;
                // PACK delta headers encode delta-stream byte length for ofs/ref-delta entries.
                // Spec: https://git-scm.com/docs/pack-format (Size encoding section).
                if delta_bytes.len() != expected_delta_stream_len {
                    return Err(PayloadAuditError {
                        reason: format!(
                            "ref-delta stream size mismatch at object {}: header={}, actual={}",
                            processed_object_count + 1,
                            expected_delta_stream_len,
                            delta_bytes.len()
                        ),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger),
                    });
                }
                let content = apply_git_delta(&base.content, &delta_bytes).map_err(|err| {
                    PayloadAuditError {
                        reason: err.to_string(),
                        blocked_entry_idx: Some(processed_object_count),
                        ledger_partial: Some(ledger.clone()),
                    }
                })?;
                entry_record.reconstructed_size = Some(content.len());
                ParsedPackObject {
                    kind: base.kind,
                    content,
                }
            }
        };

        let object_oid = object_oid_for_content(parsed.kind, &parsed.content).map_err(|err| {
            PayloadAuditError {
                reason: err.to_string(),
                blocked_entry_idx: Some(processed_object_count),
                ledger_partial: Some(ledger.clone()),
            }
        })?;
        entry_record.result_oid = Some(object_oid);
        entry_record.result_kind = Some(parsed.kind);
        entry_record.resolved = true;
        entry_record.resolved_via = Some(entry_resolved_via);
        ledger.entries.push(entry_record);
        objects_by_offset.insert(entry_offset, parsed.clone());
        objects_by_oid.insert(object_oid, parsed);
        processed_object_count += 1;
    }

    if offset != trailer_offset {
        return Err(PayloadAuditError {
            reason: format!(
                "pack contains trailing or unconsumed bytes before trailer: consumed={}, trailer={}",
                offset, trailer_offset
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger),
        });
    }
    if processed_object_count != declared_object_count {
        return Err(PayloadAuditError {
            reason: format!(
                "pack object count mismatch: declared={}, processed={}",
                declared_object_count, processed_object_count
            ),
            blocked_entry_idx: Some(processed_object_count),
            ledger_partial: Some(ledger),
        });
    }

    let materialized_index = build_materialized_object_index_from_ledger(&ledger);
    let materialized_store = build_materialized_object_store(&objects_by_oid);
    let proof = PayloadPackProof::from_entry_counters(
        pack_version,
        declared_object_count,
        processed_object_count,
        materialized_index.materialized_entry_count,
        materialized_index.unique_object_count,
        materialized_index.duplicate_entry_count_materialized,
        true,
        thin_pack_detected,
        baseline_resolutions_count,
        "sha1".to_string(),
        computed_checksum,
        trailer_checksum,
    );
    Ok(PayloadPackVerification {
        proof,
        ledger,
        materialized_index,
        materialized_store,
    })
}

/// Parses a PACK object header at `offset`.
fn parse_pack_entry_header(
    pack_data: &[u8],
    offset: usize,
) -> Result<(PackEntryKind, usize, usize)> {
    if offset >= pack_data.len() {
        bail!("pack entry header offset is out of bounds");
    }

    let first = pack_data[offset];
    let entry_kind = match (first >> 4) & 0x07 {
        1 => PackEntryKind::Commit,
        2 => PackEntryKind::Tree,
        3 => PackEntryKind::Blob,
        4 => PackEntryKind::Tag,
        6 => PackEntryKind::OfsDelta,
        7 => PackEntryKind::RefDelta,
        code => bail!("unsupported/invalid pack entry type code: {code}"),
    };

    let mut size = (first & 0x0f) as usize;
    let mut shift = 4u32;
    let mut cursor = offset + 1;
    let mut byte = first;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("pack entry header is truncated");
        }
        byte = pack_data[cursor];
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        cursor += 1;
    }

    Ok((entry_kind, size, cursor))
}

/// Parses OFS-delta encoded backward base distance.
fn parse_ofs_delta_base_distance(pack_data: &[u8], offset: usize) -> Result<(usize, usize)> {
    if offset >= pack_data.len() {
        bail!("ofs-delta base encoding offset out of bounds");
    }
    let mut cursor = offset;
    let mut byte = pack_data[cursor];
    cursor += 1;

    let mut distance = (byte & 0x7f) as usize;
    while (byte & 0x80) != 0 {
        if cursor >= pack_data.len() {
            bail!("ofs-delta base encoding is truncated");
        }
        byte = pack_data[cursor];
        cursor += 1;
        distance = ((distance + 1) << 7) + (byte & 0x7f) as usize;
    }

    Ok((distance, cursor - offset))
}

/// Decompresses one zlib stream from the start of `bytes`, returning consumed input bytes.
fn decompress_zlib_stream(bytes: &[u8]) -> Result<(usize, Vec<u8>)> {
    let mut decompressor = Decompress::new(true);
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut no_progress_streak = 0usize;

    loop {
        if consumed_total >= bytes.len() {
            bail!("unexpected end of zlib stream while reading pack entry");
        }
        let input = &bytes[consumed_total..];
        out.reserve(16 * 1024);
        let before_in = decompressor.total_in();
        let before_out = decompressor.total_out();
        let status = decompressor.decompress_vec(input, &mut out, FlushDecompress::None)?;
        let consumed = (decompressor.total_in() - before_in) as usize;
        let produced = (decompressor.total_out() - before_out) as usize;
        consumed_total += consumed;

        match status {
            Status::StreamEnd => break,
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    no_progress_streak += 1;
                    if no_progress_streak >= 8 {
                        bail!("zlib stream made no progress while parsing pack entry");
                    }
                } else {
                    no_progress_streak = 0;
                }
            }
        }
    }

    Ok((consumed_total, out))
}

/// Applies a git delta bytecode stream to `base`, producing reconstructed object content.
fn apply_git_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = 0usize;
    let (source_size, source_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += source_size_consumed;
    if source_size != base.len() {
        bail!(
            "delta source size mismatch: expected={}, actual={}",
            source_size,
            base.len()
        );
    }

    let (target_size, target_size_consumed) = parse_git_delta_varint(delta, cursor)?;
    cursor += target_size_consumed;
    let mut out = Vec::with_capacity(target_size);

    while cursor < delta.len() {
        let opcode = delta[cursor];
        cursor += 1;

        if (opcode & 0x80) != 0 {
            let mut copy_offset = 0usize;
            let mut copy_size = 0usize;

            if (opcode & 0x01) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 0")?;
                copy_offset |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x02) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 1")?;
                copy_offset |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x04) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 2")?;
                copy_offset |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if (opcode & 0x08) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy offset byte 3")?;
                copy_offset |= (delta[cursor] as usize) << 24;
                cursor += 1;
            }

            if (opcode & 0x10) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 0")?;
                copy_size |= delta[cursor] as usize;
                cursor += 1;
            }
            if (opcode & 0x20) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 1")?;
                copy_size |= (delta[cursor] as usize) << 8;
                cursor += 1;
            }
            if (opcode & 0x40) != 0 {
                ensure_remaining(delta, cursor, 1, "delta copy size byte 2")?;
                copy_size |= (delta[cursor] as usize) << 16;
                cursor += 1;
            }
            if copy_size == 0 {
                copy_size = 0x10000;
            }

            let copy_end = copy_offset
                .checked_add(copy_size)
                .ok_or_else(|| anyhow!("delta copy range overflow"))?;
            if copy_end > base.len() {
                bail!(
                    "delta copy range exceeds base object: offset={}, size={}, base={}",
                    copy_offset,
                    copy_size,
                    base.len()
                );
            }
            out.extend_from_slice(&base[copy_offset..copy_end]);
        } else if opcode != 0 {
            let literal_size = opcode as usize;
            ensure_remaining(delta, cursor, literal_size, "delta literal chunk")?;
            out.extend_from_slice(&delta[cursor..cursor + literal_size]);
            cursor += literal_size;
        } else {
            bail!("invalid delta opcode 0x00");
        }
    }

    if out.len() != target_size {
        bail!(
            "delta result size mismatch: expected={}, actual={}",
            target_size,
            out.len()
        );
    }
    Ok(out)
}

/// Parses one git delta varint from `bytes` at `offset`.
fn parse_git_delta_varint(bytes: &[u8], offset: usize) -> Result<(usize, usize)> {
    if offset >= bytes.len() {
        bail!("delta varint is out of bounds");
    }
    let mut cursor = offset;
    let mut value = 0usize;
    let mut shift = 0u32;
    loop {
        ensure_remaining(bytes, cursor, 1, "delta varint byte")?;
        let byte = bytes[cursor];
        cursor += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, cursor - offset))
}

/// Returns the canonical object id for `(kind, content)` as SHA-1.
fn object_oid_for_content(kind: PayloadObjectKind, content: &[u8]) -> Result<git2::Oid> {
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
fn load_parsed_object_from_odb(odb: &git2::Odb<'_>, oid: git2::Oid) -> Result<ParsedPackObject> {
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
fn pack_entry_kind_to_payload_kind(kind: PackEntryKind) -> PayloadObjectKind {
    match kind {
        PackEntryKind::Commit => PayloadObjectKind::Commit,
        PackEntryKind::Tree => PayloadObjectKind::Tree,
        PackEntryKind::Blob => PayloadObjectKind::Blob,
        PackEntryKind::Tag => PayloadObjectKind::Tag,
        PackEntryKind::OfsDelta | PackEntryKind::RefDelta => PayloadObjectKind::Unknown,
    }
}

/// Reads one big-endian `u32` from `bytes` at `offset`.
fn read_be_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow!("u32 read offset overflow"))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| anyhow!("u32 read out of bounds"))?;
    Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Ensures `len` bytes are available from `offset`.
fn ensure_remaining(bytes: &[u8], offset: usize, len: usize, context: &str) -> Result<()> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| anyhow!("{context}: offset overflow"))?;
    if end > bytes.len() {
        bail!("{context}: truncated data");
    }
    Ok(())
}

/// Returns lowercase hex for arbitrary bytes.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns SHA-1 digest hex for bytes.
fn sha1_hex(bytes: &[u8]) -> Result<String> {
    let mut ctx = std::mem::MaybeUninit::<openssl_sys::SHA_CTX>::uninit();
    let init_ok = unsafe { openssl_sys::SHA1_Init(ctx.as_mut_ptr()) } == 1;
    if !init_ok {
        bail!("failed to initialize SHA-1 context");
    }
    let mut ctx = unsafe { ctx.assume_init() };
    let update_ok =
        unsafe { openssl_sys::SHA1_Update(&mut ctx, bytes.as_ptr().cast(), bytes.len()) } == 1;
    if !update_ok {
        bail!("failed to update SHA-1 digest");
    }
    let mut digest = [0u8; 20];
    let final_ok = unsafe { openssl_sys::SHA1_Final(digest.as_mut_ptr(), &mut ctx) } == 1;
    if !final_ok {
        bail!("failed to finalize SHA-1 digest");
    }
    Ok(hex_encode(&digest))
}

/// Builds deduplicated materialized object index directly from ledger result rows.
fn build_materialized_object_index_from_ledger(
    ledger: &PackEntryLedger,
) -> MaterializedObjectIndex {
    let mut by_oid = HashMap::<git2::Oid, MaterializedObjectRecord>::new();
    let mut materialized_entry_count = 0usize;

    for entry in &ledger.entries {
        if !is_materialized_entry(entry) {
            continue;
        }
        let (Some(oid), Some(kind)) = (entry.result_oid, entry.result_kind) else {
            continue;
        };
        materialized_entry_count += 1;
        let reconstructed_size = entry.reconstructed_size.unwrap_or(entry.out_size);
        by_oid.entry(oid).or_insert(MaterializedObjectRecord {
            oid,
            kind,
            size_bytes: reconstructed_size,
            first_entry_idx: entry.idx,
        });
    }

    let unique_object_count = by_oid.len();
    let duplicate_entry_count_materialized =
        materialized_entry_count.saturating_sub(unique_object_count);
    let mut objects = by_oid.into_values().collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });

    MaterializedObjectIndex {
        objects,
        materialized_entry_count,
        unique_object_count,
        duplicate_entry_count_materialized,
    }
}

/// Builds verifier-owned materialized object store from parsed object map.
fn build_materialized_object_store(
    objects_by_oid: &HashMap<git2::Oid, ParsedPackObject>,
) -> MaterializedObjectStore {
    let mut objects = objects_by_oid
        .iter()
        .map(|(oid, parsed)| {
            if parsed.kind == PayloadObjectKind::Blob && parsed.content.len() > MAX_BLOB_STORE_BYTES
            {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content
                        [..LARGE_BLOB_PREVIEW_BYTES.min(parsed.content.len())]
                        .to_vec(),
                    content_truncated: true,
                }
            } else {
                MaterializedObjectData {
                    oid: *oid,
                    kind: parsed.kind,
                    size_bytes: parsed.content.len(),
                    content_bytes: parsed.content.clone(),
                    content_truncated: false,
                }
            }
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| left.oid.cmp(&right.oid));
    MaterializedObjectStore { objects }
}

/// Returns true when an entry is fully materialized and exportable as exact object bytes.
fn is_materialized_entry(entry: &PackEntryRecord) -> bool {
    entry.resolved
        && entry.result_oid.is_some()
        && entry.result_kind.is_some()
        && entry.reconstructed_size.is_some()
        && entry.resolved_via.is_some()
}

fn payload_kind_rank(kind: PayloadObjectKind) -> u8 {
    match kind {
        PayloadObjectKind::Commit => 0,
        PayloadObjectKind::Tree => 1,
        PayloadObjectKind::Blob => 2,
        PayloadObjectKind::Tag => 3,
        PayloadObjectKind::Unknown => 4,
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
