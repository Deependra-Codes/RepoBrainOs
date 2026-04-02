# ADR-001: Accuracy-First V1 Constraints

Status: Accepted
Date: 2026-03-30

## Context

An architecture review surfaced three valid risks:

1. "Semantic delta" sounded stronger than what can be guaranteed deterministically.
2. The original v1 local-first stack implied too many mandatory local databases.
3. A fully atomized knowledge model risked over-formalizing fuzzy engineering knowledge.

## Decision

RepoBrain OS will adopt the following accuracy-first constraints:

### 1. Use scoped impact estimation, not full semantic invalidation claims

The system may:

- detect direct structural impact deterministically
- propagate bounded freshness/staleness through dependency edges
- emit confidence-labeled impact summaries

The system may not:

- claim exact semantic equivalence or exact architectural meaning preservation after arbitrary code changes

### 2. Keep v1 local-first storage minimal

Required for local mode:

- SQLite
- JSON/Parquet artifacts

Optional accelerators:

- Kuzu
- DuckDB
- Qdrant

This keeps onboarding friction low while preserving a scale-up path.

### 3. Separate hard facts from soft knowledge

The knowledge model uses:

- fact records for deterministic truths
- claim records for synthesized evidence-backed understanding
- note records for fuzzy human knowledge

This prevents tribal knowledge from being falsely presented as a hard machine fact.

## Consequences

Benefits:

- more honest system claims
- lower false-confidence risk
- simpler v1 local setup
- cleaner distinction between deterministic and fuzzy knowledge

Tradeoffs:

- impact summaries are less ambitious than a true semantic oracle
- some advanced graph/vector capabilities move from mandatory v1 to optional acceleration
- more explicit uncertainty appears in outputs

## Follow-Through

This ADR is reflected in:

- [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md)
- [SDD-002](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md)
