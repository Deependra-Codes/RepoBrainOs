# Intent: Structural Adjacency And Blast Radius

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain can now inventory files and symbols from snapshot data, but `blast_radius` is still not backed by a real structural plane. That keeps impact reasoning too close to raw paths and too far from the repo’s intended exact-plus-structural backbone.

## Why Now

Imports plus file-to-symbol ownership are the smallest structural layer that can turn exact anchors into bounded neighborhood expansion. This is the minimum credible step toward a real `blast_radius` tool.

## Goals

- Extract direct local import edges during ingest.
- Reuse symbol ownership as the bridge from symbol anchors to owning files.
- Add a first graph-side blast-radius implementation over maintained snapshot data.
- Expose the result through the CLI.

## Non-Goals

- Deep semantic dependency analysis
- Multi-hop graph ranking
- Call graph extraction
- Test/build edge extraction
- Schema changes

## Constraints

- Technical:
  Keep adjacency extraction deterministic, local-first, and dependency-light.
- Product:
  Blast radius must remain bounded and honest about being structural rather than semantic.
- Time / Team:
  Prefer one-hop structural expansion over a speculative general graph framework.

## Workload And Performance Shape

- Expected input size / scale path:
  import extraction over repo-scale source files and one-hop neighborhood expansion for interactive blast-radius queries
- Hot, warm, or cold path:
  import extraction is a cold-to-warm maintenance path; blast-radius expansion is an interactive bounded path
- Latency / throughput / memory sensitivity:
  interactive blast-radius should stay bounded and predictable; extraction cost is acceptable during snapshot maintenance
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; one-hop structural expansion is intentionally simpler than deeper graph analysis in this slice

## Success Metrics

- A snapshot stores direct import edges alongside files and symbols.
- Blast radius accepts either a path or a symbol name.
- Symbol targets expand through file ownership into structural neighbors.

## Risks of Inaction

- `blast_radius` would remain mostly conceptual.
- Symbol lookup would still stop at ownership instead of helping change-safety.
- Structural retrieval would lag behind the repo’s intended v1 product shape.

## Research Scope

This slice follows the current architecture:

- structural-plane grounding from [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- production-sidecar shape from [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
- syntax-vs-semantic discipline from [SDD-006](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)

No new external research is required before this bounded implementation.

## Acceptance Shape

- Primary user-visible outcomes:
  A user can ask for blast radius on a path or symbol and receive bounded structural neighbors.
- Invariants that must remain true:
  The output stays snapshot-bound and does not claim semantic certainty.
- Verification targets:
  Ingest tests, graph tests, CLI smoke checks, and repo-native validation.
