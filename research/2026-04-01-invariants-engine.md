# Research: Invariants Engine

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

What is the smallest deterministic approach that can populate invariant guidance in the existing graph -> broker -> CLI flow without introducing semantic overclaiming?

## Candidate Options

1. Keep invariant fields empty and rely only on verification checks.
2. Emit fixed generic invariant text unrelated to scope.
3. Derive scoped invariants from snapshot-backed anchors, structural edges, and required checks with bounded deterministic sets.
4. Add semantic or LLM-generated invariant synthesis.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Keep empty invariants | Zero implementation effort | User-facing invariant surfaces stay hollow | Safe-edit guidance remains weaker than SDD intent | Low | Reject |
| 2. Generic text | Fast to ship | Low relevance per scope | High risk of boilerplate and weak trust | Low | Reject |
| 3. Snapshot-bound derived invariants | Scoped, deterministic, contract-aligned output | Requires graph logic and test updates | Limited semantic depth by design | Medium | Recommended |
| 4. Semantic synthesis | Potentially richer narratives | Higher complexity and trust risk | Overclaiming and non-determinism in hot path | High | Reject for v1 |

## Sources

- Primary:
  - [AGENTS](d:/RepoBrainOS/AGENTS.md)
  - [Engineering execution policy](d:/RepoBrainOS/docs/standards/ENGINEERING_EXECUTION_POLICY.md)
  - [Agent guardrail system](d:/RepoBrainOS/docs/standards/AGENT_GUARDRAIL_SYSTEM.md)
  - [SDD-007 production shape and v1 cut](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
  - [SDD-008 trust, readiness, and verification model](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
  - [Verification target extraction and check planning spec](d:/RepoBrainOS/plans/2026-03-30-verification-target-extraction-and-check-planning-spec.md)
  - [Evidence receipts and structural flow capsules spec](d:/RepoBrainOS/plans/2026-03-30-evidence-receipts-and-structural-flow-capsules-spec.md)
- Secondary:
  - none

## Recommendation

Adopt Option 3: derive bounded deterministic invariants directly in `repobrain-graph`, then reuse those values through existing `verification_plan.invariants` and `do_not_break` wiring.

## Direct Evidence vs Inference

- Direct:
  - `VerificationPlan` and `BriefingPack` already carry invariant fields across Rust/TypeScript/Python contracts.
  - `repobrain-graph` currently emits `invariants: Vec::new()` in both blast-radius report and verification plan.
  - `repobrain-broker` maps `verification_plan.invariants` into `do_not_break`.
  - SDD-008 requires file/symbol-level invariants as planner output and warns against broad unsupported certainty.
- Inferred:
  - deterministic anchor and structural-edge invariants are the best near-term trust improvement without semantic overreach.

## Unknowns / Follow-Ups

- whether future slices should attach evidence ids directly to each invariant statement
- whether invariant prioritization should become query-classification-aware
- whether additional deterministic sources (tests-to-symbol mapping, build targets-to-files) should feed invariant generation

## Engineering Impact

- Contract / type impact:
  no schema shape changes; existing invariant fields become populated
- Testing impact:
  graph and broker tests need explicit invariant assertions
- Runtime / latency impact:
  small hot-path overhead for bounded set assembly and formatting
- Workload assumptions:
  interactive scoped requests over snapshot-maintained exact and structural facts
- Dominant operations and cost centers:
  deduplicated string assembly from small candidate sets
- Candidate data structures / indexes:
  transient `BTreeSet<String>` for deterministic order and dedup
- Main rejected alternative and why:
  generic invariant boilerplate was rejected because it weakens scope relevance and trust
- Time complexity / constant-factor impact:
  bounded `O(k log k)` operations per request with small `k`
- Memory / allocation impact:
  one temporary bounded set and small output vectors
- Measurement plan or reason no benchmark is needed:
  no dedicated benchmark needed; bounded hot-path logic validated through existing repo-native checks
