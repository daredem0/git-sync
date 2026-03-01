// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI formatting helpers for human-readable output.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::types::{DryRunLine, StatusLine};
use crate::git::CommitAuditIdentity;

/// Renders metadata verification status into a user-facing line.
pub(crate) fn render_status_line(status: &StatusLine) -> String {
    match status {
        StatusLine::Ok => "OK".to_string(),
        StatusLine::Failed(err) => format!("FAILED ({err})"),
    }
}

/// Renders dry-run applicability status into a user-facing line.
pub(crate) fn render_dry_run_status(status: &DryRunLine) -> String {
    match status {
        DryRunLine::Ok(result) => {
            if result.can_apply_without_conflicts {
                "bundle can be applied without conflicts".to_string()
            } else {
                "bundle cannot be applied cleanly".to_string()
            }
        }
        DryRunLine::Failed(err) => format!("FAILED ({err})"),
    }
}

/// Flattens a potentially multi-line error into a single printable line.
pub(crate) fn single_line_error(err: &anyhow::Error) -> String {
    err.to_string().replace('\n', " ")
}

/// Returns `true` when an error indicates diff text is unavailable for non-text files.
pub(crate) fn is_non_text_patch_unavailable_error(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("textual diff unavailable for non-text path")
}

/// Formats commit identity as `Name <email>`.
pub(crate) fn format_identity(identity: &CommitAuditIdentity) -> String {
    format!("{} <{}>", identity.name, identity.email)
}

/// Formats a git timestamp as `seconds (UTC±hh:mm)`.
pub(crate) fn format_git_timestamp(seconds: i64, offset_minutes: i32) -> String {
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let absolute = offset_minutes.abs();
    let hours = absolute / 60;
    let minutes = absolute % 60;
    format!("{seconds} (UTC{sign}{hours:02}:{minutes:02})")
}
