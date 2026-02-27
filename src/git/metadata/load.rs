//! Git-layer load functionality.

use crate::git::archive::{caudit_sidecar_path, extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::types::{ChangedFile, CreateBundleAuditMetadata};
use crate::git::util::{parse_optional_oid, parse_status_code};
use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

/// Loads metadata from bundle input and converts it into manifest-style rows.
///
/// # Errors
///
/// Returns an error when metadata cannot be loaded or contains invalid status
/// or object ID fields.
pub fn collect_changed_files_from_bundle_input(
    bundle_input_path: &Path,
) -> Result<Vec<ChangedFile>> {
    let metadata = load_bundle_metadata_from_input(bundle_input_path)?;
    metadata
        .changed_files
        .into_iter()
        .map(|entry| {
            Ok(ChangedFile {
                status: parse_status_code(&entry.status)?,
                path: entry.path,
                old_path: entry.old_path,
                old_oid: parse_optional_oid(entry.old_oid.as_deref())?,
                new_oid: parse_optional_oid(entry.new_oid.as_deref())?,
            })
        })
        .collect()
}

/// Loads bundle metadata from either a raw `.bundle` input or packaged `.zip`.
///
/// # Errors
///
/// Returns an error when archive extraction or metadata parsing fails.
pub(crate) fn load_bundle_metadata_from_input(
    bundle_input_path: &Path,
) -> Result<CreateBundleAuditMetadata> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let metadata_path = caudit_sidecar_path(&extracted.bundle_path);
        return load_bundle_metadata_from_path(&metadata_path);
    }

    let metadata_path = caudit_sidecar_path(bundle_input_path);
    load_bundle_metadata_from_path(&metadata_path)
}

/// Loads and deserializes a metadata sidecar from disk.
///
/// # Errors
///
/// Returns an error when the path is missing/invalid or JSON decoding fails.
pub(crate) fn load_bundle_metadata_from_path(
    metadata_path: &Path,
) -> Result<CreateBundleAuditMetadata> {
    if !metadata_path.exists() {
        bail!(
            "bundle audit metadata path does not exist: {}",
            metadata_path.display()
        );
    }
    if !metadata_path.is_file() {
        bail!(
            "bundle audit metadata path is not a file: {}",
            metadata_path.display()
        );
    }

    let metadata_bytes = fs::read(metadata_path)?;
    let metadata: CreateBundleAuditMetadata = serde_json::from_slice(&metadata_bytes)?;
    Ok(metadata)
}
