use crate::git::archive::{
    caudit_sidecar_path, extract_bundle_archive, is_zip_bundle_input_path,
    resolve_patch_sidecar_path,
};
use crate::git::bundle::inspect_bundle;
use crate::git::metadata::collect::{
    collect_changed_files_for_metadata, collect_commit_chain_for_metadata,
};
use crate::git::metadata::load::load_bundle_metadata_from_path;
use crate::git::types::{CreateBundleAuditHead, CreateBundleAuditMetadata};
use crate::git::util::{bundle_version_code, sha256_hex};
use anyhow::{Result, bail};
use std::fs;
use std::path::Path;

pub fn verify_bundle_metadata_against_repo(bundle_path: &Path, repo_path: &Path) -> Result<()> {
    let metadata = verify_bundle_metadata_integrity(bundle_path)?;

    let repo = git2::Repository::open(repo_path)?;

    let from_commit_id = git2::Oid::from_str(&metadata.range_from_oid)?;
    let to_commit_id = git2::Oid::from_str(&metadata.range_to_oid)?;

    repo.find_commit(from_commit_id)?;
    repo.find_commit(to_commit_id)?;

    if to_commit_id != from_commit_id && !repo.graph_descendant_of(to_commit_id, from_commit_id)? {
        bail!(
            "metadata range is not linear in repository: to={} from={}",
            to_commit_id,
            from_commit_id
        );
    }

    let expected_commit_chain =
        collect_commit_chain_for_metadata(&repo, from_commit_id, to_commit_id)?;
    if metadata.commit_chain != expected_commit_chain {
        bail!("metadata commit_chain does not match repository truth");
    }

    let expected_changed_files =
        collect_changed_files_for_metadata(&repo, from_commit_id, to_commit_id)?;
    if metadata.changed_files != expected_changed_files {
        bail!("metadata changed_files does not match repository truth");
    }

    Ok(())
}

pub fn verify_bundle_metadata_against_repo_input(
    bundle_input_path: &Path,
    repo_path: &Path,
) -> Result<()> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        verify_bundle_metadata_against_repo(&extracted.bundle_path, repo_path)
    } else {
        verify_bundle_metadata_against_repo(bundle_input_path, repo_path)
    }
}

pub(crate) fn verify_bundle_metadata_integrity_input(bundle_input_path: &Path) -> Result<()> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        verify_bundle_metadata_integrity(&extracted.bundle_path)?;
        Ok(())
    } else {
        verify_bundle_metadata_integrity(bundle_input_path)?;
        Ok(())
    }
}

pub(crate) fn verify_bundle_metadata_integrity(
    bundle_path: &Path,
) -> Result<CreateBundleAuditMetadata> {
    if !bundle_path.exists() {
        bail!("bundle path does not exist: {}", bundle_path.display());
    }
    if !bundle_path.is_file() {
        bail!("bundle path is not a file: {}", bundle_path.display());
    }

    let metadata_path = caudit_sidecar_path(bundle_path);
    let metadata = load_bundle_metadata_from_path(&metadata_path)?;
    if metadata.schema_version != "1" {
        bail!(
            "unsupported caudit schema version: '{}'",
            metadata.schema_version
        );
    }

    let bundle_bytes = fs::read(bundle_path)?;
    let actual_bundle_size = bundle_bytes.len() as u64;
    if metadata.bundle_size_bytes != actual_bundle_size {
        bail!(
            "bundle size mismatch: metadata={}, actual={}",
            metadata.bundle_size_bytes,
            actual_bundle_size
        );
    }

    let actual_bundle_sha256 = sha256_hex(&bundle_bytes)?;
    if metadata.bundle_sha256 != actual_bundle_sha256 {
        bail!(
            "bundle sha256 mismatch: metadata={}, actual={}",
            metadata.bundle_sha256,
            actual_bundle_sha256
        );
    }

    let inspection = inspect_bundle(bundle_path)?;
    let expected_bundle_header_version = bundle_version_code(inspection.version).to_string();
    if metadata.bundle_header_version != expected_bundle_header_version {
        bail!(
            "bundle header version mismatch: metadata={}, actual={}",
            metadata.bundle_header_version,
            expected_bundle_header_version
        );
    }

    let expected_prerequisites: Vec<String> = inspection
        .prerequisites
        .iter()
        .map(|oid| oid.to_string())
        .collect();
    if metadata.prerequisites != expected_prerequisites {
        bail!("bundle prerequisites mismatch between metadata and bundle header");
    }

    let expected_heads: Vec<CreateBundleAuditHead> = inspection
        .heads
        .iter()
        .map(|head| CreateBundleAuditHead {
            oid: head.oid.to_string(),
            reference: head.reference.clone(),
        })
        .collect();
    if metadata.heads != expected_heads {
        bail!("bundle heads mismatch between metadata and bundle header");
    }

    if !metadata
        .heads
        .iter()
        .any(|head| head.reference == metadata.tip_ref && head.oid == metadata.range_to_oid)
    {
        bail!("metadata tip_ref/range_to_oid must match one bundle head entry");
    }

    if let Some(patch_sidecar) = &metadata.patch_sidecar {
        if patch_sidecar.format != "unified-diff" {
            bail!(
                "unsupported patch sidecar format: '{}'",
                patch_sidecar.format
            );
        }
        let patch_path = resolve_patch_sidecar_path(&metadata_path, patch_sidecar)?;
        let patch_bytes = fs::read(&patch_path)?;
        let actual_patch_size = patch_bytes.len() as u64;
        if patch_sidecar.size_bytes != actual_patch_size {
            bail!(
                "patch sidecar size mismatch: metadata={}, actual={}",
                patch_sidecar.size_bytes,
                actual_patch_size
            );
        }

        let actual_patch_sha256 = sha256_hex(&patch_bytes)?;
        if patch_sidecar.sha256 != actual_patch_sha256 {
            bail!(
                "patch sidecar sha256 mismatch: metadata={}, actual={}",
                patch_sidecar.sha256,
                actual_patch_sha256
            );
        }
    }

    Ok(metadata)
}
