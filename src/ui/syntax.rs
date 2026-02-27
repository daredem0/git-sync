use super::types::SyntaxHighlighter;
use std::path::Path;
use syntect::highlighting::ThemeSet;
use syntect::parsing::{SyntaxReference, SyntaxSet};

impl SyntaxHighlighter {
    pub(crate) fn load() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.get("InspiredGitHub"))
            .or_else(|| themes.themes.values().next())
            .cloned()
            .expect("syntect should provide at least one built-in theme");

        Self { syntax_set, theme }
    }

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
