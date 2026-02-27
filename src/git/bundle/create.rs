use super::inspect::inspect_bundle;
use crate::git::archive::{
    bundle_archive_path, caudit_sidecar_path, remove_file_if_exists, write_zip_archive,
};
use crate::git::metadata::{
    collect_changed_files_for_metadata, collect_commit_chain_for_metadata, write_patch_sidecar,
};
use crate::git::types::{
    CreateBundleAuditHead, CreateBundleAuditMetadata, CreateBundleOptions, CreateBundleResult,
};
use crate::git::util::{
    bundle_version_code, current_hostname, current_unix_timestamp_secs, current_username,
    sha256_hex,
};
use anyhow::{Result, bail};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn create_bundle(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
    bundle_path: &Path,
) -> Result<CreateBundleResult> {
    create_bundle_with_options(
        repo_path,
        from_rev,
        to_rev,
        bundle_path,
        CreateBundleOptions::default(),
    )
}

pub fn create_bundle_with_options(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
    bundle_path: &Path,
    options: CreateBundleOptions,
) -> Result<CreateBundleResult> {
    let repo = git2::Repository::open(repo_path)?;

    let from_obj = repo.revparse_single(from_rev)?;
    let from_commit = from_obj.peel_to_commit()?;
    let from_commit_id = from_commit.id();

    let (to_obj, to_ref) = repo.revparse_ext(to_rev)?;
    let to_commit = to_obj.peel_to_commit()?;
    let to_commit_id = to_commit.id();

    if from_commit_id != to_commit_id && !repo.graph_descendant_of(to_commit_id, from_commit_id)? {
        bail!(
            "to commit '{}' must be the same as or a descendant of from commit '{}'",
            to_rev,
            from_rev
        );
    }

    let tip_ref_name = to_ref
        .and_then(|reference| reference.name().map(|name| name.to_string()))
        .unwrap_or_else(|| format!("refs/heads/bundle-tip-{}", &to_commit_id.to_string()[..12]));

    let mut walk = repo.revwalk()?;
    walk.push(to_commit_id)?;
    walk.hide(from_commit_id)?;

    let mut packbuilder = repo.packbuilder()?;
    packbuilder.insert_walk(&mut walk)?;
    let mut pack_buffer = git2::Buf::new();
    packbuilder.write_buf(&mut pack_buffer)?;

    let mut file = File::create(bundle_path)?;
    writeln!(file, "# v2 git bundle")?;
    writeln!(file, "-{from_commit_id}")?;
    writeln!(file, "{to_commit_id} {tip_ref_name}")?;
    writeln!(file)?;
    file.write_all(&pack_buffer)?;

    let inspection = inspect_bundle(bundle_path)?;
    let changed_files = collect_changed_files_for_metadata(&repo, from_commit_id, to_commit_id)?;
    let commit_chain = collect_commit_chain_for_metadata(&repo, from_commit_id, to_commit_id)?;

    let patch_sidecar = if options.include_patch_sidecar {
        Some(write_patch_sidecar(
            &repo,
            from_commit_id,
            to_commit_id,
            bundle_path,
        )?)
    } else {
        None
    };

    let bundle_bytes = fs::read(bundle_path)?;
    let bundle_size_bytes = bundle_bytes.len() as u64;
    let bundle_sha256 = sha256_hex(&bundle_bytes)?;
    let audit_path = caudit_sidecar_path(bundle_path);
    let metadata = CreateBundleAuditMetadata {
        schema_version: "1".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix_secs: current_unix_timestamp_secs()?,
        generated_by_username: current_username(),
        generated_by_hostname: current_hostname(),
        bundle_path: bundle_path.display().to_string(),
        bundle_size_bytes,
        bundle_sha256,
        bundle_header_version: bundle_version_code(inspection.version).to_string(),
        prerequisites: inspection
            .prerequisites
            .iter()
            .map(|oid| oid.to_string())
            .collect(),
        heads: inspection
            .heads
            .iter()
            .map(|head| CreateBundleAuditHead {
                oid: head.oid.to_string(),
                reference: head.reference.clone(),
            })
            .collect(),
        range_from_oid: from_commit_id.to_string(),
        range_to_oid: to_commit_id.to_string(),
        tip_ref: tip_ref_name.clone(),
        commit_chain,
        changed_files,
        patch_sidecar,
    };
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&audit_path, metadata_json.as_bytes())?;

    let patch_audit_path = metadata
        .patch_sidecar
        .as_ref()
        .map(|sidecar| std::path::PathBuf::from(sidecar.path.clone()));
    let archive_path = bundle_archive_path(bundle_path);
    let mut archive_inputs = vec![bundle_path.to_path_buf(), audit_path.clone()];
    if let Some(patch_path) = &patch_audit_path {
        archive_inputs.push(patch_path.clone());
    }
    write_zip_archive(&archive_path, &archive_inputs)?;

    Ok(CreateBundleResult {
        from_commit_id,
        to_commit_id,
        tip_ref_name,
        bundle_path: bundle_path.to_path_buf(),
        audit_path,
        patch_audit_path,
        archive_path,
    })
}

pub fn remove_unarchived_bundle_artifacts(result: &CreateBundleResult) -> Result<()> {
    remove_file_if_exists(&result.bundle_path)?;
    remove_file_if_exists(&result.audit_path)?;
    if let Some(patch_path) = &result.patch_audit_path {
        remove_file_if_exists(patch_path)?;
    }
    Ok(())
}
