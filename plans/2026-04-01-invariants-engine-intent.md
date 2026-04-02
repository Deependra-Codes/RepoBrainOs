# Intent: Invariants Engine

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

`verification_plan.invariants` exists in the shared contract, but the graph planner currently emits an empty invariant set for scoped blast-radius queries. That means `do_not_break` and `list_invariants` are structurally wired but underpowered.

## Why Now

RepoBrain now has deterministic symbol, path, import, evidence, and verification-target extraction in the snapshot path. The missing piece is a deterministic Invariants Engine that turns those facts into scoped "must remain true" guidance before edits.

## Goals

- derive deterministic, snapshot-bound invariants for scoped blast-radius queries
- populate both `BlastRadiusReport.invariants` and `VerificationPlan.invariants`
- ensure broker `do_not_break` and CLI `list-invariants` produce non-empty scoped invariants when deterministic anchors exist
- keep invariant generation bounded, deduplicated, and deterministic

## Non-Goals

- semantic intent inference
- natural-language reasoning over docs or commit history
- probabilistic or LLM-generated invariants
- schema changes

## Constraints

- Technical:
  use only maintained snapshot facts (exact anchors, import adjacency, and planned checks)
- Product:
  invariant wording must stay structural and avoid semantic overclaims
- Time / Team:
  prefer a narrow deterministic baseline over broader speculative coverage

## Workload And Performance Shape

- Expected input size / scale path:
  one interactive blast-radius request over one-hop impacted paths and a bounded verification target set
- Hot, warm, or cold path:
  hot interactive path
- Dominant operations and expected frequency:
  bounded set construction, deduplication, and deterministic string rendering per request
- Latency / throughput / memory sensitivity:
  latency-sensitive; per-request allocations must remain small and bounded
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; small sorted sets are preferred for deterministic ordering
- Likely failure mode at 10x scale:
  overlong invariant lists if not capped, causing unnecessary response noise

## Success Metrics

- blast-radius reports include non-empty deterministic invariants for anchored path/symbol targets
- `verification_plan.invariants` equals `BlastRadiusReport.invariants`
- broker `do_not_break` contains those invariants without extra ad hoc synthesis
- repo-native validation remains green

## Risks of Inaction

- `list_invariants` remains mostly empty despite available deterministic evidence
- safe-edit guidance stays below SDD expectations for file/symbol-level invariants
- downstream consumers keep carrying invariant fields that do not convey usable constraints

## Research Scope

This slice is bounded by existing RepoBrain architecture and contracts:

- [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
- [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- [Verification target extraction and check planning spec](d:/RepoBrainOS/plans/2026-03-30-verification-target-extraction-and-check-planning-spec.md)

No external source is required for this deterministic first implementation.

## Acceptance Shape

- Primary user-visible outcomes:
  blast-radius and list-invariants surfaces return deterministic scoped invariants.
- Invariants that must remain true:
  invariant generation remains snapshot-bound, one-hop structural, and does not imply semantic certainty.
- Verification targets:
  `cargo test -p repobrain-graph -p repobrain-broker`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
