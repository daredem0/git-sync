// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Bundle processing module for receive operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use super::inspect::inspect_bundle;
use super::parse::parse_bundle_payload;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::digest::sha256_hex;
use crate::git::metadata::verify_bundle_metadata_integrity_input;
use crate::git::util::path_to_string;
use crate::git::{
    BundleHead, BundleInspection, CommitAuditEntry, CommitAuditIdentity, FileLineStat,
    HeadAuditEntry, ReceiveApplyBackend, ReceiveBundleOptions, ReceiveBundleResult,
    ReceiveIntegratePolicy, ReceiveMergeabilityCheck, ReceiveMergeabilityStatus, ReceivePlanEntry,
    ReceivePlanStatus,
};
use anyhow::{Result, anyhow, bail};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod test_hooks;
#[cfg(test)]
pub(crate) use test_hooks::force_manual_cas_for_tests;

#[derive(Debug, Clone)]
struct ApplyBundleToRepoResult {
    preflight_plan: Vec<ReceivePlanEntry>,
    mergeability_checks: Vec<ReceiveMergeabilityCheck>,
    apply_backend: Option<ReceiveApplyBackend>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPath {
    StrictIndexer,
    CompatIndexerVerifyFalse,
    CompatFetchFallback,
}

impl ImportPath {
    fn is_compatibility_fallback(self) -> bool {
        !matches!(self, Self::StrictIndexer)
    }

    fn label(self) -> &'static str {
        match self {
            Self::StrictIndexer => "strict-indexer",
            Self::CompatIndexerVerifyFalse => "compat-indexer-verify-false",
            Self::CompatFetchFallback => "compat-fetch-fallback",
        }
    }
}

/// Receives a bundle input using default receive options.
///
/// Equivalent to calling [`receive_bundle_input_with_options`] with
/// [`ReceiveBundleOptions::default`].
///
/// # Errors
///
/// Returns an error when bundle parsing/import fails.
pub fn receive_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options_and_policy(
        bundle_input_path,
        receiver_repo_path,
        ReceiveBundleOptions::default(),
        ReceiveIntegratePolicy::FastForwardOnly,
    )
}

/// Receives a bundle input (`.bundle` or packaged `.zip`) into a repository.
///
/// When `dry_run` is enabled, import and diff analysis run against a temporary
/// bare mirror and do not mutate the receiver.
///
/// # Errors
///
/// Returns an error when metadata verification fails (if enabled), archive
/// extraction fails, or bundle import cannot be applied.
pub fn receive_bundle_input_with_options(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check(
        bundle_input_path,
        receiver_repo_path,
        options,
        ReceiveIntegratePolicy::FastForwardOnly,
        false,
        false,
        false,
    )
}

/// Receives a bundle input with explicit ref-integration policy.
///
/// When `dry_run` is enabled, import and integration are evaluated against a
/// temporary mirror and do not mutate the receiver.
///
/// # Errors
///
/// Returns an error when metadata verification fails (if enabled), archive
/// extraction fails, bundle import cannot be applied, or integration policy
/// constraints are violated (for example non-fast-forward updates).
pub fn receive_bundle_input_with_options_and_policy(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
    integrate_policy: ReceiveIntegratePolicy,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check(
        bundle_input_path,
        receiver_repo_path,
        options,
        integrate_policy,
        false,
        false,
        false,
    )
}

/// Receives a bundle input with explicit ref-integration policy and optional incoming branch mirroring.
///
/// When `incoming_as_branches` is enabled, incoming heads are mirrored under
/// `refs/heads/incoming/<bundle-id>/...` in addition to the stable safe namespace.
///
/// # Errors
///
/// Returns an error when metadata verification fails (if enabled), archive
/// extraction fails, bundle import cannot be applied, or integration policy
/// constraints are violated (for example non-fast-forward updates).
pub fn receive_bundle_input_with_options_policy_and_branch_mirror(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
    integrate_policy: ReceiveIntegratePolicy,
    incoming_as_branches: bool,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check(
        bundle_input_path,
        receiver_repo_path,
        options,
        integrate_policy,
        incoming_as_branches,
        false,
        false,
    )
}

/// Receives a bundle input with explicit policy, incoming-branch mirroring, and optional mergeability checks.
///
/// When `check_mergeability` is enabled, receive runs in analysis-only mode and
/// reports whether diverged refs would merge cleanly. Target refs are not updated.
pub fn receive_bundle_input_with_options_policy_and_branch_mirror_and_mergeability_check(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
    integrate_policy: ReceiveIntegratePolicy,
    incoming_as_branches: bool,
    check_mergeability: bool,
    verbose: bool,
) -> Result<ReceiveBundleResult> {
    if options.verify_metadata {
        verify_bundle_metadata_integrity_input(bundle_input_path)?;
    }

    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        receive_bundle(
            &extracted.bundle_path,
            receiver_repo_path,
            options.dry_run,
            integrate_policy,
            incoming_as_branches,
            check_mergeability,
            verbose,
        )
    } else {
        receive_bundle(
            bundle_input_path,
            receiver_repo_path,
            options.dry_run,
            integrate_policy,
            incoming_as_branches,
            check_mergeability,
            verbose,
        )
    }
}

/// Collects head-scoped commit and line-stat entries for a bundle input.
///
/// This imports into a temporary mirror and computes per-head summaries
/// without mutating the receiver repository.
///
/// # Errors
///
/// Returns an error when bundle inspection/import or commit traversal fails.
pub fn collect_head_audit_entries_for_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<Vec<HeadAuditEntry>> {
    with_imported_bundle_input_repo(bundle_input_path, receiver_repo_path, |repo, inspection| {
        collect_head_audit_entries(repo, inspection)
    })
}

/// Collects a unified patch for one file in a bundle commit.
///
/// # Errors
///
/// Returns an error when the commit/path is unavailable, when the file is not
/// changed in the target commit, or when a textual patch is unavailable.
pub fn collect_commit_file_patch_for_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    commit_id: git2::Oid,
    path: &str,
) -> Result<String> {
    with_imported_bundle_input_repo(
        bundle_input_path,
        receiver_repo_path,
        |repo, _inspection| collect_commit_file_patch(repo, commit_id, path),
    )
}

/// Applies a bundle to the receiver repository or to a dry-run mirror.
fn receive_bundle(
    bundle_path: &Path,
    receiver_repo_path: &Path,
    dry_run: bool,
    integrate_policy: ReceiveIntegratePolicy,
    incoming_as_branches: bool,
    check_mergeability: bool,
    verbose: bool,
) -> Result<ReceiveBundleResult> {
    let analysis_only = dry_run || check_mergeability;
    let inspection = inspect_bundle(bundle_path)?;
    if inspection.heads.is_empty() {
        bail!("bundle does not contain any heads to import");
    }

    let repo = git2::Repository::open(receiver_repo_path)?;
    let bundle_bytes = fs::read(bundle_path)?;
    let bundle_id = bundle_receive_id(&bundle_bytes)?;
    let all_heads_already_applied = inspection
        .heads
        .iter()
        .map(|head| is_head_already_applied(&repo, head))
        .collect::<Result<Vec<bool>>>()?
        .into_iter()
        .all(std::convert::identity);
    if all_heads_already_applied {
        let incoming_refs = incoming_head_refs(&inspection.heads, &bundle_id);
        let preflight_plan = compute_receive_plan(&repo, &incoming_refs)?;
        let mergeability_checks =
            if check_mergeability || integrate_policy == ReceiveIntegratePolicy::Merge {
                compute_receive_mergeability_checks(&repo, &preflight_plan)?
            } else {
                Vec::new()
            };
        let can_apply_without_conflicts =
            can_apply_receive_plan(&preflight_plan, integrate_policy, &mergeability_checks);
        if !analysis_only {
            write_incoming_namespace_refs(
                &repo,
                &inspection.heads,
                &bundle_id,
                incoming_as_branches,
            )?;
        }
        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts,
            apply_backend: None,
            preflight_plan,
            mergeability_checks,
            line_stats: Vec::new(),
        });
    }

    if analysis_only {
        // Dry-run operates on a temporary mirror so we can safely import and diff.
        let temp_repo = TempBareRepo::from_existing(receiver_repo_path)?;
        let dry_run_repo = git2::Repository::open_bare(&temp_repo.path)?;
        let apply_result = apply_bundle_to_repo(
            &dry_run_repo,
            bundle_path,
            &inspection.heads,
            integrate_policy,
            incoming_as_branches,
            check_mergeability,
            verbose,
        )?;
        let preflight_plan = apply_result.preflight_plan;
        let mergeability_checks = apply_result.mergeability_checks;
        let line_stats = collect_bundle_line_stats(&dry_run_repo, &inspection)?;
        let can_apply_without_conflicts =
            can_apply_receive_plan(&preflight_plan, integrate_policy, &mergeability_checks);

        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts,
            apply_backend: None,
            preflight_plan,
            mergeability_checks,
            line_stats,
        });
    }

    let apply_result = apply_bundle_to_repo(
        &repo,
        bundle_path,
        &inspection.heads,
        integrate_policy,
        incoming_as_branches,
        check_mergeability,
        verbose,
    )?;
    let preflight_plan = apply_result.preflight_plan;
    let mergeability_checks = apply_result.mergeability_checks;
    let can_apply_without_conflicts =
        can_apply_receive_plan(&preflight_plan, integrate_policy, &mergeability_checks);

    Ok(ReceiveBundleResult {
        bundle_version: inspection.version,
        imported_heads: inspection.heads,
        can_apply_without_conflicts,
        apply_backend: apply_result.apply_backend,
        preflight_plan,
        mergeability_checks,
        line_stats: Vec::new(),
    })
}

