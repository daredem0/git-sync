set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Show all available recipes.
help:
    @just --list

# Check local tooling for the selected tier: core, quality, release, all.
preflight tier="all":
    ./scripts/check-tooling.sh {{tier}}

# Install development tooling on Arch Linux for the selected tier.
setup-arch tier="all":
    ./scripts/setup-dev-arch.sh {{tier}}

# Install development tooling on Debian/Ubuntu for the selected tier.
setup-ubuntu tier="all":
    ./scripts/setup-dev-ubuntu.sh {{tier}}

# Build debug binary.
core-build:
    cargo build --locked

# Build release binary.
core-build-release:
    cargo build --locked --release

# Run full test suite.
core-test:
    cargo test --locked --all-targets

# Show CLI help from debug binary.
core-run-help:
    cargo run --locked -- --help

# Run Clippy
quality-clippy:
    cargo clippy -- -D warnings

# Check formatting
quality-fmt:
    cargo fmt --all -- --check

# Run coverage summary.
quality-coverage:
    cargo llvm-cov --workspace --all-features --summary-only

# Generate rustdoc with Mermaid support (public items only).
quality-docs:
    RUSTDOCFLAGS="--html-in-header docs/mermaid-header.html" cargo doc --locked --no-deps --bins

# Generate rustdoc including private items.
quality-docs-private:
    RUSTDOCFLAGS="--html-in-header docs/mermaid-header.html" cargo doc --locked --no-deps --bins --document-private-items

# Generate flattened rustdoc PDF (regenerates docs with private items first).
quality-docs-pdf: quality-docs-private
    ./scripts/generate-doc-pdf.sh

# Verify dependency license policy with cargo-deny.
quality-licenses-check:
    ./scripts/check-licenses.sh

# Regenerate third-party license inventory.
quality-licenses-generate:
    ./scripts/generate-third-party-licenses.sh

# Generate man pages from README and SDD.
release-manpages:
    ./scripts/generate-manpages.sh

# Build Debian package.
release-deb:
    ./scripts/build-deb.sh

# Build Arch package.
release-arch:
    ./scripts/build-arch.sh

# Build both Linux package types.
release-packages: release-deb release-arch

# Common local CI check subset.
ci-local: core-build core-test quality-coverage quality-docs
