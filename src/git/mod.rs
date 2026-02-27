use crate::app::AppConfig;
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleVersion {
    V2,
    V3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenContext {
    pub base_commit_id: git2::Oid,
    pub tip_commit_id: Option<git2::Oid>,
    pub bundle_version: BundleVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub status: ChangeStatus,
    pub path: String,
    pub old_path: Option<String>,
    pub old_oid: Option<git2::Oid>,
    pub new_oid: Option<git2::Oid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBundleResult {
    pub from_commit_id: git2::Oid,
    pub to_commit_id: git2::Oid,
    pub tip_ref_name: String,
    pub bundle_path: PathBuf,
}

pub fn render_manifest(changes: &[ChangedFile]) -> String {
    let mut out = String::from("STATUS\tPATH\tOLD_PATH\tOLD_OID\tNEW_OID\n");
    for change in changes {
        let status = status_code(change.status);
        let old_path = change.old_path.as_deref().unwrap_or("-");
        let old_oid = oid_to_str(change.old_oid);
        let new_oid = oid_to_str(change.new_oid);
        out.push_str(status);
        out.push('\t');
        out.push_str(&change.path);
        out.push('\t');
        out.push_str(old_path);
        out.push('\t');
        out.push_str(&old_oid);
        out.push('\t');
        out.push_str(&new_oid);
        out.push('\n');
    }
    out
}

pub fn render_manifest_json(changes: &[ChangedFile]) -> Result<String> {
    let entries: Vec<JsonChangedFile> = changes
        .iter()
        .map(|change| JsonChangedFile {
            status: status_code(change.status).to_string(),
            path: change.path.clone(),
            old_path: change.old_path.clone(),
            old_oid: change.old_oid.map(|oid| oid.to_string()),
            new_oid: change.new_oid.map(|oid| oid.to_string()),
        })
        .collect();
    Ok(serde_json::to_string_pretty(&entries)?)
}

pub fn create_bundle(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
    bundle_path: &Path,
) -> Result<CreateBundleResult> {
    let repo = git2::Repository::open(repo_path)?;

    let from_obj = repo.revparse_single(from_rev)?;
    let from_commit = from_obj.peel_to_commit()?;
    let from_commit_id = from_commit.id();

    let (to_obj, to_ref) = repo.revparse_ext(to_rev)?;
    let to_commit = to_obj.peel_to_commit()?;
    let to_commit_id = to_commit.id();

    if from_commit_id != to_commit_id && !repo.graph_descendant_of(to_commit_id, from_commit_id)? {
        bail!(
            "to commit '{}' must be the same as or a descendant of from commit '{}'",
            to_rev,
            from_rev
        );
    }

    let tip_ref_name = to_ref
        .and_then(|reference| reference.name().map(|name| name.to_string()))
        .unwrap_or_else(|| format!("refs/heads/bundle-tip-{}", &to_commit_id.to_string()[..12]));

    let mut walk = repo.revwalk()?;
    walk.push(to_commit_id)?;
    walk.hide(from_commit_id)?;

    let mut packbuilder = repo.packbuilder()?;
    packbuilder.insert_walk(&mut walk)?;
    let mut pack_buffer = git2::Buf::new();
    packbuilder.write_buf(&mut pack_buffer)?;

    let mut file = File::create(bundle_path)?;
    writeln!(file, "# v2 git bundle")?;
    writeln!(file, "-{from_commit_id}")?;
    writeln!(file, "{to_commit_id} {tip_ref_name}")?;
    writeln!(file)?;
    file.write_all(&pack_buffer)?;

    Ok(CreateBundleResult {
        from_commit_id,
        to_commit_id,
        tip_ref_name,
        bundle_path: bundle_path.to_path_buf(),
    })
}

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

    let bundle_version = validate_bundle_header(&config.bundle_path)?;
    Ok(OpenContext {
        base_commit_id,
        tip_commit_id,
        bundle_version,
    })
}

fn validate_bundle_header(bundle_path: &Path) -> Result<BundleVersion> {
    let file = File::open(bundle_path)?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;

    let normalized = first_line.trim_end_matches(&['\r', '\n'][..]);
    if normalized == "# v2 git bundle" {
        return Ok(BundleVersion::V2);
    }
    if normalized == "# v3 git bundle" {
        return Ok(BundleVersion::V3);
    }

    bail!("bundle file is not a valid git bundle header");
}

pub fn collect_changed_files(
    repo_path: &Path,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<ChangedFile>> {
    let repo = git2::Repository::open(repo_path)?;
    let base_commit = repo.find_commit(base_commit_id)?;
    let tip_commit = repo.find_commit(tip_commit_id)?;
    let base_tree = base_commit.tree()?;
    let tip_tree = tip_commit.tree()?;

    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut changes = Vec::new();
    for delta in diff.deltas() {
        let old_file = delta.old_file();
        let new_file = delta.new_file();

        let (status, path, old_path) = match delta.status() {
            git2::Delta::Unmodified => continue,
            git2::Delta::Added => (ChangeStatus::Added, path_to_string(new_file.path())?, None),
            git2::Delta::Modified => (
                ChangeStatus::Modified,
                path_to_string(new_file.path().or(old_file.path()))?,
                None,
            ),
            git2::Delta::Deleted => (
                ChangeStatus::Deleted,
                path_to_string(old_file.path())?,
                None,
            ),
            git2::Delta::Renamed => (
                ChangeStatus::Renamed,
                path_to_string(new_file.path())?,
                Some(path_to_string(old_file.path())?),
            ),
            git2::Delta::Copied => (
                ChangeStatus::Copied,
                path_to_string(new_file.path())?,
                Some(path_to_string(old_file.path())?),
            ),
            git2::Delta::Typechange => (
                ChangeStatus::TypeChanged,
                path_to_string(new_file.path().or(old_file.path()))?,
                None,
            ),
            other => bail!("unsupported diff delta status for tree diff: {other:?}"),
        };

        changes.push(ChangedFile {
            status,
            path,
            old_path,
            old_oid: oid_or_none(old_file.id()),
            new_oid: oid_or_none(new_file.id()),
        });
    }

    // Deterministic order is required for stable audit artifacts.
    changes.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.old_path.cmp(&b.old_path))
    });
    Ok(changes)
}

fn path_to_string(path: Option<&Path>) -> Result<String> {
    match path {
        Some(path) => Ok(path.to_string_lossy().into_owned()),
        None => Err(anyhow!("diff entry is missing file path")),
    }
}

fn oid_or_none(oid: git2::Oid) -> Option<git2::Oid> {
    if oid.is_zero() { None } else { Some(oid) }
}

fn oid_to_str(oid: Option<git2::Oid>) -> String {
    oid.map_or_else(|| "-".to_string(), |oid| oid.to_string())
}

fn status_code(status: ChangeStatus) -> &'static str {
    match status {
        ChangeStatus::Added => "A",
        ChangeStatus::Modified => "M",
        ChangeStatus::Deleted => "D",
        ChangeStatus::Renamed => "R",
        ChangeStatus::Copied => "C",
        ChangeStatus::TypeChanged => "T",
    }
}

#[derive(Debug, Clone, Serialize)]
struct JsonChangedFile {
    status: String,
    path: String,
    old_path: Option<String>,
    old_oid: Option<String>,
    new_oid: Option<String>,
}

#[cfg(test)]
mod tests;
