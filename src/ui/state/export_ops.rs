// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI state transition logic for export ops operations.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::app::output;
use crate::git::{self, PayloadAuditLedgerMode, PayloadAuditObjectDetailMode, PayloadResolveMode};
use crate::ui::format::single_line_error;
use crate::ui::model::derive_repo_name_from_repo;
use crate::ui::types::{AppState, AuditModel, ExportNotice};
use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

impl AppState {
    /// Exports payload-audit JSON evidence and surfaces result in a dismissable overlay.
    pub(crate) fn export_payload_audit_json_full(&mut self, model: &AuditModel) {
        self.export_payload_audit_json_with_detail_mode(model, PayloadAuditObjectDetailMode::Full);
    }

    /// Exports payload-audit JSON evidence in light mode and surfaces result in a dismissable overlay.
    pub(crate) fn export_payload_audit_json_light(&mut self, model: &AuditModel) {
        self.export_payload_audit_json_with_detail_mode(model, PayloadAuditObjectDetailMode::Light);
    }

    fn export_payload_audit_json_with_detail_mode(
        &mut self,
        model: &AuditModel,
        detail_mode: PayloadAuditObjectDetailMode,
    ) {
        self.close_help();
        match write_payload_audit_export(model, detail_mode) {
            Ok(notice) => {
                self.export_notice = Some(notice);
                self.action_message = None;
            }
            Err(err) => {
                self.export_notice = None;
                self.action_message = Some(format!(
                    "failed to export paudit: {}",
                    single_line_error(&err)
                ));
            }
        }
    }
}

fn write_payload_audit_export(
    model: &AuditModel,
    detail_mode: PayloadAuditObjectDetailMode,
) -> Result<ExportNotice> {
    let now = OffsetDateTime::now_utc();
    let payload_document = match detail_mode {
        PayloadAuditObjectDetailMode::Full => {
            git::build_payload_audit_document_for_bundle_input_with_options(
                &model.bundle_path,
                &model.repo_path,
                PayloadAuditLedgerMode::Summary,
                PayloadResolveMode::PackOnly,
            )?
        }
        PayloadAuditObjectDetailMode::Light => {
            git::build_payload_audit_document_for_bundle_input_with_options_and_detail_mode(
                &model.bundle_path,
                &model.repo_path,
                PayloadAuditLedgerMode::Summary,
                PayloadAuditObjectDetailMode::Light,
                PayloadResolveMode::PackOnly,
            )?
        }
    };
    let payload_json = output::render_payload_audit_json(&payload_document)?;

    let export_path = next_available_export_path(export_path_for_model(model, now, detail_mode)?);
    fs::write(&export_path, payload_json.as_bytes())?;
    Ok(ExportNotice {
        path: export_path,
        exported_at_human_utc: human_utc_timestamp(now),
    })
}

fn export_path_for_model(
    model: &AuditModel,
    now: OffsetDateTime,
    detail_mode: PayloadAuditObjectDetailMode,
) -> Result<PathBuf> {
    let timestamp = iso_utc_timestamp_basic(now);
    let repo_token = repo_name_token(model);
    let bundle_token = bundle_name_token(&model.bundle_path);
    let mode_token = payload_object_detail_mode_token(detail_mode);
    let file_name = format!("{timestamp}_{repo_token}_{bundle_token}_{mode_token}.paudit.json");
    let output_dir = std::env::current_dir()
        .map_err(|err| anyhow!("unable to resolve current working directory: {err}"))?;
    Ok(output_dir.join(file_name))
}

fn repo_name_token(model: &AuditModel) -> String {
    let repo_name = derive_repo_name_from_repo(&model.repo_path)
        .or_else(|| {
            model
                .repo_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "repo".to_string());
    sanitize_file_name_token(&repo_name)
}

fn bundle_name_token(bundle_path: &Path) -> String {
    let bundle_name = bundle_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    let stem = bundle_name.strip_suffix(".zip").unwrap_or(bundle_name);
    sanitize_file_name_token(stem)
}

fn sanitize_file_name_token(input: &str) -> String {
    let sanitized = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn iso_utc_timestamp_basic(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn human_utc_timestamp(now: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn payload_object_detail_mode_token(mode: PayloadAuditObjectDetailMode) -> &'static str {
    match mode {
        PayloadAuditObjectDetailMode::Light => "light",
        PayloadAuditObjectDetailMode::Full => "full",
    }
}

fn next_available_export_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let mut attempt = 2usize;
    loop {
        let candidate = with_collision_suffix(&path, attempt)
            .unwrap_or_else(|_| path.with_file_name(format!("paudit-{attempt}.json")));
        if !candidate.exists() {
            return candidate;
        }
        attempt += 1;
    }
}

fn with_collision_suffix(path: &Path, attempt: usize) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("export path has no UTF-8 file name: {}", path.display()))?;
    let suffixed = if let Some(prefix) = file_name.strip_suffix(".paudit.json") {
        format!("{prefix}-{attempt}.paudit.json")
    } else if let Some((stem, extension)) = file_name.rsplit_once('.') {
        format!("{stem}-{attempt}.{extension}")
    } else {
        format!("{file_name}-{attempt}")
    };

    Ok(path.with_file_name(suffixed))
}

#[cfg(test)]
mod tests;
