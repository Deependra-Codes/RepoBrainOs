# Spec: Stable-ID Graph Relationships And Broker Identity Matching

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Stable-id graph relationships and broker identity matching intent](d:/RepoBrainOS/plans/2026-03-30-stable-id-graph-relationships-and-broker-identity-matching-intent.md)

## Problem Statement

RepoBrain now has stable fact IDs in ingest, but graph and broker still mostly operate on reconstructed string identity.

This slice should move graph reasoning and broker evidence matching onto the maintained fact IDs while introducing the first typed relationships beyond raw import-neighborhood expansion.

## Scope

- use stable symbol fact IDs in graph symbol nodes and symbol-definition receipts
- add typed graph relationships for `defines` and `references`
- include graph relationships in blast-radius reporting
- update broker symbol evidence matching to use `fact_id`
- keep existing file-centric suggestion and verification planning behavior

## Non-Goals

- semantic symbol-reference resolution
- new schemas for graph relationships outside Rust
- multi-hop graph expansion
- dense retrieval or query planning work

## Behavioral Requirements

1. Graph symbol nodes must be keyed by `IndexedSymbol.fact_id`.
2. Blast-radius reporting must include typed `defines` and `references` relationships.
3. Symbol-definition evidence receipts must be matchable by `fact_id`.
4. Broker must resolve exact-symbol evidence IDs from `fact_id` rather than locator substring matching.
5. Human-facing flow and briefing text must remain readable even when graph identity becomes fact-backed.

## Acceptance Examples

1. A symbol target such as `RepositoryScanner` should produce a stable-id symbol node in the blast radius report.
2. A file that defines a symbol should emit a `defines` relationship from the file node to the symbol node.
3. A local import should emit a `references` relationship backed by the stable import fact ID.

## Invariants

- Must always hold:
  - `defines` is exact snapshot-backed ownership, not inferred semantics
  - `references` in this slice means syntax-backed file reference relationships, not fully resolved semantic call/reference truth
  - broker suggested files continue to come from file nodes
- Must not regress:
  - current verification planning
  - current exact-path and exact-symbol anchoring
  - current blast-radius and get-brief success on existing fixtures

## Contract And Type Changes

- Schema changes:
  - none yet outside Rust implementation
- Public interface changes:
  - add typed graph relationship structs to graph reporting
  - expose stable-id symbol nodes in graph reports
- Illegal states to remove:
  - symbol-definition evidence matching that depends on path-plus-string reconstruction when a stable `fact_id` is available

## Workload And Complexity Notes

- Workload shape and expected scale:
  - one-hop snapshot graph work driven by exact path or exact symbol anchors
- Hot, warm, or cold path:
  - hot interactive path
- Chosen data structures and why:
  - keep snapshot vectors as maintained truth
  - derive bounded one-hop relationships during graph report assembly to avoid premature storage redesign
- Key operation costs:
  - graph report assembly remains bounded by one scan over relevant snapshot symbol/import vectors plus existing verification planning
  - broker evidence matching becomes cheaper and less fragile for exact symbol claims once it keys by `fact_id`
- Memory / allocation notes:
  - relationship vectors add modest per-report allocation, acceptable for interactive one-hop reports
- Measured vs inferred performance claims:
  - inferred that stable-id matching reduces fragile string matching overhead and future integration complexity more than it costs in small extra relationship objects

## Test Strategy

- Unit:
  - graph relationship generation for `defines` and `references`
  - broker exact-symbol evidence matching by `fact_id`
- Integration:
  - repo fixtures for blast radius and get-brief
- Regression:
  - file suggestions remain readable and file-based
  - stable-id symbol nodes do not break CLI blast-radius output
- Property / invariant:
  - graph relationships backed by stable receipts carry deterministic evidence IDs

## Verification Notes

- Commands:
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated `.repobrain/snapshots/worktree-inventory.json`
  - future validation record for this slice
