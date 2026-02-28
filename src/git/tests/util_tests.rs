//! Unit tests for util tests.

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
