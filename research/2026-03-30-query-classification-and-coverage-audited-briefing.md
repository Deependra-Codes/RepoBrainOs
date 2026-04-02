# Research: Query Classification And Coverage-Audited Briefing

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Research Question

What is the smallest durable step that moves `repobrain-broker` from deterministic packer toward a real retrieval planner without overclaiming current system capability?

## Candidate Options

1. Keep `get_brief` as a task-agnostic packer and only improve retrieval later.
2. Add query classification only, but no explicit coverage audit.
3. Add query classification plus required coverage slots and surface insufficiency in the resulting brief.
4. Add a full multi-pass planner with reranking and second-pass retrieval immediately.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Task-agnostic packer | Lowest implementation cost | Broker moat stays weak | Briefs look confident even when request shape differs | Low | Reject |
| 2. Classification only | Better routing language | Still no explicit trust check | Missing coverage remains implicit | Medium | Incomplete |
| 3. Classification plus coverage audit | Adds planner semantics and explicit uncertainty | Requires contract updates across mirrors | Some classes will remain intentionally incomplete for now | Medium | Recommended |
| 4. Full planner now | Higher long-run ceiling | Too much surface change for current maturity | Overbuild risk and weak testability | High | Reject for now |

## Main Conclusion

The right next step is not heavier retrieval first.

The right next step is to make the broker honest about:

- what kind of request it thinks it is serving
- which evidence coverage slots that request type requires
- whether the current deterministic slice actually filled those slots

This gives RepoBrain a real trust upgrade now, while staying aligned with the staged roadmap.

## What Existing Repo Research Suggests

### 1. The roadmap already identifies broker planning as the next compounding step

[Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md) explicitly places query classification and coverage slots after parser-backed ingest and typed graph edges.

Implication:

- this slice is the direct next build-order step, not a side quest

### 2. Retrieval quality is not enough if trust remains implicit

[Retrieval pipeline research](d:/RepoBrainOS/research/2026-03-30-retrieval-pipeline-research.md) and [architecture reality check](d:/RepoBrainOS/research/2026-03-30-architecture-reality-check.md) both stress boundedness, receipts, and explicit uncertainty over inflated confidence.

Implication:

- if a request needs decision evidence and none exists, the brief should say so explicitly

### 3. Current system capability is deterministic exact plus structural, not semantic intent recovery

The current code path can anchor exact files and symbols, expand structural neighbors, emit flow capsules, and attach verification targets.

Implication:

- the first coverage slots should be grounded in those real capabilities
- decision or rationale coverage should be allowed to fail honestly

## Design Conclusions For RepoBrain

### 1. Add derived query classification to the `BriefingPack`

Classification should be derived from `ContextRequest.task_type` plus lightweight textual hints from `goal` and `question`.

For the first slice, the important classes are:

- `entity_lookup`
- `architecture_explanation`
- `safe_edit`
- `bug_fix`
- `feature_implementation`
- `decision_why`

### 2. Add a typed coverage audit with required slots

Coverage should not be a prose-only note.

The brief should include:

- the chosen query classification
- the required slots for that class
- per-slot status with detail
- a summary and a boolean sufficiency judgment

### 3. Keep the first slot set narrow and capability-matched

The initial slots should reflect current deterministic capability:

- `exact_anchor`
- `structural_context`
- `flow_summary`
- `verification_targets`
- `impact_envelope`
- `decision_evidence`

### 4. Surface insufficiency in the brief itself

When coverage is incomplete, the pack should visibly say so in `must_know` or `reasoning_scaffold`, not only in a hidden internal field.

## Recommendation

Adopt Option 3:

- typed query classification
- typed coverage audit
- explicit insufficiency surfaced in the briefing pack

Do not add dense retrieval or full multi-pass planning in this slice.

## Direct Evidence vs Inference

- Direct:
  - the roadmap explicitly calls for query classification and coverage slots as the next broker step
  - the current system already has exact anchors, structural context, flow capsules, and verification targets
- Inferred:
  - typed coverage audit is the smallest change that meaningfully upgrades trust without pretending current retrieval is more semantic than it is

## Unknowns / Follow-Ups

- whether future `get_brief` should abstain instead of only surfacing insufficiency for some query classes
- whether broker classification should become user-visible CLI input as well as derived output
- which additional slots become necessary once decision extraction or lexical retrieval improves

## Engineering Impact

- Contract / type impact:
  - add `query_classification` and `coverage_audit` to `BriefingPack`
- Testing impact:
  - broker tests for sufficient and insufficient coverage classes
  - mirror updates across Rust, TypeScript, and Python contracts
- Runtime / latency impact:
  - negligible compared with existing graph and broker work; classification and slot audit are bounded over already-collected data
- Workload assumptions:
  - repeated snapshot-backed `get_brief` requests with varying task intent but stable deterministic retrieval
- Dominant operations and cost centers:
  - slot evaluation over exact hits, flow capsules, verification targets, and impact summary
- Candidate data structures / indexes:
  - enums plus short vectors for required slots and slot results
- Main rejected alternative and why:
  - full planner stack now was rejected because the next most useful trust gain is coverage honesty, not extra moving parts
- Time complexity / constant-factor impact:
  - classification and audit stay `O(1)` to `O(k)` over small broker-local vectors, where `k` is the number of required slots
- Memory / allocation impact:
  - small extra allocation for slot results and summary strings in the final brief
- Measurement plan or reason no benchmark is needed:
  - no separate benchmark is needed for this slice because it is a small bounded addition on top of already-measured hot paths

## Sources

- Primary:
  - [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)
  - [Retrieval pipeline research](d:/RepoBrainOS/research/2026-03-30-retrieval-pipeline-research.md)
  - [Architecture reality check](d:/RepoBrainOS/research/2026-03-30-architecture-reality-check.md)
- Secondary:
  - none
