//! Unit tests for util tests.

use super::*;

// Focus: utility-level parsing/formatting helpers used across bundle, metadata, and manifest paths.

// Verifies that parse_optional_oid handles None, valid OID strings, and invalid OID input.
#[test]
fn parse_optional_oid_handles_none_valid_and_invalid_inputs() {
    let none_result = parse_optional_oid(None).expect("None should parse to None");
    assert_eq!(none_result, None, "None input must map to None output");

    let oid_text = "1111111111111111111111111111111111111111";
    let valid_result = parse_optional_oid(Some(oid_text)).expect("valid OID should parse");
    assert_eq!(
        valid_result,
        Some(git2::Oid::from_str(oid_text).expect("must parse fixed test oid")),
        "valid OID text must parse to matching OID"
    );

    let invalid_result = parse_optional_oid(Some("invalid-oid"));
    assert!(
        invalid_result.is_err(),
        "invalid OID text should return an error"
    );
}

// Verifies that status-code parsing supports C and T codes used for copy/type-change deltas.
#[test]
fn parse_status_code_supports_copy_and_typechange() {
    assert_eq!(
        parse_status_code("C").expect("C should parse"),
        ChangeStatus::Copied,
        "status code C must map to Copied"
    );
    assert_eq!(
        parse_status_code("T").expect("T should parse"),
        ChangeStatus::TypeChanged,
        "status code T must map to TypeChanged"
    );
}

// Verifies that parse_status_code rejects unsupported status letters.
#[test]
fn parse_status_code_rejects_unknown_code() {
    let result = parse_status_code("X");
    assert!(result.is_err(), "unsupported status codes must error");
}

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

// Verifies that oid_to_str emits '-' for None and canonical hex for Some(oid).
#[test]
fn oid_to_str_formats_none_and_some_values() {
    assert_eq!(
        oid_to_str(None),
        "-".to_string(),
        "None oid should render as '-' placeholder"
    );

    let oid = git2::Oid::from_str("3333333333333333333333333333333333333333")
        .expect("must parse fixed oid");
    assert_eq!(
        oid_to_str(Some(oid)),
        oid.to_string(),
        "present oid should render as lowercase hex"
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

// Verifies that sha256_hex returns deterministic known digest output for a fixed byte sequence.
#[test]
fn sha256_hex_returns_expected_digest_for_known_input() {
    let digest = sha256_hex(b"abc").expect("sha256 hashing should succeed");
    assert_eq!(
        digest,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
        "sha256 digest should match known test vector for 'abc'"
    );
}
