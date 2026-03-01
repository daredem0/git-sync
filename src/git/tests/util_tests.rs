//! Unit tests for util tests.

use super::super::util::{
    current_hostname, current_unix_timestamp_secs, current_username, status_code,
};
use super::*;

// Focus: utility-level helpers used across bundle creation, payload audit, and metadata verification.

// Verifies that oid_or_none returns None for zero OIDs and Some for non-zero OIDs.
#[test]
fn oid_or_none_handles_zero_and_nonzero_oids() {
    let zero = git2::Oid::zero();
    assert_eq!(
        oid_or_none(zero),
        None,
        "zero oid should be normalized to None"
    );

    let nonzero = git2::Oid::from_str("2222222222222222222222222222222222222222")
        .expect("must parse fixed oid");
    assert_eq!(
        oid_or_none(nonzero),
        Some(nonzero),
        "non-zero oid should remain present"
    );
}

// Verifies that bundle_version_code maps both known bundle versions to expected textual codes.
#[test]
fn bundle_version_code_returns_expected_string_codes() {
    assert_eq!(
        bundle_version_code(BundleVersion::V2),
        "v2",
        "BundleVersion::V2 should map to code 'v2'"
    );
    assert_eq!(
        bundle_version_code(BundleVersion::V3),
        "v3",
        "BundleVersion::V3 should map to code 'v3'"
    );
}

// Verifies that path_to_string rejects missing paths and accepts present paths.
#[test]
fn path_to_string_handles_some_and_none_paths() {
    let path = std::path::Path::new("relative/path.txt");
    let rendered = path_to_string(Some(path)).expect("some path should render");
    assert_eq!(
        rendered,
        "relative/path.txt".to_string(),
        "present path should render lossily to owned string"
    );

    let missing = path_to_string(None);
    assert!(
        missing.is_err(),
        "missing path should error to protect callers from invalid diff entries"
    );
}

// Verifies that status_code maps all supported change statuses to expected manifest codes.
#[test]
fn status_code_maps_all_change_status_variants() {
    assert_eq!(status_code(ChangeStatus::Added), "A");
    assert_eq!(status_code(ChangeStatus::Modified), "M");
    assert_eq!(status_code(ChangeStatus::Deleted), "D");
    assert_eq!(status_code(ChangeStatus::Renamed), "R");
    assert_eq!(status_code(ChangeStatus::Copied), "C");
    assert_eq!(status_code(ChangeStatus::TypeChanged), "T");
}

struct EnvGuard {
    key: &'static str,
    original: Option<String>,
}

impl EnvGuard {
    fn new(key: &'static str) -> Self {
        Self {
            key,
            original: std::env::var(key).ok(),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.original {
            // SAFETY: test process is single-threaded with respect to these env mutations.
            unsafe { std::env::set_var(self.key, value) };
        } else {
            // SAFETY: test process is single-threaded with respect to these env mutations.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

// Verifies that current_username selects USER first, then USERNAME, then unknown fallback.
#[test]
fn current_username_uses_expected_environment_fallback_order() {
    let _user_guard = EnvGuard::new("USER");
    let _username_guard = EnvGuard::new("USERNAME");

    // SAFETY: test mutates process env in a controlled scope guarded by EnvGuard.
    unsafe { std::env::set_var("USER", "alice") };
    // SAFETY: test mutates process env in a controlled scope guarded by EnvGuard.
    unsafe { std::env::set_var("USERNAME", "bob") };
    assert_eq!(current_username(), "alice");

    // SAFETY: test mutates process env in a controlled scope guarded by EnvGuard.
    unsafe { std::env::set_var("USER", "   ") };
    assert_eq!(
        current_username(),
        "bob",
        "when USER is blank, USERNAME should be used"
    );

    // SAFETY: test mutates process env in a controlled scope guarded by EnvGuard.
    unsafe { std::env::set_var("USERNAME", "   ") };
    assert_eq!(
        current_username(),
        "unknown",
        "when both env vars are blank, username should fall back to unknown"
    );
}

// Verifies that current_hostname prefers HOSTNAME env value when present.
#[test]
fn current_hostname_prefers_hostname_environment_variable() {
    let _hostname_guard = EnvGuard::new("HOSTNAME");
    // SAFETY: test mutates process env in a controlled scope guarded by EnvGuard.
    unsafe { std::env::set_var("HOSTNAME", "test-host") };
    assert_eq!(current_hostname(), "test-host");
}

// Verifies that current_unix_timestamp_secs returns a plausible non-zero timestamp.
#[test]
fn current_unix_timestamp_secs_returns_nonzero_timestamp() {
    let ts = current_unix_timestamp_secs().expect("timestamp lookup should succeed");
    assert!(
        ts > 1_000_000_000,
        "timestamp should be a plausible unix-seconds value"
    );
}
