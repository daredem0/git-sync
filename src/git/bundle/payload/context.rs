//! Reachability/context derivation from materialized payload objects.

use crate::git::types::{
    BundleHead, MaterializedObjectData, MaterializedObjectIndex, PayloadObjectEntry,
    PayloadObjectKind,
};
use anyhow::{Result, anyhow, bail};
use std::collections::{HashMap, HashSet};

const BLOB_PATH_SCAN_LIMIT: usize = 12;

#[derive(Debug, Clone)]
pub(super) struct PayloadObjectContext {
    pub(super) head_index: usize,
    pub(super) commit_order: usize,
    pub(super) path: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedTreeEntry {
    pub(super) mode: String,
    pub(super) name: String,
    pub(super) oid: git2::Oid,
    pub(super) kind: PayloadObjectKind,
}

/// Builds payload object rows from the deduplicated materialized index.
pub(super) fn collect_payload_objects_from_materialized_index(
    materialized_index: &MaterializedObjectIndex,
    reachable: &HashSet<git2::Oid>,
    context_map: &HashMap<git2::Oid, PayloadObjectContext>,
) -> Vec<PayloadObjectEntry> {
    let mut objects = Vec::new();
    for row in &materialized_index.objects {
        let oid = row.oid;
        let context = context_map.get(&oid);
        objects.push(PayloadObjectEntry {
            oid,
            kind: row.kind,
            size_bytes: row.size_bytes,
            reachable_from_heads: reachable.contains(&oid),
            context_head_index: context.map(|value| value.head_index),
            context_commit_order: context.map(|value| value.commit_order),
            context_path: context.and_then(|value| value.path.clone()),
        });
    }

    objects.sort_by(|left, right| {
        payload_kind_rank(left.kind)
            .cmp(&payload_kind_rank(right.kind))
            .then_with(|| left.oid.cmp(&right.oid))
    });
    objects
}

/// Collects reachability/context/blob-path metadata directly from materialized object bytes.
pub(super) fn collect_reachability_context_from_materialized(
    heads: &[BundleHead],
    store_by_oid: &HashMap<git2::Oid, MaterializedObjectData>,
) -> (
    HashSet<git2::Oid>,
    HashMap<git2::Oid, PayloadObjectContext>,
    HashMap<git2::Oid, Vec<String>>,
) {
    let mut reachable = HashSet::<git2::Oid>::new();
    let mut context = HashMap::<git2::Oid, PayloadObjectContext>::new();
    let mut blob_paths = HashMap::<git2::Oid, Vec<String>>::new();
    let mut seen_commits = HashSet::<git2::Oid>::new();
    let mut seen_trees = HashSet::<git2::Oid>::new();

    for (head_index, head) in heads.iter().enumerate() {
        let mut commit_order = 0usize;
        let mut stack = vec![head.oid];
        while let Some(commit_id) = stack.pop() {
            if !seen_commits.insert(commit_id) {
                continue;
            }
            let Some(commit_data) = store_by_oid.get(&commit_id) else {
                continue;
            };
            if commit_data.kind != PayloadObjectKind::Commit {
                continue;
            }
            commit_order += 1;
            reachable.insert(commit_id);
            context
                .entry(commit_id)
                .or_insert_with(|| PayloadObjectContext {
                    head_index,
                    commit_order,
                    path: None,
                });

            let (tree_oid, parents) = parse_commit_tree_and_parents(&commit_data.content_bytes);
            if let Some(tree_oid) = tree_oid {
                walk_tree_from_materialized(
                    store_by_oid,
                    tree_oid,
                    "",
                    head_index,
                    commit_order,
                    &mut reachable,
                    &mut context,
                    &mut blob_paths,
                    &mut seen_trees,
                );
            }
            for parent_id in parents.into_iter().rev() {
                if !seen_commits.contains(&parent_id) {
                    stack.push(parent_id);
                }
            }
        }
    }

    for paths in blob_paths.values_mut() {
        paths.sort();
        paths.dedup();
        paths.truncate(BLOB_PATH_SCAN_LIMIT);
    }

    (reachable, context, blob_paths)
}

/// Parses raw tree object bytes into structured entries.
pub(super) fn parse_tree_entries(content: &[u8]) -> Result<Vec<ParsedTreeEntry>> {
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    while cursor < content.len() {
        let mode_end = content[cursor..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|value| cursor + value)
            .ok_or_else(|| anyhow!("tree entry mode is truncated"))?;
        let mode = std::str::from_utf8(&content[cursor..mode_end])
            .map_err(|_| anyhow!("tree entry mode is non-utf8"))?
            .to_string();
        cursor = mode_end + 1;

        let name_end = content[cursor..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|value| cursor + value)
            .ok_or_else(|| anyhow!("tree entry name is truncated"))?;
        let name = String::from_utf8_lossy(&content[cursor..name_end]).to_string();
        cursor = name_end + 1;
        ensure_remaining(content, cursor, 20, "tree entry object id")?;
        let oid = git2::Oid::from_bytes(&content[cursor..cursor + 20])?;
        cursor += 20;

        entries.push(ParsedTreeEntry {
            mode: mode.clone(),
            name,
            oid,
            kind: tree_entry_kind_from_mode(&mode),
        });
    }
    Ok(entries)
}

/// Recursively traverses materialized tree bytes and records context/path metadata.
fn walk_tree_from_materialized(
    store_by_oid: &HashMap<git2::Oid, MaterializedObjectData>,
    tree_oid: git2::Oid,
    prefix: &str,
    head_index: usize,
    commit_order: usize,
    reachable: &mut HashSet<git2::Oid>,
    context: &mut HashMap<git2::Oid, PayloadObjectContext>,
    blob_paths: &mut HashMap<git2::Oid, Vec<String>>,
    seen_trees: &mut HashSet<git2::Oid>,
) {
    if !seen_trees.insert(tree_oid) {
        return;
    }
    let Some(tree_data) = store_by_oid.get(&tree_oid) else {
        return;
    };
    if tree_data.kind != PayloadObjectKind::Tree {
        return;
    }
    reachable.insert(tree_oid);
    context
        .entry(tree_oid)
        .or_insert_with(|| PayloadObjectContext {
            head_index,
            commit_order,
            path: if prefix.is_empty() {
                None
            } else {
                Some(prefix.to_string())
            },
        });

    let Ok(entries) = parse_tree_entries(&tree_data.content_bytes) else {
        return;
    };
    for entry in entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        reachable.insert(entry.oid);
        context
            .entry(entry.oid)
            .or_insert_with(|| PayloadObjectContext {
                head_index,
                commit_order,
                path: Some(path.clone()),
            });
        if entry.kind == PayloadObjectKind::Tree {
            walk_tree_from_materialized(
                store_by_oid,
                entry.oid,
                &path,
                head_index,
                commit_order,
                reachable,
                context,
                blob_paths,
                seen_trees,
            );
        } else if entry.kind == PayloadObjectKind::Blob {
            let paths = blob_paths.entry(entry.oid).or_default();
            if paths.len() < BLOB_PATH_SCAN_LIMIT {
                paths.push(path);
            }
        }
    }
}

