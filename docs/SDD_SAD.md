# Software Design and Architecture Description (SDD/SAD)
## Project: `git-sync`

## 1. Purpose

This document describes the implemented design of `git-sync` for both engineering and audit audiences. It is written as a development-facing architecture description, but with an explicit emphasis on payload-proof guarantees and air-gap transfer safety.

The main readers are:

- developers extending command behavior, payload processing, or TUI features
- maintainers reviewing design consistency during refactors
- auditors/security reviewers validating proof claims and transfer-gate semantics

The core promise of `git-sync` is that transfer decisions can be made from explicit, inspectable evidence. The tool does not rely on a single convenience view (such as commit reachability or sidecar metadata). Instead, it combines repository truth, metadata truth, and payload truth, with the PACK payload treated as the authoritative source for completeness of transferred content.

This document is intentionally split into a static view and a dynamic view:

- static view: architecture, modules, boundaries, models, and invariants
- dynamic view: runtime command workflows and sequence behavior

## 2. System Context

`git-sync` operates in an air-gap transfer workflow where packaging, review, and controlled import are separate phases. The system context below defines that boundary, the command surfaces used at each phase, and why payload proofing is part of the transfer contract rather than an optional check.

`git-sync` is used in air-gap Git transfer workflows. A producer repository exports a range into a portable package, the package is reviewed on the receiving side, and import is performed only if policy and proof checks pass.

Primary command surfaces:

- `create`: produce a transport package from a linear commit range
- `audit` (interactive default): TUI review of history and payload
- `audit --format table|json`: non-interactive payload audit output
  - `--payload-ledger summary|full` (JSON)
  - `--resolve pack-only|baseline` (non-interactive)
- `audit --verify-metadata`: explicit metadata-to-repo verification check
- `ui`: explicit interactive entrypoint
- `receive`: import package into receiver repo with explicit integration policy
  - optional `--dry-run`, `--integrate`, `--incoming-as-branches`, `--check-mergeability`, `--verbose`, `--format table|json`

High-level workflow:

```mermaid
flowchart LR
    SRC[(Source Repository)]
    CREATE[create]
    PKG[[sync.bundle.zip]]
    AUDIT[audit]
    RECEIVE[receive]
    DST[(Receiver Repository)]

    SRC --> CREATE --> PKG
    PKG --> AUDIT
    PKG --> RECEIVE --> DST
```

Trust boundary and verification gates:

```mermaid
flowchart LR
    subgraph PROD["Producer Host"]
        SRC[(Source Repository)]
        CREATE[create]
    end
    subgraph TRANSFER["Transfer Unit"]
        PKG[[sync.bundle.zip]]
    end
    subgraph RECV["Auditor / Receiver Host"]
        AUDIT[audit UI-table-JSON]
        VM[audit --verify-metadata]
        GATE{Transfer Gate Decision}
        RECEIVE[receive and receive --dry-run]
        DST[(Receiver Repository)]
    end

    SRC --> CREATE --> PKG
    PKG --> AUDIT --> GATE
    PKG --> VM --> GATE
    GATE --> RECEIVE --> DST
```

Two audits coexist and solve different problems:

- payload audit (PACK truth): proves what bytes and objects are in the package payload
- metadata verification (repo truth): proves sidecar claims match recomputed repository truth

The payload audit is the key security argument for smuggling resistance and transfer gating.

## 3. Architectural Style

The implementation follows a layered style: command orchestration at the CLI boundary, truth computation in the Git domain, and presentation in the UI layer. Determinism, explicit truth boundaries, and fail-closed behavior are treated as design constraints across all layers.

`git-sync` is a single-process Rust application that combines CLI orchestration and an optional TUI. Core Git operations are performed in-process through `git2`/libgit2. Security-sensitive paths use explicit parsing and proof-boundary types rather than depending on implicit behavior from import side effects.

Architectural characteristics:

- deterministic transformations where possible (stable ordering and predictable rows)
- explicit domain models (`src/git/types/*`) and typed, actionable errors
- fail-closed behavior in payload-proofing and transfer-gate paths
- isolated dry-run receive path via temporary bare mirrors
- separation of truth sources:
  - transport truth (bytes in package)
  - payload truth (PACK entry stream proof)
  - repository truth (range recomputation)
  - metadata truth (sidecar claims)

### 3.1 Quality attributes and acceptance criteria

The quality attributes below are treated as acceptance criteria for architecture changes. A feature that weakens one of these attributes must introduce compensating controls and explicit test coverage.

| Quality attribute | Operational interpretation | Acceptance criteria | Primary evidence |
|---|---|---|---|
| Payload completeness assurance | Transfer decisions are based on PACK entry accounting, not reachability alone | `entries_declared == entries_parsed == entries_materialized` and `transfer_allowed=true` for accepted payloads | `pack_proof` counters, ledger output, payload tests in `src/git/tests/payload_tests.rs` |
| Fail-closed behavior | Parsing or proof ambiguity blocks approval/import paths | Malformed framing, checksum mismatch, unresolved deltas, or counter mismatch produce errors (no synthetic success) | `src/git/bundle/parse.rs`, `src/git/bundle/payload/verify/*`, negative-path tests |
| Deterministic review output | Repeated runs on identical inputs produce stable ordering/counters | Stable table/JSON row ordering and deterministic derived payload projections | deterministic payload tests and UI/render tests |
| Dry-run safety with operational parity | Impact preview uses equivalent import logic without mutating target repo | `receive --dry-run` computes applicability/stats and leaves receiver unchanged | dry-run tests in `src/git/tests/receive_tests.rs` |
| Traceable provenance | Operator-visible evidence includes tool/build identity | version appears in CLI/UI and exported documents/metadata fields | `src/version.rs`, CLI/UI paths, exported documents |

### 3.2 Key architecture decisions and trade-offs

The following decisions capture deliberate architectural trade-offs that shape both implementation and audit posture.

| Decision | Why this was chosen | Trade-off accepted | Future extension path |
|---|---|---|---|
| PACK entry stream is the proof unit | It answers what bytes crossed the boundary | More implementation complexity than simple reachability reporting | Keep reachability as context only; preserve ledger authority |
| Strict bundle framing (no heuristic PACK scan) | Removes ambiguity in payload start and parser behavior | Rejects loosely formatted but potentially recoverable inputs | Maintain strict parse contract; document failures clearly |
| Separate metadata (`.caudit`) from payload proof (`.paudit`) | Keeps claim-verification and byte-proof concerns independent | Two documents to maintain and review | Optional policy profiles can combine both decisions explicitly |
| `pack-only` as strict default resolve policy | Makes hidden external dependencies fail closed | Some payloads require explicit baseline mode | Keep baseline as explicit operator choice with recorded provenance |
| UI is presentation-only for proof data | Prevents renderer changes from altering proof semantics | Additional model/projection layer needed | Continue exposing proof tuple consistently across surfaces |
| Dry-run through temporary mirror | Mirrors receive behavior while preserving receiver state | Extra temporary repo setup cost | Keep parity tests to prevent drift from apply path |
| Strict-first receive import with guarded compatibility fallback | Keeps maximum import verification by default while handling environment-specific libgit2 thin-pack behavior | Additional fallback complexity and diagnostic surface | Keep fallback narrowly scoped and preserve fail-closed connectivity validation before ref updates |

