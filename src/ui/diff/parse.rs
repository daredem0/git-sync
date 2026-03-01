// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Diff processing helpers for parse behavior.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use crate::ui::types::PatchLineKind;

/// Computes displayed old/new line number columns for a rendered patch line.
///
/// The mutable counters are updated according to diff semantics
/// (added/deleted/context).
pub(crate) fn line_number_columns(
    kind: PatchLineKind,
    old_line: &mut Option<usize>,
    new_line: &mut Option<usize>,
) -> (String, String) {
    match kind {
        PatchLineKind::Added => {
            let display = new_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = new_line.as_mut() {
                *value += 1;
            }
            ("".to_string(), display)
        }
        PatchLineKind::Deleted => {
            let display = old_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = old_line.as_mut() {
                *value += 1;
            }
            (display, "".to_string())
        }
        PatchLineKind::Context => {
            let old_display = old_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            let new_display = new_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = old_line.as_mut() {
                *value += 1;
            }
            if let Some(value) = new_line.as_mut() {
                *value += 1;
            }
            (old_display, new_display)
        }
        _ => ("".to_string(), "".to_string()),
    }
}

/// Classifies a raw unified-diff line into semantic render groups.
pub(crate) fn classify_patch_line(line: &str) -> PatchLineKind {
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
    {
        return PatchLineKind::Header;
    }

    if line.starts_with("@@") {
        return PatchLineKind::Hunk;
    }

    if line.starts_with('+') && !line.starts_with("+++") {
        return PatchLineKind::Added;
    }

    if line.starts_with('-') && !line.starts_with("---") {
        return PatchLineKind::Deleted;
    }

    if line.starts_with(' ') {
        return PatchLineKind::Context;
    }

    PatchLineKind::Other
}

/// Parses a unified-diff hunk header and returns `(old_start, new_start)`.
pub(crate) fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_part, rest) = rest.split_once(" +")?;
    let (new_part, _) = rest.split_once(" @@")?;

    let old_start = old_part.split(',').next()?.parse::<usize>().ok()?;
    let new_start = new_part.split(',').next()?.parse::<usize>().ok()?;

    Some((old_start, new_start))
}
