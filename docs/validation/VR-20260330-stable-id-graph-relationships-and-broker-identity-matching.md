# Verification Record: Stable-ID Graph Relationships And Broker Identity Matching

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the graph-and-broker follow-up slice covering:

- stable fact ID consumption in `repobrain-graph`
- typed `defines` and `references` relationships in blast-radius reporting
- fact-ID-backed symbol-definition receipts
- broker evidence matching by `fact_id` instead of path-plus-locator reconstruction

## Intent / Spec References

- Intent: [Stable-id graph relationships and broker identity matching intent](d:/RepoBrainOS/plans/2026-03-30-stable-id-graph-relationships-and-broker-identity-matching-intent.md)
- Spec: [Stable-id graph relationships and broker identity matching spec](d:/RepoBrainOS/plans/2026-03-30-stable-id-graph-relationships-and-broker-identity-matching-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-graph`
- `cargo test -p repobrain-broker`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- get-brief --goal "understand RepositoryScanner before editing" --scope RepositoryScanner --token-budget 4096 --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-graph` now consumes stable symbol fact IDs instead of reconstructing symbol identity from `name@path` strings.

The blast-radius report now includes:

- stable-id symbol anchor nodes
- typed `defines` relationships
- typed `references` relationships

while preserving file-based impacted scope and verification planning.

Smoke validation on `d:\RepoBrainOS` produced a snapshot with:

- 159 files
- 370 symbols
- 123 imports
- 23 verification targets

Running `blast-radius RepositoryScanner` returned:

- 6 impacted nodes
- 5 relationships
- 1 `defines` edge from the owning ingest file to the `RepositoryScanner` symbol fact
- 4 `references` edges from the ingest root module to adjacent local modules

Running `get-brief` for `RepositoryScanner` then confirmed:

- the exact-symbol claim matched a `symbol_definition` receipt whose `source_ref` was the symbol `fact_id`
- structural adjacency evidence was now backed by `reference_edge` receipts keyed by stable import fact IDs
- file suggestions and verification targets remained readable and stable

## Pass / Fail Against Expectations

Pass.

This slice moved graph and broker identity onto maintained fact IDs without widening into semantic overclaiming or breaking current briefing flows.

## Performance / Complexity Validation

- Measured:
  graph and broker tests passed; CLI smoke commands passed; full repo-native validation passed
- Inferred:
  bounded one-hop relationship assembly over maintained snapshot vectors remains acceptable for the current interactive slice while removing fragile string matching from broker symbol evidence lookup
- Why no benchmark was needed, if applicable:
  this slice changes graph identity and relationship shape, but it does not add a broader retrieval search policy or a new storage engine

## Residual Risks

- `references` in this slice are syntax-backed file reference relationships, not full semantic symbol references
- blast-radius reports remain one-hop and snapshot-bound
- graph relationships are still assembled per report rather than maintained as a separately indexed graph store

## Related

- Research: [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)
- SDD / ADR: [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md), [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md), [SDD-006](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