## 4. Static View

The static view captures compile-time structure and ownership boundaries across the codebase. It shows where responsibility sits for routing, proof computation, rendering, and artifact/document modeling.

### 4.1 High-level dependency graph

```mermaid
flowchart TD
    MAIN[src/main.rs]
    CLI[src/cli.rs]
    APP[src/app/*]
    UI[src/ui/*]
    GIT[src/git/*]
    LIBGIT2[git2 / libgit2]
    TERM[Ratatui + Crossterm]
    SYN[Syntect]

    MAIN --> CLI
    MAIN --> APP
    MAIN --> UI
    MAIN --> GIT

    UI --> GIT
    UI --> TERM
    UI --> SYN

    GIT --> LIBGIT2
```

The key separation is intentional: UI is a renderer/state machine, not a proof engine. Parsing, verification, and proof invariants are owned by git-domain modules. UI consumes proof outputs and derived projections.

### 4.2 Command contract and static behavior

#### `create`

`git-sync create --repo --from --to --output [--with-patches]`

`create` validates linearity (`to` must be equal to or descendant of `from`), produces bundle + sidecars, packages them into `<output>.zip`, and then removes loose artifacts in default command flow so that the transport unit is a single archive.

#### `audit`

Interactive mode:

- `git-sync audit --repo --bundle`

Non-interactive payload mode:

- `git-sync audit --repo --bundle --format table`
- `git-sync audit --repo --bundle --format json [--payload-ledger summary|full]`
- optional resolve strategy: `--resolve pack-only|baseline`

Explicit metadata verify mode:

- `git-sync audit --repo --bundle --verify-metadata`

Interactive note:

- interactive `audit` currently permits `--resolve pack-only` only

#### `ui`

`git-sync ui --repo --bundle [--base sync/last] [--tip ...]`

This is the explicit direct TUI entrypoint.

#### `receive`

`git-sync receive --repo --bundle [--verify-metadata] [--dry-run] [--integrate create-refs-only|fast-forward-only|merge] [--incoming-as-branches] [--check-mergeability] [--verbose] [--format table|json]`

`receive` supports `.bundle` and `.zip` inputs, can verify metadata integrity before import, and computes a deterministic per-ref preflight plan before any target ref update.

Implementation-level receive behavior:

- imported heads are always preserved under `refs/sync/incoming/<bundle-id>/...`
- optional incoming branch mirrors can be written under `refs/heads/incoming/<bundle-id>/...`
- dry-run and `--check-mergeability` execute against an isolated temporary bare mirror
- `--integrate fast-forward-only` blocks diverged updates and never rewinds target refs
- `--integrate create-refs-only` never mutates target refs
- `--integrate merge` only updates diverged refs when merge simulation is clean
- `--format table|json` is supported for dry-run/check-mergeability output surfaces (`table` is default human output)
- `--check-mergeability` reports per-ref merge context (target/incoming/base), compact graph context, and conflict file paths
- `--verbose` emits receive import diagnostics that include prerequisite/object-format/alternates/shallow context on import failures

### 4.3 Module decomposition

This decomposition is organized by **authority**. The command layer decides *what* to run, the git domain computes *truth-bearing data*, and the UI decides *how that data is presented*. That split is what keeps review-oriented rendering concerns from changing proof behavior.

| Layer | Main files | Responsibility | Boundary |
|---|---|---|---|
| Entrypoint + command orchestration | `src/main.rs`, `src/cli.rs`, `src/app.rs`, `src/app/commands/*`, `src/app/output/*`, `src/version.rs`, `build.rs` | Parse CLI input, route mode-specific workflows, format non-interactive output, expose runtime/tool version | Must not implement payload proof logic itself; delegates truth computation to git domain |
| Git domain (authoritative logic) | `src/git/bundle/*`, `src/git/bundle/payload/*`, `src/git/metadata/*`, `src/git/archive.rs`, `src/git/context.rs`, `src/git/diff.rs`, `src/git/digest.rs`, `src/git/util.rs`, `src/git/types/*` | Bundle lifecycle, strict framing, payload verification, metadata collection/verification, receive/dry-run operations, domain data models | Owns proof semantics, fail-closed checks, and transfer gating inputs |
| UI domain (read-only review) | `src/ui/runtime.rs`, `src/ui/model.rs`, `src/ui/input/*`, `src/ui/state/*`, `src/ui/render/*`, `src/ui/diff/*`, `src/ui/syntax.rs`, `src/ui/tests/*` | Build interactive review model, manage navigation/input state, render overview/history/payload/diff pages | Consumes verified and derived data; does not parse/prove payload bytes |

### 4.4 Concrete source module map

This map is intentionally explicit so developers can quickly locate responsibility boundaries.

#### CLI / command layer

| File | Responsibility |
|---|---|
| `src/main.rs` | Process entrypoint and top-level command execution |
| `src/cli.rs` | CLI argument model and validation |
| `src/app/commands/mod.rs` | Command dispatch routing |
| `src/app/commands/create.rs` | `create` command flow |
| `src/app/commands/audit.rs` | `audit` modes and metadata-verify command flow |
| `src/app/commands/receive.rs` | `receive` and dry-run command output flow |
| `src/app/commands/ui.rs` | Direct TUI command entrypoint |
| `src/app/output/{table,sections,layout,kind,json}.rs` | Non-interactive output formatting |

#### Git domain layer

