//! Digest helpers shared by bundle, metadata, and payload verification paths.

use anyhow::{Result, bail};
use std::mem::MaybeUninit;

/// Returns lowercase hex for arbitrary bytes.
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns SHA-1 digest bytes for `bytes`.
///
/// # Errors
///
/// Returns an error when OpenSSL digest operations fail.
pub(crate) fn sha1_bytes(bytes: &[u8]) -> Result<[u8; 20]> {
    let mut ctx = MaybeUninit::<openssl_sys::SHA_CTX>::uninit();

    // SAFETY: SHA1_Init initializes the context pointed to by ctx.
    let init_ok = unsafe { openssl_sys::SHA1_Init(ctx.as_mut_ptr()) } == 1;
    if !init_ok {
        bail!("failed to initialize SHA-1 context");
    }

    // SAFETY: ctx was initialized successfully by SHA1_Init above.
    let mut ctx = unsafe { ctx.assume_init() };

    // SAFETY: bytes pointer and length are valid for the lifetime of this call.
    let update_ok =
        unsafe { openssl_sys::SHA1_Update(&mut ctx, bytes.as_ptr().cast(), bytes.len()) } == 1;
    if !update_ok {
        bail!("failed to update SHA-1 digest");
    }

    let mut digest = [0u8; 20];
    // SAFETY: digest points to a 20-byte output buffer and ctx is a valid hash context.
    let final_ok = unsafe { openssl_sys::SHA1_Final(digest.as_mut_ptr(), &mut ctx) } == 1;
    if !final_ok {
        bail!("failed to finalize SHA-1 digest");
    }

    Ok(digest)
}

/// Returns the SHA-1 digest of `bytes` as a lowercase hex string.
///
/// # Errors
///
/// Returns an error when SHA-1 computation fails.
pub(crate) fn sha1_hex(bytes: &[u8]) -> Result<String> {
    Ok(hex_encode(&sha1_bytes(bytes)?))
}

/// Returns SHA-256 digest bytes for `bytes`.
///
/// # Errors
///
/// Returns an error when OpenSSL digest operations fail.
pub(crate) fn sha256_bytes(bytes: &[u8]) -> Result<[u8; 32]> {
    let mut ctx = MaybeUninit::<openssl_sys::SHA256_CTX>::uninit();

    // SAFETY: SHA256_Init initializes the context pointed to by ctx.
    let init_ok = unsafe { openssl_sys::SHA256_Init(ctx.as_mut_ptr()) } == 1;
    if !init_ok {
        bail!("failed to initialize SHA-256 context");
    }

    // SAFETY: ctx was initialized successfully by SHA256_Init above.
    let mut ctx = unsafe { ctx.assume_init() };

    // SAFETY: bytes pointer and length are valid for the lifetime of this call.
    let update_ok =
        unsafe { openssl_sys::SHA256_Update(&mut ctx, bytes.as_ptr().cast(), bytes.len()) } == 1;
    if !update_ok {
        bail!("failed to update SHA-256 digest");
    }

    let mut digest = [0u8; 32];
    // SAFETY: digest points to a 32-byte output buffer and ctx is a valid hash context.
    let final_ok = unsafe { openssl_sys::SHA256_Final(digest.as_mut_ptr(), &mut ctx) } == 1;
    if !final_ok {
        bail!("failed to finalize SHA-256 digest");
    }

    Ok(digest)
}

/// Returns the SHA-256 digest of `bytes` as a lowercase hex string.
///
/// # Errors
///
/// Returns an error when SHA-256 computation fails.
pub(crate) fn sha256_hex(bytes: &[u8]) -> Result<String> {
    Ok(hex_encode(&sha256_bytes(bytes)?))
}
