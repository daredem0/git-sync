use crate::app::AppConfig;
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::FileOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleVersion {
    V2,
    V3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleHead {
    pub oid: git2::Oid,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleInspection {
    pub version: BundleVersion,
    pub prerequisites: Vec<git2::Oid>,
    pub heads: Vec<BundleHead>,
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
    pub audit_path: PathBuf,
    pub patch_audit_path: Option<PathBuf>,
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoAuditRange {
    pub base_commit_id: git2::Oid,
    pub tip_commit_id: git2::Oid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveBundleResult {
    pub bundle_version: BundleVersion,
    pub imported_heads: Vec<BundleHead>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditMetadata {
    schema_version: String,
    tool_version: String,
    generated_at_unix_secs: u64,
    generated_by_username: String,
    generated_by_hostname: String,
    bundle_path: String,
    bundle_size_bytes: u64,
    bundle_sha256: String,
    bundle_header_version: String,
    prerequisites: Vec<String>,
    heads: Vec<CreateBundleAuditHead>,
    range_from_oid: String,
    range_to_oid: String,
    tip_ref: String,
    commit_chain: Vec<CreateBundleAuditCommit>,
    changed_files: Vec<CreateBundleAuditChangedFile>,
    patch_sidecar: Option<CreateBundleAuditPatchSidecar>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditHead {
    oid: String,
    reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditCommit {
    oid: String,
    tree_oid: String,
    parent_oids: Vec<String>,
    subject: String,
    author: CreateBundleAuditSignature,
    committer: CreateBundleAuditSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditSignature {
    name: String,
    email: String,
    time_seconds: i64,
    offset_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditChangedFile {
    status: String,
    path: String,
    old_path: Option<String>,
    old_oid: Option<String>,
    new_oid: Option<String>,
    old_mode: Option<String>,
    new_mode: Option<String>,
    is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CreateBundleAuditPatchSidecar {
    path: String,
    format: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffEntry {
    status: ChangeStatus,
    path: String,
    old_path: Option<String>,
    old_oid: Option<git2::Oid>,
    new_oid: Option<git2::Oid>,
    old_mode: Option<u32>,
    new_mode: Option<u32>,
    is_binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CreateBundleOptions {
    pub include_patch_sidecar: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReceiveBundleOptions {
    pub verify_metadata: bool,
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
    create_bundle_with_options(
        repo_path,
        from_rev,
        to_rev,
        bundle_path,
        CreateBundleOptions::default(),
    )
}

pub fn create_bundle_with_options(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
    bundle_path: &Path,
    options: CreateBundleOptions,
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

    let inspection = inspect_bundle(bundle_path)?;
    let changed_files = collect_changed_files_for_metadata(&repo, from_commit_id, to_commit_id)?;
    let commit_chain = collect_commit_chain_for_metadata(&repo, from_commit_id, to_commit_id)?;

    let patch_sidecar = if options.include_patch_sidecar {
        Some(write_patch_sidecar(
            &repo,
            from_commit_id,
            to_commit_id,
            bundle_path,
        )?)
    } else {
        None
    };

    let bundle_bytes = fs::read(bundle_path)?;
    let bundle_size_bytes = bundle_bytes.len() as u64;
    let bundle_sha256 = sha256_hex(&bundle_bytes)?;
    let audit_path = caudit_sidecar_path(bundle_path);
    let metadata = CreateBundleAuditMetadata {
        schema_version: "1".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix_secs: current_unix_timestamp_secs()?,
        generated_by_username: current_username(),
        generated_by_hostname: current_hostname(),
        bundle_path: bundle_path.display().to_string(),
        bundle_size_bytes,
        bundle_sha256,
        bundle_header_version: bundle_version_code(inspection.version).to_string(),
        prerequisites: inspection
            .prerequisites
            .iter()
            .map(|oid| oid.to_string())
            .collect(),
        heads: inspection
            .heads
            .iter()
            .map(|head| CreateBundleAuditHead {
                oid: head.oid.to_string(),
                reference: head.reference.clone(),
            })
            .collect(),
        range_from_oid: from_commit_id.to_string(),
        range_to_oid: to_commit_id.to_string(),
        tip_ref: tip_ref_name.clone(),
        commit_chain,
        changed_files,
        patch_sidecar,
    };
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&audit_path, metadata_json.as_bytes())?;

    let patch_audit_path = metadata
        .patch_sidecar
        .as_ref()
        .map(|sidecar| PathBuf::from(sidecar.path.clone()));
    let archive_path = bundle_archive_path(bundle_path);
    let mut archive_inputs = vec![bundle_path.to_path_buf(), audit_path.clone()];
    if let Some(patch_path) = &patch_audit_path {
        archive_inputs.push(patch_path.clone());
    }
    write_zip_archive(&archive_path, &archive_inputs)?;

    Ok(CreateBundleResult {
        from_commit_id,
        to_commit_id,
        tip_ref_name,
        bundle_path: bundle_path.to_path_buf(),
        audit_path,
        patch_audit_path,
        archive_path,
    })
}

pub fn remove_unarchived_bundle_artifacts(result: &CreateBundleResult) -> Result<()> {
    remove_file_if_exists(&result.bundle_path)?;
    remove_file_if_exists(&result.audit_path)?;
    if let Some(patch_path) = &result.patch_audit_path {
        remove_file_if_exists(patch_path)?;
    }
    Ok(())
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

    let bundle_inspection = inspect_bundle(&config.bundle_path)?;
    Ok(OpenContext {
        base_commit_id,
        tip_commit_id,
        bundle_version: bundle_inspection.version,
    })
}

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

pub fn collect_changed_files_from_bundle_input(
    bundle_input_path: &Path,
) -> Result<Vec<ChangedFile>> {
    let metadata = load_bundle_metadata_from_input(bundle_input_path)?;
    metadata
        .changed_files
        .into_iter()
        .map(|entry| {
            Ok(ChangedFile {
                status: parse_status_code(&entry.status)?,
                path: entry.path,
                old_path: entry.old_path,
                old_oid: parse_optional_oid(entry.old_oid.as_deref())?,
                new_oid: parse_optional_oid(entry.new_oid.as_deref())?,
            })
        })
        .collect()
}

pub fn verify_bundle_metadata_against_repo(bundle_path: &Path, repo_path: &Path) -> Result<()> {
    let metadata = verify_bundle_metadata_integrity(bundle_path)?;

    let repo = git2::Repository::open(repo_path)?;

    let from_commit_id = git2::Oid::from_str(&metadata.range_from_oid)?;
    let to_commit_id = git2::Oid::from_str(&metadata.range_to_oid)?;

    repo.find_commit(from_commit_id)?;
    repo.find_commit(to_commit_id)?;

    if to_commit_id != from_commit_id && !repo.graph_descendant_of(to_commit_id, from_commit_id)? {
        bail!(
            "metadata range is not linear in repository: to={} from={}",
            to_commit_id,
            from_commit_id
        );
    }

    let expected_commit_chain =
        collect_commit_chain_for_metadata(&repo, from_commit_id, to_commit_id)?;
    if metadata.commit_chain != expected_commit_chain {
        bail!("metadata commit_chain does not match repository truth");
    }

    let expected_changed_files =
        collect_changed_files_for_metadata(&repo, from_commit_id, to_commit_id)?;
    if metadata.changed_files != expected_changed_files {
        bail!("metadata changed_files does not match repository truth");
    }

    Ok(())
}

pub fn verify_bundle_metadata_against_repo_input(
    bundle_input_path: &Path,
    repo_path: &Path,
) -> Result<()> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        verify_bundle_metadata_against_repo(&extracted.bundle_path, repo_path)
    } else {
        verify_bundle_metadata_against_repo(bundle_input_path, repo_path)
    }
}

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
        receive_bundle(&extracted.bundle_path, receiver_repo_path)
    } else {
        receive_bundle(bundle_input_path, receiver_repo_path)
    }
}

pub fn resolve_repo_audit_range(
    repo_path: &Path,
    from_rev: &str,
    to_rev: &str,
) -> Result<RepoAuditRange> {
    let repo = git2::Repository::open(repo_path)?;

    let base_obj = repo.revparse_single(from_rev)?;
    let base_commit = base_obj.peel_to_commit()?;
    let base_commit_id = base_commit.id();

    let tip_obj = repo.revparse_single(to_rev)?;
    let tip_commit = tip_obj.peel_to_commit()?;
    let tip_commit_id = tip_commit.id();

    if tip_commit_id != base_commit_id
        && !repo.graph_descendant_of(tip_commit_id, base_commit_id)?
    {
        bail!(
            "to rev '{}' must be the same commit as from rev '{}' or a descendant of it",
            to_rev,
            from_rev
        );
    }

    Ok(RepoAuditRange {
        base_commit_id,
        tip_commit_id,
    })
}

pub fn collect_changed_files(
    repo_path: &Path,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<ChangedFile>> {
    let repo = git2::Repository::open(repo_path)?;
    let diff_entries = collect_diff_entries(&repo, base_commit_id, tip_commit_id)?;
    Ok(diff_entries
        .into_iter()
        .map(|entry| ChangedFile {
            status: entry.status,
            path: entry.path,
            old_path: entry.old_path,
            old_oid: entry.old_oid,
            new_oid: entry.new_oid,
        })
        .collect())
}

fn verify_bundle_metadata_integrity_input(bundle_input_path: &Path) -> Result<()> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        verify_bundle_metadata_integrity(&extracted.bundle_path)?;
        Ok(())
    } else {
        verify_bundle_metadata_integrity(bundle_input_path)?;
        Ok(())
    }
}

fn verify_bundle_metadata_integrity(bundle_path: &Path) -> Result<CreateBundleAuditMetadata> {
    if !bundle_path.exists() {
        bail!("bundle path does not exist: {}", bundle_path.display());
    }
    if !bundle_path.is_file() {
        bail!("bundle path is not a file: {}", bundle_path.display());
    }

    let metadata_path = caudit_sidecar_path(bundle_path);
    let metadata = load_bundle_metadata_from_path(&metadata_path)?;
    if metadata.schema_version != "1" {
        bail!(
            "unsupported caudit schema version: '{}'",
            metadata.schema_version
        );
    }

    let bundle_bytes = fs::read(bundle_path)?;
    let actual_bundle_size = bundle_bytes.len() as u64;
    if metadata.bundle_size_bytes != actual_bundle_size {
        bail!(
            "bundle size mismatch: metadata={}, actual={}",
            metadata.bundle_size_bytes,
            actual_bundle_size
        );
    }

    let actual_bundle_sha256 = sha256_hex(&bundle_bytes)?;
    if metadata.bundle_sha256 != actual_bundle_sha256 {
        bail!(
            "bundle sha256 mismatch: metadata={}, actual={}",
            metadata.bundle_sha256,
            actual_bundle_sha256
        );
    }

    let inspection = inspect_bundle(bundle_path)?;
    let expected_bundle_header_version = bundle_version_code(inspection.version).to_string();
    if metadata.bundle_header_version != expected_bundle_header_version {
        bail!(
            "bundle header version mismatch: metadata={}, actual={}",
            metadata.bundle_header_version,
            expected_bundle_header_version
        );
    }

    let expected_prerequisites: Vec<String> = inspection
        .prerequisites
        .iter()
        .map(|oid| oid.to_string())
        .collect();
    if metadata.prerequisites != expected_prerequisites {
        bail!("bundle prerequisites mismatch between metadata and bundle header");
    }

    let expected_heads: Vec<CreateBundleAuditHead> = inspection
        .heads
        .iter()
        .map(|head| CreateBundleAuditHead {
            oid: head.oid.to_string(),
            reference: head.reference.clone(),
        })
        .collect();
    if metadata.heads != expected_heads {
        bail!("bundle heads mismatch between metadata and bundle header");
    }

    if !metadata
        .heads
        .iter()
        .any(|head| head.reference == metadata.tip_ref && head.oid == metadata.range_to_oid)
    {
        bail!("metadata tip_ref/range_to_oid must match one bundle head entry");
    }

    if let Some(patch_sidecar) = &metadata.patch_sidecar {
        if patch_sidecar.format != "unified-diff" {
            bail!(
                "unsupported patch sidecar format: '{}'",
                patch_sidecar.format
            );
        }
        let patch_path = resolve_patch_sidecar_path(&metadata_path, patch_sidecar)?;
        let patch_bytes = fs::read(&patch_path)?;
        let actual_patch_size = patch_bytes.len() as u64;
        if patch_sidecar.size_bytes != actual_patch_size {
            bail!(
                "patch sidecar size mismatch: metadata={}, actual={}",
                patch_sidecar.size_bytes,
                actual_patch_size
            );
        }

        let actual_patch_sha256 = sha256_hex(&patch_bytes)?;
        if patch_sidecar.sha256 != actual_patch_sha256 {
            bail!(
                "patch sidecar sha256 mismatch: metadata={}, actual={}",
                patch_sidecar.sha256,
                actual_patch_sha256
            );
        }
    }

    Ok(metadata)
}

fn receive_bundle(bundle_path: &Path, receiver_repo_path: &Path) -> Result<ReceiveBundleResult> {
    let inspection = inspect_bundle(bundle_path)?;
    if inspection.heads.is_empty() {
        bail!("bundle does not contain any heads to import");
    }

    let repo = git2::Repository::open(receiver_repo_path)?;
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

    for head in &inspection.heads {
        repo.find_commit(head.oid).map_err(|err| {
            anyhow!(
                "bundle head commit '{}' is not available after import: {err}",
                head.oid
            )
        })?;
    }

    for head in &inspection.heads {
        repo.reference(&head.reference, head.oid, true, "receive bundle import")?;
    }

    Ok(ReceiveBundleResult {
        bundle_version: inspection.version,
        imported_heads: inspection.heads,
    })
}

fn collect_changed_files_for_metadata(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<CreateBundleAuditChangedFile>> {
    let diff_entries = collect_diff_entries(repo, base_commit_id, tip_commit_id)?;
    Ok(diff_entries
        .into_iter()
        .map(|entry| CreateBundleAuditChangedFile {
            status: status_code(entry.status).to_string(),
            path: entry.path,
            old_path: entry.old_path,
            old_oid: entry.old_oid.map(|oid| oid.to_string()),
            new_oid: entry.new_oid.map(|oid| oid.to_string()),
            old_mode: entry.old_mode.map(|mode| format!("{mode:06o}")),
            new_mode: entry.new_mode.map(|mode| format!("{mode:06o}")),
            is_binary: entry.is_binary,
        })
        .collect())
}

fn collect_commit_chain_for_metadata(
    repo: &git2::Repository,
    from_commit_id: git2::Oid,
    to_commit_id: git2::Oid,
) -> Result<Vec<CreateBundleAuditCommit>> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;
    walk.push(to_commit_id)?;
    walk.hide(from_commit_id)?;

    let mut commit_chain = Vec::new();
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let parent_oids = commit
            .parent_ids()
            .map(|parent| parent.to_string())
            .collect();

        commit_chain.push(CreateBundleAuditCommit {
            oid: commit.id().to_string(),
            tree_oid: commit.tree_id().to_string(),
            parent_oids,
            subject: commit.summary().unwrap_or("").to_string(),
            author: signature_to_audit_signature(commit.author()),
            committer: signature_to_audit_signature(commit.committer()),
        });
    }

    Ok(commit_chain)
}

fn collect_diff_entries(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
) -> Result<Vec<DiffEntry>> {
    let base_commit = repo.find_commit(base_commit_id)?;
    let tip_commit = repo.find_commit(tip_commit_id)?;
    let base_tree = base_commit.tree()?;
    let tip_tree = tip_commit.tree()?;

    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut entries = Vec::new();
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

        let is_binary = old_file.is_binary() || new_file.is_binary();
        entries.push(DiffEntry {
            status,
            path,
            old_path,
            old_oid: oid_or_none(old_file.id()),
            new_oid: oid_or_none(new_file.id()),
            old_mode: old_file.exists().then(|| u32::from(old_file.mode())),
            new_mode: new_file.exists().then(|| u32::from(new_file.mode())),
            is_binary,
        });
    }

    // Deterministic order is required for stable audit artifacts.
    entries.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.old_path.cmp(&b.old_path))
    });
    Ok(entries)
}

fn write_patch_sidecar(
    repo: &git2::Repository,
    base_commit_id: git2::Oid,
    tip_commit_id: git2::Oid,
    bundle_path: &Path,
) -> Result<CreateBundleAuditPatchSidecar> {
    let base_commit = repo.find_commit(base_commit_id)?;
    let tip_commit = repo.find_commit(tip_commit_id)?;
    let base_tree = base_commit.tree()?;
    let tip_tree = tip_commit.tree()?;

    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tip_tree), None)?;
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    diff.find_similar(Some(&mut find_opts))?;

    let mut patch_bytes = Vec::<u8>::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        patch_bytes.extend_from_slice(line.content());
        true
    })?;

    let patch_path = patch_sidecar_path(bundle_path);
    fs::write(&patch_path, &patch_bytes)?;

    Ok(CreateBundleAuditPatchSidecar {
        path: patch_path.display().to_string(),
        format: "unified-diff".to_string(),
        size_bytes: patch_bytes.len() as u64,
        sha256: sha256_hex(&patch_bytes)?,
    })
}