| File | Responsibility |
|---|---|
| `src/git/bundle/create.rs` | Package generation and artifact cleanup |
| `src/git/bundle/inspect.rs` | Bundle header parsing and inspection |
| `src/git/bundle/parse.rs` | Strict bundle framing and PACK extraction |
| `src/git/bundle/payload.rs` | Payload audit API surface |
| `src/git/bundle/payload/verify/{core,preflight,entry,delta,materialized,proof}.rs` | PACK proof pipeline (parse, materialize, invariants) |
| `src/git/bundle/receive.rs` | Receive import path and dry-run analysis |
| `src/git/bundle/receive/{test_hooks,tests}.rs` | Receive fault-injection hooks and module-scoped regression tests |
| `src/git/metadata/{collect,load,patch,verify}.rs` | Metadata sidecar collection, loading, patch support, verification |
| `src/git/archive.rs` | Archive read/write and extraction IO |
| `src/git/context.rs` | Context resolution and precondition checks |
| `src/git/diff.rs` | Changed-file extraction and deterministic diff-entry ordering |
| `src/git/types/*.rs` | Domain and document data models |
| `src/git/digest.rs` | Cryptographic digest helpers |
| `src/git/util.rs` | Generic support helpers |

#### UI layer

| File | Responsibility |
|---|---|
| `src/ui/runtime.rs` | Terminal lifecycle and event loop |
| `src/ui/model.rs` | Audit model construction |
| `src/ui/input/{router,actions}.rs` | Keymap routing and action handling |
| `src/ui/state/{navigation,payload_ops,diff_ops}.rs` | View/navigation/payload state transitions |
| `src/ui/render/overview.rs` | Overview page rendering |
| `src/ui/render/commit.rs` | Commit page rendering |
| `src/ui/render/diff_view.rs` | Diff page rendering |
| `src/ui/render/payload/{mod,layout,tables/*,preview/*,detail,util}.rs` | Payload page rendering stack |
| `src/ui/diff/{parse,render,style}.rs` | Diff parse/render/style helpers |
| `src/ui/{input,render,diff}/test_api.rs` | Test-only adapter surfaces that keep production API boundaries narrow |

### 4.5 Artifact model and static data flow

`create` produces the following artifact set:

- `<name>.bundle`
- `<name>.bundle.caudit.json`
- optional `<name>.bundle.caudit.patch`
- packaged `<name>.bundle.zip`

The distributed transport unit is the `.zip` package.

A bundle contains:

- textual header (version, prerequisites, heads)
- mandatory blank-line terminator
- PACK payload beginning with literal `PACK`
- trailer checksum at end of PACK stream

`git-sync` enforces strict framing: it does not scan heuristically for `PACK` in arbitrary bytes; it parses header framing, then requires PACK at the exact boundary.

### 4.6 Metadata and payload document models

Sidecar schemas are versioned and validated in `schemas/`:

- metadata schema: `schemas/sync.bundle.caudit.schema.json`
- payload-audit schema: `schemas/sync.bundle.paudit.schema.json`

`git-sync` intentionally emits two different document types because they answer different questions.

The metadata sidecar (`.caudit.json`) is a **creation-time manifest**. It binds a produced package to a claimed repository range and records provenance/integrity material that can later be verified against both package bytes and repository truth.

| Metadata field group | What it represents | Why it exists |
|---|---|---|
| Bundle identity + integrity | path, size, hashes, bundle header version | Binds claims to concrete package bytes |
| Header linkage | prerequisites and heads extracted from bundle header | Makes bundle transport-level expectations explicit |
| Claimed range | `from`, `to`, `tip_ref` | States the intended logical change interval |
| Range evidence | `commit_chain`, `changed_files` | Human-reviewable and machine-checkable summary of claimed changes |
| Provenance + version | `generated_*`, `tool_version` | Supports traceability of who/when/with-which-tool generated the package |
| Optional patch sidecar integrity | patch path/size/hash/format | Ensures optional patch artifact is bound to same package evidence |

The payload audit document (`.paudit`) is an **audit-time proof/report document**. It describes what was actually proven from PACK bytes under the selected resolve policy and how that proof was projected for review.

| Payload document section | What it contains | How reviewers use it |
|---|---|---|
| Envelope metadata | schema/tool/provenance + bundle identity | Establishes report context and traceability |
| `transport_entries` | transport files with size/hash | Verifies packaged transport components |
| `pack_proof` | verification status, pack version, compatibility counters, entry/materialization counters, transfer gate, checksums | Primary proof tuple for completeness + integrity decisions |
| `entry_ledger` (`summary`/`full`) | authoritative stream-entry accounting with unresolved rows and optional full rows | Direct inspection of PACK entry coverage and failure context |
| `pack_summary`, `pack_objects`, `object_details` | derived object-level views and drill-down detail | Reviewer-friendly browsing after proof is established |

Relationship view of metadata and payload evidence:

```mermaid
flowchart TD
    PKG[[sync.bundle.zip / .bundle]]
    META[.caudit.json<br/>creation-time manifest]
    PVERIFY[PACK verifier + entry ledger]
    PAUDIT[.paudit document<br/>audit-time proof report]
    MVERIFY[metadata verification]
    REPO[(Repository truth)]
    DECIDE{Transfer decision inputs}

    PKG --> META
    PKG --> PVERIFY --> PAUDIT
    META --> MVERIFY
    REPO --> MVERIFY
    MVERIFY --> DECIDE
    PAUDIT --> DECIDE
```

Design consequence: metadata can be fully valid while payload proof fails, and payload proof can succeed even if metadata claims are missing or inconsistent. Therefore transfer-gate decisions that depend on completeness are anchored in payload proof (`pack_proof` + ledger), not metadata alone.

## 5. Dynamic View

Runtime behavior is documented here as command-oriented execution paths, with sequence-level detail for create, audit, payload verification, and receive flows.

### 5.1 Create package sequence

Create sequence:

```mermaid
sequenceDiagram
    participant U as User
    participant C as CLI(create)
    participant G as git::bundle::create
    participant R as libgit2 Repo
    participant A as archive writer

    U->>C: create --repo --from --to --output
    C->>G: create_bundle_with_options(...)
    G->>R: resolve OIDs + validate linear range
    G->>R: revwalk push(to), hide(from)
    G->>R: packbuilder.insert_walk(...)
    R-->>G: PACK bytes
    G->>G: write .bundle + .caudit.json (+ optional .caudit.patch)
    G->>A: write_zip_archive(...)
    A-->>G: .zip artifact
    G-->>C: CreateBundleResult
```

Execution narrative:

1. Resolve `from` and `to`, then enforce linearity (`to` descendant-or-equal `from`).
2. Build revwalk and pack payload.
3. Write bundle bytes and inspect resulting header data.
4. Build metadata (`commit_chain`, `changed_files`, integrity fields).
5. Optionally generate patch sidecar.
6. Archive all artifacts to `.zip` and return command result.
7. Command handler performs loose-artifact cleanup so the archive is the final package unit.

### 5.2 Interactive audit sequence

Interactive audit assembles an `AuditModel` with three logical views: overview, history pages, and payload pages.

