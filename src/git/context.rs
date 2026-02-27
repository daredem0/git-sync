//! Git-layer context functionality.

use crate::app::AppConfig;
use crate::git::{OpenContext, inspect_bundle};
use anyhow::{Result, bail};

/// Validates repo/bundle paths and resolves references for UI startup context.
///
/// # Errors
///
/// Returns an error when configured paths do not exist, references do not
/// resolve to commits, or `tip_ref` is not equal/descendant of `base_ref`.
pub fn open_context(config: &AppConfig) -> Result<OpenContext> {
    if !config.repo_path.exists() {
        bail!(
            "repository path does not exist: {}",
            config.repo_path.display()
        );
    }
    if !config.bundle_path.exists() {
        bail!(
            "bundle path does not exist: {}",
            config.bundle_path.display()
        );
    }
    if !config.bundle_path.is_file() {
        bail!(
            "bundle path is not a file: {}",
            config.bundle_path.display()
        );
    }

    let repo = git2::Repository::open(&config.repo_path)?;
    let base_obj = repo.revparse_single(&config.base_ref)?;
    let base_commit = base_obj.peel_to_commit()?;
    let base_commit_id = base_commit.id();

    let tip_commit_id = if let Some(tip_ref) = &config.tip_ref {
        let tip_obj = repo.revparse_single(tip_ref)?;
        let tip_commit_id = tip_obj.peel_to_commit()?.id();

        if tip_commit_id != base_commit_id
            && !repo.graph_descendant_of(tip_commit_id, base_commit_id)?
        {
            bail!(
                "tip ref '{}' must be the same commit as base ref '{}' or a descendant of it",
                tip_ref,
                config.base_ref
            );
        }

        Some(tip_commit_id)
    } else {
        None
    };

    let bundle_inspection = inspect_bundle(&config.bundle_path)?;
    Ok(OpenContext {
        base_commit_id,
        tip_commit_id,
        bundle_version: bundle_inspection.version,
    })
}
