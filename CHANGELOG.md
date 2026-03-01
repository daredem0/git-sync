# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.2] - 2026-03-01

### Added
- Added a project `Justfile` to provide a single entry point for common local workflows.
- Added payload `Entries` preview support that can render the resolved materialized object preview directly from an entry row.

### Changed
- Improved audit UI readability with semantic terminal-theme colors for integrity-relevant values and object kinds.
- Improved `+lines` / `-lines` highlighting so only non-zero deltas are colorized and zero values stay neutral.
- Added active-pane focus highlighting to make keyboard navigation between tables more obvious.
- Enabled object-style drill-down from payload `Entries` rows (when an entry resolves to a materialized object).

## [0.7.1] - 2026-03-01

### Added
- Added rustdoc PDF generation tooling:
  - `scripts/generate-doc-pdf.sh`
  - `scripts/generate-doc-pdf.mjs` (Playwright-based renderer)
- Added CI PDF generation in the `docs` job and upload of a `docs-pdf` artifact.
- Added release publishing of the generated rustdoc PDF alongside existing binaries/packages/docs archive.
- Added SPDX file headers plus module-level summary/context documentation across `src/*` Rust files.
- Expanded SDD/SAD with additional architecture/security diagrams, trust-boundary visualizations, and traceability content.

### Changed
- Improved rustdoc Mermaid rendering pipeline for GitHub/rustdoc compatibility and readability:
  - parser-safe Mermaid labels
  - responsive sizing behavior for mixed diagram sizes
  - explicit themed styling for consistent contrast
- Updated docs PDF generation to flatten crate documentation content into a single export (landing + module/item pages).
- Updated docs build path to include `--document-private-items` for fuller API coverage in generated documentation/PDF exports.

### Tests
- Increased unit/integration test coverage across git-domain, UI, and CLI paths.
- Added/expanded tests to strengthen regression protection for command behavior and payload/audit workflows.

### Documentation
- Updated README documentation workflow and PDF generation guidance, including Arch Linux local setup notes.
- Refined SDD/SAD structure and explanatory depth with stronger static/dynamic architecture coverage.

## [0.7.0] - 2026-02-28

### Added
- Payload entry-ledger rows now include `reconstructed_size` in memory, JSON export, and schema output.
- Payload `Entries` UI now shows both `HDR_SIZE` and `RECON_SIZE` columns for explicit delta-stream versus reconstructed-size review.
- Overview now shows `bundle fully reachable from heads: yes|no (...)` as an immediate history-versus-payload audit signal.
- Overview main page now supports explicit focus switching between `Heads To Import` and `Would Change` tables.
- Direct page shortcuts were added/standardized in the UI flow: `1` main overview, `2` payload page, `3` first commit detail page.
- Added proof-boundary guard type (`VerifiedPayload`) in payload PACK verification to enforce fail-closed invariant checks before exposing verification results.
- Added centralized digest module (`src/git/digest.rs`) with shared SHA-1/SHA-256 helpers and dedicated digest test coverage.

### Changed
- Clarified PACK delta size semantics: pack-entry size for delta entries is treated as delta-stream byte length (spec-aligned).
- Kept reconstructed target size validation after delta apply as a fail-closed invariant.
- Updated payload materialized-object sizing to use reconstructed object size for derived object rows.
- Reworked audit UI navigation model so `Tab` no longer toggles main views, `v` toggles overview/payload on main pages, and `3` opens commit detail instead of opening file diff directly.
- Tightened page movement behavior so overview no longer pages into commit view via right-arrow, first commit page no longer returns to overview via left-arrow, and overview remains explicitly reachable via `1` or `Esc`.
- Moved `1/2/3` navigation hints from footer into headers with explicit wording (`Press 1 main | 2 payload | 3 commit`).
- Split payload footer hints across two lines so each line fits 110-column terminals.
- Enriched overview `General` panel with payload context statistics (bundle version, advertised heads, transport entries, payload objects).
- Aligned overview panel split so top and bottom sections use the same column proportions for cleaner layout consistency.
- Restructured README workflow to focus on the core path (`create -> audit -> receive`) and moved optional/non-core CLI usage into `Additional Commands`.
- Refactored payload verification internals into focused modules (`preflight`, `entry`, `delta`, `materialized`, `proof`) while preserving fail-closed behavior.
- Split monolithic git type definitions into domain-focused `src/git/types/*` modules with stable re-exports.
- Split CLI orchestration and non-interactive output rendering into `src/app/commands/*` and `src/app/output/*`.
- Unified receive-path PACK parsing with the strict bundle-header parser (removed heuristic PACK offset scan).
- Refactored UI input handling into router/action reducers (`src/ui/input/{router,actions}.rs`) with centralized key-action mapping.
- Refactored payload renderer into focused modules (`src/ui/render/payload/{layout,tables,preview,detail,...}`).
- Reduced UI state enum footprint by boxing `PayloadModel::Ok(Box<git::PayloadAudit>)`.
- Narrowed `git::mod` export surface and isolated git test helpers into explicit `src/git/test_support.rs`.
- Consolidated runtime hash call sites onto shared digest helpers to reduce duplicate OpenSSL/FFI hashing paths.

