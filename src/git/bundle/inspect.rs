use crate::git::{BundleHead, BundleInspection, BundleVersion};
use anyhow::{Result, anyhow, bail};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn inspect_bundle(bundle_path: &Path) -> Result<BundleInspection> {
    if !bundle_path.exists() {
        bail!("bundle path does not exist: {}", bundle_path.display());
    }
    if !bundle_path.is_file() {
        bail!("bundle path is not a file: {}", bundle_path.display());
    }

    let file = File::open(bundle_path)?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;

    let normalized = first_line.trim_end_matches(&['\r', '\n'][..]);
    let version = if normalized == "# v2 git bundle" {
        BundleVersion::V2
    } else if normalized == "# v3 git bundle" {
        BundleVersion::V3
    } else {
        bail!("bundle file is not a valid git bundle header");
    };

    let mut prerequisites = Vec::new();
    let mut heads = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }

        let normalized = line.trim_end_matches(&['\r', '\n'][..]);
        if normalized.is_empty() {
            break;
        }

        if let Some(rest) = normalized.strip_prefix('-') {
            let oid_token = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("invalid bundle prerequisite line: '{normalized}'"))?;
            let oid = git2::Oid::from_str(oid_token)?;
            prerequisites.push(oid);
            continue;
        }

        let mut parts = normalized.splitn(2, ' ');
        let oid_token = parts
            .next()
            .ok_or_else(|| anyhow!("invalid bundle head line: '{normalized}'"))?;
        let reference = parts
            .next()
            .ok_or_else(|| anyhow!("bundle head line missing reference: '{normalized}'"))?;
        heads.push(BundleHead {
            oid: git2::Oid::from_str(oid_token)?,
            reference: reference.to_string(),
        });
    }

    Ok(BundleInspection {
        version,
        prerequisites,
        heads,
    })
}
