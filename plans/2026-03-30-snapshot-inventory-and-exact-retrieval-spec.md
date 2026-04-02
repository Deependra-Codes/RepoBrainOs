# Spec: Snapshot Inventory And Exact Retrieval

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Snapshot inventory and exact retrieval intent](d:/RepoBrainOS/plans/2026-03-30-snapshot-inventory-and-exact-retrieval-intent.md)

## Problem Statement

RepoBrain needs a first real production-core slice that turns a repo root into a persisted snapshot artifact and answers a minimal exact retrieval query without rebuilding repo state inline.

## Scope

- Repository file inventory scanning in `repobrain-ingest`
- Default exclusion handling for noisy or risky paths
- JSON snapshot artifact write/load
- Exact relative-path lookup over stored snapshot data
- CLI commands for scan and exact path lookup

## Non-Goals

- Symbol graph generation
- Build/test graph extraction
- Lexical retrieval
- Semantic enrichment
- Background maintenance scheduling
- Schema changes

## Behavioral Requirements

1. The scanner must walk a repository root recursively and produce a deterministic, sorted file inventory.
2. The scanner must exclude default v1 noise and secret-prone paths such as `.git/`, `node_modules/`, cache directories, `.env*`, and common binary artifact extensions.
3. The snapshot artifact store must write and load inventory JSON under a repo-local `.repobrain/snapshots/` directory.
4. Exact path lookup must operate on stored snapshot data and accept either `/` or `\` separators.
5. The CLI must expose one command to create the snapshot and one command to perform exact path lookup from that snapshot.

## Acceptance Examples

1. Running the scan command on a repo with Rust, Python, and Markdown files writes a snapshot artifact and reports the detected languages.
2. Running exact path lookup after scanning returns file metadata for `src/rust/crates/repobrain-domain/src/lib.rs`.
3. Looking up `.env` or a file only present under `node_modules/` returns no match because those paths were excluded from the snapshot.

## Invariants

- Must always hold:
  The stored inventory stays sorted by relative path so exact lookup remains predictable and logarithmic.
- Must not regress:
  The first slice must not move repo scanning or snapshot generation into the interactive hot path of unrelated requests.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  New Rust ingest types and CLI commands only.
- Illegal states to remove:
  Missing snapshot metadata or unsorted file inventories should not be representable after scan/load normalization.

## Workload And Complexity Notes

- Workload shape and expected scale:
  full repo walks during maintenance plus repeated exact file lookups against stored snapshot state
- Hot, warm, or cold path:
  scanning is cold/warm maintenance; exact lookup is the latency-sensitive read path
- Chosen data structures and why:
  sorted `Vec<IndexedFile>` keeps snapshot storage simple and cache-friendly while enabling binary search; `BTreeSet` keeps exclusions and language collection deterministic
- Key operation costs:
  scan is `O(n)` over filesystem entries plus normalization sort; exact lookup is `O(log n)` on the stored inventory
- Memory / allocation notes:
  the whole snapshot is loaded into memory as contiguous vectors; JSON artifacts trade space efficiency for a simple local-first baseline
- Measured vs inferred performance claims:
  exact lookup complexity is inferred from the chosen structures; verification measured functional command success rather than benchmark latency

## Test Strategy

- Unit:
  Scanner exclusions, language detection, exact lookup normalization, and snapshot round-trip.
- Integration:
  CLI behavior is covered indirectly through compilation and command wiring in this slice.
- Regression:
  Snapshot load must preserve excluded-path behavior and exact lookup semantics.
- Property / invariant:
  Inventory normalization keeps files sorted and languages deduplicated.

## Verification Notes

- Commands:
  `cargo test -p repobrain-ingest`, `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  Repo-local snapshot JSON and a validation record after implementation