### Tests
- Updated payload tests for `reconstructed_size` schema/document requirements and delta stream mismatch wording.
- Updated UI tests for `Entries` table header changes (`HDR_SIZE`/`RECON_SIZE`).
- Added/updated navigation tests for `1/2/3` routing, overview focus switching, commit/diff transitions, and page-boundary behavior.
- Added footer-width regression checks to keep payload footer lines within 110 columns.
- Added proof-boundary tests for invariant/counter consistency and fail-closed verification behavior.
- Added strict parser regression tests for receive-path header framing and PACK-start gap rejection.
- Added digest tests for SHA-1/SHA-256 helper consistency and known vectors.

### Documentation
- Updated README and SDD/SAD to match current payload-proof semantics, UI behavior, and audit workflow expectations.
- Updated SDD/SAD architecture chapter to match refactored source layout and added a concrete source module map with direct file mapping.

## [0.6.1] - 2026-02-28

### Added
- Payload guard for repository object format in audit/proof path:
  - explicit support for `sha1`
  - explicit fail-closed rejection for non-`sha1` object formats (for example `sha256`)
- Payload tests for object-format policy:
  - explicit `sha1` acceptance
  - explicit non-`sha1` rejection

### Changed
- Delta PACK validation now enforces delta-payload-size semantics correctly:
  - header size is checked against inflated delta payload bytes
  - reconstructed result size remains validated via delta-apply checks
- Payload proofing/runtime stability improvements for real-world ref-delta/ofs-delta bundles.

### Documentation
- V7 plan status updated with completed phases and explicit Definition-of-Done check.
- README updated to include full payload proof field set and SHA-1 object-format constraint.
- SDD/SAD open TODO extended with phase-6b object-format-aware hashing work.

## [0.6.0] - 2026-02-28

### Added
- Explicit payload resolve policy for non-interactive audit:
  - `--resolve pack-only` (strict default)
  - `--resolve baseline` (baseline ODB-assisted external ref-delta resolution)
- Entry-ledger JSON export controls:
  - `--payload-ledger summary` (default bounded ledger subsets)
  - `--payload-ledger full` (full parsed entry rows)
- Additional payload tests for resolve-mode behavior and strict unresolved-entry blocking.

### Changed
- Payload proof/output semantics now center on entry-truth counters and transfer gate:
  - `entries_declared`, `entries_parsed`, `entries_materialized`
  - `unique_objects_materialized`, `duplicate_entry_count_materialized`
  - `transfer_allowed`, `blocked_reason`
- Non-interactive table output now includes concise ledger summary and transfer-gate evidence.
- Payload audit robustness improved for prerequisite-dependent tree context (missing prerequisite trees no longer abort payload rendering/context scan).

### Documentation
- README updated for:
  - `Objects` vs `Entries` payload semantics
  - transfer-gate and entry counter meaning
  - non-interactive `--payload-ledger` and `--resolve` usage
- SDD/SAD synchronized to current architecture:
  - pack-entry ledger as authoritative proof source
  - materialized object index as derived convenience layer
  - resolve-mode boundaries and fail-closed behavior

## [0.5.0] - 2026-02-28

### Added
- New payload-audit JSON schema: `schemas/sync.bundle.paudit.schema.json`.
- Non-interactive payload JSON document model with:
  - package metadata fields aligned to `.caudit.json` style
  - `transport_entries`
  - `pack_summary`
  - `pack_objects`
  - `object_details`
- PACK-proof hardening for payload audit:
  - PACK header parsing (`pack_version`, `declared_object_count`)
  - trailer checksum verification
  - direct PACK entry iteration and reconstruction (including `ofs-delta` / `ref-delta`)
  - canonical object OID verification from reconstructed bytes
  - fail-closed handling on count/checksum/delta-base errors
- Explicit `verification_status` field in payload `pack_proof` JSON output.
- Additional payload/UI tests covering proof invariants, edge-case failures, and repo-name derivation behavior.

### Changed
- `audit` non-interactive mode now supports payload output only:
  - `--format table`
  - `--format json`
- `audit --verify-metadata` was simplified to explicit pass/fail behavior (exit code + message), independent of `--format`.
- Payload table output now includes PACK proof lines plus transport-entry hash table before pack object rows.
- Interactive overview now surfaces a dedicated bundle-integrity summary:
  - metadata verification
  - dry-run applicability
  - pack proof status
  - processed/declared count
  - checksum match status
- Interactive payload page now shows full proof details (pack version/hash/checksums) and selected-object context metadata.
- Overview repository label now renders as `repo: <path> (<repo_name>)` when remote-derived name is available.

### Removed
- Legacy manifest-based non-interactive audit path.
- `audit --format tsv` support.
- Legacy repo-range non-interactive audit flow under `audit`.

### Documentation
- README updated to reflect payload-only non-interactive audit usage and current verification flow.
- SDD/SAD rewritten to current architecture and runtime behavior, including a dedicated PACK-proof model section.

