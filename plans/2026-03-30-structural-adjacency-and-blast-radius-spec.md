# Spec: Structural Adjacency And Blast Radius

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Structural adjacency and blast radius intent](d:/RepoBrainOS/plans/2026-03-30-structural-adjacency-and-blast-radius-intent.md)

## Problem Statement

RepoBrain needs a first structural plane that can expand exact file and symbol anchors into a bounded blast-radius answer from maintained snapshot data.

## Scope

- Direct local import extraction during ingest
- Persisted import edges in the snapshot artifact
- Graph-side one-hop blast-radius expansion
- Symbol-to-file ownership seeding
- CLI support for `blast-radius`

## Non-Goals

- Semantic impact reasoning
- Multi-hop traversal tuning
- Build/test edges
- Dense retrieval
- Schema changes

## Behavioral Requirements

1. Repository scans must extract direct local import edges for the supported lexical languages in scope.
2. Snapshot normalization must keep import edges sorted and deduplicated.
3. Blast radius must accept either an exact path or an exact symbol name.
4. Symbol targets must resolve through file ownership before structural expansion.
5. The first implementation must remain one-hop and structural only: direct imports plus reverse importers.

## Acceptance Examples

1. If `src/lib.rs` declares `RepositoryScanner` and directly imports `src/inner.rs`, asking for blast radius on `RepositoryScanner` returns the symbol anchor plus `src/lib.rs` and `src/inner.rs`.
2. Asking for blast radius on `src/inner.rs` returns the touched file plus `src/lib.rs` as a reverse importer.
3. If a target is absent from the snapshot, the CLI returns a narrow failure rather than inventing neighbors.

## Invariants

- Must always hold:
  Structural blast radius remains snapshot-bound and one-hop.
- Must not regress:
  The system must not imply semantic caller or dependency truth from lexical imports alone.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  New ingest import types, graph snapshot store logic, and a CLI `blast-radius` command.
- Illegal states to remove:
  Import edges should not remain unsorted after snapshot normalization.

## Workload And Complexity Notes

- Workload shape and expected scale:
  linear import extraction during scans plus one-hop neighborhood expansion over snapshot-held symbols and import edges
- Hot, warm, or cold path:
  extraction is cold/warm maintenance; blast-radius is interactive but deliberately bounded to one hop
- Chosen data structures and why:
  sorted `Vec<IndexedImport>` and snapshot-owned symbol/file vectors keep storage simple and support bounded lookup; small visited sets keep one-hop expansion deterministic
- Key operation costs:
  import extraction is `O(total_lines)` for supported files; direct adjacency lookup is `O(log n + k)`; one-hop blast-radius is linear in the seeded neighborhood size
- Memory / allocation notes:
  adjacency remains snapshot-resident and compact; there is no heavyweight graph database or deep traversal cache in this first slice
- Measured vs inferred performance claims:
  one-hop bounded complexity is inferred from the snapshot structures; verification measured behavior with smoke commands rather than benchmark timing

## Test Strategy

- Unit:
  Import extraction and resolution per language, snapshot import lookup, graph blast-radius behavior.
- Integration:
  CLI scan and blast-radius smoke checks.
- Regression:
  Rust test fixtures under `#[cfg(test)]` must not pollute symbol ownership.
- Property / invariant:
  Direct import lookup and blast radius remain deterministic over sorted snapshot data.

## Verification Notes

- Commands:
  `cargo test -p repobrain-ingest`, `cargo test -p repobrain-graph`, `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`, `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`, `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  Updated repo-local snapshot JSON and a validation record
