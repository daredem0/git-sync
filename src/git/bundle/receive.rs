use super::inspect::inspect_bundle;
use crate::git::archive::{extract_bundle_archive, is_zip_bundle_input_path};
use crate::git::metadata::verify_bundle_metadata_integrity_input;
use crate::git::util::path_to_string;
use crate::git::{
    BundleHead, BundleInspection, FileLineStat, ReceiveBundleOptions, ReceiveBundleResult,
};
use anyhow::{Result, anyhow, bail};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn receive_bundle_input(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
) -> Result<ReceiveBundleResult> {
    receive_bundle_input_with_options(
        bundle_input_path,
        receiver_repo_path,
        ReceiveBundleOptions::default(),
    )
}

pub fn receive_bundle_input_with_options(
    bundle_input_path: &Path,
    receiver_repo_path: &Path,
    options: ReceiveBundleOptions,
) -> Result<ReceiveBundleResult> {
    if options.verify_metadata {
        verify_bundle_metadata_integrity_input(bundle_input_path)?;
    }

    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        receive_bundle(&extracted.bundle_path, receiver_repo_path, options.dry_run)
    } else {
        receive_bundle(bundle_input_path, receiver_repo_path, options.dry_run)
    }
}

fn receive_bundle(
    bundle_path: &Path,
    receiver_repo_path: &Path,
    dry_run: bool,
) -> Result<ReceiveBundleResult> {
    let inspection = inspect_bundle(bundle_path)?;
    if inspection.heads.is_empty() {
        bail!("bundle does not contain any heads to import");
    }

    let repo = git2::Repository::open(receiver_repo_path)?;
    if inspection
        .heads
        .iter()
        .map(|head| is_head_already_applied(&repo, head))
        .collect::<Result<Vec<bool>>>()?
        .into_iter()
        .all(std::convert::identity)
    {
        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts: true,
            line_stats: Vec::new(),
        });
    }

    if dry_run {
        let temp_repo = TempBareRepo::from_existing(receiver_repo_path)?;
        let dry_run_repo = git2::Repository::open_bare(&temp_repo.path)?;
        apply_bundle_to_repo(&dry_run_repo, bundle_path, &inspection.heads)?;
        let line_stats = collect_bundle_line_stats(&dry_run_repo, &inspection)?;

        return Ok(ReceiveBundleResult {
            bundle_version: inspection.version,
            imported_heads: inspection.heads,
            can_apply_without_conflicts: true,
            line_stats,
        });
    }

    apply_bundle_to_repo(&repo, bundle_path, &inspection.heads)?;

    Ok(ReceiveBundleResult {
        bundle_version: inspection.version,
        imported_heads: inspection.heads,
        can_apply_without_conflicts: true,
        line_stats: Vec::new(),
    })
}

fn apply_bundle_to_repo(
    repo: &git2::Repository,
    bundle_path: &Path,
    heads: &[BundleHead],
) -> Result<()> {
    let bundle_bytes = fs::read(bundle_path)?;
    let pack_offset = bundle_bytes
        .windows(4)
        .position(|window| window == b"PACK")
        .ok_or_else(|| anyhow!("bundle does not contain PACK payload"))?;
    let pack_data = &bundle_bytes[pack_offset..];

    let odb = repo.odb()?;
    let pack_dir = repo.path().join("objects").join("pack");
    fs::create_dir_all(&pack_dir)?;
    let mut indexer = git2::Indexer::new(Some(&odb), &pack_dir, 0o644, true)?;
    indexer.write_all(pack_data)?;
    indexer.commit()?;

    for head in heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    for head in heads {
        if is_head_already_applied(&repo, head)? {
            continue;
        }
        repo.reference(&head.reference, head.oid, true, "receive bundle import")?;
    }

    Ok(())
}

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
        let (additions, deletions) = match git2::Patch::from_diff(&diff, index)? {
            Some(patch) => {
                let (_, additions, deletions) = patch.line_stats()?;
                (additions, deletions)
            }
            None => (0, 0),
        };
        stats.push(FileLineStat {
            path,
            additions,
            deletions,
        });
    }
    Ok(stats)
}

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

struct TempBareRepo {
    path: PathBuf,
}

impl TempBareRepo {
    fn from_existing(source_repo_path: &Path) -> Result<Self> {
        let temp_path = std::env::temp_dir().join(format!(
            "git-sync-audit-receive-dry-run-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| anyhow!("system clock is before unix epoch"))?
                .as_nanos()
        ));
        fs::create_dir_all(&temp_path)?;

        let repo = git2::Repository::init_bare(&temp_path)?;
        let source = source_repo_path.to_string_lossy();
        let mut remote = repo.remote_anonymous(source.as_ref())?;
        remote.fetch(&["+refs/*:refs/*"], None, None)?;

        Ok(Self { path: temp_path })
    }
}

impl Drop for TempBareRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
