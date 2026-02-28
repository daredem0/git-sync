//! JSON output rendering helpers.

use anyhow::Result;

/// Renders non-interactive payload audit document as pretty-printed JSON.
pub fn render_payload_audit_json(document: &crate::git::PayloadAuditDocument) -> Result<String> {
    Ok(serde_json::to_string_pretty(document)?)
}
