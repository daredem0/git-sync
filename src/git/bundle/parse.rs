// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Bundle processing module for parse operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::{BundleHead, BundleInspection, BundleVersion};
use anyhow::{Result, anyhow, bail};

#[derive(Debug)]
pub(crate) struct BundlePayload<'a> {
    pub(crate) inspection: BundleInspection,
    pub(crate) pack_data: &'a [u8],
}

/// Parses bundle header and returns inspection metadata plus exact PACK payload slice.
pub(crate) fn parse_bundle_payload(bundle_bytes: &[u8]) -> Result<BundlePayload<'_>> {
    let mut cursor = 0usize;
    let version_line = read_bundle_header_line(bundle_bytes, &mut cursor)?
        .ok_or_else(|| anyhow!("bundle payload is missing version line"))?;
    let version = match version_line.as_str() {
        "# v2 git bundle" => BundleVersion::V2,
        "# v3 git bundle" => BundleVersion::V3,
        _ => bail!("bundle file is not a valid git bundle header"),
    };

    let mut prerequisites = Vec::<git2::Oid>::new();
    let mut heads = Vec::<BundleHead>::new();
    loop {
        let line = read_bundle_header_line(bundle_bytes, &mut cursor)?
            .ok_or_else(|| anyhow!("bundle header terminated before PACK payload"))?;
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix('-') {
            let oid_token = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("invalid bundle prerequisite line: '{line}'"))?;
            prerequisites.push(git2::Oid::from_str(oid_token)?);
            continue;
        }

        let mut parts = line.splitn(2, ' ');
        let oid_token = parts
            .next()
            .ok_or_else(|| anyhow!("invalid bundle head line: '{line}'"))?;
        let reference = parts
            .next()
            .ok_or_else(|| anyhow!("bundle head line missing reference: '{line}'"))?;
        heads.push(BundleHead {
            oid: git2::Oid::from_str(oid_token)?,
            reference: reference.to_string(),
        });
    }

    if bundle_bytes.len().saturating_sub(cursor) < 4 {
        bail!("bundle header is not followed by PACK payload");
    }
    if &bundle_bytes[cursor..cursor + 4] != b"PACK" {
        bail!("bundle header terminator is not followed by PACK payload");
    }

    Ok(BundlePayload {
        inspection: BundleInspection {
            version,
            prerequisites,
            heads,
        },
        pack_data: &bundle_bytes[cursor..],
    })
}

fn read_bundle_header_line(bundle_bytes: &[u8], cursor: &mut usize) -> Result<Option<String>> {
    if *cursor >= bundle_bytes.len() {
        return Ok(None);
    }
    let start = *cursor;
    let rel_end = bundle_bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|value| start + value);
    let end = rel_end.unwrap_or(bundle_bytes.len());
    *cursor = if rel_end.is_some() { end + 1 } else { end };

    let mut line = &bundle_bytes[start..end];
    if line.ends_with(b"\r") {
        line = &line[..line.len() - 1];
    }
    let text = std::str::from_utf8(line)
        .map_err(|_| anyhow!("bundle header contains non-utf8 line bytes"))?
        .to_string();
    Ok(Some(text))
}
