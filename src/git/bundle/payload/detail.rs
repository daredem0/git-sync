//! Payload object-detail rendering helpers.

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