/// Imports a bundle PACK stream into the repository object database and refs.
///
/// # Errors
///
/// Returns an error when the PACK payload cannot be located/imported, when
/// imported head commits are missing, or ref updates fail.
fn apply_bundle_to_repo(
    repo: &git2::Repository,
    bundle_path: &Path,
    heads: &[BundleHead],
    integrate_policy: ReceiveIntegratePolicy,
    incoming_as_branches: bool,
    check_mergeability: bool,
    verbose: bool,
) -> Result<ApplyBundleToRepoResult> {
    let bundle_bytes = fs::read(bundle_path)?;
    let bundle_id = bundle_receive_id(&bundle_bytes)?;
    let parsed_bundle = parse_bundle_payload(&bundle_bytes)?;
    let pack_data = parsed_bundle.pack_data;

    let odb = repo.odb()?;
    add_repo_disk_alternates_to_odb(repo, &odb)?;
    let pack_dir = repo.path().join("objects").join("pack");
    fs::create_dir_all(&pack_dir)?;
    let indexer_result = import_bundle_pack_with_indexer(
        repo,
        &odb,
        &pack_dir,
        bundle_path,
        &parsed_bundle.inspection,
        pack_data,
        true,
        verbose,
    );
    let mut import_path = ImportPath::StrictIndexer;
    if let Err(indexer_error) = indexer_result {
        if should_try_fetch_import_fallback(&indexer_error) {
            import_bundle_pack_with_indexer(
                repo,
                &odb,
                &pack_dir,
                bundle_path,
                &parsed_bundle.inspection,
                pack_data,
                false,
                verbose,
            )
            .map(|_| {
                import_path = ImportPath::CompatIndexerVerifyFalse;
            })
            .or_else(|verify_disabled_error| {
                import_bundle_pack_with_libgit2_fetch(repo, bundle_path, heads, &bundle_id)
                    .map(|_| {
                        import_path = ImportPath::CompatFetchFallback;
                    })
                    .map_err(|fetch_error| {
                        anyhow!(
                            "{indexer_error}\n\
                             fallback import with indexer(verify=false) also failed: {verify_disabled_error}\n\
                             fallback import via libgit2 fetch also failed: {fetch_error}"
                        )
                    })
            })?;
        } else {
            return Err(indexer_error);
        }
    }
    if import_path.is_compatibility_fallback() {
        validate_import_connectivity_for_heads(
            repo,
            heads,
            &parsed_bundle.inspection.prerequisites,
            import_path,
        )?;
    }

    for head in heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    let incoming_refs =
        write_incoming_namespace_refs(repo, heads, &bundle_id, incoming_as_branches)?;
    let preflight_plan = compute_receive_plan(repo, &incoming_refs)?;
    let mergeability_checks =
        if check_mergeability || integrate_policy == ReceiveIntegratePolicy::Merge {
            compute_receive_mergeability_checks(repo, &preflight_plan)?
        } else {
            Vec::new()
        };
    let apply_backend = if check_mergeability {
        None
    } else {
        validate_receive_plan(&preflight_plan, integrate_policy, &mergeability_checks)?;
        apply_receive_plan(
            repo,
            &preflight_plan,
            integrate_policy,
            &bundle_id,
            &mergeability_checks,
        )?
    };

    Ok(ApplyBundleToRepoResult {
        preflight_plan,
        mergeability_checks,
        apply_backend,
    })
}

/// Validates imported object connectivity for bundle heads after compatibility fallback paths.
///
/// This is a fail-closed guard to keep receive safety properties aligned with the strict import
/// path even when we had to import with compatibility fallbacks.
fn validate_import_connectivity_for_heads(
    repo: &git2::Repository,
    heads: &[BundleHead],
    prerequisites: &[git2::Oid],
    import_path: ImportPath,
) -> Result<()> {
    let prerequisite_set = prerequisites.iter().copied().collect::<BTreeSet<_>>();
    let mut visited_commits = BTreeSet::<git2::Oid>::new();
    let mut visited_trees = BTreeSet::<git2::Oid>::new();

    for head in heads {
        let mut revwalk = repo.revwalk().map_err(|err| {
            anyhow!(
                "post-import connectivity check ({}) failed to create revwalk for '{}': {err}",
                import_path.label(),
                head.reference
            )
        })?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        revwalk.push(head.oid).map_err(|err| {
            anyhow!(
                "post-import connectivity check ({}) failed to push head '{}' ({}): {err}",
                import_path.label(),
                head.reference,
                head.oid
            )
        })?;
        for prerequisite in prerequisites {
            revwalk.hide(*prerequisite).map_err(|err| {
                anyhow!(
                    "post-import connectivity check ({}) failed to hide prerequisite '{}' for '{}': {err}",
                    import_path.label(),
                    prerequisite,
                    head.reference
                )
            })?;
        }

        for oid_result in revwalk {
            let commit_oid = oid_result.map_err(|err| {
                anyhow!(
                    "post-import connectivity check ({}) failed while walking '{}' history: {err}",
                    import_path.label(),
                    head.reference
                )
            })?;
            if !visited_commits.insert(commit_oid) {
                continue;
            }
            let commit = repo.find_commit(commit_oid).map_err(|err| {
                anyhow!(
                    "post-import connectivity check ({}) missing commit '{}' in '{}' history: {err}",
                    import_path.label(),
                    commit_oid,
                    head.reference
                )
            })?;
            for parent_index in 0..commit.parent_count() {
                let parent_oid = commit.parent_id(parent_index)?;
                if prerequisite_set.contains(&parent_oid) {
                    continue;
                }
                repo.find_commit(parent_oid).map_err(|err| {
                    anyhow!(
                        "post-import connectivity check ({}) missing parent commit '{}' referenced by '{}' in '{}': {err}",
                        import_path.label(),
                        parent_oid,
                        commit_oid,
                        head.reference
                    )
                })?;
            }
            validate_tree_connectivity(
                repo,
                commit.tree_id(),
                &mut visited_trees,
                &head.reference,
                import_path,
            )?;
        }
    }

    Ok(())
}

/// Recursively validates that all tree/blob/tag objects referenced from one tree are present.
fn validate_tree_connectivity(
    repo: &git2::Repository,
    tree_oid: git2::Oid,
    visited_trees: &mut BTreeSet<git2::Oid>,
    head_ref: &str,
    import_path: ImportPath,
) -> Result<()> {
    if !visited_trees.insert(tree_oid) {
        return Ok(());
    }
    let tree = repo.find_tree(tree_oid).map_err(|err| {
        anyhow!(
            "post-import connectivity check ({}) missing tree '{}' in '{}': {err}",
            import_path.label(),
            tree_oid,
            head_ref
        )
    })?;
    for entry in &tree {
        let entry_oid = entry.id();
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                validate_tree_connectivity(repo, entry_oid, visited_trees, head_ref, import_path)?;
            }
            Some(_) => {
                repo.find_object(entry_oid, None).map_err(|err| {
                    anyhow!(
                        "post-import connectivity check ({}) missing object '{}' in '{}': {err}",
                        import_path.label(),
                        entry_oid,
                        head_ref
                    )
                })?;
            }
            None => {
                let path = entry.name().unwrap_or("<unknown>");
                bail!(
                    "post-import connectivity check ({}) encountered unknown object type for tree entry '{}' ({}) in '{}'",
                    import_path.label(),
                    path,
                    entry_oid,
                    head_ref
                );
            }
        }
    }
    Ok(())
}

