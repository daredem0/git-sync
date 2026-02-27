use super::inspect::inspect_bundle;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::metadata::verify_bundle_metadata_integrity_input;
use crate::git::{BundleHead, ReceiveBundleOptions, ReceiveBundleResult};
use anyhow::{Result, anyhow, bail};
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn receive_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options(
        bundle_input_path,
        receiver_repo_path,
        ReceiveBundleOptions::default(),
    )
}

pub fn receive_bundle_input_with_options(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
) -> Result<ReceiveBundleResult> {
    if options.verify_metadata {
        verify_bundle_metadata_integrity_input(bundle_input_path)?;
    }

    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        receive_bundle(&extracted.bundle_path, receiver_repo_path)
    } else {
        receive_bundle(bundle_input_path, receiver_repo_path)
    }
}

fn receive_bundle(bundle_path: &Path, receiver_repo_path: &Path) -> Result<ReceiveBundleResult> {
    let inspection = inspect_bundle(bundle_path)?;
    if inspection.heads.is_empty() {
        bail!("bundle does not contain any heads to import");
    }

    let repo = git2::Repository::open(receiver_repo_path)?;
    if inspection
        .heads
        .iter()
        .map(|head| is_head_already_applied(&repo, head))
        .collect::<Result<Vec<bool>>>()?
        .into_iter()
        .all(std::convert::identity)
    {
        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
        });
    }

    let bundle_bytes = fs::read(bundle_path)?;
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .ok_or_else(|| anyhow!("bundle does not contain PACK payload"))?;
    let pack_data = &bundle_bytes[pack_offset..];

    let odb = repo.odb()?;
    let pack_dir = repo.path().join("objects").join("pack");
    fs::create_dir_all(&pack_dir)?;
    let mut indexer = git2::Indexer::new(Some(&odb), &pack_dir, 0o644, true)?;
    indexer.write_all(pack_data)?;
    indexer.commit()?;

    for head in &inspection.heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    for head in &inspection.heads {
        if is_head_already_applied(&repo, head)? {
            continue;
        }
        repo.reference(&head.reference, head.oid, true, "receive bundle import")?;
    }

    Ok(ReceiveBundleResult {
        bundle_version: inspection.version,
        imported_heads: inspection.heads,
    })
}

pub(crate) fn is_head_already_applied(repo: &git2::Repository, head: &BundleHead) -> Result<bool> {
    let current_target = match repo.find_reference(&head.reference) {
        Ok(reference) => reference.target().or_else(|| {
            reference
                .resolve()
                .ok()
                .and_then(|resolved| resolved.target())
        }),
        Err(err) if err.code() == git2::ErrorCode::NotFound => None,
        Err(err) => return Err(err.into()),
    };

    let Some(current_target) = current_target else {
        return Ok(false);
    };
    if current_target != head.oid {
        return Ok(false);
    }

    Ok(repo.find_commit(head.oid).is_ok())
}
