//! `receive` command handler.

use anyhow::Result;
use std::path::PathBuf;

use crate::git::{
    BundleVersion, ReceiveBundleOptions, receive_bundle_input, receive_bundle_input_with_options,
};

pub(super) fn run(
    repo: PathBuf,
    bundle: PathBuf,
    verify_metadata: bool,
    dry_run: bool,
) -> Result<()> {
    let result = if verify_metadata || dry_run {
        receive_bundle_input_with_options(
            &bundle,
            &repo,
            ReceiveBundleOptions {
                verify_metadata,
                dry_run,
            },
        )?
    } else {
        receive_bundle_input(&bundle, &repo)?
    };

    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    if dry_run {
        println!(
            "bundle can be applied without conflicts: version={}, would_import_heads={}",
            version,
            result.imported_heads.len()
        );
    } else {
        println!(
            "bundle received: version={}, imported_heads={}",
            version,
            result.imported_heads.len()
        );
    }

    for head in &result.imported_heads {
        println!("HEAD\t{}\t{}", head.oid, head.reference);
    }

    if dry_run {
        render_dry_run_line_stats(&result.line_stats);
    }

    Ok(())
}

fn render_dry_run_line_stats(line_stats: &[crate::git::FileLineStat]) {
    println!();
    println!("would change (per-file line diff summary):");

    let path_header = "PATH";
    let adds_header = "+LINES";
    let dels_header = "-LINES";

    let path_width = std::cmp::max(
        path_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.path.len())
            .max()
            .unwrap_or(0),
    );
    let adds_width = std::cmp::max(
        adds_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.additions.to_string().len())
            .max()
            .unwrap_or(0),
    );
    let dels_width = std::cmp::max(
        dels_header.len(),
        line_stats
            .iter()
            .map(|stat| stat.deletions.to_string().len())
            .max()
            .unwrap_or(0),
    );

    println!(
        "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
        path_header, adds_header, dels_header
    );

    if line_stats.is_empty() {
        println!("(no file content changes)");
        return;
    }

    for stat in line_stats {
        println!(
            "{:<path_width$}  {:>adds_width$}  {:>dels_width$}",
            stat.path, stat.additions, stat.deletions
        );
    }
}