/// Imports one bundle pack stream into the repository object database via libgit2 indexer.
fn import_bundle_pack_with_indexer(
    repo: &git2::Repository,
    odb: &git2::Odb<'_>,
    pack_dir: &Path,
    bundle_path: &Path,
    inspection: &BundleInspection,
    pack_data: &[u8],
    verify: bool,
    verbose: bool,
) -> Result<()> {
    let init_stage = if verify {
        "indexer initialization (verify=true)"
    } else {
        "indexer initialization (verify=false)"
    };
    let write_stage = if verify {
        "pack write (verify=true)"
    } else {
        "pack write (verify=false)"
    };
    let commit_stage = if verify {
        "indexer commit (verify=true)"
    } else {
        "indexer commit (verify=false)"
    };

    let mut indexer = git2::Indexer::new(Some(odb), pack_dir, 0o644, verify).map_err(|err| {
        with_verbose_indexer_diagnostics(
            err,
            verbose,
            repo,
            bundle_path,
            inspection,
            pack_data.len(),
            init_stage,
        )
    })?;
    indexer.write_all(pack_data).map_err(|err| {
        with_verbose_indexer_diagnostics(
            err,
            verbose,
            repo,
            bundle_path,
            inspection,
            pack_data.len(),
            write_stage,
        )
    })?;
    indexer.commit().map_err(|err| {
        with_verbose_indexer_diagnostics(
            err,
            verbose,
            repo,
            bundle_path,
            inspection,
            pack_data.len(),
            commit_stage,
        )
    })?;
    Ok(())
}

/// Returns whether fetch fallback should be attempted for this indexer import failure.
fn should_try_fetch_import_fallback(error: &anyhow::Error) -> bool {
    let text = error.to_string().to_ascii_lowercase();
    text.contains("packfile is missing")
        || text.contains("class=indexer")
        || text.contains("class=indexer(")
}

/// Imports bundle objects via libgit2 fetch into temporary staging refs as fallback.
fn import_bundle_pack_with_libgit2_fetch(
    repo: &git2::Repository,
    bundle_path: &Path,
    heads: &[BundleHead],
    bundle_id: &str,
) -> Result<()> {
    let mut refspecs = BTreeSet::<String>::new();
    let mut staging_refs = BTreeSet::<String>::new();
    for head in heads {
        let source_ref = head.reference.as_str();
        let staging_ref = fetch_staging_ref_name(bundle_id, source_ref);
        refspecs.insert(format!("+{source_ref}:{staging_ref}"));
        staging_refs.insert(staging_ref);
    }
    if refspecs.is_empty() {
        bail!("bundle fetch fallback requires at least one advertised head");
    }

    let refspec_list = refspecs.iter().map(String::as_str).collect::<Vec<_>>();
    let mut candidate_errors = Vec::<String>::new();
    let mut imported = false;
    for candidate in bundle_fetch_remote_candidates(bundle_path)? {
        match repo.remote_anonymous(candidate.as_str()) {
            Ok(mut remote) => match remote.fetch(&refspec_list, None, None) {
                Ok(()) => {
                    imported = true;
                    break;
                }
                Err(err) => {
                    candidate_errors.push(format!("{candidate} => fetch failed: {err}"));
                }
            },
            Err(err) => {
                candidate_errors.push(format!("{candidate} => remote init failed: {err}"));
            }
        }
    }
    if !imported {
        bail!(
            "libgit2 bundle fetch fallback failed for all URL candidates: {}",
            candidate_errors.join(" | ")
        );
    }

    cleanup_temporary_fetch_refs(repo, &staging_refs)?;
    Ok(())
}

