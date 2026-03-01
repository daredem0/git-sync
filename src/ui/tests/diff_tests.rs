// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! UI tests for diff behavior and rendering.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

// Focus: patch parsing/classification and syntax-highlighted diff rendering behavior.

use super::super::diff::{classify_patch_line, parse_hunk_header, render_patch_with_syntax};
use super::super::types::{PatchLineKind, SyntaxHighlighter};

// Verifies that patch line classification detects headers, hunks, additions, deletions, and context lines.
#[test]
fn classify_patch_line_detects_core_kinds() {
    assert_eq!(
        classify_patch_line("diff --git a/a.txt b/a.txt"),
        PatchLineKind::Header
    );
    assert_eq!(classify_patch_line("@@ -1,2 +1,2 @@"), PatchLineKind::Hunk);
    assert_eq!(classify_patch_line("+added"), PatchLineKind::Added);
    assert_eq!(classify_patch_line("-removed"), PatchLineKind::Deleted);
    assert_eq!(classify_patch_line(" context"), PatchLineKind::Context);
}

// Verifies that file-header lines with +++/--- are classified as headers, not add/delete content.
#[test]
fn classify_patch_line_treats_file_headers_as_headers() {
    assert_eq!(
        classify_patch_line("+++ b/src/main.rs"),
        PatchLineKind::Header
    );
    assert_eq!(
        classify_patch_line("--- a/src/main.rs"),
        PatchLineKind::Header
    );
}

// Verifies that hunk header parsing extracts old and new line starts.
#[test]
fn parse_hunk_header_extracts_line_starts() {
    assert_eq!(parse_hunk_header("@@ -12,3 +48,7 @@ fn x"), Some((12, 48)));
    assert_eq!(parse_hunk_header("not a hunk"), None);
}

// Verifies that rendered patch output includes line-number prefix and styled content rows.
#[test]
fn render_patch_with_syntax_includes_line_number_column() {
    let highlighter = SyntaxHighlighter::load();
    let patch = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let rendered = render_patch_with_syntax("a.txt", patch, &highlighter);
    assert!(
        !rendered.lines.is_empty(),
        "rendered diff should contain lines for a valid patch"
    );
    let first = rendered.lines[0]
        .spans
        .first()
        .map(|span| span.content.to_string())
        .unwrap_or_default();
    assert!(
        first.contains('│'),
        "rendered rows should include a line-number column separator"
    );
}

// Verifies that syntax resolution falls back to plain text when file extension is unknown.
#[test]
fn resolve_syntax_for_unknown_extension_falls_back_to_plain_text() {
    let highlighter = SyntaxHighlighter::load();
    let (_, syntax_name) = highlighter.resolve_syntax_for_path("file.unknownext");
    assert_eq!(
        syntax_name,
        highlighter.syntax_set.find_syntax_plain_text().name,
        "unknown extensions should fall back to plain text syntax"
    );
}
