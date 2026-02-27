use super::types::{DryRunLine, StatusLine};
use crate::git::CommitAuditIdentity;

pub(crate) fn render_status_line(status: &StatusLine) -> String {
    match status {
        StatusLine::Ok => "OK".to_string(),
        StatusLine::Failed(err) => format!("FAILED ({err})"),
    }
}

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

pub(crate) fn single_line_error(err: &anyhow::Error) -> String {
    err.to_string().replace('\n', " ")
}

pub(crate) fn is_non_text_patch_unavailable_error(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("textual diff unavailable for non-text path")
}

pub(crate) fn format_identity(identity: &CommitAuditIdentity) -> String {
    format!("{} <{}>", identity.name, identity.email)
}

pub(crate) fn format_git_timestamp(seconds: i64, offset_minutes: i32) -> String {
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let absolute = offset_minutes.abs();
    let hours = absolute / 60;
    let minutes = absolute % 60;
    format!("{seconds} (UTC{sign}{hours:02}:{minutes:02})")
}
