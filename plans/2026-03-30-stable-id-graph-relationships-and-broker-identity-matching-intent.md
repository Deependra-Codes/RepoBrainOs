# Intent: Stable-ID Graph Relationships And Broker Identity Matching

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

`repobrain-graph` still treats most identity as reconstructed strings:

- symbol nodes are rendered from `name@path`
- symbol evidence matching still depends on path and locator fragments
- blast radius is still primarily a file-neighborhood expansion with import edges

That means the new stable fact IDs in `repobrain-ingest` are not yet compounding through graph and broker.

## Why Now

The ingest layer now emits parser-backed facts with deterministic IDs.

The next high-gain move is to let graph and broker consume those IDs directly so:

- graph identity becomes stable across layers
- typed relationships can start from `defines` and `references`
- broker evidence matching stops depending on fragile string reconstruction

## Goals

- use stable symbol fact IDs as graph-level symbol identity
- add first typed graph relationships for `defines` and `references`
- keep blast radius and briefing surfaces working while shifting internal truth to fact-backed identity
- make broker evidence matching use fact IDs for symbol-definition claims

## Non-Goals

- full semantic reference resolution
- call graph extraction
- query classification changes
- build graph redesign
- schema promotion for graph contracts outside current Rust slice

## Constraints

- Technical:
  - must remain honest about syntax-backed reference quality
  - must not break current `scan`, `blast-radius`, or `get-brief` flows
  - must keep file suggestions stable and readable for broker/CLI consumers
- Product:
  - graph improvements should make trust stronger, not just add more raw structure
  - symbol identities should become more stable without making user-facing output opaque
- Time / Team:
  - this should be a bounded graph/broker evolution, not a full graph rewrite

## Workload And Performance Shape

- Expected input size / scale path:
  - read-heavy snapshot queries over a maintained in-memory snapshot, with one-hop structural graph work on interactive requests
- Hot, warm, or cold path:
  - hot interactive path for `blast-radius` and `get-brief`
- Latency / throughput / memory sensitivity:
  - graph expansion must stay bounded to seed-local relationships; additional structure should reuse maintained snapshot data rather than trigger rescans
- Acceptable simplicity-over-speed tradeoff, if any:
  - linear scans across snapshot vectors are acceptable in this slice only when bounded to one-hop work and when they avoid wider architectural churn

## Success Metrics

- symbol nodes in graph reports are keyed by stable fact IDs
- graph reports include typed `defines` and `references` relationships
- symbol-definition evidence matching in broker uses fact IDs, not path-plus-locator fragments
- `blast-radius` and `get-brief` still produce useful results on the current repo

## Risks of Inaction

- stable fact IDs remain trapped in ingest instead of becoming a cross-layer primitive
- broker trust keeps depending on string reconstruction
- graph remains mostly a path adjacency helper rather than a fact-backed reasoning layer

## Research Scope

- research note: [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)
- parser-backed ingest slice artifacts
- current graph and broker implementation surfaces

## Acceptance Shape

- Primary user-visible outcomes:
  - `blast-radius` and `get-brief` continue to work while graph identity becomes fact-backed
- Invariants that must remain true:
  - graph must not overclaim semantic symbol references
  - broker file suggestions remain file-path based and readable
  - verification planning remains stable
- Verification targets:
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