```mermaid
sequenceDiagram
    participant UI as ui::run
    participant M as build_audit_model
    participant RV as receive(dry-run)
    participant CP as collect_head_audit_entries
    participant PS as open_payload_session

    UI->>UI: render loading screen
    UI->>M: build model
    M->>RV: receive_bundle_input_with_options(dry_run=true)
    M->>CP: collect_head_audit_entries_for_bundle_input(...)
    M->>PS: open_payload_session(...)
    PS->>PS: parse + verify pack, build ledger + details
    M-->>UI: AuditModel
```

The model is intentionally mixed-source:

- overview combines metadata status, dry-run applicability, and payload proof summary
- history pages explain expected change intent by commit/file evolution
- payload pages expose full PACK-level proof context and object details
- startup draws a loading screen before model construction so large-bundle initialization remains explicit to the operator

The payload page also differentiates between derived and authoritative views:

- `Objects` subview: deduplicated, reviewer-friendly projection
- `Entries` subview: authoritative stream-order ledger used for completeness proof

### 5.3 Non-interactive audit sequence

Non-interactive mode is the automation and archival path. It is deliberately split into different evidence-producing operations so that CI and policy engines can ask a precise question and receive a precise answer.

| Mode | Inputs | Output | Best used for |
|---|---|---|---|
| Payload proof from bundle | `--repo --bundle --format table|json` (optional `--resolve pack-only|baseline`) | Proof counters, checksums, transfer status, transport/object rows, ledger (`summary` or `full`) | Transfer approval workflows, evidence archiving, machine policy checks |
| Repository-range manifest | `--repo --from --to --format ...` | Changed-file manifest from repository truth | Intent review and “what changed in repo terms” reporting |
| Metadata verification | `--verify-metadata --repo --bundle` | Pass/fail with detailed mismatch context | Sidecar integrity and sidecar-vs-repo equivalence checks |

`--format table` is designed for quick human review of the proof tuple. `--format json` emits the full payload-audit document for pipelines and long-term evidence retention.

Object-format guard: the payload proof path currently supports `sha1` object-format repositories only. Non-`sha1` formats are rejected fail-closed before payload parsing begins.

### 5.4 Payload proof runtime sequence

This is the central verification path for transfer safety.

```mermaid
sequenceDiagram
    participant A as audit (payload)
    participant IN as payload::input
    participant P as bundle::parse
    participant V as payload::verify
    participant Z as verify::zlib
    participant D as verify::delta
    participant PR as verify::proof

    A->>IN: load bundle bytes (.bundle or from .zip)
    IN-->>A: bundle_bytes
    A->>P: parse_bundle_payload(bundle_bytes)
    P-->>A: (inspection, pack_data slice)
    A->>V: verify_pack_payload(pack_data, resolve_policy)
    V->>V: parse pack header (declared N)
    loop for each entry i in 0..N-1
        V->>Z: inflate entry stream (or delta stream)
        alt delta entry
            V->>D: resolve base (pack-only or baseline)
            V->>D: apply delta -> canonical target bytes
        end
        V->>V: compute OID from canonical bytes
        V->>V: append ledger row i
    end
    V->>V: verify trailer checksum
    V-->>PR: PayloadPackVerification
    PR->>PR: enforce invariants (fail closed)
    PR-->>A: VerifiedPayload (or error)
```

The important behavioral property is that all downstream representations (table output, JSON output, and payload TUI views) are projections of this verified result, not alternate parsers.

### 5.5 Receive and dry-run sequence

Receive is implemented as a plan-first workflow. The import path first materializes incoming heads into a safe namespace, then computes a deterministic preflight plan, and only then applies policy-specific target updates. This design keeps behavior explainable and non-destructive by default.

The receive import stage itself is strict-first. It attempts object import with connectivity verification enabled, and only if a known libgit2 thin-pack edge case is detected it enters compatibility fallback paths. Compatibility fallback imports are followed by an explicit post-import connectivity traversal before any ref updates are allowed.

```mermaid
sequenceDiagram
    participant U as User
    participant R as receive
    participant B as bundle parse and import
    participant I as import strategy
    participant C as connectivity validator
    participant P as preflight planner
    participant V as policy validator
    participant A as apply backend
    participant TMP as temp bare mirror
    participant REPO as receiver

    U->>R: receive --repo --bundle [options]
    alt dry-run or check-mergeability
        R->>TMP: init mirror plus fetch refs from REPO and mirror alternates
        R->>B: import package into TMP mirror
        B->>I: strict indexer import verify=true
        alt strict import fails with missing-object indexer case
            I->>I: retry indexer import verify=false
            alt verify=false import also fails
                I->>I: retry via libgit2 fetch fallback
            end
            I->>C: run post-import connectivity traversal for imported heads
            C-->>I: pass or fail closed
        end
        R->>P: compute per-ref preflight statuses
        R->>V: validate selected integration policy
        R-->>U: would-change summary plus plan output
    else apply mode
        R->>B: import package into REPO
        B->>I: strict indexer import verify=true
        alt strict import fails with missing-object indexer case
            I->>I: retry indexer import verify=false
            alt verify=false import also fails
                I->>I: retry via libgit2 fetch fallback
            end
            I->>C: run post-import connectivity traversal for imported heads
            C-->>I: pass or fail closed
        end
        R->>P: compute per-ref preflight statuses
        R->>V: validate selected integration policy
        V->>A: apply validated target updates
        A-->>R: backend and outcome details
        R-->>U: apply summary and safety report
    end
```

The following flowchart presents the same receive path as a single decision-oriented control flow, including preflight classification and policy gates.

