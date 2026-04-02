# Intent: Verification Target Extraction And Check Planning

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain can now identify files, symbols, and one-hop structural blast radius, but it still stops short of the most important safe-edit question: which concrete checks should run after a likely change.

## Why Now

The trust model and production shape both require verification planning for safe-edit flows. Without deterministic check discovery, `blast_radius` remains informative but not yet meaningfully action-guiding.

## Goals

- extract deterministic verification targets from repo-local manifests and test layouts
- persist those targets in the maintained snapshot
- plan required and recommended checks from blast-radius scope
- surface coverage gaps and stop conditions when no deterministic checks are known
- expose the result through the CLI for demoable safe-edit guidance

## Non-Goals

- automatic test execution
- semantic test selection
- per-language dependency graph resolution beyond current structural scope
- schema changes

## Constraints

- Technical:
  keep extraction deterministic, dependency-light, and snapshot-bound
- Product:
  suggest checks honestly without implying full verification coverage
- Time / Team:
  prefer a narrow deterministic baseline over ambitious but weakly grounded heuristics

## Workload And Performance Shape

- Expected input size / scale path:
  manifest and test-layout discovery during snapshot maintenance, plus scoped check planning for interactive blast-radius queries
- Hot, warm, or cold path:
  extraction is cold-to-warm maintenance; check planning is an interactive bounded path
- Latency / throughput / memory sensitivity:
  interactive planning should stay lightweight by reusing maintained snapshot data; extraction cost is acceptable during scan
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; linear scans over a small verification-target set are acceptable in this deterministic first slice

## Success Metrics

- scans persist verification targets alongside files, symbols, and imports
- blast radius returns required and recommended checks for impacted package roots when known
- missing deterministic coverage is surfaced explicitly rather than hidden

## Risks of Inaction

- safe-edit guidance stays weaker than the trust SDD promises
- blast radius remains mostly descriptive instead of actionable
- users still need to rediscover basic verification commands manually

## Research Scope

This slice is already bounded by the current RepoBrain architecture:

- verification planning from [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- production-sidecar safe-edit support from [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)

No additional external research is required before this deterministic baseline.

## Acceptance Shape

- Primary user-visible outcomes:
  `blast-radius` returns concrete required and recommended checks when deterministic targets are known.
- Invariants that must remain true:
  check suggestions stay snapshot-bound and disclose missing coverage instead of bluffing.
- Verification targets:
  ingest tests, graph tests, CLI smoke checks, and repo-native validation.
