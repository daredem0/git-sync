# git-sync-audit

Air-gap Git sync and audit tool built in Rust.

## Current capabilities

- Create transport packages as `.zip` archives containing:
  - `<name>.bundle`
  - `<name>.bundle.caudit.json`
  - optional `<name>.bundle.caudit.patch`
- Audit packages interactively (TUI overview) via `audit` without `--format`.
- Audit non-interactively as TSV/JSON manifests via `audit --format ...`.
- Verify package metadata against repository truth.
- Receive packages into a receiver repo.
- Simulate receive (`--dry-run`) with:
  - applicability check (`bundle can be applied without conflicts`)
  - per-file line summary (`PATH`, `+LINES`, `-LINES`)

Implementation notes:
- Uses `libgit2` bindings through the `git2` crate.
- Core Git operations are implemented in-process (no `git` CLI calls in core logic).

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

## Commands

### Create bundle package

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle
```

Output:
- only `sync.bundle.zip` is kept on disk by the CLI

Include a unified patch sidecar:

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle --with-patches
```

### Audit (interactive TUI overview; default)

```bash
cargo run -- audit --repo /path/to/repo --bundle /path/to/sync.bundle.zip
```

Behavior:
- Opens the first audit page (general package info, verification status, heads to import, would-change summary).
- Exit with `q` or `Esc`.

### Audit (non-interactive bundle manifest)

```bash
cargo run -- audit --bundle sync.bundle.zip --format tsv
```

### Audit (non-interactive repo range manifest)

```bash
cargo run -- audit --repo . --from <from-rev> --to <to-rev> --format tsv
```

### Verify package metadata against a repo

```bash
cargo run -- audit --bundle sync.bundle.zip --repo . --verify-metadata --format tsv
```

### Receive bundle into receiver repo

```bash
cargo run -- receive --repo /path/to/receiver-repo --bundle /path/to/sync.bundle.zip --verify-metadata
```

### Receive dry-run (no receiver writes)

```bash
cargo run -- receive --repo /path/to/receiver-repo --bundle /path/to/sync.bundle.zip --verify-metadata --dry-run
```

Also available:
- `ui` command exists as a direct TUI entrypoint (`ui --repo ... --bundle ...`), but `audit` is the primary audit command.

## Constraints

- `create --from ... --to ...` requires `to` to be the same as or a descendant of `from`.
- `audit` behavior depends on `--format`:
  - no `--format`: interactive TUI mode; requires `--repo` and `--bundle`
  - `--format tsv|json`: non-interactive mode; supports bundle mode (`--bundle`) or repo-range mode (`--repo --from --to`)
- `audit --verify-metadata` is non-interactive and requires `--bundle`, `--repo`, and `--format`.
- `receive` requires the receiver repo to already contain prerequisite history referenced by the bundle.
- `receive --verify-metadata` verifies bundle and sidecar integrity before import.
- `receive --dry-run` performs apply simulation in an isolated temporary bare repo and does not modify the receiver repo.
- Metadata schema is defined in `schemas/sync.bundle.caudit.schema.json`.

## Open TODO

- Add commit-by-commit interactive audit pages (tree view + per-file diff per commit).
- Add package authenticity verification using detached `Ed25519` signatures.
- Signing target: the final transfer artifact (`sync.bundle.zip`) as raw bytes.
- Planned output on create: `sync.bundle.zip.sig`.
- Planned verification inputs on audit/receive: detached signature + trusted public key.
- Enforcement goal: reject package when signature is missing or invalid.
