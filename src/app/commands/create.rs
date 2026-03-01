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

pub(super) fn run(
    repo: PathBuf,
    from: String,
    to: String,
    output: PathBuf,
    with_patches: bool,
) -> Result<()> {
    run_with(
        repo,
        from,
        to,
        output,
        with_patches,
        create_bundle,
        create_bundle_with_options,
        remove_unarchived_bundle_artifacts,
    )
}

fn run_with<FCreate, FCreateWithOptions, FCleanup>(
    repo: PathBuf,
    from: String,
    to: String,
    output: PathBuf,
    with_patches: bool,
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
    let result = if with_patches {
        create_with_options(
            &repo,
            &from,
            &to,
            &output,
            CreateBundleOptions {
                include_patch_sidecar: true,
            },
        )?
    } else {
        create(&repo, &from, &to, &output)?
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
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn sample_result(with_patch: bool) -> CreateBundleResult {
        CreateBundleResult {
            bundle_path: PathBuf::from("sync.bundle"),
            audit_path: PathBuf::from("sync.bundle.caudit.json"),
            patch_audit_path: if with_patch {
                Some(PathBuf::from("sync.bundle.caudit.patch"))
            } else {
                None
            },
            archive_path: PathBuf::from("sync.bundle.zip"),
            from_commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                .expect("must parse test oid"),
            to_commit_id: git2::Oid::from_str("2222222222222222222222222222222222222222")
                .expect("must parse test oid"),
            tip_ref_name: "refs/heads/main".to_string(),
        }
    }

    #[test]
    fn run_with_uses_create_path_when_with_patches_is_false() {
        let called_create = Rc::new(RefCell::new(false));
        let called_create_with_options = Rc::new(RefCell::new(false));
        let called_cleanup = Rc::new(RefCell::new(false));

        let c1 = Rc::clone(&called_create);
        let c2 = Rc::clone(&called_create_with_options);
        let c3 = Rc::clone(&called_cleanup);
        let result = run_with(
            PathBuf::from("/tmp/repo"),
            "from".to_string(),
            "to".to_string(),
            PathBuf::from("sync.bundle"),
            false,
            move |_repo, _from, _to, _output| {
                *c1.borrow_mut() = true;
                Ok(sample_result(false))
            },
            move |_repo, _from, _to, _output, _options| {
                *c2.borrow_mut() = true;
                Ok(sample_result(true))
            },
            move |_result| {
                *c3.borrow_mut() = true;
                Ok(())
            },
        );

        assert!(
            result.is_ok(),
            "create path should succeed with mocked dependencies"
        );
        assert!(
            *called_create.borrow(),
            "plain create function should be called"
        );
        assert!(
            !*called_create_with_options.borrow(),
            "with-options create function must not be called when with_patches=false"
        );
        assert!(
            *called_cleanup.borrow(),
            "cleanup function should run after create succeeds"
        );
    }

    #[test]
    fn run_with_uses_create_with_options_when_with_patches_is_true() {
        let called_create = Rc::new(RefCell::new(false));
        let called_create_with_options = Rc::new(RefCell::new(false));
        let captured_include_patch = Rc::new(RefCell::new(false));

        let c1 = Rc::clone(&called_create);
        let c2 = Rc::clone(&called_create_with_options);
        let c3 = Rc::clone(&captured_include_patch);
        let result = run_with(
            PathBuf::from("/tmp/repo"),
            "from".to_string(),
            "to".to_string(),
            PathBuf::from("sync.bundle"),
            true,
            move |_repo, _from, _to, _output| {
                *c1.borrow_mut() = true;
                Ok(sample_result(false))
            },
            move |_repo, _from, _to, _output, options| {
                *c2.borrow_mut() = true;
                *c3.borrow_mut() = options.include_patch_sidecar;
                Ok(sample_result(true))
            },
            |_result| Ok(()),
        );

        assert!(
            result.is_ok(),
            "with-patches create path should succeed with mocked dependencies"
        );
        assert!(
            !*called_create.borrow(),
            "plain create function must not be called when with_patches=true"
        );
        assert!(
            *called_create_with_options.borrow(),
            "with-options create function should be called"
        );
        assert!(
            *captured_include_patch.borrow(),
            "with-patches mode must set include_patch_sidecar=true"
        );
    }

    #[test]
    fn run_with_propagates_cleanup_error() {
        let result = run_with(
            PathBuf::from("/tmp/repo"),
            "from".to_string(),
            "to".to_string(),
            PathBuf::from("sync.bundle"),
            false,
            |_repo, _from, _to, _output| Ok(sample_result(false)),
            |_repo, _from, _to, _output, _options| Ok(sample_result(true)),
            |_result| Err(anyhow!("cleanup failed")),
        );

        let error = result.expect_err("cleanup failure should be returned to caller");
        assert!(
            error.to_string().contains("cleanup failed"),
            "error should preserve cleanup failure context"
        );
    }
}
