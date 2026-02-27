use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "git-sync-audit-ui-{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

pub(crate) fn commit_from_files(
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
            .expect("must insert file entry");
    }
    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must find written tree");
    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("UI Test", "ui-test@example.com")
        .expect("must create commit signature");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}

pub(crate) fn commit_from_entries(
    repo: &git2::Repository,
    message: &str,
    entries: &[(&str, &[u8], i32)],
    parent_oids: &[git2::Oid],
) -> git2::Oid {
    let mut builder = repo.treebuilder(None).expect("must create tree builder");
    for (path, content, mode) in entries {
        let blob_id = repo.blob(content).expect("must create blob object");
        builder
            .insert(*path, blob_id, *mode)
            .expect("must insert tree entry");
    }
    let tree_id = builder.write().expect("must write tree");
    let tree = repo.find_tree(tree_id).expect("must find written tree");
    let parent_commits: Vec<git2::Commit<'_>> = parent_oids
        .iter()
        .map(|oid| repo.find_commit(*oid).expect("must resolve parent"))
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let sig = git2::Signature::now("UI Test", "ui-test@example.com")
        .expect("must create commit signature");
    repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
        .expect("must create commit")
}