/// Removes temporary fetch-staging refs used only for object import fallback.
fn cleanup_temporary_fetch_refs(repo: &git2::Repository, refs: &BTreeSet<String>) -> Result<()> {
    for ref_name in refs {
        match repo.find_reference(ref_name) {
            Ok(mut reference) => {
                reference.delete()?;
            }
            Err(err) if err.code() == git2::ErrorCode::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// Converts bundle path input into a libgit2-compatible remote URL.
///
/// Local filesystem paths are normalized to `file://` URLs because some
/// libgit2 builds reject plain path strings as unsupported protocols.
fn bundle_fetch_remote_candidates(bundle_path: &Path) -> Result<Vec<String>> {
    let bundle_text = bundle_path.to_string_lossy().to_string();
    if bundle_text.contains("://") {
        return Ok(vec![bundle_text]);
    }

    let absolute_path = if bundle_path.is_absolute() {
        bundle_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(bundle_path)
    };
    let canonical = absolute_path
        .canonicalize()
        .unwrap_or_else(|_| absolute_path.clone());
    Ok(vec![
        canonical.to_string_lossy().to_string(),
        format!("file://{}", canonical.to_string_lossy()),
    ])
}

#[derive(Debug, Clone)]
struct IncomingHeadRef {
    target_ref: String,
    incoming_ref: String,
    incoming_oid: git2::Oid,
}

/// Derives a stable short identifier for incoming namespace refs.
///
/// Uses the SHA-256 of bundle bytes and truncates to 12 hex chars.
fn bundle_receive_id(bundle_bytes: &[u8]) -> Result<String> {
    sha256_hex(bundle_bytes).map(|digest| digest.chars().take(12).collect())
}

/// Builds incoming-ref descriptors for all bundle heads without mutating refs.
fn incoming_head_refs(heads: &[BundleHead], bundle_id: &str) -> Vec<IncomingHeadRef> {
    heads
        .iter()
        .map(|head| IncomingHeadRef {
            target_ref: head.reference.clone(),
            incoming_ref: incoming_ref_name(bundle_id, &head.reference),
            incoming_oid: head.oid,
        })
        .collect()
}

/// Writes all incoming bundle heads to `refs/sync/incoming/<bundle-id>/...`.
fn write_incoming_namespace_refs(
    repo: &git2::Repository,
    heads: &[BundleHead],
    bundle_id: &str,
    incoming_as_branches: bool,
) -> Result<Vec<IncomingHeadRef>> {
    let incoming_refs = incoming_head_refs(heads, bundle_id);
    for incoming in &incoming_refs {
        repo.reference(
            &incoming.incoming_ref,
            incoming.incoming_oid,
            true,
            "receive bundle import (incoming namespace)",
        )?;
        if incoming_as_branches {
            let incoming_branch_ref = incoming_branch_ref_name(bundle_id, &incoming.target_ref);
            repo.reference(
                &incoming_branch_ref,
                incoming.incoming_oid,
                true,
                "receive bundle import (incoming branch mirror)",
            )?;
        }
    }
    Ok(incoming_refs)
}

/// Builds one incoming namespace ref name for a bundle head reference.
fn incoming_ref_name(bundle_id: &str, head_reference: &str) -> String {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    format!("refs/sync/incoming/{bundle_id}/{suffix}")
}

/// Builds one incoming branch-mirror ref name for a bundle head reference.
fn incoming_branch_ref_name(bundle_id: &str, head_reference: &str) -> String {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    format!("refs/heads/incoming/{bundle_id}/{suffix}")
}

/// Builds one merge-test ref name for a bundle head reference.
fn merge_test_ref_name(bundle_id: &str, head_reference: &str) -> String {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    format!("refs/sync/merge-test/{bundle_id}/{suffix}")
}

/// Builds one temporary fetch-staging ref name for a bundle head reference.
fn fetch_staging_ref_name(bundle_id: &str, head_reference: &str) -> String {
    let suffix = head_reference
        .strip_prefix("refs/")
        .unwrap_or(head_reference);
    format!("refs/sync/fetch-staging/{bundle_id}/{suffix}")
}

/// Computes a deterministic per-head preflight plan for receive integration.
fn compute_receive_plan(
    repo: &git2::Repository,
    incoming_refs: &[IncomingHeadRef],
) -> Result<Vec<ReceivePlanEntry>> {
    let mut plan = Vec::with_capacity(incoming_refs.len());
    for incoming in incoming_refs {
        let target_oid = resolve_reference_target(repo, &incoming.target_ref)?;
        let status = match target_oid {
            None => ReceivePlanStatus::TargetMissing,
            Some(current) if current == incoming.incoming_oid => ReceivePlanStatus::AlreadyPresent,
            Some(current) => {
                if repo.graph_descendant_of(incoming.incoming_oid, current)? {
                    ReceivePlanStatus::FastForwardOk
                } else if repo.graph_descendant_of(current, incoming.incoming_oid)? {
                    ReceivePlanStatus::TargetAhead
                } else {
                    ReceivePlanStatus::DivergedMergeRequired
                }
            }
        };
        let merge_base_oid = match target_oid {
            Some(current) => repo.merge_base(current, incoming.incoming_oid).ok(),
            None => None,
        };

        plan.push(ReceivePlanEntry {
            target_ref: incoming.target_ref.clone(),
            target_oid,
            incoming_oid: incoming.incoming_oid,
            merge_base_oid,
            preserved_incoming_ref: incoming.incoming_ref.clone(),
            status,
        });
    }
    Ok(plan)
}

/// Computes mergeability simulation results for diverged preflight rows.
fn compute_receive_mergeability_checks(
    repo: &git2::Repository,
    preflight_plan: &[ReceivePlanEntry],
) -> Result<Vec<ReceiveMergeabilityCheck>> {
    let mut checks = Vec::new();
    for row in preflight_plan {
        if row.status != ReceivePlanStatus::DivergedMergeRequired {
            continue;
        }

        let Some(target_oid) = row.target_oid else {
            checks.push(ReceiveMergeabilityCheck {
                target_ref: row.target_ref.clone(),
                target_oid: None,
                target_summary: None,
                incoming_oid: row.incoming_oid,
                incoming_summary: commit_summary(repo, row.incoming_oid),
                merge_base_oid: row.merge_base_oid,
                merge_base_summary: row.merge_base_oid.and_then(|oid| commit_summary(repo, oid)),
                status: ReceiveMergeabilityStatus::Unknown,
                detail: Some("target oid unavailable for merge simulation".to_string()),
                conflict_paths: Vec::new(),
            });
            continue;
        };

        let (status, detail, conflict_paths) =
            match evaluate_mergeability(repo, target_oid, row.incoming_oid) {
                Ok((status, detail, conflict_paths)) => (status, detail, conflict_paths),
                Err(err) => (
                    ReceiveMergeabilityStatus::Unknown,
                    Some(format!("merge simulation failed: {err}")),
                    Vec::new(),
                ),
            };
        checks.push(ReceiveMergeabilityCheck {
            target_ref: row.target_ref.clone(),
            target_oid: Some(target_oid),
            target_summary: commit_summary(repo, target_oid),
            incoming_oid: row.incoming_oid,
            incoming_summary: commit_summary(repo, row.incoming_oid),
            merge_base_oid: row.merge_base_oid,
            merge_base_summary: row.merge_base_oid.and_then(|oid| commit_summary(repo, oid)),
            status,
            detail,
            conflict_paths,
        });
    }

    Ok(checks)
}

/// Returns the commit subject line for an OID if available.
fn commit_summary(repo: &git2::Repository, oid: git2::Oid) -> Option<String> {
    repo.find_commit(oid)
        .ok()
        .and_then(|commit| commit.summary().map(ToOwned::to_owned))
}

/// Evaluates mergeability for two commits without creating a merge commit.
fn evaluate_mergeability(
    repo: &git2::Repository,
    target_oid: git2::Oid,
    incoming_oid: git2::Oid,
) -> Result<(ReceiveMergeabilityStatus, Option<String>, Vec<String>)> {
    let target_commit = repo.find_commit(target_oid)?;
    let incoming_commit = repo.find_commit(incoming_oid)?;
    let index = repo.merge_commits(&target_commit, &incoming_commit, None)?;
    if index.has_conflicts() {
        let conflict_paths = collect_conflict_paths(&index)?;
        Ok((
            ReceiveMergeabilityStatus::Conflicted,
            Some("merge simulation produced index conflicts".to_string()),
            conflict_paths,
        ))
    } else {
        Ok((
            ReceiveMergeabilityStatus::Clean,
            Some("merge simulation completed without conflicts".to_string()),
            Vec::new(),
        ))
    }
}

/// Collects unique conflict paths from an in-memory merge index.
fn collect_conflict_paths(index: &git2::Index) -> Result<Vec<String>> {
    let mut paths = std::collections::BTreeSet::<String>::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        for entry in [conflict.ancestor, conflict.our, conflict.their]
            .into_iter()
            .flatten()
        {
            paths.insert(String::from_utf8_lossy(&entry.path).to_string());
        }
    }
    Ok(paths.into_iter().collect())
}

/// Creates a merge commit object for merge-policy receive integration.
fn create_receive_merge_commit(
    repo: &git2::Repository,
    target_ref: &str,
    target_oid: git2::Oid,
    incoming_oid: git2::Oid,
) -> Result<git2::Oid> {
    let target_commit = repo.find_commit(target_oid)?;
    let incoming_commit = repo.find_commit(incoming_oid)?;
    let mut index = repo.merge_commits(&target_commit, &incoming_commit, None)?;
    if index.has_conflicts() {
        let conflict_paths = collect_conflict_paths(&index)?;
        let rendered_paths = if conflict_paths.is_empty() {
            "<none>".to_string()
        } else {
            conflict_paths.join(", ")
        };
        bail!(
            "unable to create merge commit for '{}' because merge conflicts were detected: {}",
            target_ref,
            rendered_paths
        );
    }

    let tree_oid = index.write_tree_to(repo)?;
    let tree = repo.find_tree(tree_oid)?;
    let signature = receive_merge_signature(repo)?;
    let merge_message = format!(
        "git-sync receive merge for {target_ref}\n\nTarget: {target_oid}\nIncoming: {incoming_oid}\n"
    );
    let merge_oid = repo.commit(
        None,
        &signature,
        &signature,
        &merge_message,
        &tree,
        &[&target_commit, &incoming_commit],
    )?;
    Ok(merge_oid)
}

/// Chooses a signature for receive-created merge commits.
fn receive_merge_signature(repo: &git2::Repository) -> Result<git2::Signature<'static>> {
    let (name, email) = match repo.signature() {
        Ok(signature) => (
            signature.name().unwrap_or("git-sync").to_string(),
            signature.email().unwrap_or("git-sync@local").to_string(),
        ),
        Err(_) => ("git-sync".to_string(), "git-sync@local".to_string()),
    };
    Ok(git2::Signature::now(&name, &email)?)
}

/// Validates an integration plan under the selected policy.
fn validate_receive_plan(
    preflight_plan: &[ReceivePlanEntry],
    integrate_policy: ReceiveIntegratePolicy,
    mergeability_checks: &[ReceiveMergeabilityCheck],
) -> Result<()> {
    match integrate_policy {
        ReceiveIntegratePolicy::CreateRefsOnly => Ok(()),
        ReceiveIntegratePolicy::FastForwardOnly => {
            let diverged = preflight_plan
                .iter()
                .filter(|entry| entry.status == ReceivePlanStatus::DivergedMergeRequired)
                .collect::<Vec<_>>();
            if !diverged.is_empty() {
                bail!(format_non_fast_forward_diagnostics(&diverged));
            }
            Ok(())
        }
        ReceiveIntegratePolicy::Merge => {
            let mut blocked_rows = Vec::<&ReceivePlanEntry>::new();
            for row in preflight_plan {
                if row.status != ReceivePlanStatus::DivergedMergeRequired {
                    continue;
                }
                let mergeability = mergeability_checks
                    .iter()
                    .find(|check| check.target_ref == row.target_ref);
                if !matches!(
                    mergeability.map(|check| check.status),
                    Some(ReceiveMergeabilityStatus::Clean)
                ) {
                    blocked_rows.push(row);
                }
            }
            if !blocked_rows.is_empty() {
                bail!(format_merge_policy_diagnostics(
                    &blocked_rows,
                    mergeability_checks
                ));
            }
            Ok(())
        }
    }
}

/// Returns whether the current receive plan can be fully applied for a policy.
fn can_apply_receive_plan(
    preflight_plan: &[ReceivePlanEntry],
    integrate_policy: ReceiveIntegratePolicy,
    mergeability_checks: &[ReceiveMergeabilityCheck],
) -> bool {
    match integrate_policy {
        ReceiveIntegratePolicy::CreateRefsOnly => true,
        ReceiveIntegratePolicy::FastForwardOnly => !preflight_plan
            .iter()
            .any(|row| row.status == ReceivePlanStatus::DivergedMergeRequired),
        ReceiveIntegratePolicy::Merge => {
            for row in preflight_plan {
                if row.status != ReceivePlanStatus::DivergedMergeRequired {
                    continue;
                }
                let mergeability = mergeability_checks
                    .iter()
                    .find(|check| check.target_ref == row.target_ref);
                if !matches!(
                    mergeability.map(|check| check.status),
                    Some(ReceiveMergeabilityStatus::Clean)
                ) {
                    return false;
                }
            }
            true
        }
    }
}

/// Applies validated ref updates according to the selected integration policy.
fn apply_receive_plan(
    repo: &git2::Repository,
    preflight_plan: &[ReceivePlanEntry],
    integrate_policy: ReceiveIntegratePolicy,
    bundle_id: &str,
    mergeability_checks: &[ReceiveMergeabilityCheck],
) -> Result<Option<ReceiveApplyBackend>> {
    if matches!(integrate_policy, ReceiveIntegratePolicy::CreateRefsOnly) {
        return Ok(None);
    }

    let updates = planned_ref_updates_from_plan(
        repo,
        preflight_plan,
        integrate_policy,
        bundle_id,
        mergeability_checks,
    )?;
    if updates.is_empty() {
        return Ok(None);
    }

    #[cfg(test)]
    if test_hooks::force_manual_cas_apply_enabled() {
        apply_ref_updates_with_manual_cas(repo, &updates)?;
        return Ok(Some(ReceiveApplyBackend::ManualCasRollback));
    }

    match repo.transaction() {
        Ok(tx) => {
            apply_ref_updates_with_transaction(repo, tx, &updates)?;
            Ok(Some(ReceiveApplyBackend::RefTransaction))
        }
        Err(_transaction_error) => {
            apply_ref_updates_with_manual_cas(repo, &updates)?;
            Ok(Some(ReceiveApplyBackend::ManualCasRollback))
        }
    }
}

#[derive(Debug, Clone)]
struct PlannedRefUpdate {
    ref_name: String,
    expected_old_oid: Option<git2::Oid>,
    new_oid: git2::Oid,
}

#[derive(Debug, Clone, Default)]
struct RollbackOutcome {
    restored_refs: Vec<String>,
    deleted_refs: Vec<String>,
    failed_refs: Vec<(String, String)>,
}

/// Builds deterministic target-ref update rows from the validated preflight plan.
fn planned_ref_updates_from_plan(
    repo: &git2::Repository,
    preflight_plan: &[ReceivePlanEntry],
    integrate_policy: ReceiveIntegratePolicy,
    bundle_id: &str,
    mergeability_checks: &[ReceiveMergeabilityCheck],
) -> Result<Vec<PlannedRefUpdate>> {
    let mut updates = Vec::new();
    for row in preflight_plan {
        match row.status {
            ReceivePlanStatus::TargetMissing | ReceivePlanStatus::FastForwardOk => {
                updates.push(PlannedRefUpdate {
                    ref_name: row.target_ref.clone(),
                    expected_old_oid: row.target_oid,
                    new_oid: row.incoming_oid,
                });
            }
            ReceivePlanStatus::AlreadyPresent | ReceivePlanStatus::TargetAhead => {}
            ReceivePlanStatus::DivergedMergeRequired => match integrate_policy {
                ReceiveIntegratePolicy::CreateRefsOnly => {}
                ReceiveIntegratePolicy::FastForwardOnly => {
                    bail!(
                        "internal receive plan error: diverged row '{}' reached apply stage",
                        row.target_ref
                    );
                }
                ReceiveIntegratePolicy::Merge => {
                    let mergeability = mergeability_checks
                        .iter()
                        .find(|check| check.target_ref == row.target_ref);
                    if !matches!(
                        mergeability.map(|check| check.status),
                        Some(ReceiveMergeabilityStatus::Clean)
                    ) {
                        bail!(
                            "internal receive plan error: merge policy reached apply stage for blocked row '{}'",
                            row.target_ref
                        );
                    }
                    let Some(target_oid) = row.target_oid else {
                        bail!(
                            "internal receive plan error: merge integration requires an existing target oid for diverged row '{}'",
                            row.target_ref
                        );
                    };
                    let merge_oid = create_receive_merge_commit(
                        repo,
                        &row.target_ref,
                        target_oid,
                        row.incoming_oid,
                    )?;
                    let merge_test_ref = merge_test_ref_name(bundle_id, &row.target_ref);
                    updates.push(PlannedRefUpdate {
                        ref_name: merge_test_ref.clone(),
                        expected_old_oid: resolve_reference_target(repo, &merge_test_ref)?,
                        new_oid: merge_oid,
                    });
                    updates.push(PlannedRefUpdate {
                        ref_name: row.target_ref.clone(),
                        expected_old_oid: row.target_oid,
                        new_oid: merge_oid,
                    });
                }
            },
        }
    }
    Ok(updates)
}

/// Applies updates using a libgit2 reference transaction with pre-commit CAS checks.
fn apply_ref_updates_with_transaction(
    repo: &git2::Repository,
    mut transaction: git2::Transaction<'_>,
    updates: &[PlannedRefUpdate],
) -> Result<()> {
    for (lock_index, update) in updates.iter().enumerate() {
        #[cfg(not(test))]
        let _ = lock_index;

        #[cfg(test)]
        if test_hooks::transaction_fail_at_lock_ref_index() == Some(lock_index) {
            bail!(format_apply_failure(
                "ref_transaction",
                "lock ref failed",
                Some(&update.ref_name),
                "injected transaction lock-ref failure",
                &[],
                &RollbackOutcome::default(),
            ));
        }

        transaction.lock_ref(&update.ref_name).map_err(|err| {
            anyhow!(
                "unable to lock target ref '{}' for transactional update: {err}",
                update.ref_name
            )
        })?;
    }

    for (set_index, update) in updates.iter().enumerate() {
        #[cfg(not(test))]
        let _ = set_index;

        #[cfg(test)]
        if test_hooks::transaction_fail_at_set_target_index() == Some(set_index) {
            bail!(format_apply_failure(
                "ref_transaction",
                "stage target update failed",
                Some(&update.ref_name),
                "injected transaction set-target failure",
                &[],
                &RollbackOutcome::default(),
            ));
        }

        ensure_expected_ref_target(repo, update)?;
        transaction
            .set_target(
                &update.ref_name,
                update.new_oid,
                None,
                "receive bundle import (integration update)",
            )
            .map_err(|err| {
                anyhow!(
                    "unable to stage transactional target update for '{}': {err}",
                    update.ref_name
                )
            })?;
    }

    #[cfg(test)]
    if test_hooks::transaction_inject_commit_failure() {
        let applied_updates = detect_applied_updates(repo, updates);
        let rollback = rollback_applied_updates(repo, &applied_updates);
        bail!(format_apply_failure(
            "ref_transaction",
            "transaction commit failed",
            None,
            "injected transaction commit failure",
            &applied_updates,
            &rollback,
        ));
    }

    if let Err(err) = transaction.commit() {
        let applied_updates = detect_applied_updates(repo, updates);
        let rollback = rollback_applied_updates(repo, &applied_updates);
        bail!(format_apply_failure(
            "ref_transaction",
            "transaction commit failed",
            None,
            &err.to_string(),
            &applied_updates,
            &rollback,
        ));
    }

    Ok(())
}

/// Applies updates one by one via CAS-aware direct ref updates and rolls back on failure.
fn apply_ref_updates_with_manual_cas(
    repo: &git2::Repository,
    updates: &[PlannedRefUpdate],
) -> Result<()> {
    let mut applied_updates = Vec::<PlannedRefUpdate>::new();
    for (update_index, update) in updates.iter().enumerate() {
        #[cfg(not(test))]
        let _ = update_index;

        #[cfg(test)]
        test_hooks::maybe_inject_manual_cas_mutation_before_check(repo, update_index)?;

        if let Err(err) = ensure_expected_ref_target(repo, update) {
            let rollback = rollback_applied_updates(repo, &applied_updates);
            bail!(format_apply_failure(
                "manual_cas_rollback",
                "CAS precondition failed",
                Some(&update.ref_name),
                &err.to_string(),
                &applied_updates,
                &rollback,
            ));
        }

        #[cfg(test)]
        if test_hooks::manual_cas_fail_at_update_index() == Some(update_index) {
            let rollback = rollback_applied_updates(repo, &applied_updates);
            bail!(format_apply_failure(
                "manual_cas_rollback",
                "target update failed",
                Some(&update.ref_name),
                "injected manual-cas update failure",
                &applied_updates,
                &rollback,
            ));
        }

        let update_result = match update.expected_old_oid {
            Some(expected_old) => repo
                .reference_matching(
                    &update.ref_name,
                    update.new_oid,
                    true,
                    expected_old,
                    "receive bundle import (integration update)",
                )
                .map(|_| ()),
            None => repo
                .reference(
                    &update.ref_name,
                    update.new_oid,
                    false,
                    "receive bundle import (integration update)",
                )
                .map(|_| ()),
        };

        if let Err(err) = update_result {
            let rollback = rollback_applied_updates(repo, &applied_updates);
            bail!(format_apply_failure(
                "manual_cas_rollback",
                "target update failed",
                Some(&update.ref_name),
                &err.to_string(),
                &applied_updates,
                &rollback,
            ));
        }

        applied_updates.push(update.clone());
    }

    Ok(())
}

/// Verifies that the target ref still matches the expected old OID before update.
fn ensure_expected_ref_target(repo: &git2::Repository, update: &PlannedRefUpdate) -> Result<()> {
    let current = resolve_reference_target(repo, &update.ref_name)?;
    if current == update.expected_old_oid {
        return Ok(());
    }

    bail!(
        "expected old target {} but found {}",
        format_optional_oid(update.expected_old_oid),
        format_optional_oid(current),
    )
}

/// Detects updates that are currently materialized at their expected new OID.
fn detect_applied_updates(
    repo: &git2::Repository,
    updates: &[PlannedRefUpdate],
) -> Vec<PlannedRefUpdate> {
    updates
        .iter()
        .filter_map(
            |update| match resolve_reference_target(repo, &update.ref_name) {
                Ok(Some(current)) if current == update.new_oid => Some(update.clone()),
                _ => None,
            },
        )
        .collect()
}

/// Rolls back applied target updates to their old OIDs (or deletes newly created refs).
fn rollback_applied_updates(
    repo: &git2::Repository,
    applied_updates: &[PlannedRefUpdate],
) -> RollbackOutcome {
    let mut outcome = RollbackOutcome::default();

    for update in applied_updates.iter().rev() {
        let current = match resolve_reference_target(repo, &update.ref_name) {
            Ok(current) => current,
            Err(err) => {
                outcome.failed_refs.push((
                    update.ref_name.clone(),
                    format!("unable to resolve current target before rollback: {err}"),
                ));
                continue;
            }
        };

        if current != Some(update.new_oid) {
            continue;
        }

        #[cfg(test)]
        if test_hooks::should_inject_rollback_failure_for_ref(&update.ref_name) {
            outcome.failed_refs.push((
                update.ref_name.clone(),
                "injected rollback failure".to_string(),
            ));
            continue;
        }

        match update.expected_old_oid {
            Some(old_oid) => {
                if let Err(err) = repo.reference(
                    &update.ref_name,
                    old_oid,
                    true,
                    "receive bundle rollback after failed update",
                ) {
                    outcome
                        .failed_refs
                        .push((update.ref_name.clone(), err.to_string()));
                } else {
                    outcome.restored_refs.push(update.ref_name.clone());
                }
            }
            None => match repo.find_reference(&update.ref_name) {
                Ok(mut reference) => {
                    if let Err(err) = reference.delete() {
                        outcome
                            .failed_refs
                            .push((update.ref_name.clone(), err.to_string()));
                    } else {
                        outcome.deleted_refs.push(update.ref_name.clone());
                    }
                }
                Err(err) if err.code() == git2::ErrorCode::NotFound => {}
                Err(err) => {
                    outcome
                        .failed_refs
                        .push((update.ref_name.clone(), err.to_string()));
                }
            },
        }
    }

    outcome
}

/// Formats a detailed failure report for receive target-update failures.
fn format_apply_failure(
    backend: &str,
    stage: &str,
    failed_ref: Option<&str>,
    reason: &str,
    applied_updates: &[PlannedRefUpdate],
    rollback: &RollbackOutcome,
) -> String {
    let mut message = format!(
        "unable to apply receive target updates ({backend}) at stage '{stage}': {reason}\n"
    );
    if let Some(ref_name) = failed_ref {
        let _ = std::fmt::Write::write_fmt(&mut message, format_args!("failed ref: {ref_name}\n"));
    }

    if applied_updates.is_empty() {
        message.push_str("updated refs before failure: (none)\n");
    } else {
        message.push_str("updated refs before failure:\n");
        for update in applied_updates {
            let _ = std::fmt::Write::write_fmt(
                &mut message,
                format_args!(
                    "- {}: {} -> {}\n",
                    update.ref_name,
                    format_optional_oid(update.expected_old_oid),
                    update.new_oid
                ),
            );
        }
    }

    if rollback.restored_refs.is_empty()
        && rollback.deleted_refs.is_empty()
        && rollback.failed_refs.is_empty()
    {
        message.push_str("rollback: no ref changes required\n");
        return message;
    }

    if !rollback.restored_refs.is_empty() {
        message.push_str("rollback restored refs:\n");
        for ref_name in &rollback.restored_refs {
            let _ = std::fmt::Write::write_fmt(&mut message, format_args!("- {ref_name}\n"));
        }
    }
    if !rollback.deleted_refs.is_empty() {
        message.push_str("rollback deleted refs:\n");
        for ref_name in &rollback.deleted_refs {
            let _ = std::fmt::Write::write_fmt(&mut message, format_args!("- {ref_name}\n"));
        }
    }
    if !rollback.failed_refs.is_empty() {
        message.push_str("rollback failures:\n");
        for (ref_name, err) in &rollback.failed_refs {
            let _ =
                std::fmt::Write::write_fmt(&mut message, format_args!("- {}: {}\n", ref_name, err));
        }
    }

    message
}

/// Formats optional OIDs in diagnostics.
fn format_optional_oid(oid: Option<git2::Oid>) -> String {
    oid.map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_string())
}