/// Parses commit bytes for tree and parent object ids.
fn parse_commit_tree_and_parents(content: &[u8]) -> (Option<git2::Oid>, Vec<git2::Oid>) {
    let mut tree = None;
    let mut parents = Vec::new();
    let text = String::from_utf8_lossy(content);
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("tree ") {
            tree = git2::Oid::from_str(value.trim()).ok();
        } else if let Some(value) = line.strip_prefix("parent ")
            && let Ok(parent) = git2::Oid::from_str(value.trim())
        {
            parents.push(parent);
        }
    }
    (tree, parents)
}

/// Returns stable display rank for object-kind ordering.
fn payload_kind_rank(kind: PayloadObjectKind) -> u8 {
    match kind {
        PayloadObjectKind::Commit => 0,
        PayloadObjectKind::Tree => 1,
        PayloadObjectKind::Blob => 2,
        PayloadObjectKind::Tag => 3,
        PayloadObjectKind::Unknown => 4,
    }
}

/// Ensures `len` bytes are available from `offset`.
fn ensure_remaining(bytes: &[u8], offset: usize, len: usize, context: &str) -> Result<()> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| anyhow!("{context}: offset overflow"))?;
    if end > bytes.len() {
        bail!("{context}: truncated data");
    }
    Ok(())
}

/// Maps tree-entry mode to payload object kind.
fn tree_entry_kind_from_mode(mode: &str) -> PayloadObjectKind {
    match mode {
        "40000" | "040000" => PayloadObjectKind::Tree,
        "160000" => PayloadObjectKind::Commit,
        _ => PayloadObjectKind::Blob,
    }
}
