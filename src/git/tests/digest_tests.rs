//! Unit tests for digest helpers.
//!
//! Focus: centralized SHA-1/SHA-256 and hex encoding behavior used by proof-sensitive paths.

use super::*;

// Verifies that SHA-1 hex hashing returns the known digest for the canonical "abc" vector.
#[test]
fn sha1_hex_returns_expected_digest_for_known_input() {
    let digest = sha1_hex(b"abc").expect("sha1 hashing should succeed");
    assert_eq!(
        digest,
        "a9993e364706816aba3e25717850c26c9cd0d89d".to_string(),
        "sha1 digest should match known test vector for 'abc'"
    );
}

// Verifies that SHA-1 raw bytes and hex rendering remain consistent.
#[test]
fn sha1_bytes_and_hex_encode_are_consistent() {
    let raw = sha1_bytes(b"git-sync").expect("sha1 byte hashing should succeed");
    let rendered = hex_encode(&raw);
    let direct = sha1_hex(b"git-sync").expect("sha1 hex hashing should succeed");
    assert_eq!(
        rendered, direct,
        "hex rendering of raw sha1 bytes must match direct sha1 hex helper"
    );
}

// Verifies that SHA-256 hex hashing remains deterministic for a fixed known vector.
#[test]
fn sha256_hex_returns_expected_digest_for_known_input() {
    let digest = sha256_hex(b"abc").expect("sha256 hashing should succeed");
    assert_eq!(
        digest,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
        "sha256 digest should match known test vector for 'abc'"
    );
}
