// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Syntax highlighting support for text previews.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

use super::types::SyntaxHighlighter;
use std::path::Path;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

impl SyntaxHighlighter {
    /// Loads bundled syntax and theme sets used by the diff renderer.
    ///
    /// Prefers `base16-ocean.dark`, then falls back to available built-ins.
    pub(crate) fn load() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.get("InspiredGitHub"))
            .or_else(|| themes.themes.values().next())
            .cloned()
            .unwrap_or_else(Theme::default);

        Self { syntax_set, theme }
    }

    /// Resolves a syntax definition from a file path.
    ///
    /// Falls back to plain text when no extension-specific syntax exists.
    pub(crate) fn resolve_syntax_for_path<'a>(
        &'a self,
        path: &str,
    ) -> (&'a SyntaxReference, String) {
        let extension = Path::new(path).extension().and_then(|ext| ext.to_str());
        let syntax = extension
            .and_then(|ext| self.syntax_set.find_syntax_by_extension(ext))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        (syntax, syntax.name.to_string())
    }
}
