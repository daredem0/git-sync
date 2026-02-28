# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 2026-02-28

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

[Unreleased]: https://github.com/daredem0/git-sync/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/daredem0/git-sync/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/daredem0/git-sync/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/daredem0/git-sync/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/daredem0/git-sync/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/daredem0/git-sync/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/daredem0/git-sync/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/daredem0/git-sync/compare/0.2.0...v0.2.1
[0.2.0]: https://github.com/daredem0/git-sync/compare/0.0.1...0.2.0
[0.0.1]: https://github.com/daredem0/git-sync/releases/tag/0.0.1