```mermaid
flowchart TD
    START["receive --repo --bundle [options]"] --> TARGET{"Mode"}
    TARGET -->|"dry-run or check-mergeability"| MIRROR["Create temporary bare mirror, fetch receiver refs, and mirror alternates object paths"]
    TARGET -->|"apply"| REPO["Open real receiver repository"]

    MIRROR --> IMPORT["Import bundle and map incoming heads"]
    REPO --> VERIFY{"verify metadata flag"}
    VERIFY -->|"yes"| VMETA["Run metadata integrity verification"]
    VERIFY -->|"no"| IMPORT
    VMETA --> IMPORT

    IMPORT --> STRATEGY["Import strategy<br/>1 strict indexer verify=true<br/>2 fallback indexer verify=false on specific missing-object failures<br/>3 final fallback libgit2 fetch with path and file URL candidates"]
    STRATEGY --> COMPAT{"Compatibility fallback used"}
    COMPAT -->|"yes"| CONNECT["Run post-import connectivity traversal for imported heads and trees<br/>fail closed on missing object reachability"]
    COMPAT -->|"no"| PRESERVE["Write preserved incoming refs under refs/sync/incoming/<bundle-id>/...<br/>optional branch mirrors under refs/heads/incoming/<bundle-id>/..."]
    CONNECT --> PRESERVE
    PRESERVE --> PLAN["Compute preflight plan per incoming ref<br/>status: target_missing, fast_forward_ok, already_present, target_ahead, diverged_merge_required"]
    PLAN --> CHECKMERGE{"check-mergeability flag"}

    CHECKMERGE -->|"yes"| MSIM["Run merge simulation for diverged refs<br/>collect clean/conflicted/unknown and conflict files"]
    MSIM --> MEND["Report mergeability diagnostics<br/>No target ref updates"]

    CHECKMERGE -->|"no"| DRY{"dry-run flag"}
    DRY -->|"yes"| DEND["Report preflight plan plus would-change summary<br/>No target ref updates"]
    DRY -->|"no"| POLICY{"integrate policy"}

    POLICY -->|"create-refs-only"| CREATESAFE["Write preserved incoming refs only<br/>Leave target refs unchanged"]
    CREATESAFE --> APPLYDONE["Success result"]

    POLICY -->|"fast-forward-only"| FFGATE{"Any diverged_merge_required row"}
    FFGATE -->|"yes"| FFFAIL["Fail with merge-required diagnostics<br/>No target ref updates"]
    FFGATE -->|"no"| APPLYTARGET["Apply allowed target updates"]

    POLICY -->|"merge"| MGATE["Simulate merges for diverged rows"]
    MGATE --> MOK{"All diverged rows clean"}
    MOK -->|"no"| MFAIL["Fail with conflict diagnostics<br/>No target ref updates"]
    MOK -->|"yes"| MERGEAPPLY["Create merge commits where needed<br/>Prepare target updates and merge-test refs"]
    MERGEAPPLY --> APPLYTARGET

    APPLYTARGET --> BACKEND{"Apply backend"}
    BACKEND -->|"ref transaction"| TXN["Atomic ref update transaction"]
    BACKEND -->|"manual CAS fallback"| CAS["CAS updates with rollback protection"]
    TXN --> APPLYDONE
    CAS --> APPLYDONE
```

Preflight statuses are:

- `target_missing`: target ref does not exist
- `fast_forward_ok`: target can advance strictly forward
- `already_present`: target already equals incoming
- `target_ahead`: incoming is older and already contained by target
- `diverged_merge_required`: target/incoming histories diverged

Integration-policy behavior:

- `create-refs-only`: never updates target refs; writes preserved incoming refs only
- `fast-forward-only`: updates only `target_missing` and `fast_forward_ok`; rejects diverged rows
- `merge`: requires clean mergeability for diverged rows; writes merge-test refs and updates targets with resulting merge commits

Safety and non-destructive controls:

- incoming refs are preserved under `refs/sync/incoming/<bundle-id>/...` before target updates
- optional incoming branch mirrors are written under `refs/heads/incoming/<bundle-id>/...`
- dry-run mirrors inherit receiver alternates configuration so import/connectivity checks run against the same object universe
- target updates use ref-transaction backend when available
- fallback apply path uses CAS checks with rollback on failure
- mixed-plan fast-forward failures fail before applying partial target updates
- import path is strict-first (`indexer verify=true`), with compatibility fallback only on known missing-object indexer failures
- compatibility fallback imports must pass post-import connectivity validation before planning or apply continues

Behavioral points:

- optional metadata integrity checks can run before import
- header framing is parsed strictly (same framing rules as payload audit)
- imported head commit existence is verified after object import
- dry-run computes impact without mutating receiver state
- check-mergeability mode reports clean/conflicted/unknown status per diverged ref without target mutation
- operator-facing output is structured in deterministic sections (`preflight checks`, `changes`, `summary`, `result`)
- mergeability diagnostics include commit summaries and explicit `conflict files` lists per diverged ref
- `receive --verbose` enriches import failure diagnostics with receiver object-store context (alternates, prerequisites visibility, object format, shallow marker)

### 5.6 Evidence derivation and consumption flow

The evidence pipeline is designed so every transfer-relevant decision can be traced back to either repository recomputation or payload byte proof, with both paths observable in machine and human outputs.

```mermaid
flowchart LR
    CREATE[create]
    PKG[[sync.bundle.zip]]
    CA[.caudit manifest]
    PVERIFY[PACK verify + ledger]
    PA[.paudit JSON/table/TUI projections]
    MV[audit --verify-metadata]
    GATE{Transfer gate decision}
    RECEIVE[receive]

    CREATE --> PKG
    PKG --> CA --> MV --> GATE
    PKG --> PVERIFY --> PA --> GATE
    GATE --> RECEIVE
```

## 6. PACK Proofing Model and Security Argument

Payload proofing is the central assurance mechanism of the project. The following sections define the proof unit, the enforced invariants, and the evidence surfaces used by both operators and automation.

### 6.1 Authoritative truth model

The proof model separates authoritative and convenience layers:

- `PackEntryLedger` (authoritative)
  - one row per parsed PACK entry in stream order
  - includes index, offset, kind, size, base refs, resolution state/source
- materialized-object index (derived)
  - deduplicated object inventory for UI/object browsing
  - derived from resolved ledger rows

Completeness is a property of entry accounting, not of repository reachability enumeration.

### 6.2 What is proved

For the verified PACK payload bytes:

1. PACK preflight is valid (`PACK`, supported version, declared count)
2. trailer checksum matches recomputed checksum
3. exactly `entries_declared` entries are parsed into ledger rows
4. each entry is decoded and inflated
5. delta entries are reconstructed under explicit resolve policy
6. canonical object identity is recomputed for materialized entries
7. transfer gate is allowed only when all declared entries are materialized

### 6.3 Resolve modes and dependency policy

`pack-only` (strict default):

- base objects must be resolvable from in-pack context
- unresolved external base causes fail-closed error

`baseline`:

- `ref-delta` base lookup may use provided baseline ODB
- ledger records this via `resolved_via=baseline`

This makes external dependency usage explicit and auditable.

### 6.4 Fail-closed conditions

Audit/proof aborts on:

- malformed or truncated PACK data
- unsupported/invalid entry encoding
- zlib inflate failures
- delta decode/apply failures
- unresolved delta base under selected policy
- size mismatches
- checksum mismatch
- declared/parsed/materialized counter mismatch

Errors preserve useful context (`reason`, `blocked_entry_idx`, `ledger_partial`) so failure is diagnosable without reducing strictness.

### 6.5 Security argument (selling point)