/// Resolves a reference name to its peeled direct target OID.
fn resolve_reference_target(repo: &git2::Repository, ref_name: &str) -> Result<Option<git2::Oid>> {
    let reference = match repo.find_reference(ref_name) {
        Ok(reference) => reference,
        Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let target = reference.target().or_else(|| {
        reference
            .resolve()
            .ok()
            .and_then(|resolved| resolved.target())
    });
    Ok(target)
}

/// Formats per-ref diagnostics for non-fast-forward integration failures.
fn format_non_fast_forward_diagnostics(diagnostics: &[&ReceivePlanEntry]) -> String {
    let mut message =
        String::from("unable to integrate bundle heads with --integrate fast-forward-only:\n");
    for diagnostic in diagnostics {
        let merge_base = diagnostic
            .merge_base_oid
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let _ = std::fmt::Write::write_fmt(
            &mut message,
            format_args!(
                "- target ref: {}\n  target oid: {}\n  incoming oid: {}\n  merge-base oid: {}\n  reason: diverged (non-fast-forward)\n  next-step: merge required; incoming ref preserved at {}\n",
                diagnostic.target_ref,
                format_optional_oid(diagnostic.target_oid),
                diagnostic.incoming_oid,
                merge_base,
                diagnostic.preserved_incoming_ref
            ),
        );
    }
    message
}

/// Formats per-ref diagnostics for merge-policy failures.
fn format_merge_policy_diagnostics(
    diagnostics: &[&ReceivePlanEntry],
    mergeability_checks: &[ReceiveMergeabilityCheck],
) -> String {
    let mut message = String::from("unable to integrate bundle heads with --integrate merge:\n");
    for diagnostic in diagnostics {
        let merge_base = diagnostic
            .merge_base_oid
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "<none>".to_string());
        let mergeability = mergeability_checks
            .iter()
            .find(|check| check.target_ref == diagnostic.target_ref);
        let reason = match mergeability.map(|check| check.status) {
            Some(ReceiveMergeabilityStatus::Conflicted) => "merge would conflict",
            Some(ReceiveMergeabilityStatus::Unknown) => "mergeability check failed",
            Some(ReceiveMergeabilityStatus::Clean) => "mergeability precheck did not pass",
            None => "mergeability check missing",
        };
        let _ = std::fmt::Write::write_fmt(
            &mut message,
            format_args!(
                "- target ref: {}\n  target oid: {}\n  incoming oid: {}\n  merge-base oid: {}\n  reason: {}\n",
                diagnostic.target_ref,
                format_optional_oid(diagnostic.target_oid),
                diagnostic.incoming_oid,
                merge_base,
                reason,
            ),
        );

        if let Some(check) = mergeability {
            if !check.conflict_paths.is_empty() {
                message.push_str("  conflict files:\n");
                for path in &check.conflict_paths {
                    let _ =
                        std::fmt::Write::write_fmt(&mut message, format_args!("  - {}\n", path));
                }
            }
            if let Some(detail) = &check.detail {
                let _ = std::fmt::Write::write_fmt(
                    &mut message,
                    format_args!("  detail: {}\n", detail),
                );
            }
        }

        let _ = std::fmt::Write::write_fmt(
            &mut message,
            format_args!(
                "  next-step: merge required; incoming ref preserved at {}\n",
                diagnostic.preserved_incoming_ref
            ),
        );
    }
    message
}

