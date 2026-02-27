//! Git-layer manifest functionality.

use crate::git::ChangedFile;
use crate::git::util::{oid_to_str, status_code};
use anyhow::Result;
use serde::Serialize;

/// Renders changed files as a tab-separated manifest table.
///
/// The first row is a header and each subsequent row uses stable column order:
/// `STATUS`, `PATH`, `OLD_PATH`, `OLD_OID`, `NEW_OID`.
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

/// Renders changed files as pretty-printed JSON.
///
/// # Errors
///
/// Returns an error if serialization fails.
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

#[derive(Debug, Clone, Serialize)]
struct JsonChangedFile {
    status: String,
    path: String,
    old_path: Option<String>,
    old_oid: Option<String>,
    new_oid: Option<String>,
}
