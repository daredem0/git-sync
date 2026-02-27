# git-sync-audit

Scaffold for an air-gap Git sync audit tool.

Planned characteristics:
- Rust implementation.
- Uses `libgit2` bindings via the `git2` crate (no `git` CLI in core logic).
- Supports bundle/repo inspection and a terminal UI workflow.

Current status:
- Project and command structure are bootstrapped.
- `create` writes a bundle and a `.caudit.json` metadata sidecar.
- `.caudit.json` is compact by default (no inline file patch content).
- `create --with-patches` adds an optional `.caudit.patch` sidecar.
- `audit --bundle` renders changed-file manifests from bundled metadata.
- `audit --repo --from --to` renders changed-file manifests directly from a repository range.
- `audit --verify-metadata` validates bundled metadata against repository truth.

## Build

```bash
cargo build
```

For an optimized release binary:

```bash
cargo build --release
```

## Run unit tests

```bash
cargo test
```

Run only the end-to-end workflow integration test:

```bash
cargo test --test bundle_workflow_integration -- --nocapture
```

## Create bundle + metadata

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle
```

This creates:
- `sync.bundle.zip` only

The archive contains:
- `sync.bundle`
- `sync.bundle.caudit.json`

To include a full unified patch sidecar:

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle --with-patches
```

This additionally creates:
- no extra loose files; `sync.bundle.caudit.patch` is included inside the zip archive

The metadata JSON schema is defined at:
- `schemas/sync.bundle.caudit.schema.json`

For audit commands, pass the zip package path via `--bundle` (for example `sync.bundle.zip`).
`audit --bundle ...` renders changed-file manifest data from the bundled `.caudit.json`.

## Verify bundle metadata against a repo

```bash
cargo run -- audit --bundle sync.bundle.zip --repo . --verify-metadata --format tsv
```

This validates:
- bundle hash and size recorded in `.caudit.json`
- bundle header fields (`version`, `prerequisites`, `heads`)
- metadata `commit_chain` and `changed_files` against repository truth for `range_from_oid..range_to_oid`
- optional patch sidecar hash/size when present

## End-to-end test flow with merge fixture

Generate a deterministic repo with merge commits and anchor tags (`sync/base`, `sync/tip`):

```bash
./scripts/generate-merge-graph-repo.sh /tmp/git-sync-fixture
```

Create the bundle package:

```bash
cargo run -- create --repo /tmp/git-sync-fixture --from sync/base --to sync/tip --output /tmp/sync.bundle
```

Audit from the bundle package:

```bash
cargo run -- audit --bundle /tmp/sync.bundle.zip --format tsv
```

Audit the same range directly from the repo:

```bash
cargo run -- audit --repo /tmp/git-sync-fixture --from sync/base --to sync/tip --format tsv
```

Verify metadata against the repo:

```bash
cargo run -- audit --bundle /tmp/sync.bundle.zip --repo /tmp/git-sync-fixture --verify-metadata --format tsv
```
