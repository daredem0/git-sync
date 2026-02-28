//! Build-time version injection derived from git describe output.

use std::process::Command;

fn main() {
    // Rebuild when git refs move so the embedded version stays current.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    println!("cargo:rerun-if-env-changed=GIT_SYNC_AUDIT_VERSION_OVERRIDE");

    let version = resolve_version();
    println!("cargo:rustc-env=GIT_SYNC_AUDIT_VERSION={version}");
}

fn resolve_version() -> String {
    if let Ok(override_version) = std::env::var("GIT_SYNC_AUDIT_VERSION_OVERRIDE") {
        let trimmed = override_version.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    git_describe()
        .map(normalize_tag_prefix)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

fn git_describe() -> Option<String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = Command::new("git")
        .args([
            "-C",
            &manifest_dir,
            "describe",
            "--tags",
            "--dirty",
            "--always",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_tag_prefix(version: String) -> String {
    if let Some(stripped) = version.strip_prefix('v') {
        if stripped
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit())
        {
            return stripped.to_string();
        }
    }

    version
}
