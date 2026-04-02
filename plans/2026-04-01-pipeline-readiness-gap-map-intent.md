# Intent: Pipeline Readiness Gap Map

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

RepoBrain has a working deterministic v1 slice, but there is no single docs-first map that clearly separates:

- implemented pipeline stages
- not-yet-setup stages
- intentionally deferred stages

This makes roadmap and execution order harder to align during ongoing development.

## Why Now

The core blast-radius, get-brief, explain-flow, and list-invariants path is now functional. The next highest-leverage step is to lock an explicit readiness map before adding more implementation scope.

## Goals

- document current pipeline status from code and executable commands
- identify not-yet-setup pipeline segments with concrete evidence
- distinguish hard gaps from intentionally deferred work
- define docs-first build order for remaining pipeline setup

## Non-Goals

- implementing missing pipeline stages in this slice
- changing contracts or runtime behavior
- adding new retrieval channels

## Constraints

- Technical:
  status claims must map to current source code and runnable commands
- Product:
  readiness claims must be honest and bounded to the deterministic slice
- Time / Team:
  produce clear docs without broad code churn

## Workload And Performance Shape

- Expected input size / scale path:
  one repo-wide documentation audit over existing Rust/TS surfaces
- Hot, warm, or cold path:
  cold documentation and planning path
- Dominant operations and expected frequency:
  source inspection, CLI smoke commands, and doc updates
- Latency / throughput / memory sensitivity:
  not hot-path sensitive
- Acceptable simplicity-over-speed tradeoff, if any:
  simple explicit documentation is preferred over dense abstraction
- Likely failure mode at 10x scale:
  roadmap drift if status docs stop matching implementation

## Success Metrics

- one auditable status document exists for pipeline readiness
- each major gap has a code or command anchor
- repo indexes stay in sync after docs additions

## Risks of Inaction

- implementation starts before shared understanding of what is still missing
- duplicated or mis-prioritized work across pipeline stages
- confidence claims drift away from current code reality

## Research Scope

This slice is repo-local and uses primary implementation surfaces:

- `repobrain-cli`, `repobrain-ingest`, `repobrain-graph`, `repobrain-broker`
- existing SDD constraints for trust/readiness and production shape

No external source is required.

## Acceptance Shape

- Primary user-visible outcomes:
  clear docs for implemented vs not-yet-setup pipeline stages
- Invariants that must remain true:
  status claims must remain evidence-backed and date-stamped
- Verification targets:
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- get-brief --goal "why is RepositoryScanner designed this way?" --scope RepositoryScanner --task-type query --token-budget 4096 --repo-root d:\RepoBrainOS`
  `cargo xtask sync`
  `cargo xtask check`
