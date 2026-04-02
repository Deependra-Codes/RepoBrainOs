# Intent: Snapshot-Owned Reverse Import Indexes And Hot-Path Perf Baselines

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain still had two remaining gaps after the last lookup refactors:

- reverse import lookup was still owned and rebuilt in `repobrain-graph` instead of the maintained snapshot
- hot graph and broker paths still lacked explicit measured thresholds

## Why Now

These are the last obvious places where the system could still look structurally disciplined while leaving a real ownership or measurement gap in the interactive path.

## Goals

- move reverse import lookup ownership into the snapshot layer
- remove graph-store-local reverse adjacency rebuild
- add a repo-native perf command with explicit p95 thresholds for `blast-radius` and `get-brief`
- keep the measurement path stable and low-chaos

## Non-Goals

- integrating perf thresholds into every default validation command
- measuring every repo command
- adding a heavyweight benchmark dependency

## Constraints

- Technical:
  - preserve backward compatibility for older snapshot artifacts
- Product:
  - no user-visible regression in `blast-radius` or `get-brief`
- Time / Team:
  - prefer a small deterministic perf fixture over broad benchmark infrastructure

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale reverse import neighborhoods and repeated interactive graph / broker requests
- Hot, warm, or cold path:
  hot interactive read path on top of warm maintained snapshot state
- Dominant operations and expected frequency:
  reverse-neighbor expansion for blast radius; full `get-brief` assembly on a warm snapshot
- Latency / throughput / memory sensitivity:
  latency-sensitive interactive path; modest extra snapshot memory is acceptable to remove graph-local rebuild cost
- Acceptable simplicity-over-speed tradeoff, if any:
  keep the perf command implementation simple, but not the underlying lookup shape
- Likely failure mode at 10x scale:
  reverse-neighbor access keeps paying unnecessary setup cost and perf claims stay unmeasured

## Success Metrics

- reverse import lookup becomes snapshot-owned and graph-local rebuild disappears
- `cargo xtask perf` reports and enforces p95 thresholds for `blast-radius` and `get-brief`
- full repo validation remains green

## Risks of Inaction

- graph keeps owning maintained state that belongs in ingest
- latency claims for the hottest current product paths stay inference-only
- regressions are caught only by feel rather than by a repeatable command

## Research Scope

- snapshot-owned reverse lookup structure
- synthetic workload design for stable graph/broker perf measurement

## Acceptance Shape

- Primary user-visible outcomes:
  stable product behavior and a new repo-native perf surface
- Invariants that must remain true:
  legacy snapshots still load; graph and broker outputs remain useful
- Verification targets:
  `cargo test -p repobrain-ingest`
  `cargo test -p repobrain-graph`
  `cargo test -p repobrain-broker`
  `cargo xtask perf`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
