# git-sync

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

Build:

```bash
cargo build --release
```

Run with Cargo during development:

```bash
cargo run -- --help
```

## Typical Workflow

### 1) Create a package from a commit range

```bash
cargo run -- create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle
```

Include a patch sidecar:

```bash
cargo run -- create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle \
  --with-patches
```

### 2) Audit the package interactively (TUI)

```bash
cargo run -- audit \
  --repo /path/to/repo \
  --bundle /path/to/sync.bundle.zip
```

This opens a terminal UI with:
- overview page (verification status, heads to import, dry-run summary)
- commit pages with per-file line stats
- file-level diff viewer

### 3) Audit in non-interactive mode

From bundle metadata:

```bash
cargo run -- audit --bundle sync.bundle.zip --format tsv
cargo run -- audit --bundle sync.bundle.zip --format json
```

Directly from repository history:

```bash
cargo run -- audit --repo . --from <from-rev> --to <to-rev> --format tsv
cargo run -- audit --repo . --from <from-rev> --to <to-rev> --format json
```

### 4) Verify metadata against repository truth

```bash
cargo run -- audit \
  --bundle sync.bundle.zip \
  --repo . \
  --verify-metadata \
  --format tsv
```

### 5) Receive into target repository

```bash
cargo run -- receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata
```

Dry-run receive (no writes to receiver repo):

```bash
cargo run -- receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata \
  --dry-run
```

## Interactive UI Preview

Run the interactive audit UI:

```bash
cargo run -- audit --repo /path/to/repo --bundle /path/to/sync.bundle.zip
```

### Page 1: package overview

Shows:
- metadata verification result
- heads to import
- would-change per-file line summary
- total page position in the audit session

Preview:
```text
┌git-sync────────────────────────────────────────────────────────────────────────────────────┐
│Audit Overview (page 1/10)                                                                        │
│This page shows package validity, import heads, and would-change summary                          │
│Use h/l or left/right to move pages                                                               │
│                                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
┌General───────────────────────────────────────────────────────────────────────────────────────────┐
│repo: /tmp/test                                                                                   │
│bundle: sync.bundle.zip                                                                           │
│base_ref: sync/last | tip_ref: -                                                                  │
│metadata verification: OK                                                                         │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Heads To Import (bundle v2)────────────────┐┌Would Change (per-file line diff summary)────────────┐
│OID                    REF                 ││PATH                               +LINES    -LINES  │
│d7854706e7eb730140de1  refs/heads/main     ││(no file content changes)          -         -       │
│                                           ││                                                     │
│                                           ││                                                     │
│                                           ││                                                     │
│                                           ││                                                     │
│                                           ││                                                     │
│                                           ││                                                     │
└───────────────────────────────────────────┘└─────────────────────────────────────────────────────┘
h/Left prev page | l/Right next page | j/k or Up/Down move | Enter open diff | ? help | q quit
```

### Page 2..N: commit detail pages

For each commit in the audited range, this page shows:
- commit position (example: `3/9`)
- commit id + subject
- committer date + `name <email>`
- author date + `name <email>`
- changed files in that commit with `+LINES` / `-LINES`

Preview:
```text
┌Commit Detail─────────────────────────────────────────────────────────────────────────────────────┐
│Commit 4/13 | aa7406fc5178e46f570027914655aeb27b550a15                                            │
│Change: Add zip-bundle audit flow and end-to-end fixture test                                     │
│committer date: 1772219543 (UTC+01:00)                                                            │
│committer: Florian Leuze <f.leuze@outlook.de>                                                     │
│author date: 1772219543 (UTC+01:00)                                                               │
│author: Florian Leuze <f.leuze@outlook.de>                                                        │
│Changed files: 6                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Changed Files (this commit)───────────────────────────────────────────────────────────────────────┐
│PATH                                                                            +LINES    -LINES  │
│README.md                                                                       4         1       │
│scripts/generate-merge-graph-repo.sh                                            158       0       │
│src/git/mod.rs                                                                  198       64      │
│src/git/tests.rs                                                                73        0       │
│src/main.rs                                                                     9         10      │
│tests/bundle_workflow_integration.rs                                            209       0       │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
h/Left prev page | l/Right next page | j/k or Up/Down move | Enter open diff | ? help | q quit
```

### Diff view (opened from commit page with `Enter`)

This view opens on top of the commit page for the currently selected file.

It shows:
- selected commit id and subject
- selected file path
- detected syntax name used for highlighting
- first-parent patch with old/new line number columns
- diff semantic coloring (`+` / `-` / hunk/header) plus syntax-aware line highlighting

Preview:

