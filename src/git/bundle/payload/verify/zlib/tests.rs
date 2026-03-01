// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for git/bundle/payload/verify/zlib.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::decompress_zlib_stream;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write as _;

fn compress(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .expect("must write zlib input bytes");
    encoder.finish().expect("must finalize zlib stream")
}

#[test]
fn decompress_zlib_stream_decodes_valid_stream() {
    let input = b"payload-body";
    let encoded = compress(input);

    let (consumed, out) =
        decompress_zlib_stream(&encoded).expect("valid zlib stream should decode");
    assert_eq!(consumed, encoded.len());
    assert_eq!(out, input);
}

#[test]
fn decompress_zlib_stream_rejects_truncated_stream() {
    let encoded = compress(b"payload-body");
    let truncated = &encoded[..encoded.len() - 1];

    let error = decompress_zlib_stream(truncated).expect_err("truncated zlib stream must fail");
    assert!(
        error
            .to_string()
            .contains("unexpected end of zlib stream while reading pack entry"),
        "error should report unexpected end of zlib stream"
    );
}