/// Aggregates per-file line deltas across all imported heads.
///
/// Aggregation is keyed by path and sums additions/deletions from each head.
fn collect_bundle_line_stats(
    repo: &git2::Repository,
    inspection: &BundleInspection,
) -> Result<Vec<FileLineStat>> {
    let mut aggregated = std::collections::BTreeMap::<String, (usize, usize)>::new();

    for head in &inspection.heads {
        let stats_for_head = collect_line_stats_for_head(repo, head, &inspection.prerequisites)?;
        for stat in stats_for_head {
            let entry = aggregated.entry(stat.path).or_insert((0, 0));
            entry.0 += stat.additions;
            entry.1 += stat.deletions;
        }
    }

    Ok(aggregated
        .into_iter()
        .map(|(path, (additions, deletions))| FileLineStat {
            path,
            additions,
            deletions,
        })
        .collect())
}

/// Computes per-file line stats for a single imported head.
///
/// Non-text changes are represented as `0/0` line deltas.
fn collect_line_stats_for_head(
    repo: &git2::Repository,
    head: &BundleHead,
    prerequisites: &[git2::Oid],
) -> Result<Vec<FileLineStat>> {
    let head_commit = repo.find_commit(head.oid)?;
    let tip_tree = head_commit.tree()?;
    let base_tree = resolve_base_tree_for_head(repo, &head_commit, prerequisites)?;

    let mut diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut stats = Vec::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = path_to_string(delta.new_file().path().or(delta.old_file().path()))?;
        let (additions, deletions) = if is_non_text_delta(&delta) {
            (0, 0)
        } else {
            match git2::Patch::from_diff(&diff, index)? {
                Some(patch) => {
                    let (_, additions, deletions) = patch.line_stats()?;
                    (additions, deletions)
                }
                None => (0, 0),
            }
        };
        stats.push(FileLineStat {
            path,
            additions,
            deletions,
        });
    }
    Ok(stats)
}

