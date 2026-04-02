# Research: L2 Relational Semantic Backend

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

How should RepoBrain implement the first executable L2 relational semantic stage while preserving deterministic behavior, bounded latency, and explicit evidence semantics?

## Candidate Options

1. Keep L2 as planned-only backlog.
2. Gate L2 on external solver tooling only.
3. Implement deterministic in-process relational alignment/comparison now, with solver-backed hardening as follow-up.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Planned-only L2 | No implementation effort | Ladder remains incomplete | User-visible roadmap gap | Low | Reject |
| 2. Solver-only L2 | Stronger theoretical ceiling | Heavy environment dependency | Frequent inconclusive runs and setup friction | Medium-High | Reject for first slice |
| 3. Deterministic baseline L2 | Executable now, bounded, explainable | Weaker than theorem proving | Can be misread if wording is sloppy | Medium | Recommended |

## Primary Sources

- SymDiff (paired relational checks):
  - <https://www.microsoft.com/en-us/research/publication/symdiff-a-language-agnostic-semantic-diff-tool-for-imperative-programs/>
- Differential equivalence checking inspiration (matching/alignment sensitivity):
  - DDEC OOPSLA 2013: <https://dl.acm.org/doi/10.1145/2509136.2509504>
  - KestRel: <https://arxiv.org/abs/2305.04745>
- RepoBrain architecture constraints:
  - [Semantic equivalence ladder](d:/RepoBrainOS/research/2026-04-01-semantic-equivalence-ladder.md)
  - [ADR-001](d:/RepoBrainOS/docs/adr-001-accuracy-first-v1-constraints.md)

## Findings

- Relational semantic checks require high-quality alignment; exact-name-only pairing is insufficient for refactors.
- A bounded alignment ladder (exact -> same-name -> signature-aware similarity) is deterministic and practical for CLI.
- Snapshot-only evidence is enough to provide relational signals, but not enough to claim proof-grade equivalence.
- Bounded policy controls are required for predictable interactive latency.

## Recommended L2 Policy

- Keep L2 opt-in (`--stage l2-relational-semantic`).
- Use explicit policy controls for pair/candidate/time bounds and alignment sensitivity.
- Emit machine-readable receipts with:
  - stage and backend
  - status (`no_difference_observed`, `observed_difference`, `inconclusive`)
  - bounds and timeout
  - assumptions and witness
- Keep wording explicit that L2 baseline uses bounded relational signatures, not unrestricted theorem proving.

## Direct Evidence vs Inference

- Direct:
  - snapshot facts provide deterministic symbol/import context for changed paths
  - alignment and bounded comparison can run without external solver dependency
- Inferred:
  - deterministic L2 baseline is the fastest path to stage-complete ladder behavior while preserving honesty about guarantee level

## Unknowns / Follow-Ups

- solver-backed paired obligations for deeper relation checks
- cross-language function alignment precision under large refactors
- richer witness traces for CI-grade debugging

## Engineering Impact

- Contract / type impact:
  no schema changes; CLI output remains within existing equivalence contract/evidence structures
- Testing impact:
  add L2 unit coverage for alignment and relational status mapping
- Runtime / latency impact:
  bounded by configured candidate/pair/time policy; complexity is controlled by explicit caps
