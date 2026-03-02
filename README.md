# git-sync

[![CI](https://github.com/daredem0/git-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/daredem0/git-sync/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/daredem0/git-sync/graph/badge.svg?branch=main)](https://codecov.io/gh/daredem0/git-sync)
[![Docs](https://img.shields.io/badge/docs-github%20pages-2ea44f?logo=github)](https://daredem0.github.io/git-sync/)
[![Release](https://img.shields.io/github/v/release/daredem0/git-sync)](https://github.com/daredem0/git-sync/releases)
[![License](https://img.shields.io/github/license/daredem0/git-sync)](./LICENSE)
[![Rust Edition](https://img.shields.io/badge/rust-2024%20edition-black?logo=rust)](https://www.rust-lang.org/)

`git-sync` is a command-line tool for moving Git history across disconnected environments while keeping that transfer auditable.

It helps you:
- create a transport package from a commit range
- review what is in that package (interactive or machine-readable)
- verify that package metadata matches repository truth
- receive the package into another repository
- preview receive impact before writing anything (`--dry-run`)

## Who This Is For

- Users who need a practical air-gap Git sync workflow
- Reviewers who need to inspect exactly what a package would import
- Developers extending the tool or integrating it into automation

## What You Get

A package created by this tool is a `.zip` archive containing:
- `<name>.bundle`
- `<name>.bundle.caudit.json`
- optional `<name>.bundle.caudit.patch` (with `--with-patches`)

Note: the `create` command keeps only `<name>.bundle.zip` on disk by default.

## Quick Start

If `git-sync` is already installed:

```bash
git-sync --help
```

From source:

```bash
cargo build --release
./target/release/git-sync --help
```

All command examples below use `git-sync` directly.

## Typical Workflow

The primary workflow has three steps:
1. create bundle package
2. audit bundle package (before transfer)
3. receive bundle package (optionally `--dry-run` first)

### 1) Create bundle package (source side)

```bash
git-sync create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle
```

Output: `sync.bundle.zip`

### 2) Audit bundle package before transfer (source side, interactive)

```bash
git-sync audit \
  --repo /path/to/source-repo \
  --bundle /path/to/sync.bundle.zip
```

This is the control gate for air-gap transfer.  
Audit verifies metadata automatically and lets the reviewer inspect history and full payload.

#### Audit UI pages (what to review)

1. Main overview (`1`)
- `General`: context (tool/repo/bundle refs)
- `Bundle Integrity`: proof and safety status
- `Heads To Import`: advertised refs
- `Would Change`: per-file line impact for selected head

2. Commit detail (`3` or `Enter` on selected head)
- per-commit reviewer view (`commit id`, subject, author/committer identity/time)
- changed files with line counts
- `Enter` on file opens diff view

3. Payload page (`2` or `v`)
- transport entries (zip contents, size, SHA256)
- `Objects` subview: materialized object inventory (review convenience)
- `Entries` subview: authoritative PACK-entry ledger (proof source)
- object/entry preview and object detail drilldown

#### Audit page previews

Main overview:

```text
┌git-sync──────────────────────────────────────────────────────────────────────────────────────────────────┐
│Audit Overview (page 1/1)                                                                                 │
│This page shows package validity, import heads, and would-change summary                                  │
│Press 1 main | 2 payload | 3 commit                                                                       │
│                                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌General────────────────────────────────────────┐┌Bundle Integrity─────────────────────────────────────────┐
│tool version: 0.6.1-5-ga8365cc-dirty           ││metadata verification: OK                                │
│repo: . (git-sync)                             ││dry-run applicability: bundle can be applied without     │
│bundle:                                        ││conflicts                                                │
│../git-sync-examples/sync_local.bundle.zip     ││pack proof: OK                                           │
│base_ref: sync/last | tip_ref: -               ││pack entries parsed: 153/153                             │
│bundle version: v2                             ││pack entries materialized: 153/153                       │
│advertised heads: 1                            ││transfer gate: allowed                                   │
│transport entries: 2                           ││pack checksum: match                                     │
│payload objects: 153                           ││bundle fully reachable from heads: yes                   │
│                                               ││thin pack detected: no                                   │
└───────────────────────────────────────────────┘└─────────────────────────────────────────────────────────┘
┌Heads To Import (bundle v2) [active]───────────┐┌Would Change (selected head: refs/heads/main)────────────┐
│OID                        REF                 ││PATH                                   +LINES    -LINES  │
│05b1f9a42fd3831e72f1487e7  refs/heads/main     ││Cargo.lock                             333       0       │
│                                               ││Cargo.toml                             2         0       │
│                                               ││LICENSE                                201       0       │
│                                               ││README.md                              203       9       │
│                                               ││schemas/sync.bundle.caudit.schema.jso  278       0       │
│                                               ││scripts/generate-merge-graph-repo.sh   158       0       │
│                                               ││src/cli.rs                             166       7       │
│                                               ││src/git/archive.rs                     178       0       │
│                                               ││src/git/bundle/create.rs               161       0       │
│                                               ││src/git/bundle/inspect.rs              72        0       │
│                                               ││src/git/bundle/mod.rs                  13        0       │
└───────────────────────────────────────────────┘└─────────────────────────────────────────────────────────┘
Tab switch heads/would-change focus | j/k or Up/Down move selection                                         
v toggle history/payload | Enter open selected head | Esc overview/quit | ? help | q quit                   
```

Commit detail:

```text
┌Commit Detail─────────────────────────────────────────────────────────────────────────────────────────────┐
│Head 1/1 | refs/heads/main                                                                                │
│Commit 3/13 | 7146b57b194992e81e7c5b47ea7fcfd47a78fbaa                                                    │
│Change: Generate zip file containing artifacts                                                            │
│Press 1 main | 2 payload | 3 commit                                                                       │
│committer date: 1772217612 (UTC+01:00)                                                                    │
│committer: Florian Leuze <f.leuze@outlook.de>                                                             │
│author date: 1772217612 (UTC+01:00)                                                                       │
│author: Florian Leuze <f.leuze@outlook.de>                                                                │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Changed Files (this commit)───────────────────────────────────────────────────────────────────────────────┐
│PATH                                                                                    +LINES    -LINES  │
│Cargo.lock                                                                              332       0       │
│Cargo.toml                                                                              1         0       │
│README.md                                                                               16        1       │
│schemas/sync.bundle.caudit.schema.json                                                  10        0       │
│src/cli.rs                                                                              2         0       │
│src/git/mod.rs                                                                          269       11      │
│src/git/tests.rs                                                                        153       0       │
│src/main.rs                                                                             62        30      │
│                                                                                                          │
│                                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────┘
h/Left prev page | l/Right next page | j/k or Up/Down move selection                                        
Enter open selected diff | Esc overview/quit | ? help | q quit                                              
```

Payload page:

```text
┌git-sync─────────────────────────────────────────────────────────────────────────────────────────────────┐
│Payload View                                                                                             │
│Press 1 main | 2 payload | 3 commit                                                                      │
│status: ok | pack version: 2                                                                             │
│entries: 153/153 | materialized: 153/153                                                                 │
│unique objects: 153 | duplicates: 0                                                                      │
│transfer: allowed | hash: sha1 | checksum: ok                                                            │
│thin pack: no | baseline resolutions: 0                                                                  │
│computed checksum: a428eff5a39ca27b828a5542fe66326d91cbad15                                              │
│trailer checksum: a428eff5a39ca27b828a5542fe66326d91cbad15                                               │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Transport Entries──────────────────────────────┐┌Pack Preview────────────────────────────────────────────┐
│ENTRY                  SIZE       SHA256       ││selected: 05b1f9a42fd3831e72f1487e760b635461956bae (comm│
│sync_local.bundle      110458     6bd8c0359321 ││reachable from heads: yes                               │
└───────────────────────────────────────────────┘│context head: #1                                        │
┌Pack Objects (153 total, 1 heads, sort: canonic┐│context commit order: 1                                 │
│OID          TYPE     SIZE       REACHABLE     ││context path: -                                         │
│05b1f9a42fd3 commit   768        yes           ││                                                        │
│1b91006ee9d4 commit   676        yes           ││commit 05b1f9a42fd3831e72f1487e760b635461956bae         │
│2983f7913a8d commit   283        yes           ││tree 253304710bf320637a58b25b114389753233d5bd           │
│30b9dd00d4ca commit   267        yes           ││parent 4f0388416d0ceeb327e65cdffa61e0e1b8476368         │
│440ec8ae7645 commit   261        yes           ││... (11 more lines)                                     │
└───────────────────────────────────────────────┘└────────────────────────────────────────────────────────┘
j/k or Up/Down select object | PgUp/PgDn jump 10 | s cycle sort | e toggle objects/entries                 
Enter open object detail | v toggle history/payload | ? help | q quit                                      
```

#### Audit values auditors should use as decision gates

In **Bundle Integrity**, these values are critical:
- `metadata verification: OK`
  - sidecar metadata is consistent with repository truth.
- `pack proof: OK`
  - PACK parsing/invariant checks passed.
- `pack entries parsed: N/N`
  - every declared PACK entry was parsed.
- `pack entries materialized: N/N`
  - every declared entry was reconstructed/materialized under policy.
- `pack checksum: match`
  - audited bytes match trailer checksum.
- `transfer gate: allowed`
  - final fail-closed gate; transfer should proceed only when allowed.
- `bundle fully reachable from heads: yes|no`
  - if `no`, commit pages are not sufficient alone; reviewer must inspect payload view.

#### How an auditor validates what they see is proof-backed

Use this sequence:
1. Confirm `pack proof: OK`, `parsed N/N`, `materialized N/N`, and `checksum match`.
2. Inspect `Entries` subview in payload page for authoritative PACK-entry coverage.
3. Use `Objects`/commit/diff views for human-readable content review.
4. Require `transfer gate: allowed` before export across the air gap.

### 3) Receive bundle package (target side)

Apply:

```bash
git-sync receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata
```

Dry-run first (recommended):

```bash
git-sync receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata \
  --dry-run
```

## Additional Commands

### Create with patch sidecar

Adds `<name>.bundle.caudit.patch` to the zip package:

```bash
git-sync create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle \
  --with-patches
```

### Metadata-only verification (non-interactive)

Useful for CI/policy gates that need pass/fail exit codes:

```bash
git-sync audit \
  --bundle /path/to/sync.bundle.zip \
  --repo /path/to/source-repo \
  --verify-metadata
```

### Non-interactive payload audit output

Human-readable table:

```bash
git-sync audit \
  --repo /path/to/repo \
  --bundle /path/to/sync.bundle.zip \
  --format table \
  --resolve pack-only
```

JSON evidence output:

```bash
git-sync audit \
  --repo /path/to/repo \
  --bundle /path/to/sync.bundle.zip \
  --format json \
  --payload-ledger summary \
  --resolve pack-only
```

Full ledger JSON:

```bash
git-sync audit \
  --repo /path/to/repo \
  --bundle /path/to/sync.bundle.zip \
  --format json \
  --payload-ledger full \
  --resolve baseline
```

`--format json` includes proof fields, counters, transport entries, and payload sections suitable for archival/reporting.

## Audit Completeness Guarantee (PACK-Level, Fail-Closed)

What you see in `git-sync` payload audit is the full bundle payload, not a filtered reachable-history view.

A Git bundle payload is a Git PACK stream. PACK provides:
- a header with declared entry count `N`
- a trailer checksum over the pack bytes

`git-sync` uses these guarantees as proof anchors:
- unambiguous PACK start:
  - parse bundle header to blank-line terminator
  - require next bytes to be exactly `PACK` (no byte-pattern heuristics)
- integrity check:
  - recompute PACK trailer checksum and require exact match
- completeness check:
  - read declared entry count `entries_declared = N`
  - parse and validate exactly `N` entries into authoritative `PackEntryLedger` rows
  - require `entries_parsed == entries_declared`

Why this is complete:
1. Completeness:
   - declared `N` entries
   - one ledger row per parsed entry
   - `ledger.len() == N`
2. Integrity:
   - checksum verification binds ledger/proof to exact payload bytes audited
3. Fail-closed transfer gate:
   - transfer is allowed only when checksum is valid and:
   - `entries_parsed == entries_declared`
   - `entries_materialized == entries_declared`

Result:
- additional smuggled objects cannot be hidden from audit output:
  - either they appear as extra PACK entries (and ledger rows), or
  - declared-count/checksum/parse invariants break and audit blocks transfer
- unreachable objects are still covered, because proof is PACK-entry-based, not reachability-based

## Interactive UI Keys

Main pages:
- `1`: main overview
- `2`: payload page
- `3`: open first commit detail page for selected head
- `v`: toggle overview <-> payload
- `?`: toggle help overlay
- `q`: quit

Overview:
- `Tab`: switch focus between `Heads To Import` and `Would Change`
- `j` / `k`: move selection in focused table
- `Enter`: open selected head commit detail
- `Esc`: quit

Commit detail pages:
- `h` / `Left`: previous commit page (not from first commit to overview)
- `l` / `Right`: next commit page
- `j` / `k`: move changed-file selection
- `Enter`: open diff for selected file
- `Esc`: return to overview

Diff and payload object detail:
- `j` / `k`: vertical scroll
- `h` / `l`: horizontal scroll
- `PgUp` / `PgDn`: fast scroll
- `Home`: reset scroll
- `Esc`: close detail view

Payload page:
- `e`: toggle `Objects` <-> `Entries`
- `s`: cycle sort mode (Objects subview only)
- `j` / `k`: move selected row
- `PgUp` / `PgDn`: jump 10 rows
- `Enter`: open selected object detail (Objects subview)

## Constraints and Behavior Notes

- `create --from ... --to ...` requires `to` to be equal to or a descendant of `from`.
- `audit` without `--format` is interactive TUI mode and requires `--repo` and `--bundle`.
- Interactive `audit` currently supports only `--resolve pack-only`.
- Interactive `audit` includes metadata verification against the provided `--repo`.
- Payload view includes both:
  - `Entries` = authoritative PACK-entry proof ledger
  - `Objects` = derived materialized-object convenience view
- `REACHABLE=no` on object rows means not reachable from advertised heads (informational, not proof completeness).
- Non-interactive payload audit requires both `--repo` and `--bundle`, and supports:
  - `--format table|json`
  - `--payload-ledger summary|full` (JSON mode)
  - `--resolve pack-only|baseline` (non-interactive modes)
- Payload proof currently supports repositories using `sha1` object format only; non-`sha1` formats fail closed.
- `audit --verify-metadata` is the explicit non-interactive verification path and requires `--bundle` and `--repo`.
- `receive` requires prerequisite history to already exist in the receiver repository.
- `receive --verify-metadata` validates bundle/sidecar integrity before import.
- `receive --dry-run` applies into an isolated temporary bare mirror and does not mutate the receiver repo.

## For Developers
Development tasks are grouped into tiers:
- `core`: build and test
- `quality`: coverage, docs, and license reports
- `release`: manpages and Linux packaging
- `all`: everything above

### Committing 
Commit Message Shape Rules (git-sync)

1. First line format: "<Prefix>: Imperative title case summary" (no trailing period).
2. Allowed prefixes: Add:, Change:, Fix:, Refactor:, Doc:, chore.
3. Leave exactly one blank line after the first line.
4. Body uses dash bullets ("- "), one change per line, no extra blank lines between bullets.
5. Keep bullets short and parallel in structure; wrap only if needed and indent continuation lines.
6. Use bullets to state what changed and why; avoid long prose paragraphs.
7. Only use more than 3 bullets for very large commits

### Developer Setup and Task Runner (`just`)

Install `just`:

```bash
# via cargo (works on all platforms with Rust installed)
cargo install --locked just
```

Install dependencies and verify tooling:

```bash
# Arch Linux
./scripts/setup-dev-arch.sh all

# Debian/Ubuntu
./scripts/setup-dev-ubuntu.sh all

# verify local tooling
just preflight all
```

Use `just` as the primary entry point:

```bash
just help
just core-build
just core-test
just quality-coverage
just quality-docs-private
just quality-docs-pdf
just release-manpages
just release-packages
```

### Build and Run

```bash
just core-build
just core-build-release
just core-run-help

# direct commands
cargo build
cargo build --release
./target/debug/git-sync --help
./target/release/git-sync --help
```

### Tests

Run full test suite:

```bash
just core-test

# direct command
cargo test
```

Run integration workflow test only:

```bash
cargo test --test bundle_workflow_integration -- --nocapture
```

### Coverage

```bash
just quality-coverage

# direct command
cargo llvm-cov --workspace --all-features --summary-only
```

### Third-Party License Compliance

Install tools:

```bash
cargo install --locked cargo-deny
cargo install --locked cargo-about
```

Check dependency licenses against policy (`deny.toml`):

```bash
just quality-licenses-check

# direct command
./scripts/check-licenses.sh
```

Generate/update third-party license inventory:

```bash
just quality-licenses-generate

# direct command
./scripts/generate-third-party-licenses.sh
```

If you want verbose cargo-about diagnostics while generating:

```bash
CARGO_ABOUT_LOG_LEVEL=warn ./scripts/generate-third-party-licenses.sh
```

Generated output:
- [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md)

### Linux Packages (Debian + Arch)

Prerequisites:

```bash
# Debian/Ubuntu (man pages + .deb toolchain deps)
sudo apt-get update
sudo apt-get install -y pandoc dpkg-dev

# Arch Linux (man pages + makepkg)
sudo pacman -S --needed pandoc base-devel

# cargo-deb (required for Debian package script)
cargo install --locked cargo-deb
```

Generate man pages from documentation (`README.md` and `SDD_SAD.md` -> section 7 man pages):

```bash
just release-manpages

# direct command
./scripts/generate-manpages.sh
```

This writes:
- `target/man/git-sync.1.gz`
- `target/man/git-sync-readme.7.gz`
- `target/man/git-sync-architecture.7.gz`

Build a Debian package (`.deb`):

```bash
just release-deb

# direct command
./scripts/build-deb.sh
```

Build an Arch package (`.pkg.tar.zst`):

```bash
just release-arch

# direct command
./scripts/build-arch.sh
```

Use prebuilt release inputs (binary + manpages in `target/`) without rebuilding:

```bash
GIT_SYNC_USE_PREBUILT=1 ./scripts/build-deb.sh
GIT_SYNC_USE_PREBUILT=1 ./scripts/build-arch.sh
```

Install the generated Arch package:

```bash
sudo pacman -U target/arch/git-sync-bin-*.pkg.tar.zst
```

Optional: install debug symbols package:

```bash
sudo pacman -U target/arch/git-sync-bin-debug-*.pkg.tar.zst
```

Verify installed man pages:

```bash
man git-sync
man 7 git-sync-readme
man 7 git-sync-architecture
```

Notes:
- Arch packaging uses a prebuilt release binary through `packaging/arch/PKGBUILD`.
- Debian packaging uses `[package.metadata.deb]` in `Cargo.toml`.
- Both package paths are printed by the scripts (`target/debian` and `target/arch`).

### Release Workflow (`cargo release` + CI)

Target flow:
1. Run `cargo release` locally to create the release commit and tag.
2. Push commit and tag.
3. CI builds and packages with a consistent version everywhere.

Default local release command (no crates.io publish):

```bash
cargo release <major.minor.patch> --no-publish --execute
```

CI release/version behavior:
- `scripts/verify-tag-version.sh` enforces: `git tag` version == `Cargo.toml` version.
- On tagged commits, release binary build sets `GIT_SYNC_VERSION_OVERRIDE` from the Git tag.
- `build-release` builds release binary + manpages exactly once and uploads them as `release-package-inputs`.
- `package-deb` and `package-arch` download `release-package-inputs` and package from those prebuilt files.
- `package-crate` runs `cargo package --locked` and uploads the produced `.crate`.
- `docs` also generates a PDF from rustdoc content (including Mermaid diagrams).
- `release` publishes debug/release binaries, Debian/Arch packages, docs archive, docs PDF, and coverage report to GitHub Releases.
- Release notes are generated from the matching version section in `CHANGELOG.md`.

This keeps the release pipeline immutable for tagged builds and avoids rebuilding release binaries in package jobs.

### Generate Documentation

Generate Rust API docs:

```bash
just quality-docs

# direct command
cargo doc --no-deps
```

Generate docs with Mermaid diagrams rendered (requires internet access for the Mermaid JS module):

```bash
just quality-docs

# direct command
RUSTDOCFLAGS="--html-in-header docs/mermaid-header.html" cargo doc --no-deps --bins
```

Generate and open docs in browser:

```bash
cargo doc --no-deps --open
```

Generate docs including private items (useful for internal development):

```bash
just quality-docs-private

# direct command
cargo doc --no-deps --document-private-items
```

Generate a PDF from the rendered crate docs (includes Mermaid diagrams, Arch Linux local setup):

```bash
just quality-docs-pdf

# one-time Arch prerequisites if missing:
sudo pacman -S --needed nss nspr atk at-spi2-atk gtk3 libdrm libxkbcommon pango cairo alsa-lib libxcomposite libxdamage libxfixes libxrandr libx11 libxext libxrender libxi libxtst libcups mesa ttf-liberation
npm install --no-save playwright@1.52.0
npx playwright install chromium
RUSTDOCFLAGS="--html-in-header docs/mermaid-header.html" cargo doc --no-deps --bins --document-private-items
./scripts/generate-doc-pdf.sh
```

Notes:
- PDF generation flattens the crate rustdoc into one document (landing page plus module and item pages).
- Default entry is `git_sync/index.html` and the full crate tree is stitched automatically.
- You can still choose a different start page by passing a third argument:

```bash
./scripts/generate-doc-pdf.sh target/doc target/docs-pdf/git_sync-rustdoc-all.pdf all.html
```

### Additional Project Documentation

- Architecture/design: [`SDD_SAD.md`](SDD_SAD.md)
- Metadata schema (create sidecar): `schemas/sync.bundle.caudit.schema.json`
- Payload audit schema (non-interactive JSON): `schemas/sync.bundle.paudit.schema.json`

## Technical Notes

- Runtime Git operations are in-process via `git2` (libgit2 bindings); core `create`/`audit`/`receive` paths do not shell out to `git`.
- Receiving the same package repeatedly is idempotent: existing refs/objects are reused and results remain deterministic.
- Binary/symlink (non-text) file changes are handled safely: line counts are `0/0` and diff-open actions no-op for unavailable textual patches.
