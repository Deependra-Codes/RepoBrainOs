# Verification Record: Evidence Receipts And Structural Flow Capsules

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the trust-uplift slice covering:

- snapshot-bound capture timestamps for maintained inventory snapshots
- deterministic evidence receipts for exact anchors, direct structural edges, and planned verification targets
- first structural flow capsules from direct adjacency
- broker wiring for `must_know.evidence_ids`, `relevant_flows`, and `evidence_index`

## Intent / Spec References

- Intent: [Evidence receipts and structural flow capsules intent](d:/RepoBrainOS/plans/2026-03-30-evidence-receipts-and-structural-flow-capsules-intent.md)
- Spec: [Evidence receipts and structural flow capsules spec](d:/RepoBrainOS/plans/2026-03-30-evidence-receipts-and-structural-flow-capsules-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-ingest`
- `cargo test -p repobrain-graph`
- `cargo test -p repobrain-broker`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
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

RepoBrain now emits snapshot-bound evidence receipts and a first structural flow capsule layer for scoped briefing requests.

The maintained snapshot now carries a capture timestamp, and the structural layer emits receipts for:

- exact symbol and exact path anchors
- direct import edges and reverse import edges
- planned verification targets

Smoke validation on `d:\RepoBrainOS` produced a snapshot with 152 files, 337 symbols, 118 imports, and 23 verification targets.

Running `get-brief` for `RepositoryScanner` returned:

- three `must_know` items with non-empty `evidence_ids`
- one `relevant_flows` entry describing the structural flow around the symbol
- an `evidence_index` with 9 receipts
- a required-check receipt for `cargo test (in src/rust/crates/repobrain-ingest)`

The emitted receipts were snapshot-bound through the captured-at timestamp stored in the current inventory artifact.

## Pass / Fail Against Expectations

Pass.

This slice upgrades `get_brief` from shaped guidance into cited guidance while keeping the new flow layer explicitly structural and bounded.

## Performance / Complexity Validation

- Measured:
  targeted ingest, graph, and broker tests passed; CLI smoke commands and full repo-native validation passed
- Inferred:
  receipt and flow assembly remain bounded to exact anchors, one-hop structural edges, and selected verification targets
- Why no benchmark was needed, if applicable:
  this slice adds small evidence and flow vectors on top of maintained snapshot data instead of introducing a broader retrieval plane

## Residual Risks

- the first flow capsule layer is structural, not semantic
- evidence receipts cite maintained locations and planned checks but do not yet include snippet hashes
- `relevant_decisions` remains intentionally empty

## Related

- Plan: [V1 foundation plan](d:/RepoBrainOS/plans/v1-foundation-plan.md)
- SDD / ADR: [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md), [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md), [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md), [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
