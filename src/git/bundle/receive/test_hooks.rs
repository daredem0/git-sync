// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Test-only fault-injection hooks for receive apply/rollback behavior.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Supports deterministic failure-path testing without changing production code paths.

use super::*;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

static FORCE_MANUAL_CAS_APPLY: AtomicBool = AtomicBool::new(false);
static MANUAL_CAS_FAIL_AT_UPDATE_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static TRANSACTION_INJECT_COMMIT_FAILURE: AtomicBool = AtomicBool::new(false);
static TRANSACTION_FAIL_AT_LOCK_REF_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static TRANSACTION_FAIL_AT_SET_TARGET_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
static FORCE_MANUAL_CAS_MUTEX: Mutex<()> = Mutex::new(());
static MANUAL_CAS_MUTATE_BEFORE_CHECK: Mutex<Option<ManualCasMutationBeforeCheck>> =
    Mutex::new(None);
static ROLLBACK_FAIL_FOR_REF: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub(super) struct ManualCasMutationBeforeCheck {
    pub(super) update_index: usize,
    pub(super) ref_name: String,
    pub(super) mutate_to_oid: Option<git2::Oid>,
}

/// Test-only guard to force receive to use manual CAS apply backend.
pub(crate) struct ForcedManualCasGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for ForcedManualCasGuard {
    fn drop(&mut self) {
        FORCE_MANUAL_CAS_APPLY.store(false, Ordering::SeqCst);
        MANUAL_CAS_FAIL_AT_UPDATE_INDEX.store(usize::MAX, Ordering::SeqCst);
        TRANSACTION_INJECT_COMMIT_FAILURE.store(false, Ordering::SeqCst);
        TRANSACTION_FAIL_AT_LOCK_REF_INDEX.store(usize::MAX, Ordering::SeqCst);
        TRANSACTION_FAIL_AT_SET_TARGET_INDEX.store(usize::MAX, Ordering::SeqCst);
        *MANUAL_CAS_MUTATE_BEFORE_CHECK
            .lock()
            .expect("manual-cas mutation config mutex should not be poisoned") = None;
        *ROLLBACK_FAIL_FOR_REF
            .lock()
            .expect("rollback fault config mutex should not be poisoned") = None;
    }
}

/// Enables manual-CAS backend forcing for deterministic fallback-path tests.
pub(crate) fn force_manual_cas_for_tests() -> ForcedManualCasGuard {
    configure_fault_injection_for_tests(true, None, None, None, false, None, None)
}

pub(super) fn configure_fault_injection_for_tests(
    force_manual_cas: bool,
    manual_cas_fail_at_update_index: Option<usize>,
    manual_cas_mutate_before_check: Option<ManualCasMutationBeforeCheck>,
    rollback_fail_for_ref: Option<String>,
    transaction_inject_commit_failure: bool,
    transaction_fail_at_lock_ref_index: Option<usize>,
    transaction_fail_at_set_target_index: Option<usize>,
) -> ForcedManualCasGuard {
    let lock = FORCE_MANUAL_CAS_MUTEX
        .lock()
        .expect("manual-cas test mutex should not be poisoned");
    FORCE_MANUAL_CAS_APPLY.store(force_manual_cas, Ordering::SeqCst);
    MANUAL_CAS_FAIL_AT_UPDATE_INDEX.store(
        manual_cas_fail_at_update_index.unwrap_or(usize::MAX),
        Ordering::SeqCst,
    );
    TRANSACTION_INJECT_COMMIT_FAILURE.store(transaction_inject_commit_failure, Ordering::SeqCst);
    TRANSACTION_FAIL_AT_LOCK_REF_INDEX.store(
        transaction_fail_at_lock_ref_index.unwrap_or(usize::MAX),
        Ordering::SeqCst,
    );
    TRANSACTION_FAIL_AT_SET_TARGET_INDEX.store(
        transaction_fail_at_set_target_index.unwrap_or(usize::MAX),
        Ordering::SeqCst,
    );
    *MANUAL_CAS_MUTATE_BEFORE_CHECK
        .lock()
        .expect("manual-cas mutation config mutex should not be poisoned") =
        manual_cas_mutate_before_check;
    *ROLLBACK_FAIL_FOR_REF
        .lock()
        .expect("rollback fault config mutex should not be poisoned") = rollback_fail_for_ref;
    ForcedManualCasGuard { _lock: lock }
}

pub(super) fn force_manual_cas_apply_enabled() -> bool {
    FORCE_MANUAL_CAS_APPLY.load(Ordering::SeqCst)
}

pub(super) fn transaction_fail_at_lock_ref_index() -> Option<usize> {
    let value = TRANSACTION_FAIL_AT_LOCK_REF_INDEX.load(Ordering::SeqCst);
    (value != usize::MAX).then_some(value)
}

pub(super) fn transaction_fail_at_set_target_index() -> Option<usize> {
    let value = TRANSACTION_FAIL_AT_SET_TARGET_INDEX.load(Ordering::SeqCst);
    (value != usize::MAX).then_some(value)
}

pub(super) fn transaction_inject_commit_failure() -> bool {
    TRANSACTION_INJECT_COMMIT_FAILURE.load(Ordering::SeqCst)
}

pub(super) fn manual_cas_fail_at_update_index() -> Option<usize> {
    let value = MANUAL_CAS_FAIL_AT_UPDATE_INDEX.load(Ordering::SeqCst);
    (value != usize::MAX).then_some(value)
}

pub(super) fn maybe_inject_manual_cas_mutation_before_check(
    repo: &git2::Repository,
    update_index: usize,
) -> Result<()> {
    let mut configured = MANUAL_CAS_MUTATE_BEFORE_CHECK
        .lock()
        .expect("manual-cas mutation config mutex should not be poisoned");
    let Some(mutation) = configured.clone() else {
        return Ok(());
    };
    if mutation.update_index != update_index {
        return Ok(());
    }

    match mutation.mutate_to_oid {
        Some(oid) => {
            repo.reference(
                &mutation.ref_name,
                oid,
                true,
                "receive fault injection: mutate ref before CAS precondition check",
            )?;
        }
        None => match repo.find_reference(&mutation.ref_name) {
            Ok(mut reference) => {
                reference.delete()?;
            }
            Err(err) if err.code() == git2::ErrorCode::NotFound => {}
            Err(err) => return Err(err.into()),
        },
    }
    *configured = None;
    Ok(())
}

pub(super) fn should_inject_rollback_failure_for_ref(ref_name: &str) -> bool {
    let configured = ROLLBACK_FAIL_FOR_REF
        .lock()
        .expect("rollback fault config mutex should not be poisoned");
    let Some(configured_ref) = configured.as_deref() else {
        return false;
    };
    configured_ref == ref_name || configured_ref == "*"
}
