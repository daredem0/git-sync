# git-sync-audit

Air-gap Git sync and audit tool built in Rust.

Implementation notes:
- Uses `libgit2` bindings through the `git2` crate.
- Core Git operations are implemented in-process (no `git` CLI calls in core logic).
- Bundle transport format is a `.zip` package containing:
  - `<name>.bundle`
  - `<name>.bundle.caudit.json`
  - optional `<name>.bundle.caudit.patch`

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

## Commands

### Create bundle package

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle
```

Output:
- only `sync.bundle.zip` is kept on disk by the CLI

To include a full unified patch sidecar:

```bash
cargo run -- create --repo . --from <from-rev> --to <to-rev> --output sync.bundle --with-patches
```

### Audit bundle package

```bash
cargo run -- audit --bundle sync.bundle.zip --format tsv
```

### Audit repo range

```bash
cargo run -- audit --repo . --from <from-rev> --to <to-rev> --format tsv
```

### Verify bundle metadata against a repo

```bash
cargo run -- audit --bundle sync.bundle.zip --repo . --verify-metadata --format tsv
```

### Receive bundle into receiver repo

```bash
cargo run -- receive --repo /path/to/receiver-repo --bundle /path/to/sync.bundle.zip --verify-metadata
```

## Constraints

- `create --from ... --to ...` requires `to` to be the same as or a descendant of `from`.
- `audit` mode is exclusive:
  - bundle mode: `--bundle` only
  - repo mode: `--repo --from --to`
- `audit --verify-metadata` requires both `--bundle` and `--repo` and checks metadata against repository truth.
- `receive` requires the receiver repo to already contain prerequisite history referenced by the bundle.
- `receive --verify-metadata` verifies bundle and sidecar integrity before import.
- Metadata schema is defined in `schemas/sync.bundle.caudit.schema.json`.
