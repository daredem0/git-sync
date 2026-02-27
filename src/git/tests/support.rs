use super::*;
use std::path::PathBuf;

// Focus: shared fixture/build helpers reused across git unit-test modules.
pub(super) fn create_linear_bundle_fixture(
    suffix: &str,
    include_patch_sidecar: bool,
) -> (PathBuf, CreateBundleResult, git2::Oid, git2::Oid) {
    let repo_dir = temp_repo_dir(suffix);
    std::fs::create_dir_all(&repo_dir).expect("must create source repo dir");
    let repo = git2::Repository::init(&repo_dir).expect("must init source git repo");

    let base_commit_id = commit_from_files(&repo, "base commit", &[("f.txt", "base")], &[]);
    let tip_commit_id = commit_from_files(
        &repo,
        "tip commit",
        &[("f.txt", "tip"), ("new.txt", "added")],
        &[base_commit_id],
    );
    repo.reference("refs/heads/base", base_commit_id, true, "create base ref")
        .expect("must create base ref");
    repo.reference("refs/heads/tip", tip_commit_id, true, "create tip ref")
        .expect("must create tip ref");

    let bundle_path = repo_dir.join("range.bundle");
    let result = if include_patch_sidecar {
        create_bundle_with_options(
            &repo_dir,
            "refs/heads/base",
            "refs/heads/tip",
            &bundle_path,
            CreateBundleOptions {
                include_patch_sidecar: true,
            },
        )
        .expect("create_bundle_with_options should succeed")
    } else {
        create_bundle(&repo_dir, "refs/heads/base", "refs/heads/tip", &bundle_path)
            .expect("create_bundle should succeed")
    };

    (repo_dir, result, base_commit_id, tip_commit_id)
}

pub(super) fn read_json_value(path: &std::path::Path) -> serde_json::Value {
    let bytes = std::fs::read(path).expect("must read json file");
    serde_json::from_slice(&bytes).expect("json content should be valid")
}

pub(super) fn write_json_value(path: &std::path::Path, value: &serde_json::Value) {
    let serialized = serde_json::to_vec_pretty(value).expect("must serialize json value");
    std::fs::write(path, serialized).expect("must write json file");
}

pub(super) fn write_test_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    use std::io::Write as _;
    use zip::write::FileOptions;

    let file = std::fs::File::create(path).expect("must create zip file");
    let mut writer = zip::ZipWriter::new(file);
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                FileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .expect("must start zip entry");
        writer
            .write_all(bytes)
            .expect("must write zip entry content");
    }
    writer.finish().expect("must finish zip archive");
}

pub(super) fn temp_repo_dir(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-audit-{}-{}-{}",
        suffix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ))
}

pub(super) fn commit_from_files(
    repo: &git2::Repository,
    message: &str,
    files: &[(&str, &str)],
    parent_oids: &[git2::Oid],
) -> git2::Oid {
    let mut builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content) in files {
        let blob_id = repo
            .blob(content.as_bytes())
            .expect("must create blob object");
        builder
            .insert(*path, blob_id, 0o100644)
            .expect("must insert file entry in tree");
    }
    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must find tree");

    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must find parent commit"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("Test User", "test@example.com").expect("must create signature");

    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}
