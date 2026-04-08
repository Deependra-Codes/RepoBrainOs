# Spec: Retrieval Pipeline (Lexical + Bounded Graph + Coverage Audit)

Status: draft
Date: 2026-04-03
Owner: RepoBrain OS

## Intent Reference

- Intent: [Retrieval pipeline intent](d:/RepoBrainOS/plans/2026-04-03-retrieval-pipeline-intent.md)

## Problem Statement

RepoBrain lacks a production retrieval pipeline that combines lexical indexing, bounded structural expansion, and deterministic coverage audits. This prevents reliable responses for non-exact queries and limits safe-edit guidance.

## Scope

- add a retrieval crate for lexical search, bounded graph expansion, and coverage auditing
- implement a local-first lexical index interface with a lightweight baseline backend
- implement bounded graph expansion over import/reverse-import adjacency
- implement retrieval-aware coverage audits and a targeted second-pass plan
- expose retrieval output in broker integration (follow-on step)

## Non-Goals

- dense or hybrid retrieval as a default v1 path
- learned reranking or model-driven retrieval policies
- full semantic decision mining beyond deterministic evidence

## Behavioral Requirements

1. Lexical retrieval must return deterministic candidates under a bounded budget.
2. Graph expansion must respect hop and candidate limits from serving policy.
3. Coverage audit must declare required slots per query class and mark missing slots explicitly.
4. Targeted second pass must run only when required slots are missing and latency allows it.
5. Evidence receipts must accompany lexical and structural candidates.

## Acceptance Examples

1. A safe-edit request with a non-exact scope hint returns lexical anchors, bounded structural neighbors, and a coverage audit marked sufficient.
2. A decision-why request without ADR evidence returns a coverage audit marked insufficient, with a missing decision-evidence slot.
3. Interactive latency classes cap graph expansion at two hops and enforce candidate limits.

## Invariants

- Must always hold:
  - retrieval never exceeds hop or candidate budgets
  - coverage audit is deterministic and explicit about missing slots
  - evidence receipts are attached for major candidates
- Must not regress:
  - existing exact-lookup behavior
  - broker output shape and verification planning

## Contract And Type Changes

- Schema changes:
  - none required in this slice
- Public interface changes:
  - add internal retrieval types in a new crate
- Illegal states to remove:
  - retrieval results without explicit coverage status for required slots

## Workload And Complexity Notes

- Workload shape and expected scale:
  interactive lexical lookup plus bounded adjacency expansion per request
- Hot, warm, or cold path:
  hot interactive broker path
- Dominant operations and expected frequency:
  tokenized lexical search, adjacency expansion, slot audit
- Chosen data structures and why:
  - lexical index: local-first store (FTS5 preferred for v1)
  - adjacency: in-memory vectors + BTreeSet for dedupe
- Main alternative considered:
  on-demand rg search rejected due to latency instability
- Index ownership / maintenance notes:
  lexical index is snapshot-scoped and refreshed on changed files only
- Key operation costs:
  bounded by candidate budgets and hop limits
- Memory / allocation notes:
  small per-request buffers; index on disk
- Measured vs inferred performance claims:
  inferred for v1; bounded by static budgets
- Benchmark expectation or reason none is needed:
  initial integration will rely on `xtask perf`; add targeted benchmarks after baseline is stable

## Test Strategy

- Unit:
  lexical index tokenization and query ranking
  bounded expansion respects hop/candidate limits
  coverage audit slot status derivation
- Integration:
  broker integrates retrieval outputs into `BriefingPack`
- Regression:
  repo-wide quality gates and snapshot tests for `get_brief`
- Property / invariant:
  retrieval results must never exceed configured budgets

## Verification Notes

- Commands:
  - `cargo test -p repobrain-retrieval`
  - `cargo test -p repobrain-broker`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - retrieval crate, coverage audit updates, broker integration, and validation record
