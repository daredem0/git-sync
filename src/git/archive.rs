// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Git-domain module for archive functionality.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::CreateBundleAuditPatchSidecar;
use anyhow::{Result, anyhow, bail};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Returns the metadata sidecar path for a bundle (`.caudit.json`).
///
/// The output preserves the original bundle path and appends a suffix.
pub(crate) fn caudit_sidecar_path(bundle_path: &Path) -> PathBuf {
    let mut sidecar = bundle_path.as_os_str().to_os_string();
    sidecar.push(".caudit.json");
    PathBuf::from(sidecar)
}

/// Returns the patch sidecar path for a bundle (`.caudit.patch`).
///
/// The output preserves the original bundle path and appends a suffix.
pub(crate) fn patch_sidecar_path(bundle_path: &Path) -> PathBuf {
    let mut sidecar = bundle_path.as_os_str().to_os_string();
    sidecar.push(".caudit.patch");
    PathBuf::from(sidecar)
}

/// Returns the archive package path for a bundle (`.zip`).
///
/// The output preserves the original bundle path and appends a suffix.
pub(crate) fn bundle_archive_path(bundle_path: &Path) -> PathBuf {
    let mut archive = bundle_path.as_os_str().to_os_string();
    archive.push(".zip");
    PathBuf::from(archive)
}

/// Writes a zip archive containing the provided files as sibling entries.
///
/// # Errors
///
/// Returns an error when any input file is missing/non-file or when zip I/O
/// fails.
pub(crate) fn write_zip_archive(archive_path: &Path, files: &[PathBuf]) -> Result<()> {
    let archive_file = File::create(archive_path)?;
    let mut archive = ZipWriter::new(archive_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for file_path in files {
        if !file_path.exists() {
            bail!("archive input path does not exist: {}", file_path.display());
        }
        if !file_path.is_file() {
            bail!("archive input path is not a file: {}", file_path.display());
        }

        let file_name = file_path
            .file_name()
            .ok_or_else(|| anyhow!("archive input has no file name: {}", file_path.display()))?;
        let file_name = file_name.to_string_lossy();
        archive.start_file(file_name, options)?;
        let bytes = fs::read(file_path)?;
        archive.write_all(&bytes)?;
    }

    archive.finish()?;
    Ok(())
}

/// Returns `true` when the input path points to a `.zip` bundle archive.
pub(crate) fn is_zip_bundle_input_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

#[derive(Debug)]
pub(crate) struct ExtractedBundleArchive {
    pub(crate) temp_dir: PathBuf,
    pub(crate) bundle_path: PathBuf,
}

impl Drop for ExtractedBundleArchive {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

/// Extracts a bundle archive and returns the temporary extracted `.bundle` path.
///
/// The temporary directory is removed automatically when the returned
/// [`ExtractedBundleArchive`] is dropped.
///
/// # Errors
///
/// Returns an error when the archive is missing/invalid or when it does not
/// contain exactly one `.bundle` file.
pub(crate) fn extract_bundle_archive(archive_path: &Path) -> Result<ExtractedBundleArchive> {
    if !archive_path.exists() {
        bail!(
            "bundle archive path does not exist: {}",
            archive_path.display()
        );
    }
    if !archive_path.is_file() {
        bail!(
            "bundle archive path is not a file: {}",
            archive_path.display()
        );
    }

    let archive_file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)?;

    let temp_dir = std::env::temp_dir().join(format!(
        "git-sync-extract-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| anyhow!("system clock is before unix epoch"))?
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let mut bundle_paths = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().ends_with('/') {
            continue;
        }

        let file_name = Path::new(entry.name())
            .file_name()
            .ok_or_else(|| anyhow!("zip entry has no file name: '{}'", entry.name()))?;
        let output_path = temp_dir.join(file_name);

        let mut output_file = File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output_file)?;

        if output_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bundle"))
        {
            bundle_paths.push(output_path);
        }
    }

    if bundle_paths.is_empty() {
        bail!(
            "bundle archive does not contain a .bundle entry: {}",
            archive_path.display()
        );
    }
    if bundle_paths.len() > 1 {
        bail!(
            "bundle archive must contain exactly one .bundle entry, found {}",
            bundle_paths.len()
        );
    }

    Ok(ExtractedBundleArchive {
        temp_dir,
        bundle_path: bundle_paths.remove(0),
    })
}

/// Removes a file and ignores missing-path errors.
///
/// # Errors
///
/// Returns an error when deletion fails for any reason other than
/// `NotFound`.
pub(crate) fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(anyhow!(
            "failed to remove temporary artifact '{}': {err}",
            path.display()
        )),
    }
}

/// Resolves the patch sidecar path recorded in metadata.
///
/// Resolution first tries the explicit stored path, then a same-directory
/// sibling next to the metadata file.
///
/// # Errors
///
/// Returns an error when neither location resolves to an existing file.
pub(crate) fn resolve_patch_sidecar_path(
    metadata_path: &Path,
    patch_sidecar: &CreateBundleAuditPatchSidecar,
) -> Result<PathBuf> {
    let explicit_path = PathBuf::from(&patch_sidecar.path);
    if explicit_path.exists() && explicit_path.is_file() {
        return Ok(explicit_path);
    }

    let file_name = Path::new(&patch_sidecar.path)
        .file_name()
        .ok_or_else(|| anyhow!("patch sidecar path in metadata has no file name"))?;
    let sibling_path = metadata_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(file_name);
    if sibling_path.exists() && sibling_path.is_file() {
        return Ok(sibling_path);
    }

    bail!(
        "patch sidecar path does not exist: {} (or sibling {})",
        explicit_path.display(),
        sibling_path.display()
    );
}
