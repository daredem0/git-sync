// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Diff support module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod parse;
mod render;
mod style;

pub(crate) use render::render_patch_with_syntax;

#[cfg(test)]
pub(crate) use test_api::{classify_patch_line, line_number_columns, parse_hunk_header};
#[cfg(test)]
mod test_api;
