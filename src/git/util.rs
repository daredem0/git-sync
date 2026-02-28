//! Git-layer util functionality.

use crate::git::{BundleVersion, ChangeStatus};
use anyhow::{Result, anyhow, bail};
use std::fs;
use std::mem::MaybeUninit;
use std::path::Path;

/// Returns the current UNIX timestamp in seconds.
///
/// # Errors
///
/// Returns an error when the system clock is before the UNIX epoch.
pub(crate) fn current_unix_timestamp_secs() -> Result<u64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| anyhow!("system clock is before unix epoch"))?;
    Ok(duration.as_secs())
}

/// Returns the current username from common environment variables.
///
/// Falls back to `"unknown"` when no non-empty value is available.
pub(crate) fn current_username() -> String {
    for key in ["USER", "USERNAME"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Returns the current hostname from environment or `/etc/hostname`.
///
/// Falls back to `"unknown-host"` when no non-empty value is available.
pub(crate) fn current_hostname() -> String {
    if let Ok(value) = std::env::var("HOSTNAME") {
        let value = value.trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }

    if let Ok(contents) = fs::read_to_string("/etc/hostname") {
        let value = contents.trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }

    "unknown-host".to_string()
}

/// Returns the SHA-256 digest of `bytes` as a lowercase hex string.
///
/// # Errors
///
/// Returns an error when OpenSSL digest operations fail.
pub(crate) fn sha256_hex(bytes: &[u8]) -> Result<String> {
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

    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Converts an optional git path into a lossy UTF-8 string.
///
/// # Errors
///
/// Returns an error when git omitted the path for a diff entry.
pub(crate) fn path_to_string(path: Option<&Path>) -> Result<String> {
    match path {
        Some(path) => Ok(path.to_string_lossy().into_owned()),
        None => Err(anyhow!("diff entry is missing file path")),
    }
}

/// Converts a possibly-zero OID into `None` for absent values.
pub(crate) fn oid_or_none(oid: git2::Oid) -> Option<git2::Oid> {
    if oid.is_zero() { None } else { Some(oid) }
}

/// Encodes a change status as the manifest short code.
pub(crate) fn status_code(status: ChangeStatus) -> &'static str {
    match status {
        ChangeStatus::Added => "A",
        ChangeStatus::Modified => "M",
        ChangeStatus::Deleted => "D",
        ChangeStatus::Renamed => "R",
        ChangeStatus::Copied => "C",
        ChangeStatus::TypeChanged => "T",
    }
}

/// Encodes a bundle header version for metadata fields.
pub(crate) fn bundle_version_code(version: BundleVersion) -> &'static str {
    match version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    }
}
