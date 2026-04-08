# Intent: Retrieval Pipeline (Lexical + Bounded Graph + Coverage Audit)

Status: draft
Date: 2026-04-03
Owner: RepoBrain OS

## Problem Statement

RepoBrain currently relies on exact anchors plus snapshot-backed structural graph outputs, but it lacks a real lexical retrieval plane and a retrieval-aware coverage audit. That means non-exact queries and broad questions cannot be handled reliably, and the system cannot deterministically confirm whether evidence slots are sufficiently covered for the request type.

## Why Now

The V1 product shape depends on query-aware retrieval with lexical, structural, and coverage-audited evidence. Without this pipeline, `get_brief`, `blast_radius`, and safe-edit guidance cannot reach the latency-first, evidence-backed bar defined in the SDDs.

## Goals

- add a real lexical retrieval plane with a local-first index
- add bounded structural expansion from lexical and exact anchors
- add retrieval-aware coverage audits with explicit slot status
- enable a targeted second pass when required slots remain missing
- keep evidence receipts and freshness labels attached to retrieval output

## Non-Goals

- dense retrieval as a default path
- learned rerankers or neural retrieval policy
- full decision mining or semantic extraction beyond current deterministic scope

## Constraints

- Technical:
  - hot-path latency must remain bounded by `repobrain-serving` budgets
  - retrieval must bind to a snapshot and avoid unbounded graph traversal
  - evidence receipts must remain deterministic and stable
- Product:
  - safe-edit requests should not overclaim coverage
  - failures should surface explicit gaps rather than silent misses
- Time / Team:
  - v1 should ship with a minimal, trustworthy lexical + structural baseline

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale lexical index with bounded per-request candidate budgets
- Hot, warm, or cold path:
  hot interactive path for IDE-safe requests
- Dominant operations and expected frequency:
  lexical lookup, bounded adjacency expansion, coverage audit
- Latency / throughput / memory sensitivity:
  interactive p95 under 800ms; index maintenance must be background-friendly
- Acceptable simplicity-over-speed tradeoff, if any:
  prefer a simple local-first index over heavy dependencies in v1
- Likely failure mode at 10x scale:
  candidate explosion without strict budgets or graph bounds

## Success Metrics

- `get_brief` uses lexical + bounded structural retrieval when exact anchors are absent
- coverage audit reports explicit gaps for missing required slots
- retrieval stages remain within latency budgets for `interactive` requests
- no unbounded traversal on hot paths

## Risks of Inaction

- interactive requests stay brittle and exact-anchor dependent
- retrieval quality remains opaque for safe-edit tasks
- V1 product shape cannot meet evidence-backed expectations

## Research Scope

- lexical indexing options (SQLite FTS5 vs embedded Rust index)
- multi-path retrieval evidence from recent code retrieval papers
- graph expansion bounds and coverage-audit strategies

## Acceptance Shape

- Primary user-visible outcomes:
  lexical fallback and bounded expansion contribute to `BriefingPack` evidence
- Invariants that must remain true:
  retrieval never exceeds configured hop/candidate budgets
- Verification targets:
  `cargo test -p repobrain-retrieval`
  `cargo test -p repobrain-broker`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
