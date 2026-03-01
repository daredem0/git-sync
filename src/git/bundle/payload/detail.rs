// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload audit module for detail operations.
//!
//! Part of the authoritative git-domain layer for bundle, metadata, and payload proof logic.
//! Prioritizes deterministic behavior and fail-closed validation in safety-critical paths.

use crate::git::types::{MaterializedObjectData, PayloadObjectDetail, PayloadObjectKind};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

use super::context::parse_tree_entries;

struct ObjectDetailLines {
    lines: Vec<String>,
    is_text_blob: bool,
    text_line_count: Option<usize>,
}

/// Collects payload detail lines from verifier-owned materialized object store.
pub(super) fn collect_payload_object_detail_from_store(
    store: &HashMap<git2::Oid, MaterializedObjectData>,
    blob_paths_by_oid: &HashMap<git2::Oid, Vec<String>>,
    object_id: git2::Oid,
) -> Result<PayloadObjectDetail> {
    let stored = store.get(&object_id).ok_or_else(|| {
        anyhow!(
            "payload object {} is not available in materialized store",
            object_id
        )
    })?;
    let kind = stored.kind;
    let size_bytes = stored.size_bytes;
    let detail_lines = object_detail_lines_from_materialized(stored)?;
    let blob_paths = if kind == PayloadObjectKind::Blob {
        blob_paths_by_oid
            .get(&object_id)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let syntax_path_hint = if detail_lines.is_text_blob {
        blob_paths
            .first()
            .cloned()
            .or_else(|| Some("blob.txt".to_string()))
    } else {
        None
    };

    Ok(PayloadObjectDetail {
        oid: object_id,
        kind,
        size_bytes,
        syntax_path_hint,
        blob_paths,
        text_line_count: detail_lines.text_line_count,
        lines: detail_lines.lines,
    })
}

/// Renders object-specific detail lines for payload drill-down/preview view from materialized bytes.
fn object_detail_lines_from_materialized(
    object: &MaterializedObjectData,
) -> Result<ObjectDetailLines> {
    match object.kind {
        PayloadObjectKind::Commit => {
            let text = String::from_utf8_lossy(&object.content_bytes);
            let mut lines = vec![format!("commit {}", object.oid)];
            let mut in_message = false;
            for line in text.lines() {
                if !in_message {
                    if line.is_empty() {
                        in_message = true;
                        lines.push(String::new());
                        continue;
                    }
                    if line.starts_with("tree ")
                        || line.starts_with("parent ")
                        || line.starts_with("author ")
                        || line.starts_with("committer ")
                    {
                        lines.push(line.to_string());
                    }
                } else {
                    lines.push(line.to_string());
                }
            }
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Tree => {
            let mut lines = vec![format!("tree {}", object.oid), String::new()];
            let entries = parse_tree_entries(&object.content_bytes)?;
            for entry in entries {
                lines.push(format!(
                    "{:>6} {:<7} {} {}",
                    entry.mode,
                    payload_kind_code(entry.kind),
                    entry.oid,
                    entry.name
                ));
            }
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Blob => render_blob_detail_lines(object),
        PayloadObjectKind::Tag => {
            let text = String::from_utf8_lossy(&object.content_bytes);
            let mut lines = vec![format!("tag {}", object.oid), String::new()];
            lines.extend(text.lines().map(str::to_string));
            Ok(ObjectDetailLines {
                lines,
                is_text_blob: false,
                text_line_count: None,
            })
        }
        PayloadObjectKind::Unknown => Ok(ObjectDetailLines {
            lines: vec![
                format!("object {}", object.oid),
                "unsupported object type for detail rendering".to_string(),
            ],
            is_text_blob: false,
            text_line_count: None,
        }),
    }
}

/// Renders blob detail lines including preview policy when content is truncated.
fn render_blob_detail_lines(object: &MaterializedObjectData) -> Result<ObjectDetailLines> {
    if let Ok(text) = std::str::from_utf8(&object.content_bytes) {
        let text_line_count = if object.content_truncated {
            None
        } else {
            Some(text.lines().count())
        };
        let mut lines = vec![
            format!("text blob {}", object.oid),
            format!("size: {} bytes", object.size_bytes),
            if object.content_truncated {
                format!(
                    "preview only: {} / {} bytes",
                    object.content_bytes.len(),
                    object.size_bytes
                )
            } else {
                format!("text lines: {}", text_line_count.unwrap_or(0))
            },
            String::new(),
        ];
        lines.extend(text.lines().map(str::to_string));
        if object.content_truncated {
            lines.push(String::new());
            lines.push("full content not retained in-memory (large blob preview mode)".to_string());
        }
        return Ok(ObjectDetailLines {
            lines,
            is_text_blob: true,
            text_line_count,
        });
    }

    let preview_len = object.content_bytes.len().min(256);
    let mut lines = vec![
        format!("binary blob {}", object.oid),
        format!("size: {} bytes", object.size_bytes),
        if object.content_truncated {
            format!(
                "preview only: {} / {} bytes",
                object.content_bytes.len(),
                object.size_bytes
            )
        } else {
            format!("hex preview (first {preview_len} bytes):")
        },
        String::new(),
    ];
    for chunk in object.content_bytes[..preview_len].chunks(16) {
        lines.push(
            chunk
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    Ok(ObjectDetailLines {
        lines,
        is_text_blob: false,
        text_line_count: None,
    })
}

/// Returns stable string code for payload object kinds.
fn payload_kind_code(kind: PayloadObjectKind) -> &'static str {
    match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::collect_payload_object_detail_from_store;
    use crate::git::types::{MaterializedObjectData, PayloadObjectKind};
    use std::collections::HashMap;

    fn oid(value: &str) -> git2::Oid {
        git2::Oid::from_str(value).expect("must parse test oid")
    }

    fn object(
        kind: PayloadObjectKind,
        oid: git2::Oid,
        bytes: &[u8],
        truncated: bool,
    ) -> MaterializedObjectData {
        MaterializedObjectData {
            oid,
            kind,
            size_bytes: if truncated {
                bytes.len() + 16
            } else {
                bytes.len()
            },
            content_bytes: bytes.to_vec(),
            content_truncated: truncated,
        }
    }

    #[test]
    fn collect_payload_object_detail_returns_error_for_unknown_object_id() {
        let store = HashMap::new();
        let paths = HashMap::new();
        let missing = oid("ffffffffffffffffffffffffffffffffffffffff");
        let error = collect_payload_object_detail_from_store(&store, &paths, missing)
            .expect_err("missing payload object id should fail detail lookup");
        assert!(
            error
                .to_string()
                .contains("is not available in materialized store"),
            "error should explain missing materialized object"
        );
    }

    #[test]
    fn collect_payload_object_detail_renders_commit_headers_and_message() {
        let commit_oid = oid("1111111111111111111111111111111111111111");
        let tree_oid = oid("2222222222222222222222222222222222222222");
        let parent_oid = oid("3333333333333333333333333333333333333333");
        let commit = format!(
            "tree {tree_oid}\nparent {parent_oid}\nauthor A <a@example.com> 1 +0000\ncommitter C <c@example.com> 2 +0000\n\nsubject\nbody line\n"
        );

        let mut store = HashMap::new();
        store.insert(
            commit_oid,
            object(
                PayloadObjectKind::Commit,
                commit_oid,
                commit.as_bytes(),
                false,
            ),
        );

        let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), commit_oid)
            .expect("commit detail rendering should succeed");
        assert_eq!(detail.lines[0], format!("commit {commit_oid}"));
        assert!(detail.lines.iter().any(|line| line.starts_with("tree ")));
        assert!(detail.lines.iter().any(|line| line.starts_with("author ")));
        assert!(detail.lines.iter().any(|line| line == "subject"));
        assert!(detail.lines.iter().any(|line| line == "body line"));
    }

    #[test]
    fn collect_payload_object_detail_renders_tree_entries() {
        let tree_oid = oid("4444444444444444444444444444444444444444");
        let blob_oid = oid("5555555555555555555555555555555555555555");
        let mut tree_bytes = Vec::new();
        tree_bytes.extend_from_slice(b"100644 file.txt\0");
        tree_bytes.extend_from_slice(blob_oid.as_bytes());

        let mut store = HashMap::new();
        store.insert(
            tree_oid,
            object(PayloadObjectKind::Tree, tree_oid, &tree_bytes, false),
        );

        let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), tree_oid)
            .expect("tree detail rendering should succeed");
        assert_eq!(detail.lines[0], format!("tree {tree_oid}"));
        assert!(
            detail.lines.iter().any(|line| line.contains("100644")
                && line.contains("blob")
                && line.contains("file.txt")),
            "tree detail should include formatted entry row"
        );
    }

    #[test]
    fn collect_payload_object_detail_rejects_malformed_tree_content() {
        let tree_oid = oid("6666666666666666666666666666666666666666");
        let malformed = b"100644-without-space-or-nul";
        let mut store = HashMap::new();
        store.insert(
            tree_oid,
            object(PayloadObjectKind::Tree, tree_oid, malformed, false),
        );

        let error = collect_payload_object_detail_from_store(&store, &HashMap::new(), tree_oid)
            .expect_err("malformed tree bytes should fail");
        assert!(
            error.to_string().contains("tree entry"),
            "tree parse failures should mention tree entry decoding"
        );
    }

    #[test]
    fn collect_payload_object_detail_renders_text_blob_with_line_count_and_path_hint() {
        let blob_oid = oid("7777777777777777777777777777777777777777");
        let mut store = HashMap::new();
        store.insert(
            blob_oid,
            object(
                PayloadObjectKind::Blob,
                blob_oid,
                b"line one\nline two\n",
                false,
            ),
        );
        let mut paths = HashMap::new();
        paths.insert(blob_oid, vec!["src/lib.rs".to_string()]);

        let detail = collect_payload_object_detail_from_store(&store, &paths, blob_oid)
            .expect("text blob detail rendering should succeed");
        assert_eq!(detail.text_line_count, Some(2));
        assert_eq!(detail.syntax_path_hint.as_deref(), Some("src/lib.rs"));
        assert!(detail.lines.iter().any(|line| line == "text lines: 2"));
    }

    #[test]
    fn collect_payload_object_detail_uses_default_path_hint_for_text_blob_without_paths() {
        let blob_oid = oid("8888888888888888888888888888888888888888");
        let mut store = HashMap::new();
        store.insert(
            blob_oid,
            object(PayloadObjectKind::Blob, blob_oid, b"single line\n", false),
        );

        let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
            .expect("text blob detail rendering should succeed");
        assert_eq!(detail.syntax_path_hint.as_deref(), Some("blob.txt"));
    }

    #[test]
    fn collect_payload_object_detail_marks_truncated_text_blob_preview() {
        let blob_oid = oid("9999999999999999999999999999999999999999");
        let mut store = HashMap::new();
        store.insert(
            blob_oid,
            object(PayloadObjectKind::Blob, blob_oid, b"head\npreview\n", true),
        );

        let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
            .expect("truncated text blob detail should succeed");
        assert_eq!(
            detail.text_line_count, None,
            "truncated text preview should not claim full text-line count"
        );
        assert!(
            detail
                .lines
                .iter()
                .any(|line| line.contains("preview only:")),
            "truncated text preview should include preview-only marker"
        );
        assert!(
            detail
                .lines
                .iter()
                .any(|line| line.contains("full content not retained in-memory")),
            "truncated text preview should explain retention policy"
        );
    }

    #[test]
    fn collect_payload_object_detail_renders_binary_blob_hex_preview() {
        let blob_oid = oid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let mut store = HashMap::new();
        store.insert(
            blob_oid,
            object(
                PayloadObjectKind::Blob,
                blob_oid,
                &[0xff, 0x00, 0x41, 0x7f],
                false,
            ),
        );

        let detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), blob_oid)
            .expect("binary blob detail rendering should succeed");
        assert!(
            detail.lines[0].starts_with("binary blob"),
            "binary blob detail header should identify binary blob"
        );
        assert!(
            detail.lines.iter().any(|line| line.contains("hex preview")),
            "binary blob detail should include hex-preview label"
        );
    }

    #[test]
    fn collect_payload_object_detail_renders_tag_and_unknown_objects() {
        let tag_oid = oid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let unknown_oid = oid("cccccccccccccccccccccccccccccccccccccccc");
        let mut store = HashMap::new();
        store.insert(
            tag_oid,
            object(
                PayloadObjectKind::Tag,
                tag_oid,
                b"object deadbeef\ntype commit\ntag v1\n\nmessage\n",
                false,
            ),
        );
        store.insert(
            unknown_oid,
            object(PayloadObjectKind::Unknown, unknown_oid, b"", false),
        );

        let tag_detail = collect_payload_object_detail_from_store(&store, &HashMap::new(), tag_oid)
            .expect("tag detail rendering should succeed");
        assert_eq!(tag_detail.lines[0], format!("tag {tag_oid}"));
        assert!(tag_detail.lines.iter().any(|line| line == "tag v1"));

        let unknown_detail =
            collect_payload_object_detail_from_store(&store, &HashMap::new(), unknown_oid)
                .expect("unknown-kind detail rendering should succeed");
        assert!(
            unknown_detail
                .lines
                .iter()
                .any(|line| line.contains("unsupported object type")),
            "unknown-kind details should emit unsupported-type explanation"
        );
    }
}
