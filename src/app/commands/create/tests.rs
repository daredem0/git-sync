// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for app/commands/create.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

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
        CreateRunInput {
            repo: PathBuf::from("/tmp/repo"),
            from: "from".to_string(),
            to: "to".to_string(),
            output: PathBuf::from("sync.bundle"),
            with_patches: false,
            assume_present: Vec::new(),
        },
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
        CreateRunInput {
            repo: PathBuf::from("/tmp/repo"),
            from: "from".to_string(),
            to: "to".to_string(),
            output: PathBuf::from("sync.bundle"),
            with_patches: true,
            assume_present: Vec::new(),
        },
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
        CreateRunInput {
            repo: PathBuf::from("/tmp/repo"),
            from: "from".to_string(),
            to: "to".to_string(),
            output: PathBuf::from("sync.bundle"),
            with_patches: false,
            assume_present: Vec::new(),
        },
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

#[test]
fn run_with_uses_create_with_options_when_assume_present_is_non_empty() {
    let called_create = Rc::new(RefCell::new(false));
    let called_create_with_options = Rc::new(RefCell::new(false));
    let captured_assume_present = Rc::new(RefCell::new(Vec::<String>::new()));

    let c1 = Rc::clone(&called_create);
    let c2 = Rc::clone(&called_create_with_options);
    let c3 = Rc::clone(&captured_assume_present);
    let result = run_with(
        CreateRunInput {
            repo: PathBuf::from("/tmp/repo"),
            from: "from".to_string(),
            to: "to".to_string(),
            output: PathBuf::from("sync.bundle"),
            with_patches: false,
            assume_present: vec!["refs/heads/stable".to_string()],
        },
        move |_repo, _from, _to, _output| {
            *c1.borrow_mut() = true;
            Ok(sample_result(false))
        },
        move |_repo, _from, _to, _output, options| {
            *c2.borrow_mut() = true;
            *c3.borrow_mut() = options.assume_present_revs;
            Ok(sample_result(false))
        },
        |_result| Ok(()),
    );

    assert!(
        result.is_ok(),
        "assume-present create path should succeed with mocked dependencies"
    );
    assert!(
        !*called_create.borrow(),
        "plain create function must not be called when assume-present is configured"
    );
    assert!(
        *called_create_with_options.borrow(),
        "with-options create function should be used when assume-present is configured"
    );
    assert_eq!(
        *captured_assume_present.borrow(),
        vec!["refs/heads/stable".to_string()],
        "assume-present revisions should be forwarded to create options unchanged"
    );
}
