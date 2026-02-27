# git-sync-audit

Air-gap Git sync and audit tool built in Rust.

## What It Implements

- Creates transport packages as `.zip` archives containing:
  - `<name>.bundle`
  - `<name>.bundle.caudit.json`
  - optional `<name>.bundle.caudit.patch`
- Audits packages interactively (TUI) via `audit` without `--format`.
- Audits non-interactively as TSV/JSON manifests via `audit --format ...`.
- Verifies package metadata against repository truth.
- Receives packages into a receiver repo.
- Simulates receive (`--dry-run`) with:
  - applicability check (`bundle can be applied without conflicts`)
  - per-file line summary (`PATH`, `+LINES`, `-LINES`)

## Build

```bash
cargo build
```

For an optimized release binary:

```bash
cargo build --release
```

## Test

```bash
cargo test
```

Run only the end-to-end integration test:

```bash
cargo test --test bundle_workflow_integration -- --nocapture
```

## Coverage

```bash
cargo llvm-cov --workspace --all-features --summary-only
```

## CLI Workflow

### 1) Create package

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle
```

Result:
- CLI keeps only `sync.bundle.zip` on disk.

Include a unified patch sidecar:

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle --with-patches
```

### 2) Audit (interactive, default)

```bash
cargo run -- audit --repo /path/to/repo --bundle /path/to/sync.bundle.zip
```

This opens the TUI. Start on overview page, then page through commits.

### 3) Audit (non-interactive bundle manifest)

```bash
cargo run -- audit --bundle sync.bundle.zip --format tsv
```

### 4) Audit (non-interactive repo range manifest)

```bash
cargo run -- audit --repo . --from <from-rev> --to <to-rev> --format tsv
```

### 5) Verify package metadata against a repo

```bash
cargo run -- audit --bundle sync.bundle.zip --repo . --verify-metadata --format tsv
```

### 6) Receive package

```bash
cargo run -- receive --repo /path/to/receiver-repo --bundle /path/to/sync.bundle.zip --verify-metadata
```

### 7) Receive dry-run (no writes to receiver repo)

```bash
cargo run -- receive --repo /path/to/receiver-repo --bundle /path/to/sync.bundle.zip --verify-metadata --dry-run
```

Also available:
- `ui` command exists as a direct TUI entrypoint (`ui --repo ... --bundle ...`), but `audit` is the primary audit command.

## Interactive Audit UI

`audit` without `--format` opens the interactive audit UI.

### Navigation keys

- `h` / `Left`: previous page
- `l` / `Right`: next page
- `j` / `Down`: move selection down (commit file list)
- `k` / `Up`: move selection up (commit file list)
- `g`: first page
- `G`: last page
- `Enter`: open diff view for selected file on commit pages
- `?`: toggle help
- `q` / `Esc`: quit

### Page 1: package overview

```bash
cargo run -- audit --repo /path/to/repo --bundle /path/to/sync.bundle.zip
```

Shows:
- metadata verification result
- heads to import
- would-change per-file line summary
- total page position in the audit session

Preview:
```text
┌git-sync-audit────────────────────────────────────────────────────────────────────────────────────┐
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

Diff view controls:
- `j` / `Down`: scroll down
- `k` / `Up`: scroll up
- `h` / `Left`: horizontal scroll left
- `l` / `Right`: horizontal scroll right
- `PgUp` / `PgDn`: fast vertical scroll
- `Home`: reset scroll position
- `Esc`: close diff view and return to commit page

Preview:

```text
┌Diff View─────────────────────────────────────────────────────────────────────────────────────────┐
│Commit 4/? | aa7406fc5178e46f570027914655aeb27b550a15                                             │
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

## Constraints

- `create --from ... --to ...` requires `to` to be the same as or a descendant of `from`.
- `audit` behavior depends on `--format`:
  - no `--format`: interactive TUI mode; requires `--repo` and `--bundle`
  - `--format tsv|json`: non-interactive mode; supports bundle mode (`--bundle`) or repo-range mode (`--repo --from --to`)
- `audit --verify-metadata` is non-interactive and requires `--bundle`, `--repo`, and `--format`.
- `receive` requires receiver repo to already contain prerequisite history referenced by the bundle.
- `receive --verify-metadata` verifies bundle and sidecar integrity before import.
- `receive --dry-run` simulates apply in an isolated temporary bare repo and does not modify the receiver repo.
- Metadata schema is defined in `schemas/sync.bundle.caudit.schema.json`.

## Open TODO

- Add package authenticity verification using detached `Ed25519` signatures.
- Signing target: final transfer artifact (`sync.bundle.zip`) as raw bytes.
- Planned output on create: `sync.bundle.zip.sig`.
- Planned verification inputs on audit/receive: detached signature + trusted public key.
- Enforcement goal: reject package when signature is missing or invalid.

## Implementation Notes

- Uses `libgit2` bindings through the `git2` crate.
- Core Git operations are implemented in-process (no `git` CLI calls in core logic).