The security idea behind `git-sync` is that transfer approval must be based on evidence that is both byte-anchored and complete, not on convenience summaries. In practice, this means the proof unit is the PACK entry stream itself. If an object is in the payload, it must appear in the declared entry count and therefore in ledger accounting, whether or not that object is reachable from an advertised head.

This design intentionally avoids the classic failure mode where review tooling only inspects reachable commits and trees. Reachability answers "what is visible from selected refs," but it does not answer "what bytes crossed the boundary." For air-gap workflows, the second question is the one that matters most. The verifier therefore binds claims to exact payload bytes (framing + checksum), enforces exact declared-entry accounting, and only allows transfer when the full tuple is internally consistent.

Operationally, the argument is not "trust this parser." The argument is "observe these invariants and counters that must all align." That is why the same proof tuple is exposed in table output, JSON export, and TUI surfaces. The goal is that a reviewer can reconstruct the decision logic from emitted evidence without needing hidden implementation assumptions.

#### 6.5.1 Threat model and objective

Threat model:

- package may be malformed or crafted to hide unexpected payload content
- metadata may be misleading or stale
- reachability-only views can omit unreachable but present objects

Objective:

- prove complete accounting of payload entries crossing the air-gap boundary
- fail closed when complete accounting cannot be established under policy

#### 6.5.2 Source of truth: PACK bytes

Payload truth comes from the PACK stream bytes, not from sidecar metadata and not from commit reachability alone. This blocks a common class of smuggling attempts where objects are present in payload but not visible in head-reachable views.

#### 6.5.3 Unambiguous PACK identification

`git-sync` parses bundle header framing and requires exact PACK placement after header terminator. No heuristic byte scanning is used. This ties proof to a precise byte range.

#### 6.5.4 Completeness via declared count and exact iteration

The verifier reads declared entry count `N`, appends exactly one ledger row per parsed entry, and enforces `entries_parsed == entries_declared`. Any early stop, overrun, or parse inconsistency fails closed.

#### 6.5.5 Integrity binding via trailer checksum

The trailer checksum binds proof and ledger to exact payload bytes. Byte-level tampering is detected before proof acceptance.

#### 6.5.6 Per-entry validation and canonical identity

Each entry is validated at stream level and reconstruction level:

- decode and inflate checks
- delta input and output-size constraints
- canonical object-id recomputation from object bytes

For delta rows, both stream-size and reconstructed-size semantics are visible and checkable.

#### 6.5.7 Explicit dependency policy

Resolve mode is operator-visible and recorded (`pack-only` vs `baseline`). If external dependency is disallowed and needed, the proof blocks.

#### 6.5.8 Fail-closed transfer gate

Transfer is allowed only when the full proof tuple is consistent. Any violation blocks transfer and returns structured diagnostics.

#### 6.5.9 Smuggling resistance for unreachable objects

Because proof unit is entry stream, unreachable objects are still ledgered and audited. Reachability is rendered as reviewer context, not used as completeness criterion.

#### 6.5.10 Boundaries and non-goals

- current payload proof path assumes SHA-1 PACK/object semantics
- package authorship authenticity is out of scope without separate signing
- proof guarantees payload integrity/completeness, not producer trust

#### 6.5.11 Guarantee summary

When proof succeeds:

- PACK start is unambiguous
- payload bytes are checksum-bound
- declared entry set is fully accounted for in ledger/materialization counters
- transfer-gate acceptance is explicit and policy-bound

#### 6.5.12 Transfer-gate decision tree

```mermaid
flowchart TD
    S[Bundle input] --> B{Strict bundle framing valid?}
    B -- no --> X1[Block: invalid framing / ambiguous PACK start]
    B -- yes --> C{Trailer checksum valid?}
    C -- no --> X2[Block: payload integrity mismatch]
    C -- yes --> D{entries_parsed == entries_declared?}
    D -- no --> X3[Block: incomplete or inconsistent entry accounting]
    D -- yes --> E{entries_materialized == entries_declared?}
    E -- no --> X4[Block: unresolved or non-materialized entries]
    E -- yes --> F{Resolve policy satisfied?}
    F -- no --> X5[Block: dependency policy violation]
    F -- yes --> A[Allow transfer: transfer_allowed=true]
```

Proof model flow:

```mermaid
flowchart TD
    A[Locate PACK bytes] --> B[Parse PACK header and declared entry count]
    B --> C[Verify trailer checksum]
    C --> D[Iterate PACK entries]
    D --> E[Decode/decompress entry]
    E --> F[Resolve base: in-pack or optional baseline ODB]
    F --> G[Reconstruct canonical bytes and OID]
    G --> H[Append ledger row]
    H --> I{parsed == declared?}
    I -- no --> X[Fail closed with ledger_partial]
    I -- yes --> J[Build materialized object index from resolved ledger rows]
    J --> K{materialized == declared?}
    K -- no --> X
    K -- yes --> L[Emit payload models/output]
```

### 6.6 Proof exposure in outputs

Proof evidence is deliberately exposed in every review surface so that operators do not need to “trust hidden internals.” The same core tuple is presented with different levels of detail depending on whether the consumer is a person in a terminal or an automation system.

| Surface | Evidence shown | Typical use |
|---|---|---|
| TUI overview (`Bundle Integrity`) | Proof status, transfer gate state, high-level counters | Fast operator confidence check before drill-down |
| TUI payload page header | Parsed/materialized/unique/duplicate counters and checksum context | Interactive technical review of payload status |
| Non-interactive table | Concise proof tuple + ledger summary | Human-readable CI logs and release artifacts |
| Non-interactive JSON | Full `pack_proof` + `entry_ledger` (`summary`/`full`) + derived object sections | Machine policy, archival evidence, reproducible audit records |

### 6.7 Current proof-path boundaries

- HASH algorithm is currently SHA-1 for PACK/object proof path
- proof failures return errors; no synthetic successful document is emitted
- interactive mode currently enforces pack-only resolve policy

## 7. UI Interaction Model

The UI is implemented as a constrained state machine. It is intentionally read-only in audit flows; state transitions change what is shown, not repository content.

Major modes:

- history overview
- history commit pages
- diff view
- payload main view
- payload object detail view

```mermaid
stateDiagram-v2
    [*] --> HistoryOverview
    HistoryOverview --> HistoryCommit: Enter selected head
    HistoryOverview --> PayloadMain: v or 2
    HistoryCommit --> PayloadMain: 2
    PayloadMain --> HistoryOverview: v or 1
    PayloadMain --> HistoryCommit: 3
    HistoryCommit --> DiffView: Enter selected file
    DiffView --> HistoryCommit: Esc
    PayloadMain --> PayloadObjectDetail: Enter selected object or resolved entry
    PayloadObjectDetail --> PayloadMain: Esc
    HistoryCommit --> HistoryOverview: Esc
    HistoryOverview --> [*]: q or Esc
    PayloadMain --> [*]: q or Esc
```

