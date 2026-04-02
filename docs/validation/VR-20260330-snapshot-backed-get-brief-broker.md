# Verification Record: Snapshot-Backed Get-Brief Broker

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the first real `get_brief` slice covering:

- a reusable Rust broker boundary for snapshot-backed briefing assembly
- deterministic compilation of `BriefingPack` from exact anchors, structural adjacency, and verification planning
- CLI exposure through `repobrain get-brief`
- bounded empty behavior for unsupported fields such as flows, decisions, and evidence receipts

## Intent / Spec References

- Intent: [Snapshot-backed get-brief broker intent](d:/RepoBrainOS/plans/2026-03-30-snapshot-backed-get-brief-broker-intent.md)
- Spec: [Snapshot-backed get-brief broker spec](d:/RepoBrainOS/plans/2026-03-30-snapshot-backed-get-brief-broker-spec.md)

## Commands / Checks Run

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

RepoBrain now has a reusable snapshot-backed `get_brief` path through a dedicated `repobrain-broker` crate.

The broker reuses:

- exact anchors from ingest
- one-hop structural adjacency and verification planning from graph
- readiness, snapshot binding, and overlay disclosure from serving
- model-aware packing from compiler

Smoke validation on `d:\RepoBrainOS` produced a real serialized `BriefingPack` for `RepositoryScanner` with snapshot binding `D:\RepoBrainOS::worktree`, readiness `ready`, deterministic must-know items, suggested files from the ingest neighborhood, and verification targets including `cargo test (in src/rust/crates/repobrain-ingest)` and `cargo check (in src/rust/crates/repobrain-ingest)`.

Unsupported higher-level fields stayed intentionally narrow:

- `relevant_flows` remained empty
- `relevant_decisions` remained empty
- `evidence_index` remained empty

## Pass / Fail Against Expectations

Pass.

This slice turns the repo's maintained retrieval state into the first real task-facing `BriefingPack` without inventing semantic knowledge that the current system does not yet have.

## Performance / Complexity Validation

- Measured:
  broker tests, CLI smoke commands, and full repo-native validation passed
- Inferred:
  the broker reuses maintained exact lookups and one-hop structural expansion, then performs only small packing-stage set and vector work
- Why no benchmark was needed, if applicable:
  this slice adds bounded composition on top of already-maintained snapshot data rather than introducing a new heavy retrieval plane

## Residual Risks

- `get_brief` currently requires an exact path or exact symbol scope
- `relevant_flows`, `relevant_decisions`, and evidence receipts are still intentionally empty
- impact summaries are structural and deterministic, not semantic
- readiness is caller-provided; this slice does not yet infer degraded or overlay-only states on its own

## Related

- Plan: [V1 foundation plan](d:/RepoBrainOS/plans/v1-foundation-plan.md)
- SDD / ADR: [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md), [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md), [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md), [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
