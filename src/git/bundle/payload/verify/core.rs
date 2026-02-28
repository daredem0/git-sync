//! Core PACK stream verification/materialization engine.

use crate::git::types::{
    PackEntryBaseRef, PackEntryKind, PackEntryLedger, PackEntryRecord, PayloadAuditError,
    PayloadPackProof, PayloadPackVerification, ResolutionSource,
};
use anyhow::anyhow;
use std::collections::HashMap;

use super::delta::apply_git_delta;
use super::materialized::{
    build_materialized_object_index_from_ledger, build_materialized_object_store,
};
use super::object::{
    ParsedPackObject, hex_encode, load_parsed_object_from_odb, object_oid_for_content,
    pack_entry_kind_to_payload_kind, sha1_hex,
};
use super::pack::{parse_ofs_delta_base_distance, parse_pack_entry_header, read_be_u32};
use super::zlib::decompress_zlib_stream;

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