Key interaction properties:

- `1`, `2`, `3` provide direct navigation shortcuts
- `v` toggles history/payload from main page
- `Tab` switches overview focus between head and would-change tables
- payload supports sort cycling, page jumps, and entry/object subview toggles
- `?` opens a contextual help overlay with three pages (`Hotkeys`, `Glossary`, `Audit Guide`)
- while help is open, paging keys (`PgUp/PgDn`, `h/l`, arrows, `j/k`, `Tab`) switch help pages instead of mutating the underlying review selection
- Esc closes deep modes first (diff/detail), then unwinds to overview, then exits

### 7.1 Contextual help overlay

The help system is modeled as a transient overlay that can be entered from any primary UI mode. Its content is mode-aware and intentionally split by reviewer intent:

- `Hotkeys`: interaction and navigation controls for the active mode
- `Glossary`: terms currently visible on the active page (for example proof counters, object kinds, and delta terminology)
- `Audit Guide`: practical review guidance for auditors who are not expected to be experts in Git internals or PACK encoding

```mermaid
stateDiagram-v2
    state "Any active review mode" as ActiveMode
    ActiveMode --> HelpOverlay: ?
    HelpOverlay --> ActiveMode: ? or Esc
    HelpOverlay --> HelpOverlay: PgUp/PgDn or h/l or arrows or j/k or Tab (cycle pages)
    HelpOverlay --> [*]: q
```

The overlay text uses the same semantic color language as the core UI where practical (for example object kinds and delta terms), which reduces context switching during audit review.

## 8. Auditability Guarantees and Limits

Auditability depends on explicit guarantees and explicit limits. The following sections state what the current implementation can defend, where boundaries remain, and how invariants map to distinct truth sources.

### 8.1 Guarantees

The guarantees listed here are the guarantees the current implementation can defend with code-level evidence and tests. They are intentionally phrased as operational properties, not aspirational goals.

- linear range enforcement during package creation
- metadata integrity binding (hash/size checks)
- metadata-vs-repository truth verification path
- dry-run isolation from receiver mutation
- check-mergeability isolation from receiver mutation with explicit conflict-path reporting
- fail-closed PACK proof pipeline
- authoritative ledger surfaced to UI and JSON
- explicit transfer gate (`transfer_allowed`, `blocked_reason`)
- unreachable-object visibility as context fields
- idempotent receive behavior for already-applied heads
- receive-time preservation of incoming heads in a stable safe namespace
- fast-forward-only non-rewind behavior (`target_ahead` is treated as a no-op)
- rollback-protected target updates (ref transaction or CAS with rollback fallback)
- strict-first receive import with compatibility fallbacks only on specific indexer missing-object failures
- mandatory post-fallback connectivity traversal before any receive ref update path continues

### 8.2 Limits

These limits define where additional controls are still needed. They are not hidden caveats; they are part of the explicit trust boundary of the current design.

- no detached package signature verification yet (authenticity out of scope)
- receive-time `--verify-metadata` focuses on metadata integrity, not full repo-truth recomputation by default path
- payload proof supports SHA-1 object format only today
- interactive audit currently runs with pack-only resolve mode

### 8.3 Truth sources and invariant mapping

For design reviews and audits, the most important discipline is to avoid mixing truth sources. The table below makes explicit which subsystem is authoritative for which claim.

| Review surface / command | Authoritative source | What it proves | What it does not prove |
|---|---|---|---|
| Payload audit (`audit` payload table/json, interactive payload views) | Strict bundle framing + PACK verifier + ledger/proof invariants | PACK payload completeness and integrity under resolve policy | Metadata correctness or producer authenticity |
| Metadata verification (`audit --verify-metadata`) | Sidecar integrity checks + repository recomputation equivalence | Sidecar claims match package and repository truth | PACK completeness by itself |
| History views (interactive commit/file pages) | Imported graph traversal + commit/file extraction | Human-readable change intent/context | Full payload coverage |
| Receive / dry-run / check-mergeability | Import/apply behavior in receiver or mirror | Operational applicability, mergeability diagnostics, and impact | Replacement for payload proof checks |

The architectural invariants that tie these surfaces together are:

| Invariant | Definition | Enforced by |
|---|---|---|
| I1 | Create-range linearity (`from..to` is linear-descendant or equal) | `create` range resolution/validation |
| I2 | Metadata transport integrity (bundle/patch hash-size bindings) | metadata integrity verification |
| I3 | Metadata-vs-repo equivalence (`commit_chain` / `changed_files`) | metadata verify against repository recomputation |
| I4 | Payload proof tuple consistency (framing, checksum, declared/parsed/materialized counters) | PACK verifier + proof boundary checks |
| I5 | Receive idempotency and no-op classification (`already_present` and `target_ahead`) | receive preflight classification and apply planning |
| I6 | Dry-run/check-mergeability isolation (same import logic, isolated target) | mirror execution model for non-mutating receive analysis |
| I7 | Non-destructive receive policy safety (no backward fast-forward updates, no partial mixed-plan target mutation) | receive plan validation + target update backend behavior |
| I8 | Compatibility fallback import safety (fallback allowed only for narrow failures, with post-import connectivity fail-closed gate) | receive import strategy and connectivity validation before planning/apply |

### 8.4 Claim-to-implementation traceability matrix

This matrix links the document's assurance claims to concrete implementation files and representative executable tests. It is intended as a maintenance guardrail: if code or tests move, this table should be updated with the same rigor as the claim text.

