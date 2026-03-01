// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload audit module for input operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::digest::sha256_hex;
use crate::git::types::PayloadTransportEntry;
use anyhow::{Result, bail};
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug)]
pub(super) struct LoadedPayloadInput {
    pub(super) bundle_name: String,
    pub(super) bundle_bytes: Vec<u8>,
    pub(super) transport_entries: Vec<PayloadTransportEntry>,
}

pub(super) fn load_payload_input(bundle_input_path: &Path) -> Result<LoadedPayloadInput> {
    if is_zip_bundle_input_path(bundle_input_path) {
        return load_zip_payload_input(bundle_input_path);
    }

    let bundle_bytes = fs::read(bundle_input_path)?;
    let bundle_name = bundle_input_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| bundle_input_path.display().to_string());
    let transport_entries = collect_transport_entries_for_plain_bundle(bundle_input_path)?;

    Ok(LoadedPayloadInput {
        bundle_name,
        bundle_bytes,
        transport_entries,
    })
}

#[cfg(test)]
pub(super) fn load_bundle_bytes_for_input(bundle_input_path: &Path) -> Result<Vec<u8>> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        return fs::read(&extracted.bundle_path).map_err(Into::into);
    }
    fs::read(bundle_input_path).map_err(Into::into)
}

fn load_zip_payload_input(bundle_input_path: &Path) -> Result<LoadedPayloadInput> {
    let transport_entries = collect_transport_entries_for_zip(bundle_input_path)?;
    let extracted = extract_bundle_archive(bundle_input_path)?;
    let bundle_bytes = fs::read(&extracted.bundle_path)?;
    let bundle_name = extracted
        .bundle_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| extracted.bundle_path.display().to_string());

    Ok(LoadedPayloadInput {
        bundle_name,
        bundle_bytes,
        transport_entries,
    })
}

fn collect_transport_entries_for_plain_bundle(
    bundle_input_path: &Path,
) -> Result<Vec<PayloadTransportEntry>> {
    if !bundle_input_path.exists() {
        bail!(
            "bundle input path does not exist: {}",
            bundle_input_path.display()
        );
    }
    if !bundle_input_path.is_file() {
        bail!(
            "bundle input path is not a file: {}",
            bundle_input_path.display()
        );
    }
    let bytes = fs::read(bundle_input_path)?;
    let name = bundle_input_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| bundle_input_path.display().to_string());
    Ok(vec![PayloadTransportEntry {
        name,
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes)?,
    }])
}

fn collect_transport_entries_for_zip(archive_path: &Path) -> Result<Vec<PayloadTransportEntry>> {
    let archive_file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().ends_with('/') {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        entries.push(PayloadTransportEntry {
            name: entry.name().to_string(),
            size_bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes)?,
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}
