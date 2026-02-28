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

Core workflow is steps 1, 2, 4, 5, and 6 below.  
Step 3 and the later non-interactive audit section are optional evidence/reporting paths.

### 1) Create package on source side

```bash
git-sync create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle
```

Include a patch sidecar:

```bash
git-sync create \
  --repo /path/to/source-repo \
  --from <from-rev> \
  --to <to-rev> \
  --output sync.bundle \
  --with-patches
```

This produces `sync.bundle.zip`.

### 2) Audit package on source side before transfer (TUI)

```bash
git-sync audit \
  --repo /path/to/source-repo \
  --bundle /path/to/sync.bundle.zip
```

This opens a terminal UI with:
- overview page (verification status, heads to import, dry-run summary)
- commit pages with per-file line stats
- file-level diff viewer

Use this step to confirm exactly what would leave the source network.  
Interactive `audit` verifies package metadata against the provided `--repo` automatically.

### 3) Optional: verify metadata against source repository truth (auditor evidence)

Interactive audit already performs this check.  
Use this explicit non-interactive command when you need machine-readable proof for an auditor:

```bash
git-sync audit \
  --bundle sync.bundle.zip \
  --repo /path/to/source-repo \
  --verify-metadata \
  --format tsv
```

### 4) Transfer package across the air gap

Move `sync.bundle.zip` to the disconnected target side using your approved transfer method.

### 5) Audit package on target side after transfer (TUI, recommended)

```bash
git-sync audit \
  --repo /path/to/target-repo \
  --bundle /path/to/sync.bundle.zip
```

On target side, `--repo` provides the receiver context used for applicability and dry-run checks.

### Optional) Non-interactive audit output (not part of core workflow)

From bundle metadata (manifest/metadata view):

```bash
git-sync audit --bundle sync.bundle.zip --format tsv
git-sync audit --bundle sync.bundle.zip --format json
```

This mode reports what is recorded in the package metadata. For full in-repo history comparison, use repo-range mode.

Directly from repository history (repository-truth view):

```bash
git-sync audit --repo . --from <from-rev> --to <to-rev> --format tsv
git-sync audit --repo . --from <from-rev> --to <to-rev> --format json
```

### 6) Receive into target repository

```bash
git-sync receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata
```

Dry-run receive (no writes to receiver repo):

```bash
git-sync receive \
  --repo /path/to/receiver-repo \
  --bundle /path/to/sync.bundle.zip \
  --verify-metadata \
  --dry-run
```

## Interactive UI Preview

Run the interactive audit UI:

```bash
git-sync audit --repo /path/to/repo --bundle /path/to/sync.bundle.zip
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
- Interactive `audit` includes metadata verification against the provided `--repo`.
- `audit --verify-metadata` is the explicit non-interactive verification path and requires `--bundle`, `--repo`, and `--format`.
- `receive` requires prerequisite history to already exist in the receiver repository.
- `receive --verify-metadata` validates bundle/sidecar integrity before import.
- `receive --dry-run` applies into an isolated temporary bare mirror and does not mutate the receiver repo.

## For Developers

### Build and Run

```bash
cargo build
cargo build --release
./target/debug/git-sync --help
./target/release/git-sync --help
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
- `release` publishes debug/release binaries, Debian/Arch packages, docs archive, and coverage report to GitHub Releases.
- Release notes are generated from the matching version section in `CHANGELOG.md`.

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

## Technical Notes

- Runtime Git operations are in-process via `git2` (libgit2 bindings); core `create`/`audit`/`receive` paths do not shell out to `git`.
- Receiving the same package repeatedly is idempotent: existing refs/objects are reused and results remain deterministic.
- Binary/symlink (non-text) file changes are handled safely: line counts are `0/0` and diff-open actions no-op for unavailable textual patches.
