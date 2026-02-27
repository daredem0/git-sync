//! Diff parsing and syntax-aware rendering helpers for the UI.

mod parse;
mod render;
mod style;

pub(crate) use render::render_patch_with_syntax;

#[cfg(test)]
pub(crate) use parse::{classify_patch_line, line_number_columns, parse_hunk_header};
