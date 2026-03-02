// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Test-only adapter exports for diff parsing helpers.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Keeps production exports minimal while allowing focused parser tests.

pub(crate) use super::parse::{classify_patch_line, line_number_columns, parse_hunk_header};
