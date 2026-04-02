# Docs Index

This directory is the architecture source of truth for RepoBrain OS.

## Reading Order

1. [README](d:/RepoBrainOS/README.md)
2. [AGENTS](d:/RepoBrainOS/AGENTS.md)
3. [SDD-001: Repository Cognition And Context Uplift Engine](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md)
4. [SDD-002: Stack And Research Direction](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md)
5. [SDD-004: Retrieval And Grounding Pipeline](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
6. [SDD-005: Latency-First Serving Strategy](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
7. [SDD-006: Contract Compilation And Semantic Extraction](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)
8. [SDD-007: Production Shape And V1 Cut](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
9. [SDD-008: Trust, Readiness, And Verification Model](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
10. [SDD-003: Repo Layout And Boundaries](d:/RepoBrainOS/docs/sdd-003-repo-layout-and-boundaries.md)
11. [ADR-001: Accuracy-First V1 Constraints](d:/RepoBrainOS/docs/adr-001-accuracy-first-v1-constraints.md)
12. [ADR-002: Reality-Check Corrections For V1](d:/RepoBrainOS/docs/adr-002-reality-check-corrections.md)
13. [Standards](d:/RepoBrainOS/docs/standards/README.md)
14. [Templates](d:/RepoBrainOS/docs/templates/README.md)
15. [Validation](d:/RepoBrainOS/docs/validation/README.md)

## What Lives Here

- `sdd-*` documents define major system design decisions.
- `adr-*` documents capture corrections, tradeoffs, and constraints.
- `standards/` defines the rules we want to keep durable.
- `templates/` keeps future docs consistent.
- `validation/` stores proof that important claims were actually checked.

## Architecture Spine

The architecture is intentionally layered:

- `SDD-001` defines the product brain and context uplift concept.
- `SDD-002` defines the phased stack and storage/compute strategy.
- `SDD-004` defines the realistic retrieval and grounding pipeline.
- `SDD-005` defines the latency-first serving strategy for actual developer workflows.
- `SDD-006` defines how contracts compile across languages and where syntax extraction stops and semantic extraction begins.
- `SDD-007` defines the actual production product shape so the system does not become a beautiful but unusable architecture exercise.
- `SDD-008` defines the trust boundary, readiness model, overlay semantics, and verification planner that make the sidecar safe to use in real coding loops.
- `SDD-003` defines repo layout, package boundaries, and anti-chaos rules.
- `ADR-001` constrains v1 around accuracy and low-friction local-first usage.
- `ADR-002` captures the major realism corrections that keep the product idea honest under scale and messy real repos.
- `standards/` now also defines execution policy and type/spec/test discipline for AI-assisted engineering.
- `AGENTS.md` defines the default operating contract for AI contributors.

## Novelty Thesis

RepoBrain is not trying to be:

- another AI doc generator
- another vector search wrapper
- another code graph viewer

RepoBrain is trying to be:

- a repository cognition system
- a context uplift system
- a query-aware retrieval and grounding system
- a latency-aware serving system
- an accuracy-first, model-adaptive context layer for coding agents
