//! Zlib stream helpers for PACK entry decoding.

use anyhow::{Result, bail};
use flate2::{Decompress, FlushDecompress, Status};

/// Decompresses one zlib stream from the start of `bytes`, returning consumed input bytes.
pub(super) fn decompress_zlib_stream(bytes: &[u8]) -> Result<(usize, Vec<u8>)> {
    let mut decompressor = Decompress::new(true);
    let mut out = Vec::new();
    let mut consumed_total = 0usize;
    let mut no_progress_streak = 0usize;

    loop {
        if consumed_total >= bytes.len() {
            bail!("unexpected end of zlib stream while reading pack entry");
        }
        let input = &bytes[consumed_total..];
        out.reserve(16 * 1024);
        let before_in = decompressor.total_in();
        let before_out = decompressor.total_out();
        let status = decompressor.decompress_vec(input, &mut out, FlushDecompress::None)?;
        let consumed = (decompressor.total_in() - before_in) as usize;
        let produced = (decompressor.total_out() - before_out) as usize;
        consumed_total += consumed;

        match status {
            Status::StreamEnd => break,
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    no_progress_streak += 1;
                    if no_progress_streak >= 8 {
                        bail!("zlib stream made no progress while parsing pack entry");
                    }
                } else {
                    no_progress_streak = 0;
                }
            }
        }
    }

    Ok((consumed_total, out))
}

#[cfg(test)]
mod tests {
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
}
