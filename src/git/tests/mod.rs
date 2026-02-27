use super::*;

mod support;

// Focus: open_context validation and context resolution behavior.
mod context_tests;
// Focus: changed-file diff detection and manifest rendering output.
mod diff_manifest_tests;
// Focus: bundle creation behavior and create-time audit sidecars.
mod bundle_create_tests;
// Focus: bundle header parsing and repo range resolution.
mod inspect_range_tests;
// Focus: receive-path behavior, idempotency, and head-application checks.
mod receive_tests;
// Focus: metadata parsing/integrity/repo-verification behavior.
mod metadata_tests;
// Focus: archive extraction/writing and artifact-cleanup helpers.
mod archive_tests;
// Focus: utility helper behavior for parsing, formatting, hashing, and OID/path conversions.
mod util_tests;
