# Intent: Evidence Receipts And Structural Flow Capsules

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain can now compile a snapshot-backed `BriefingPack`, but its main claims still arrive without evidence receipts and without even a first deterministic flow layer. That leaves the brief correctly shaped but under-explained.

## Why Now

The current `get_brief` slice is already useful, but the next major trust gain is letting the brief cite why a claim exists and expose a compact structural flow summary for the current scope. That closes the biggest remaining gap between shaped output and evidence-backed output.

## Goals

- add evidence receipts for exact anchors, structural edges, and planned verification targets
- bind those receipts to the maintained snapshot instead of inventing ad hoc briefing-time metadata
- add a first deterministic structural flow capsule layer from direct anchor and import adjacency
- populate `must_know.evidence_ids`, `relevant_flows`, and `evidence_index` in `get_brief`

## Non-Goals

- semantic flow synthesis
- decision mining
- contradiction handling
- evidence snippet extraction
- schema changes

## Constraints

- Technical:
  keep evidence generation deterministic and snapshot-bound, and keep flow capsules explicitly structural rather than semantic
- Product:
  improve trust and drill-down without pretending the system understands intent-level flows yet
- Time / Team:
  prefer one honest structural flow capsule and strong receipts over a broader but fuzzier abstraction layer

## Workload And Performance Shape

- Expected input size / scale path:
  snapshot maintenance adds bounded metadata, and interactive requests assemble receipts and one-hop flow capsules from already-maintained state
- Hot, warm, or cold path:
  snapshot capture time is warm maintenance; evidence and flow assembly for a scoped request stays on the interactive hot path
- Latency / throughput / memory sensitivity:
  the hot path must stay bounded to exact anchors, direct imports, and existing verification plans
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; small receipt and flow vectors are acceptable as long as they are derived from maintained snapshot data and not whole-repo recomputation

## Success Metrics

- `get_brief` returns non-empty `evidence_index` entries for supported exact and structural claims
- `must_know` items carry evidence ids where the claim is directly supported
- `relevant_flows` becomes non-empty for supported scoped requests

## Risks of Inaction

- the brief still feels less trustworthy than the architecture promises
- users cannot easily trace claims back to maintained repo evidence
- later `explain_flow` work risks starting without a reusable deterministic baseline

## Research Scope

This slice is bounded by the current RepoBrain architecture:

- evidence receipts and flow capsule expectations from [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md)
- retrieval unit and structural-plane guidance from [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- deferred higher-level regeneration guidance from [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- trust-envelope expectations from [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)

No additional external research is required for this deterministic first slice.

## Acceptance Shape

- Primary user-visible outcomes:
  `get-brief` cites evidence ids, returns an evidence index, and includes a structural flow summary for the current scope.
- Invariants that must remain true:
  the system must not present structural flow capsules as semantic certainty, and every emitted evidence receipt must be snapshot-bound.
- Verification targets:
  ingest tests, graph tests, broker tests, CLI smoke checks, and full repo-native validation.
