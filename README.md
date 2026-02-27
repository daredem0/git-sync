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
- `Enter`: reserved for file diff view (planned)
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
h/Left prev page | l/Right next page | j/k or Up/Down move | Enter open (planned) | ? help | q quit
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
│Commit 3/9 | fef480558abd352fe2d1c16dd01e8c5587567217                                             │
│feat(login): wire login settings                                                                  │
│committer date: 1704067200 (UTC+00:00)                                                            │
│committer: Audit Bot <audit@example.com>                                                          │
│author date: 1704067200 (UTC+00:00)                                                               │
│author: Audit Bot <audit@example.com>                                                             │
│Changed files: 2                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
┌Changed Files (this commit)───────────────────────────────────────────────────────────────────────┐
│PATH                                                                            +LINES    -LINES  │
│config.ini                                                                      1         0       │
│src/login.txt                                                                   1         0       │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
│                                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
h/Left prev page | l/Right next page | j/k or Up/Down move | Enter open (planned) | ? help | q quit
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

- Add interactive file-diff view on commit pages (`Enter` action).
- Add package authenticity verification using detached `Ed25519` signatures.
- Signing target: final transfer artifact (`sync.bundle.zip`) as raw bytes.
- Planned output on create: `sync.bundle.zip.sig`.
- Planned verification inputs on audit/receive: detached signature + trusted public key.
- Enforcement goal: reject package when signature is missing or invalid.

## Implementation Notes

- Uses `libgit2` bindings through the `git2` crate.
- Core Git operations are implemented in-process (no `git` CLI calls in core logic).
