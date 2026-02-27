# git-sync-audit

`git-sync-audit` is a command-line tool for moving Git history across disconnected environments while keeping that transfer auditable.

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