## [0.4.0] - 2026-02-28

### Added
- Multi-head history navigation in interactive audit (overview head selection plus per-head commit paging).
- Payload audit main view with transport-entry table, pack-object table, and object drill-down.
- Payload object-detail syntax highlighting (object-content view only, no synthetic diffs).
- Payload navigation improvements: `PgUp`/`PgDn` jumps by 10 objects.
- Payload sort-mode cycling (`s`) with a new context-oriented grouping mode.
- Context metadata on payload objects (head index, commit order, path) to support audit-friendly grouping.

### Changed
- Interactive commit pages are now driven from bundle objects, while metadata verification remains surfaced in overview.
- Payload view layout now dedicates the full right pane to object preview and keeps transport/pack tables on the left.
- Payload preview now renders line numbers only for actual text-content lines, not metadata header lines.
- Payload object detail rendering now includes line-number gutters.
- Payload preview truncation marker (`... (N more lines)`) is dynamically anchored at the last visible row.
- History/Payload view switching is constrained to the main page with context-aware footer hints.
- Removed deprecated flat commit-audit path and standardized on head-scoped collectors.

### Fixed
- Fixed payload blob-path discovery so reachable objects resolve paths through reachable commit history (not only head trees).
- Improved payload preview responsiveness by reusing imported payload sessions, caching detail/preview data, and highlighting only visible preview lines.
- Bounded blob path scanning during preview/detail generation to keep UI interactions responsive.
- Package metadata now records maintainer/packager as `f.leuze@outlook.de` for Debian and Arch outputs.

## [0.3.2] - 2026-02-28

### Added
- CI release workflow now extracts release notes from the matching `CHANGELOG.md` version section and publishes them with the GitHub Release.

### Changed
- README was restructured toward an operator-first flow with clearer pre-air-gap audit guidance.
- Command examples now use `git-sync` directly instead of `cargo run`.
- Workflow documentation now distinguishes required core steps from optional non-interactive audit/reporting steps.

## [0.3.1] - 2026-02-28

### Fixed
- Updated CI conditions so GitHub Pages deployment runs for tag builds as intended.
- Improved release-asset publish behavior for tagged releases.

## [0.3.0] - 2026-02-28

### Added
- Automatic GitHub Release publication in CI for tagged builds, including packaged artifacts.

### Changed
- Renamed the project and binary naming from `git-sync-audit` to `git-sync` across code, packaging, and documentation.

### Fixed
- Corrected prebuilt artifact download paths in packaging jobs so Debian and Arch package steps can consume release outputs reliably.

## [0.2.1] - 2026-02-28

### Added
- Keep a Changelog style `CHANGELOG.md`.

### Changed
- Cleaned up release workflow behavior to align versioning and packaging steps.
- Added package metadata fields (`repository`, `homepage`, `documentation`) in `Cargo.toml`.
- Updated README UI preview content.

## [0.2.0] - 2026-02-28

### Added
- Build-time version injection derived from Git tags, exposed through CLI `--version` and the UI overview page.
- Third-party license compliance tooling and generated dependency license inventory support.
- Linux packaging pipeline and assets for Debian (`.deb`) and Arch Linux (`.pkg.tar.zst`).
- CI publication of packaging artifacts and generated documentation via GitHub Pages.

### Changed
- Expanded developer documentation with `SDD_SAD.md`.
- Improved rustdoc output to include project docs and Mermaid diagram rendering.

### Fixed
- Coverage CI workflow reliability improvements.

## [0.0.1] - 2026-02-27

### Added
- Initial CLI workflows for `create`, `audit`, `ui`, and `receive`.
- Bundle creation for commit ranges and packaging as transportable `.bundle.zip` artifacts.
- Audit metadata sidecar generation (`.bundle.caudit.json`) with optional patch sidecar support.
- Receive flow with metadata integrity verification and `--dry-run` impact preview.
- Interactive TUI audit pages for overview, commit inspection, and file-level diff viewing with syntax highlighting.
- Broad Git/UI unit and integration test coverage for core workflows and failure paths.

### Changed
- Refactored Git and UI modules for clearer layering and maintainability.
- Improved zip-bundle audit flow and metadata-driven validation behavior.

### Fixed
- Corrected commit-count rendering in the diff view.
- Added missing binary/non-text handling tests and related behavior checks.

### Documentation
- Added Rust doc comments across the codebase and initial README improvements.

[Unreleased]: https://github.com/daredem0/git-sync/compare/v0.7.2...HEAD
[0.7.2]: https://github.com/daredem0/git-sync/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/daredem0/git-sync/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/daredem0/git-sync/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/daredem0/git-sync/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/daredem0/git-sync/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/daredem0/git-sync/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/daredem0/git-sync/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/daredem0/git-sync/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/daredem0/git-sync/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/daredem0/git-sync/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/daredem0/git-sync/compare/0.2.0...v0.2.1
[0.2.0]: https://github.com/daredem0/git-sync/compare/0.0.1...0.2.0
[0.0.1]: https://github.com/daredem0/git-sync/releases/tag/0.0.1
