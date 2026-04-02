# Intent: Snapshot-Backed Get-Brief Broker

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain now has snapshot inventory, exact lookup, structural blast radius, and verification planning, but it still does not compile those maintained facts into the product's main briefing surface: `get_brief`.

## Why Now

The production shape explicitly calls for `get_brief` before broader evidence and freshness enrichment. Without a real broker path, the repo has strong lower-level pieces but no end-to-end task-aware briefing pack for a coding agent to consume.

## Goals

- add a thin Rust broker boundary that owns snapshot-backed `get_brief` composition
- compile deterministic `BriefingPack` outputs from exact anchors, structural adjacency, and verification planning
- expose the first real `get-brief` path through the CLI for live demos
- keep the slice honest by leaving unsupported fields narrow or empty instead of inventing semantic certainty

## Non-Goals

- semantic retrieval
- flow mining or decision mining
- evidence receipt generation
- automatic command execution
- MCP server implementation in this slice

## Constraints

- Technical:
  keep the broker thin and reuse existing `repobrain-ingest`, `repobrain-graph`, `repobrain-serving`, and `repobrain-compiler` crates instead of duplicating their logic
- Product:
  return a useful brief for safe-edit style requests without overstating certainty beyond exact and one-hop structural evidence
- Time / Team:
  prefer a deterministic baseline that the repo can verify today over a broader but weakly grounded broker

## Workload And Performance Shape

- Expected input size / scale path:
  interactive scope-bound briefing requests over an already-maintained snapshot
- Hot, warm, or cold path:
  interactive hot path built on warm maintained data
- Latency / throughput / memory sensitivity:
  the broker should stay light by reusing exact lookups and bounded structural expansion instead of reparsing the repo
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; small `Vec` packing work is acceptable after bounded retrieval, but the slice should avoid avoidable whole-snapshot rescans on the request path

## Success Metrics

- `get-brief` returns a non-empty `BriefingPack` for a valid symbol or path scope
- the brief includes suggested files and verification targets derived from maintained snapshot state
- the new broker boundary is reusable from CLI now and MCP later

## Risks of Inaction

- RepoBrain remains a set of lower-level capabilities without its main task-facing compilation surface
- demos continue to show pieces instead of the promised trustworthy briefing pack
- future integration work risks spreading briefing assembly logic across CLI and MCP layers

## Research Scope

This slice is bounded by the current RepoBrain architecture:

- context broker and briefing-pack expectations from [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md)
- latency-safe broker composition from [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- production order and `get_brief` priority from [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
- trust envelope expectations from [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)

No additional external research is required for this deterministic first broker slice.

## Acceptance Shape

- Primary user-visible outcomes:
  `get-brief` returns a snapshot-bound briefing pack with must-know guidance, suggested files, and verification targets for a scoped request.
- Invariants that must remain true:
  the brief stays honest about exact versus structural knowledge and does not fabricate flows, decisions, or evidence receipts.
- Verification targets:
  broker tests, CLI smoke checks, and full repo-native validation.