fn signature_to_audit_signature(signature: git2::Signature<'_>) -> CreateBundleAuditSignature {
    let timestamp = signature.when();
    CreateBundleAuditSignature {
        name: signature.name().unwrap_or("").to_string(),
        email: signature.email().unwrap_or("").to_string(),
        time_seconds: timestamp.seconds(),
        offset_minutes: timestamp.offset_minutes(),
    }
}

fn caudit_sidecar_path(bundle_path: &Path) -> PathBuf {
    let mut sidecar = bundle_path.as_os_str().to_os_string();
    sidecar.push(".caudit.json");
    PathBuf::from(sidecar)
}

fn patch_sidecar_path(bundle_path: &Path) -> PathBuf {
    let mut sidecar = bundle_path.as_os_str().to_os_string();
    sidecar.push(".caudit.patch");
    PathBuf::from(sidecar)
}

fn bundle_archive_path(bundle_path: &Path) -> PathBuf {
    let mut archive = bundle_path.as_os_str().to_os_string();
    archive.push(".zip");
    PathBuf::from(archive)
}

fn write_zip_archive(archive_path: &Path, files: &[PathBuf]) -> Result<()> {
    let archive_file = File::create(archive_path)?;
    let mut archive = ZipWriter::new(archive_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for file_path in files {
        if !file_path.exists() {
            bail!("archive input path does not exist: {}", file_path.display());
        }
        if !file_path.is_file() {
            bail!("archive input path is not a file: {}", file_path.display());
        }

        let file_name = file_path
            .file_name()
            .ok_or_else(|| anyhow!("archive input has no file name: {}", file_path.display()))?;
        let file_name = file_name.to_string_lossy();
        archive.start_file(file_name, options)?;
        let bytes = fs::read(file_path)?;
        archive.write_all(&bytes)?;
    }

    archive.finish()?;
    Ok(())
}

fn is_zip_bundle_input_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

fn load_bundle_metadata_from_input(bundle_input_path: &Path) -> Result<CreateBundleAuditMetadata> {
    if is_zip_bundle_input_path(bundle_input_path) {
        let extracted = extract_bundle_archive(bundle_input_path)?;
        let metadata_path = caudit_sidecar_path(&extracted.bundle_path);
        return load_bundle_metadata_from_path(&metadata_path);
    }

    let metadata_path = caudit_sidecar_path(bundle_input_path);
    load_bundle_metadata_from_path(&metadata_path)
}

fn load_bundle_metadata_from_path(metadata_path: &Path) -> Result<CreateBundleAuditMetadata> {
    if !metadata_path.exists() {
        bail!(
            "bundle audit metadata path does not exist: {}",
            metadata_path.display()
        );
    }
    if !metadata_path.is_file() {
        bail!(
            "bundle audit metadata path is not a file: {}",
            metadata_path.display()
        );
    }

    let metadata_bytes = fs::read(metadata_path)?;
    let metadata: CreateBundleAuditMetadata = serde_json::from_slice(&metadata_bytes)?;
    Ok(metadata)
}

fn parse_optional_oid(value: Option<&str>) -> Result<Option<git2::Oid>> {
    value
        .map(git2::Oid::from_str)
        .transpose()
        .map_err(Into::into)
}

fn parse_status_code(status: &str) -> Result<ChangeStatus> {
    match status {
        "A" => Ok(ChangeStatus::Added),
        "M" => Ok(ChangeStatus::Modified),
        "D" => Ok(ChangeStatus::Deleted),
        "R" => Ok(ChangeStatus::Renamed),
        "C" => Ok(ChangeStatus::Copied),
        "T" => Ok(ChangeStatus::TypeChanged),
        _ => bail!("unsupported change status code in metadata: '{status}'"),
    }
}

fn resolve_patch_sidecar_path(
    metadata_path: &Path,
    patch_sidecar: &CreateBundleAuditPatchSidecar,
) -> Result<PathBuf> {
    let explicit_path = PathBuf::from(&patch_sidecar.path);
    if explicit_path.exists() && explicit_path.is_file() {
        return Ok(explicit_path);
    }

    let file_name = Path::new(&patch_sidecar.path)
        .file_name()
        .ok_or_else(|| anyhow!("patch sidecar path in metadata has no file name"))?;
    let sibling_path = metadata_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(file_name);
    if sibling_path.exists() && sibling_path.is_file() {
        return Ok(sibling_path);
    }

    bail!(
        "patch sidecar path does not exist: {} (or sibling {})",
        explicit_path.display(),
        sibling_path.display()
    );
}

#[derive(Debug)]
struct ExtractedBundleArchive {
    temp_dir: PathBuf,
    bundle_path: PathBuf,
}

impl Drop for ExtractedBundleArchive {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn extract_bundle_archive(archive_path: &Path) -> Result<ExtractedBundleArchive> {
    if !archive_path.exists() {
        bail!(
            "bundle archive path does not exist: {}",
            archive_path.display()
        );
    }
    if !archive_path.is_file() {
        bail!(
            "bundle archive path is not a file: {}",
            archive_path.display()
        );
    }

    let archive_file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)?;

    let temp_dir = std::env::temp_dir().join(format!(
        "git-sync-audit-extract-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| anyhow!("system clock is before unix epoch"))?
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)?;

    let mut bundle_paths = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name().ends_with('/') {
            continue;
        }

        let file_name = Path::new(entry.name())
            .file_name()
            .ok_or_else(|| anyhow!("zip entry has no file name: '{}'", entry.name()))?;
        let output_path = temp_dir.join(file_name);

        let mut output_file = File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output_file)?;

        if output_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bundle"))
        {
            bundle_paths.push(output_path);
        }
    }

    if bundle_paths.is_empty() {
        bail!(
            "bundle archive does not contain a .bundle entry: {}",
            archive_path.display()
        );
    }
    if bundle_paths.len() > 1 {
        bail!(
            "bundle archive must contain exactly one .bundle entry, found {}",
            bundle_paths.len()
        );
    }

    Ok(ExtractedBundleArchive {
        temp_dir,
        bundle_path: bundle_paths.remove(0),
    })
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(anyhow!(
            "failed to remove temporary artifact '{}': {err}",
            path.display()
        )),
    }
}

fn current_unix_timestamp_secs() -> Result<u64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| anyhow!("system clock is before unix epoch"))?;
    Ok(duration.as_secs())
}

fn current_username() -> String {
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

fn current_hostname() -> String {
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

fn sha256_hex(bytes: &[u8]) -> Result<String> {
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

fn bundle_version_code(version: BundleVersion) -> &'static str {
    match version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
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
