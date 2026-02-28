//! Git-layer load functionality.

use crate::git::types::CreateBundleAuditMetadata;
use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

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
