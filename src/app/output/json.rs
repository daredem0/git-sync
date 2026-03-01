// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Output formatting logic for json views.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use anyhow::Result;

/// Renders non-interactive payload audit document as pretty-printed JSON.
pub fn render_payload_audit_json(document: &crate::git::PayloadAuditDocument) -> Result<String> {
    Ok(serde_json::to_string_pretty(document)?)
}
