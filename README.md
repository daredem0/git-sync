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
- history overview (verification status, heads to import, dry-run summary)
- history commit pages with per-file line stats and file-level diff viewer
- payload view (transport entries + full pack-object inventory + object preview/detail)

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
- would-change per-file line summary for the currently selected head
- total page position in the audit session

Preview:
```text
┌git-sync───────────────────────────────────────────────────────────────────────────────────────────────┐
│Audit Overview (page 1/14)                                                                             │
│This page shows package validity, import heads, and would-change summary                               │
│Use h/l or left/right to move pages                                                                    │
│                                                                                                       │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌General────────────────────────────────────────────────────────────────────────────────────────────────┐
│tool version: 0.3.2-13-g7a57de1-dirty                                                                  │
│repo: .                                                                                                │
│bundle: ../git-sync-examples/sync_local.bundle.zip                                                     │
│base_ref: sync/last | tip_ref: -                                                                       │
│metadata verification: OK                                                                              │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Heads To Import (bundle v2)──────────────────┐┌Would Change (selected head: refs/heads/main)───────────┐
│OID                      REF                 ││PATH                                  +LINES    -LINES  │
│05b1f9a42fd3831e72f1487  refs/heads/main     ││Cargo.lock                            333       0       │
│                                             ││Cargo.toml                            2         0       │
│                                             ││LICENSE                               201       0       │
│                                             ││README.md                             203       9       │
│                                             ││schemas/sync.bundle.caudit.schema.js  278       0       │
│                                             ││scripts/generate-merge-graph-repo.sh  158       0       │
│                                             ││src/cli.rs                            166       7       │
└─────────────────────────────────────────────┘└────────────────────────────────────────────────────────┘
Tab/v toggle history/payload | h/Left prev page | l/Right next page | j/k or Up/Down move selection      
Enter open selected head/diff | Esc overview/quit | ? help | q quit                                      
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
┌Commit Detail──────────────────────────────────────────────────────────────────────────────────────────┐
│Head 1/1 | refs/heads/main                                                                             │
│Commit 2/13 | 440ec8ae7645d0954ee0f26fbb5aca63a8925e91                                                 │
│Change: Generate metadata file when bundling                                                           │
│committer date: 1772215309 (UTC+01:00)                                                                 │
│committer: Florian Leuze <f.leuze@outlook.de>                                                          │
│author date: 1772215309 (UTC+01:00)                                                                    │
│author: Florian Leuze <f.leuze@outlook.de>                                                             │
│Changed files: 8                                                                                       │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Changed Files (this commit)────────────────────────────────────────────────────────────────────────────┐
│PATH                                                                                 +LINES    -LINES  │
│Cargo.lock                                                                           1         0       │
│Cargo.toml                                                                           1         0       │
│README.md                                                                            26        1       │
│schemas/sync.bundle.caudit.schema.json                                               268       0       │
│src/cli.rs                                                                           142       5       │
│src/git/mod.rs                                                                       493       13      │
│src/git/tests.rs                                                                     357       0       │
│src/main.rs                                                                          61        25      │
│                                                                                                       │
│                                                                                                       │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
h/Left prev page | l/Right next page | j/k or Up/Down move selection                                     
Enter open selected head/diff | Esc overview/quit | ? help | q quit                                      
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
┌Diff View──────────────────────────────────────────────────────────────────────────────────────────────┐
│Commit 2/13 | 440ec8ae7645d0954ee0f26fbb5aca63a8925e91                                                 │
│Change: Generate metadata file when bundling                                                           │
│file: src/main.rs                                                                                      │
│syntax: Rust | selected file index: 8                                                                  │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Patch (first-parent commit diff)───────────────────────────────────────────────────────────────────────┐
│              │ diff --git a/src/main.rs b/src/main.rs                                                 │
│              │ index b9a2912..f669ced 100644                                                          │
│              │ --- a/src/main.rs                                                                      │
│              │ +++ b/src/main.rs                                                                      │
│              │ @@ -6,8 +6,12 @@ mod ui;                                                               │
│     6      6 │  use anyhow::Result;                                                                   │
│     7      7 │  use app::AppConfig;                                                                   │
│     8      8 │  use clap::Parser;                                                                     │
│     9        │ -use cli::{Cli, Command, OutputFormat};                                                │
│    10        │ -use git::{collect_changed_files, render_manifest, render_manifest_json};              │
│            9 │ +use cli::{AuditTarget, Cli, Command, OutputFormat, resolve_audit_target};             │
│           10 │ +use git::{                                                                            │
│           11 │ +    CreateBundleOptions, collect_changed_files, create_bundle, create_bundle_with_opti│
│           12 │ +    render_bundle_inspection_json, render_bundle_inspection_tsv, render_manifest,     │
│           13 │ +    render_manifest_json, resolve_repo_audit_range,                                   │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset                
Esc back | ? help | q quit                                                                               
```