```text
┌Diff View─────────────────────────────────────────────────────────────────────────────────────────┐
│Commit 4/13 | aa7406fc5178e46f570027914655aeb27b550a15                                            │
│Change: Add zip-bundle audit flow and end-to-end fixture test                                     │
│file: src/git/mod.rs                                                                              │
│syntax: Rust | selected file index: 3                                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Patch (first-parent commit diff)──────────────────────────────────────────────────────────────────┐
│   227        │ -            .collect(),                                                          │
│   228        │ -    };                                                                           │
│   229        │ -    Ok(serde_json::to_string_pretty(&serializable)?)                             │
│   230        │ -}                                                                                │
│   231        │ -                                                                                 │
│   232    194 │  pub fn create_bundle(                                                            │
│   233    195 │      repo_path: &Path,                                                            │
│   234    196 │      from_rev: &str,                                                              │
│              │ @@ -491,6 +453,25 @@ pub fn inspect_bundle(bundle_path: &Path) -> Result<BundleIns│
│   491    453 │      })                                                                           │
│   492    454 │  }                                                                                │
│   493    455 │                                                                                   │
│          456 │ +pub fn collect_changed_files_from_bundle_input(                                  │
│          457 │ +    bundle_input_path: &Path,                                                    │
│          458 │ +) -> Result<Vec<ChangedFile>> {                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset | Esc back
```

## Interactive UI Keys

Page mode:
- `h` / `Left`: previous page
- `l` / `Right`: next page
- `j` / `Down`: move selection down
- `k` / `Up`: move selection up
- `g`: first page
- `G`: last page
- `Enter`: open diff for selected file
- `?`: toggle help
- `q` / `Esc`: quit

Diff mode:
- `j` / `Down`: scroll down
- `k` / `Up`: scroll up
- `h` / `Left`: scroll left
- `l` / `Right`: scroll right
- `PgUp` / `PgDn`: fast vertical scroll
- `Home`: reset diff scroll
- `Esc`: close diff view

## Constraints and Behavior Notes

- `create --from ... --to ...` requires `to` to be equal to or a descendant of `from`.
- `audit` without `--format` is interactive TUI mode and requires `--repo` and `--bundle`.
- `audit --verify-metadata` is non-interactive and requires `--bundle`, `--repo`, and `--format`.
- `receive` requires prerequisite history to already exist in the receiver repository.
- `receive --verify-metadata` validates bundle/sidecar integrity before import.
- `receive --dry-run` applies into an isolated temporary bare mirror and does not mutate the receiver repo.

## For Developers

### Build and Run

```bash
cargo build
cargo build --release
cargo run -- --help
```

### Tests

Run full test suite:

```bash
cargo test
```

Run integration workflow test only:

```bash
cargo test --test bundle_workflow_integration -- --nocapture
```

### Coverage

```bash
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
./scripts/check-licenses.sh
```

Generate/update third-party license inventory:

```bash
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
# for man page generation
sudo pacman -S pandoc

# for Debian package creation
cargo install --locked cargo-deb

# for Arch package creation (makepkg)
sudo pacman -S base-devel
```

Generate man pages from documentation (`README.md` and `SDD_SAD.md` -> section 7 man pages):

```bash
./scripts/generate-manpages.sh
```

This writes:
- `target/man/git-sync.1.gz`
- `target/man/git-sync-readme.7.gz`
- `target/man/git-sync-architecture.7.gz`

Build a Debian package (`.deb`):

```bash
cargo install --locked cargo-deb
./scripts/build-deb.sh
```

Build an Arch package (`.pkg.tar.zst`):

```bash
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

### Release Workflow (cargo-release + CI)

Target flow:
1. Run `cargo-release` locally to create the release commit and tag.
2. Push commit and tag.
3. CI builds and packages with a consistent version everywhere.

Default local release command (no crates.io publish):

```bash
cargo release <major.minor.patch> --no-publish --execute
```

Example:

```bash
cargo release 0.2.0 --no-publish --execute
```

CI release/version behavior:
- `scripts/verify-tag-version.sh` enforces: `git tag` version == `Cargo.toml` version.
- On tagged commits, release binary build sets `GIT_SYNC_VERSION_OVERRIDE` from the Git tag.
- `build-release` builds release binary + manpages exactly once and uploads them as `release-package-inputs`.
- `package-deb` and `package-arch` download `release-package-inputs` and package from those prebuilt files.
- `package-crate` runs `cargo package --locked` and uploads the produced `.crate`.

This keeps the release pipeline immutable for tagged builds and avoids rebuilding release binaries in package jobs.

### Generate Documentation

Generate Rust API docs:

```bash
cargo doc --no-deps
```

Generate docs with Mermaid diagrams rendered (requires internet access for the Mermaid JS module):

```bash
RUSTDOCFLAGS="--html-in-header docs/mermaid-header.html" cargo doc --no-deps --bins
```

Generate and open docs in browser:

```bash
cargo doc --no-deps --open
```

Generate docs including private items (useful for internal development):

```bash
cargo doc --no-deps --document-private-items
```

### Additional Project Documentation

- Architecture/design: [`SDD_SAD.md`](SDD_SAD.md)
- Metadata schema: `schemas/sync.bundle.caudit.schema.json`

## Implementation Snapshot

- Language: Rust (edition 2024)
- Git operations: `git2` (libgit2 bindings)
- TUI: `ratatui` + `crossterm`
- Core logic runs in-process (no `git` CLI subprocesses in runtime Git paths)
