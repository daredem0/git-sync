// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! CLI command handler for create flows.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::Result;
use std::path::PathBuf;

use crate::git::{
    CreateBundleOptions, CreateBundleResult, create_bundle, create_bundle_with_options,
    remove_unarchived_bundle_artifacts,
};

#[derive(Debug)]
struct CreateRunInput {
    repo: PathBuf,
    from: String,
    to: String,
    output: PathBuf,
    with_patches: bool,
    assume_present: Vec<String>,
}

pub(super) fn run(
    repo: PathBuf,
    from: String,
    to: String,
    output: PathBuf,
    with_patches: bool,
    assume_present: Vec<String>,
) -> Result<()> {
    let input = CreateRunInput {
        repo,
        from,
        to,
        output,
        with_patches,
        assume_present,
    };
    run_with(
        input,
        create_bundle,
        create_bundle_with_options,
        remove_unarchived_bundle_artifacts,
    )
}

fn run_with<FCreate, FCreateWithOptions, FCleanup>(
    input: CreateRunInput,
    create: FCreate,
    create_with_options: FCreateWithOptions,
    cleanup: FCleanup,
) -> Result<()>
where
    FCreate: FnOnce(&std::path::Path, &str, &str, &std::path::Path) -> Result<CreateBundleResult>,
    FCreateWithOptions: FnOnce(
        &std::path::Path,
        &str,
        &str,
        &std::path::Path,
        CreateBundleOptions,
    ) -> Result<CreateBundleResult>,
    FCleanup: FnOnce(&CreateBundleResult) -> Result<()>,
{
    let use_options_path = input.with_patches || !input.assume_present.is_empty();
    let result = if use_options_path {
        create_with_options(
            &input.repo,
            &input.from,
            &input.to,
            &input.output,
            CreateBundleOptions {
                include_patch_sidecar: input.with_patches,
                assume_present_revs: input.assume_present,
            },
        )?
    } else {
        create(&input.repo, &input.from, &input.to, &input.output)?
    };

    let patch_audit_display = result
        .patch_audit_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());

    cleanup(&result)?;
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

#[cfg(test)]
mod tests;