/// Builds head-scoped audit entries for imported bundle heads.
fn collect_head_audit_entries(
    repo: &git2::Repository,
    inspection: &BundleInspection,
) -> Result<Vec<HeadAuditEntry>> {
    let mut entries = Vec::new();
    for head in &inspection.heads {
        let commit_ids =
            collect_imported_commit_ids_for_head(repo, head, &inspection.prerequisites)?;
        let commits = commit_ids
            .into_iter()
            .map(|commit_id| build_commit_audit_entry(repo, commit_id))
            .collect::<Result<Vec<_>>>()?;
        let line_stats = collect_line_stats_for_head(repo, head, &inspection.prerequisites)?;
        entries.push(HeadAuditEntry {
            head: head.clone(),
            line_stats,
            commits,
        });
    }
    Ok(entries)
}

/// Enumerates imported commits for one head, excluding shared prerequisites.
///
/// Returned OIDs are ordered oldest-first for stable per-head page progression.
fn collect_imported_commit_ids_for_head(
    repo: &git2::Repository,
    head: &BundleHead,
    prerequisites: &[git2::Oid],
) -> Result<Vec<git2::Oid>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME | git2::Sort::REVERSE)?;
    revwalk.push(head.oid)?;
    for prerequisite in prerequisites {
        revwalk.hide(*prerequisite)?;
    }

    let mut commits = Vec::new();
    for oid_result in revwalk {
        commits.push(oid_result?);
    }
    Ok(commits)
}

/// Builds one commit-level audit entry with identity and file line stats.
fn build_commit_audit_entry(
    repo: &git2::Repository,
    commit_id: git2::Oid,
) -> Result<CommitAuditEntry> {
    let commit = repo.find_commit(commit_id)?;
    let tip_tree = commit.tree()?;
    let base_tree = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };

    let files = collect_line_stats_for_tree_diff(repo, base_tree.as_ref(), &tip_tree)?;
    let committer = commit.committer();
    let author = commit.author();
    Ok(CommitAuditEntry {
        commit_id,
        subject: commit
            .summary()
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| "<no subject>".to_string()),
        committer: CommitAuditIdentity {
            name: committer.name().unwrap_or("<unknown>").to_string(),
            email: committer.email().unwrap_or("<unknown>").to_string(),
            time_seconds: committer.when().seconds(),
            offset_minutes: committer.when().offset_minutes(),
        },
        author: CommitAuditIdentity {
            name: author.name().unwrap_or("<unknown>").to_string(),
            email: author.email().unwrap_or("<unknown>").to_string(),
            time_seconds: author.when().seconds(),
            offset_minutes: author.when().offset_minutes(),
        },
        files,
    })
}

/// Returns a textual patch for one file in a single commit.
///
/// # Errors
///
/// Returns an error for missing commits/paths, non-text changes, or when the
/// file is not part of the commit diff.
fn collect_commit_file_patch(
    repo: &git2::Repository,
    commit_id: git2::Oid,
    path: &str,
) -> Result<String> {
    let commit = repo.find_commit(commit_id)?;
    let tip_tree = commit.tree()?;
    let base_tree = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };

    let mut diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    for (index, delta) in diff.deltas().enumerate() {
        let old_path = path_to_string(delta.old_file().path())?;
        let new_path = path_to_string(delta.new_file().path())?;
        if old_path != path && new_path != path {
            continue;
        }
        if is_non_text_delta(&delta) {
            bail!("textual diff unavailable for non-text path '{path}'");
        }

        let patch = git2::Patch::from_diff(&diff, index)?;
        let Some(mut patch) = patch else {
            return Ok(format!(
                "diff --git a/{old_path} b/{new_path}\nBinary file changed; textual diff unavailable.\n"
            ));
        };

        let patch_buf = patch.to_buf()?;
        let patch_text = String::from_utf8_lossy(patch_buf.as_ref()).to_string();
        return Ok(patch_text);
    }

    bail!("file '{path}' is not changed in commit '{commit_id}'")
}

/// Runs a callback against a temporary repo with the bundle input imported.
///
/// This isolates read/analysis operations from the live receiver repository.
fn with_imported_bundle_input_repo<T>(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    func: impl FnOnce(&git2::Repository, &BundleInspection) -> Result<T>,
) -> Result<T> {
    // Analysis helpers run against a temporary imported repo to avoid mutating the receiver.
    let temp_repo = TempBareRepo::from_existing(receiver_repo_path)?;
    let repo = git2::Repository::open_bare(&temp_repo.path)?;

    let inspection = if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let inspection = inspect_bundle(&extracted.bundle_path)?;
        let _ = apply_bundle_to_repo(
            &repo,
            &extracted.bundle_path,
            &inspection.heads,
            ReceiveIntegratePolicy::CreateRefsOnly,
            false,
            false,
            false,
        )?;
        inspection
    } else {
        let inspection = inspect_bundle(bundle_input_path)?;
        let _ = apply_bundle_to_repo(
            &repo,
            bundle_input_path,
            &inspection.heads,
            ReceiveIntegratePolicy::CreateRefsOnly,
            false,
            false,
            false,
        )?;
        inspection
    };

    func(&repo, &inspection)
}

/// Computes line stats for a direct tree-to-tree diff.
///
/// Returned rows are path-sorted for stable UI rendering.
fn collect_line_stats_for_tree_diff(
    repo: &git2::Repository,
    base_tree: Option<&git2::Tree<'_>>,
    tip_tree: &git2::Tree<'_>,
) -> Result<Vec<FileLineStat>> {
    let mut diff = repo.diff_tree_to_tree(base_tree, Some(tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut stats = Vec::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = path_to_string(delta.new_file().path().or(delta.old_file().path()))?;
        let (additions, deletions) = if is_non_text_delta(&delta) {
            (0, 0)
        } else {
            match git2::Patch::from_diff(&diff, index)? {
                Some(patch) => {
                    let (_, additions, deletions) = patch.line_stats()?;
                    (additions, deletions)
                }
                None => (0, 0),
            }
        };
        stats.push(FileLineStat {
            path,
            additions,
            deletions,
        });
    }

    stats.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(stats)
}

/// Resolves the baseline tree used for per-head dry-run diffing.
///
/// For multi-prerequisite bundles, this requires exactly one prerequisite that
/// is an ancestor of the head.
fn resolve_base_tree_for_head<'repo>(
    repo: &'repo git2::Repository,
    head_commit: &git2::Commit<'repo>,
    prerequisites: &[git2::Oid],
) -> Result<Option<git2::Tree<'repo>>> {
    if prerequisites.is_empty() {
        return if head_commit.parent_count() == 0 {
            Ok(None)
        } else {
            Ok(Some(head_commit.parent(0)?.tree()?))
        };
    }

    if prerequisites.len() == 1 {
        let base_commit = repo.find_commit(prerequisites[0])?;
        return Ok(Some(base_commit.tree()?));
    }

    let mut matching_prerequisites = Vec::new();
    for prerequisite in prerequisites {
        if repo.graph_descendant_of(head_commit.id(), *prerequisite)? {
            matching_prerequisites.push(*prerequisite);
        }
    }

    if matching_prerequisites.len() != 1 {
        bail!(
            "unable to determine unique dry-run base for head '{}' with {} prerequisites",
            head_commit.id(),
            prerequisites.len()
        );
    }

    let base_commit = repo.find_commit(matching_prerequisites[0])?;
    Ok(Some(base_commit.tree()?))
}