| Claim | Invariant(s) | Primary code locations | Representative tests |
|---|---|---|---|
| Create rejects non-linear ranges | I1 | `src/git/bundle/create.rs`, `src/git/context.rs` | `create_bundle_fails_when_to_commit_is_not_descendant_of_from_commit`, `open_context_fails_when_tip_ref_is_not_descendant_of_base_ref` |
| Metadata integrity binds claims to transport artifacts | I2 | `src/git/metadata/verify.rs`, `src/git/metadata/load.rs` | `verify_bundle_metadata_integrity_rejects_header_version_mismatch`, `verify_bundle_metadata_integrity_rejects_patch_sidecar_sha_mismatch` |
| Metadata claims match repository truth when verified | I3 | `src/git/metadata/collect.rs`, `src/git/metadata/verify.rs` | `verify_bundle_metadata_against_repo_rejects_commit_chain_mismatch`, `verify_bundle_metadata_against_repo_rejects_changed_files_mismatch` |
| Payload completeness and integrity are enforced fail-closed | I4 | `src/git/bundle/parse.rs`, `src/git/bundle/payload/verify/*`, `src/git/bundle/payload/verify/proof.rs` | `bundle_pack_offset_is_read_from_header_not_scanned`, `verify_pack_payload_validates_trailer_checksum`, `pack_ledger_contains_exactly_declared_entry_count`, `strict_mode_blocks_when_unresolved_entries_remain` |
| Re-applying same package is idempotent and incoming-older bundles are non-destructive no-ops | I5 | `src/git/bundle/receive.rs`, `src/git/types/receive.rs` | `receive_bundle_input_is_idempotent_when_same_package_is_applied_twice`, `receive_fast_forward_only_accepts_target_ahead_and_keeps_target_ref_unchanged` |
| Dry-run and check-mergeability mirror receive logic without mutating target repo | I6 | `src/git/bundle/receive.rs`, `src/git/bundle/receive/tests.rs`, `src/app/commands/receive.rs` | `receive_bundle_input_with_options_dry_run_does_not_modify_receiver_repo`, `receive_dry_run_prints_would_change_table_for_pending_import`, `receive_check_mergeability_reports_diverged_ref_merge_status_without_mutating_receiver`, `temp_bare_repo_from_existing_inherits_unreachable_source_objects` |
| Fast-forward receive rejects diverged/mixed plans without partial target mutation | I7 | `src/git/bundle/receive.rs`, `src/app/commands/receive.rs` | `receive_integrate_fast_forward_only_rejects_mixed_plan_without_partial_target_updates`, `receive_fast_forward_only_rejects_diverged_target_and_preserves_incoming_namespace_refs` |
| Receive compatibility fallback remains fail-closed by explicit connectivity validation after fallback import | I8 | `src/git/bundle/receive.rs`, `src/git/bundle/receive/tests.rs` | `missing_objects_indexer_error_enables_fetch_import_fallback`, `connectivity_validation_accepts_simple_head_history`, `connectivity_validation_rejects_missing_head_commit` |

## 9. Build and Versioning

Build and version handling are defined to keep identity traceable from compilation to runtime and exported audit artifacts. The details below describe resolution order and where version values are surfaced.

Version resolution order:

1. `GIT_SYNC_VERSION_OVERRIDE`
2. `git describe --tags --dirty --always` (normalized `v` prefix)
3. `CARGO_PKG_VERSION` fallback

Version usage:

- runtime version (`APP_VERSION`) is used by CLI/TUI (`--version` and UI overview)
- create metadata currently carries package/tool version fields for traceability
- payload audit document includes runtime tool version in exported evidence

## 10. Test Strategy

The suite is structured to preserve command behavior, proof invariants, and UI consistency.

Test layers:

- module-scoped unit tests are colocated with implementation (`src/**/tests.rs`, `src/**/tests/*`, and focused domain suites like `src/git/tests/*`, `src/ui/tests/*`)
- integration tests in `tests/*`

Representative covered areas:

- create/inspect/archive behavior
- metadata integrity and metadata-vs-repo verification behavior
- receive and dry-run applicability/idempotency paths
- receive integration policy paths (`create-refs-only`, `fast-forward-only`, `merge`) including target-ahead no-op and diverged failure handling
- receive update safety paths (ref-transaction and CAS rollback fallback, including fault-injection coverage)
- dry-run mirror object-visibility parity via inherited alternates wiring
- receive import compatibility paths (`indexer verify=true` then guarded fallback chain) and post-fallback connectivity-validation gate behavior
- receive mergeability simulation output paths (status, merge context, and conflict-file reporting)
- commit and file-level extraction for review pages
- payload session/object detail behaviors
- PACK mismatch and unresolved-delta rejection paths
- CLI path contracts (`tests/main_cli_paths.rs`)
- end-to-end workflow (`tests/bundle_workflow_integration.rs`)
- scripted receive matrix execution (`tests/receive_matrix_script_integration.rs`)
- reproducible mergeability-warning fixture generation (`scripts/generate-mergeability-warning-repos.sh`)

## 11. Operational Auditor Checklist

The checklist below translates proof invariants into a practical transfer-review procedure. It focuses on the minimum evidence tuple that must hold before approval.

When an auditor asks whether the displayed payload is complete, show these values together:

- checksum verified: `true`
- `entries_declared = N`
- `entries_parsed = N`
- `entries_materialized = N`
- `transfer_allowed = true`

Explain that PACK declares `N`; ledger parsing and materialization both prove accounting for all `N`; checksum binds that accounting to exact bytes. Any mismatch blocks transfer.

## 12. Boundaries and Assumptions

Guarantees hold only within stated environmental and cryptographic assumptions. The assumptions and non-goals below define where additional controls are required for stronger assurance claims.

Guarantees depend on:

- correct local execution environment (tool/process not subverted)
- correct PACK format semantics and parser adherence
- cryptographic primitive assumptions for SHA-1/SHA-256 checks

Non-goals in current implementation:

- detached authenticity/signature framework
- trust attestation of producer machine/runtime

## 13. Open TODOs

Open items are tracked here as intentionally deferred architecture and assurance work. The list is scoped to changes that materially improve trust guarantees, compatibility, or policy integration.

- detached package signature verification (authenticity)
- optional stricter proof artifacting (for example explicit parsed-entry OID set emission/checks)
- object-format-aware payload proofing (phase 6b)
  - add SHA-256-capable object-id/hash abstraction (remove hardwired SHA-1 assumptions)
  - parse PACK trailer and ref-delta base IDs with algorithm-specific hash length
  - make baseline-assisted delta resolution and reporting algorithm-aware end-to-end
- policy-driven receive gates based on audit evidence

## 14. Appendix: Code locations for proof claims

This appendix provides claim-to-code traceability for design and audit review. Each proof claim is mapped to the concrete modules that enforce it.

- strict bundle framing: `src/git/bundle/parse.rs`
  - header parse through blank line, then exact PACK slice extraction
- PACK verification and ledger: `src/git/bundle/payload/verify.rs` and `verify/*`
  - preflight parse, entry iteration, inflate/delta reconstruction, checksum verification
  - ledger and materialized-store creation
- proof invariants: `src/git/bundle/payload/verify/proof.rs`
  - fail-closed boundary checks (`VerifiedPayload`)
- payload session/export mapping:
  - `src/git/bundle/payload/session.rs`
  - `src/git/bundle/payload/document.rs`
  - `src/git/bundle/payload/detail.rs`
  - `src/git/bundle/payload/context.rs`
