// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Git-domain test module wiring.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::test_support::*;
use super::*;

mod support;

// Focus: open_context validation and context resolution behavior.
mod context_tests;
// Focus: changed-file diff-entry detection and deterministic ordering behavior.
mod diff_manifest_tests;
// Focus: bundle creation behavior and create-time audit sidecars.
mod bundle_create_tests;
// Focus: bundle header parsing and malformed-input handling.
mod inspect_range_tests;
// Focus: receive-path behavior, idempotency, and head-application checks.
mod receive_tests;
// Focus: payload audit transport/object listing and object detail drill-down behavior.
mod payload_tests;
// Focus: metadata parsing/integrity/repo-verification behavior.
mod metadata_tests;
// Focus: archive extraction/writing and artifact-cleanup helpers.
mod archive_tests;
// Focus: utility helper behavior for parsing, formatting, hashing, and OID/path conversions.
mod util_tests;
// Focus: centralized digest helper behavior for SHA-1/SHA-256 and hex rendering.
mod digest_tests;
