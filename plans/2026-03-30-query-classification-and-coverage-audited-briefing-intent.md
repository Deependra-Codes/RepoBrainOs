# Intent: Query Classification And Coverage-Audited Briefing

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

`repobrain-broker` could already package exact anchors, structural neighbors, flow capsules, and verification planning, but it still did not say what kind of request it was answering or whether the available evidence was sufficient for that request shape.

## Why Now

The roadmap explicitly names broker query classification and coverage slots as the next step after parser-backed ingest and typed graph relationships.

## Goals

- add derived query classification to the broker output
- add typed coverage audit with required slots per query class
- surface incomplete coverage honestly in the resulting `BriefingPack`
- keep the first version aligned with current deterministic capability

## Non-Goals

- full reranking or second-pass retrieval
- dense retrieval
- semantic decision extraction

## Constraints

- Technical:
  - keep Rust, TypeScript, Python, and schema mirrors aligned
- Product:
  - do not overclaim decision or rationale understanding
- Time / Team:
  - prefer a small typed contract upgrade over a broad planner rewrite

## Workload And Performance Shape

- Expected input size / scale path:
  one broker request over already-collected exact and structural evidence
- Hot, warm, or cold path:
  hot interactive broker path
- Dominant operations and expected frequency:
  lightweight classification and slot audit per `get_brief`
- Latency / throughput / memory sensitivity:
  latency-sensitive, but extra bounded vector work is acceptable
- Acceptable simplicity-over-speed tradeoff, if any:
  typed audit bookkeeping is preferred over more implicit logic
- Likely failure mode at 10x scale:
  classification stays cheap; drift risk is contract sync, not runtime cost

## Success Metrics

- `BriefingPack` exposes `query_classification` and `coverage_audit`
- safe-edit requests can report sufficient coverage
- decision-why requests with no decision evidence report incomplete coverage explicitly
- repo validation remains green

## Risks of Inaction

- broker remains a thin packer rather than a planner-like surface
- users cannot tell when a brief is incomplete for the request they actually asked
- future retrieval upgrades lack a typed place to declare coverage sufficiency

## Research Scope

- broker planner step from the roadmap
- current deterministic capability boundaries for initial coverage slots

## Acceptance Shape

- Primary user-visible outcomes:
  `get_brief` exposes query classification and coverage sufficiency
- Invariants that must remain true:
  exact-scope anchor behavior remains deterministic and evidence-backed
- Verification targets:
  `cargo test -p repobrain-domain`
  `cargo test -p repobrain-compiler`
  `cargo test -p repobrain-broker`
  `cargo xtask perf`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