/// Returns `true` when the referenced head already points at an imported commit.
///
/// This guards repeated receive operations from rewriting unchanged refs.
pub(crate) fn is_head_already_applied(repo: &git2::Repository, head: &BundleHead) -> Result<bool> {
    let current_target = match repo.find_reference(&head.reference) {
        Ok(reference) => reference.target().or_else(|| {
            reference
                .resolve()
                .ok()
                .and_then(|resolved| resolved.target())
        }),
        Err(err) if err.code() == git2::ErrorCode::NotFound => None,
        Err(err) => return Err(err.into()),
    };

    let Some(current_target) = current_target else {
        return Ok(false);
    };
    if current_target != head.oid {
        return Ok(false);
    }

    Ok(repo.find_commit(head.oid).is_ok())
}

/// Returns `true` when a diff delta is not a regular text-file change.
fn is_non_text_delta(delta: &git2::DiffDelta<'_>) -> bool {
    let old_file = delta.old_file();
    let new_file = delta.new_file();

    if old_file.is_binary() || new_file.is_binary() {
        return true;
    }

    let old_mode = u32::from(old_file.mode());
    let new_mode = u32::from(new_file.mode());

    let old_regular = !old_file.exists() || is_regular_blob_mode(old_mode);
    let new_regular = !new_file.exists() || is_regular_blob_mode(new_mode);

    !(old_regular && new_regular)
}

/// Returns `true` for standard git regular-file modes.
fn is_regular_blob_mode(mode: u32) -> bool {
    mode == 0o100644 || mode == 0o100755
}

#[cfg(test)]
mod tests;

struct TempBareRepo {
    path: PathBuf,
}

impl TempBareRepo {
    /// Creates a temporary bare mirror of the receiver repository.
    ///
    /// The mirror is populated via anonymous remote fetch and deleted on drop.
    fn from_existing(source_repo_path: &Path) -> Result<Self> {
        let temp_path = std::env::temp_dir().join(format!(
            "git-sync-receive-dry-run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| anyhow!("system clock is before unix epoch"))?
                .as_nanos()
        ));
        fs::create_dir_all(&temp_path)?;

        let source_repo = git2::Repository::open(source_repo_path)?;
        let repo = git2::Repository::init_bare(&temp_path)?;
        configure_temp_repo_alternates(&temp_path, &source_repo)?;
        let source = source_repo_path.to_string_lossy();
        let mut remote = repo.remote_anonymous(source.as_ref())?;
        remote.fetch(&["+refs/*:refs/*"], None, None)?;

        Ok(Self { path: temp_path })
    }
}

/// Configures alternates so the dry-run mirror can read all objects from source ODB.
///
/// This preserves object visibility parity with the live receiver, including
/// local objects not reachable from any current ref.
fn configure_temp_repo_alternates(
    temp_repo_git_dir: &Path,
    source_repo: &git2::Repository,
) -> Result<()> {
    let source_objects_dir = source_repo.path().join("objects");
    if !source_objects_dir.is_dir() {
        bail!(
            "source repository objects directory is missing: {}",
            source_objects_dir.display()
        );
    }

    let mut alternate_dirs = BTreeSet::<PathBuf>::new();
    alternate_dirs.insert(source_objects_dir.clone());

    let source_alternates = source_objects_dir.join("info").join("alternates");
    if source_alternates.is_file() {
        let content = fs::read_to_string(&source_alternates)?;
        for raw_line in content.lines() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            alternate_dirs.insert(resolve_alternate_entry_path(&source_objects_dir, trimmed));
        }
    }

    let temp_alternates = temp_repo_git_dir
        .join("objects")
        .join("info")
        .join("alternates");
    if let Some(parent) = temp_alternates.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut serialized = String::new();
    for alternate in alternate_dirs {
        let _ = std::fmt::Write::write_fmt(
            &mut serialized,
            format_args!("{}\n", alternate.to_string_lossy()),
        );
    }
    fs::write(temp_alternates, serialized.as_bytes())?;

    Ok(())
}

/// Resolves one alternate entry path to an absolute objects directory path.
fn resolve_alternate_entry_path(base_objects_dir: &Path, entry: &str) -> PathBuf {
    let raw = Path::new(entry);
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base_objects_dir.join(raw)
    };
    joined.canonicalize().unwrap_or(joined)
}

/// Ensures all persisted `objects/info/alternates` entries are active in this ODB handle.
///
/// This is required so thin-pack indexing resolves external delta bases
/// consistently across environments.
fn add_repo_disk_alternates_to_odb(repo: &git2::Repository, odb: &git2::Odb<'_>) -> Result<()> {
    let objects_dir = repo.path().join("objects");
    let alternates_path = objects_dir.join("info").join("alternates");
    if !alternates_path.is_file() {
        return Ok(());
    }

    let content = fs::read_to_string(alternates_path)?;
    let mut seen = BTreeSet::<PathBuf>::new();
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let resolved = resolve_alternate_entry_path(&objects_dir, trimmed);
        if !resolved.is_dir() || !seen.insert(resolved.clone()) {
            continue;
        }
        let resolved_text = resolved.to_string_lossy().to_string();
        odb.add_disk_alternate(&resolved_text)?;
    }
    Ok(())
}

/// Enriches indexer errors with environment diagnostics when verbose mode is enabled.
fn with_verbose_indexer_diagnostics(
    err: impl std::fmt::Display,
    verbose: bool,
    repo: &git2::Repository,
    bundle_path: &Path,
    inspection: &BundleInspection,
    pack_len: usize,
    stage: &str,
) -> anyhow::Error {
    if !verbose {
        return anyhow!(err.to_string());
    }

    let repo_git_dir = repo.path().to_string_lossy().to_string();
    let repo_is_bare = repo.is_bare();
    let workdir = repo
        .workdir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let bundle_path_text = bundle_path.to_string_lossy().to_string();
    let bundle_version = match inspection.version {
        crate::git::BundleVersion::V2 => "v2",
        crate::git::BundleVersion::V3 => "v3",
    };

    let object_format = repo
        .config()
        .ok()
        .and_then(|cfg| cfg.get_string("extensions.objectformat").ok())
        .unwrap_or_else(|| "sha1".to_string());
    let shallow_marker_exists = repo.path().join("shallow").is_file();

    let (missing_prerequisites, missing_prerequisite_count) = repo
        .odb()
        .ok()
        .map(|odb| {
            let missing = inspection
                .prerequisites
                .iter()
                .filter(|oid| !odb.exists(**oid))
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let count = missing.len();
            (missing, count)
        })
        .unwrap_or_else(|| {
            (
                vec!["<unable to inspect receiver odb>".to_string()],
                inspection.prerequisites.len(),
            )
        });

    let objects_dir = repo.path().join("objects");
    let alternates_path = objects_dir.join("info").join("alternates");
    let alternates_entries = if alternates_path.is_file() {
        read_alternates_file_entries(&objects_dir, &alternates_path)
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|entry| entry.display().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|read_err| vec![format!("<unable to parse alternates: {read_err}>")])
    } else {
        Vec::new()
    };
    let alternates_summary = if alternates_entries.is_empty() {
        "none".to_string()
    } else {
        alternates_entries.join(", ")
    };

    let missing_prereq_preview = if missing_prerequisites.is_empty() {
        "none".to_string()
    } else {
        missing_prerequisites
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let missing_prereq_truncated = missing_prerequisites.len() > 8;
    let truncated_suffix = if missing_prereq_truncated {
        ", ..."
    } else {
        ""
    };

    anyhow!(
        "unable to import bundle pack during {stage}: {err}\n\
         verbose diagnostics:\n\
         - receiver git dir: {repo_git_dir}\n\
         - receiver workdir: {workdir}\n\
         - receiver bare repo: {repo_is_bare}\n\
         - bundle path: {bundle_path_text}\n\
         - bundle version: {bundle_version}\n\
         - bundle heads: {}\n\
         - bundle prerequisites: {}\n\
         - pack payload bytes: {pack_len}\n\
         - object format: {object_format}\n\
         - shallow marker present: {shallow_marker_exists}\n\
         - alternates file: {}\n\
         - alternates entries: {alternates_summary}\n\
         - missing prerequisite objects in receiver odb: {missing_prerequisite_count}/{} [{}{}]\n\
         hint: if this is 'packfile is missing N objects', the bundle is thin and requires base objects not visible in this repository/alternates.",
        inspection.heads.len(),
        inspection.prerequisites.len(),
        alternates_path.display(),
        inspection.prerequisites.len(),
        missing_prereq_preview,
        truncated_suffix
    )
}

/// Reads and resolves one alternates file into canonical object-directory paths.
fn read_alternates_file_entries(
    base_objects_dir: &Path,
    alternates_path: &Path,
) -> Result<Vec<PathBuf>> {
    let content = fs::read_to_string(alternates_path)?;
    let mut entries = BTreeSet::<PathBuf>::new();
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        entries.insert(resolve_alternate_entry_path(base_objects_dir, trimmed));
    }
    Ok(entries.into_iter().collect())
}

impl Drop for TempBareRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