### Payload view: transport entries + pack-object inventory

This is the authoritative payload-oriented page. It shows:
- all non-pack transport entries in the `.zip` (name, size, SHA256)
- all imported pack objects (OID, type, size, reachable flag)
- a right-side preview of the currently selected object
- sorting modes (`canonical`, `context`) toggled with `s`

```text
┌git-sync───────────────────────────────────────────────────────────────────────────────────────────────┐
│Payload View                                                                                           │
│Transport package entries, selected-object preview, and full pack object listing                       │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Transport Entries─────────────────────────────┐┌Pack Preview───────────────────────────────────────────┐
│ENTRY                  SIZE       SHA256      ││selected: 05b1f9a42fd3831e72f1487e760b635461956bae (com│
│sync_local.bundle      110458     6bd8c0359321││                                                       │
│sync_local.bundle.caud 18799      31f0fde4f62c││commit 05b1f9a42fd3831e72f1487e760b635461956bae        │
│                                              ││tree 253304710bf320637a58b25b114389753233d5bd          │
│                                              ││parent 4f0388416d0ceeb327e65cdffa61e0e1b8476368        │
│                                              ││author Florian Leuze <f.leuze@outlook.de> 1772226508 60│
│                                              ││committer Florian Leuze <f.leuze@outlook.de> 1772226508│
└──────────────────────────────────────────────┘│                                                       │
┌Pack Objects (153 total, 1 heads, sort: canoni┐│Change: Add commit-level audit pages in TUI with author│
│OID          TYPE     SIZE       REACHABLE    ││                                                       │
│05b1f9a42fd3 commit   768        yes          ││- extend commit audit entries to include author/committ│
│1b91006ee9d4 commit   676        yes          ││- populate commit metadata in bundle receive/audit coll│
│2983f7913a8d commit   283        yes          ││- add paged TUI commit detail view and navigation keyma│
│30b9dd00d4ca commit   267        yes          ││- keep `Enter` mapped as planned placeholder for future│
│440ec8ae7645 commit   261        yes          ││- add tests for commit audit identity propagation and U│
│4f0388416d0c commit   776        yes          ││- restructure README and document interactive audit UI │
│7146b57b1949 commit   263        yes          ││                                                       │
└──────────────────────────────────────────────┘└───────────────────────────────────────────────────────┘
j/k or Up/Down select object | PgUp/PgDn jump 10 | s cycle sort | Enter open object detail               
Tab/v toggle history/payload | ? help | q quit                                                           
```

### Payload view: text-blob preview

For text blobs, preview includes syntax-highlighted content with line numbers.  
Long previews are clipped to the panel height and end with a `... (N more lines)` marker.

```text
┌git-sync───────────────────────────────────────────────────────────────────────────────────────────────┐
│Payload View                                                                                           │
│Transport package entries, selected-object preview, and full pack object listing                       │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Transport Entries─────────────────────────────┐┌Pack Preview───────────────────────────────────────────┐
│ENTRY                  SIZE       SHA256      ││selected: 5da6d656861f52bc46368ea6b570e1bcb8ef170a (blo│
│sync_local.bundle      110458     6bd8c0359321││                                                       │
│sync_local.bundle.caud 18799      31f0fde4f62c││blob 5da6d656861f52bc46368ea6b570e1bcb8ef170a          │
│                                              ││size: 67903 bytes                                      │
│                                              ││content: text                                          │
│                                              ││text lines: 1818                                       │
│                                              ││blob paths: 1                                          │
└──────────────────────────────────────────────┘│  - src/git/tests.rs                                   │
┌Pack Objects (153 total, 1 heads, sort: canoni┐│                                                       │
│OID          TYPE     SIZE       REACHABLE    ││content preview:                                       │
│46ee30615323 blob     2525       yes          ││1 │ use super::*;                                      │
│494e1878759f blob     5654       yes          ││2 │ use std::path::PathBuf;                            │
│4b77ef80e654 blob     650        yes          ││3 │                                                    │
│4c6ba1c9bca1 blob     6154       yes          ││4 │ // Verifies that open_context rejects a repository │
│5551223aa9ec blob     366        yes          ││5 │ #[test]                                            │
│5662e7c31dcc blob     8184       yes          ││6 │ fn open_context_fails_when_repo_path_does_not_exist│
│5da6d656861f blob     67903      yes          ││... (1812 more lines)                                  │
└──────────────────────────────────────────────┘└───────────────────────────────────────────────────────┘
j/k or Up/Down select object | PgUp/PgDn jump 10 | s cycle sort | Enter open object detail               
Tab/v toggle history/payload | ? help | q quit                                                           
```

