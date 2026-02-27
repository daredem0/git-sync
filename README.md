# git-sync-audit

Scaffold for an air-gap Git sync audit tool.

Planned characteristics:
- Rust implementation.
- Uses `libgit2` bindings via the `git2` crate (no `git` CLI in core logic).
- Supports bundle/repo inspection and a terminal UI workflow.

Current status:
- Project and command structure are bootstrapped.
- Functional auditing and UI behavior are intentionally not implemented yet.

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
