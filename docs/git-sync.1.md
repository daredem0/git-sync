# git-sync

## Name

git-sync - air-gap Git sync and audit CLI

## Synopsis

**git-sync** [**--version**] [**--help**] \<command\> [options]

## Description

`git-sync` creates auditable Git transfer packages for disconnected
environments and provides tooling to review, verify, and receive them.

Primary command groups:

- `create` - create a package from a linear commit range
- `audit` - inspect package/repo changes (interactive TUI or machine-readable output)
- `ui` - open the interactive audit interface directly
- `receive` - import package content into a target repository

## Commands

### create

Create a transport package:

`git-sync create --repo <path> --from <rev> --to <rev> --output <bundle> [--assume-present <rev> ...]`

Optional:

- `--with-patches` - include a unified patch sidecar in the package
- `--assume-present <rev>` - repeatable; exclude objects already reachable from `<rev>` when that commit is reachable from `--to`

### audit

Interactive mode:

`git-sync audit --repo <path> --bundle <bundle.zip>`

Non-interactive mode:

`git-sync audit --bundle <bundle.zip> --format tsv|json`

or

`git-sync audit --repo <path> --from <rev> --to <rev> --format tsv|json`

Verification mode:

`git-sync audit --bundle <bundle.zip> --repo <path> --verify-metadata --format tsv|json`

### ui

Open the interactive UI explicitly:

`git-sync ui --repo <path> --bundle <bundle.zip> [--base <rev>] [--tip <rev>]`

### receive

Receive package content into a repository:

`git-sync receive --repo <path> --bundle <bundle.zip> [--verify-metadata] [--dry-run]`

## Exit Status

Returns `0` on success and non-zero on failure.

## See Also

- `git-sync-readme(7)`
- `git-sync-architecture(7)`