### Payload object detail view (opened from payload page with `Enter`)

This is the full object-detail view for the selected pack object.  
It supports vertical/horizontal scrolling and reuses syntax highlighting for text blobs.

```text
┌git-sync───────────────────────────────────────────────────────────────────────────────────────────────┐
│Payload Object Detail                                                                                  │
│oid: 5da6d656861f52bc46368ea6b570e1bcb8ef170a                                                          │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Object Content─────────────────────────────────────────────────────────────────────────────────────────┐
│   8 │ // Verifies that open_context rejects a repository path that does not exist.                    │
│   9 │ #[test]                                                                                         │
│  10 │ fn open_context_fails_when_repo_path_does_not_exist() {                                         │
│  11 │     let repo_path = std::env::temp_dir().join(format!(                                          │
│  12 │         "git-sync-audit-missing-repo-{}-{}",                                                    │
│  13 │         std::process::id(),                                                                     │
│  14 │         std::time::SystemTime::now()                                                            │
│  15 │             .duration_since(std::time::UNIX_EPOCH)                                              │
│  16 │             .expect("system clock should be after unix epoch")                                  │
│  17 │             .as_nanos()                                                                         │
│  18 │     ));                                                                                         │
│  19 │                                                                                                 │
│  20 │     let cfg = AppConfig {                                                                       │
│  21 │         repo_path,                                                                              │
│  22 │         bundle_path: PathBuf::from("unused.bundle"),                                            │
│  23 │         base_ref: "sync/last".to_string(),                                                      │
│  24 │         tip_ref: None,                                                                          │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset                
Esc back to payload list | ? help | q quit                                                               
```

## Interactive UI Keys

Overview page (page 1):
- `Tab` / `v`: toggle History and Payload views
- `1`: switch to History view
- `2`: switch to Payload view
- In History view: `j` / `k` selects head, `Enter` opens commit pages for selected head
- In Payload view: `j` / `k` selects object, `PgUp` / `PgDn` jumps by 10 objects, `s` cycles sort mode, `Enter` opens object detail

History commit pages:
- `h` / `Left`: previous commit page
- `l` / `Right`: next commit page
- `j` / `Down`: move changed-file selection down
- `k` / `Up`: move changed-file selection up
- `g`: first history page (overview)
- `G`: last commit page (for selected head)
- `Enter`: open diff for selected changed file
- `Esc`: return to overview

Diff view:
- `j` / `Down`: scroll down
- `k` / `Up`: scroll up
- `h` / `Left`: scroll left
- `l` / `Right`: scroll right
- `PgUp` / `PgDn`: fast vertical scroll
- `Home`: reset diff scroll
- `Esc`: close diff view and return to commit page

Payload object detail view:
- `j` / `Down`: scroll down
- `k` / `Up`: scroll up
- `h` / `Left`: scroll left
- `l` / `Right`: scroll right
- `PgUp` / `PgDn`: fast vertical scroll
- `Home`: reset detail scroll
- `Esc`: close object detail and return to payload list

Global:
- `?`: toggle help overlay
- `q`: quit
- `Esc`: quit from overview/payload main page

## Constraints and Behavior Notes

- `create --from ... --to ...` requires `to` to be equal to or a descendant of `from`.
- `audit` without `--format` is interactive TUI mode and requires `--repo` and `--bundle`.
- Interactive `audit` includes metadata verification against the provided `--repo`.
- Interactive `audit` shows two top-level views: History (head/commit/file diffs) and Payload (transport + pack objects).
- History pages are head-scoped for multi-head bundles: select a head on overview, then `Enter` to inspect that head's commit chain.
- Payload view includes all imported pack objects; `REACHABLE=no` marks objects not reachable from advertised heads.
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
