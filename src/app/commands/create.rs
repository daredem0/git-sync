//! `create` command handler.

use anyhow::Result;
use std::path::PathBuf;

use crate::git::{
    CreateBundleOptions, create_bundle, create_bundle_with_options,
    remove_unarchived_bundle_artifacts,
};

pub(super) fn run(
    repo: PathBuf,
    from: String,
    to: String,
    output: PathBuf,
    with_patches: bool,
) -> Result<()> {
    let result = if with_patches {
        create_bundle_with_options(
            &repo,
            &from,
            &to,
            &output,
            CreateBundleOptions {
                include_patch_sidecar: true,
            },
        )?
    } else {
        create_bundle(&repo, &from, &to, &output)?
    };

    let patch_audit_display = result
        .patch_audit_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());

    remove_unarchived_bundle_artifacts(&result)?;
    println!(
        "bundle package created: archive={}, from={}, to={}, tip_ref={}, included_patch={}",
        result.archive_path.display(),
        result.from_commit_id,
        result.to_commit_id,
        result.tip_ref_name,
        if patch_audit_display == "-" {
            "no"
        } else {
            "yes"
        }
    );

    Ok(())
}
