# Intent: Symbol Inventory And Exact Symbol Lookup

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain can now persist a file inventory snapshot, but it still cannot answer even the most basic symbol-oriented question from stored snapshot data.

## Why Now

Exact symbol lookup is the next smallest deterministic capability after exact path lookup. It gives the retrieval stack a real symbol anchor without requiring semantic adapters, lexical ranking, or graph traversal yet.

## Goals

- Extract a deterministic, syntax-level symbol inventory during repository scanning.
- Keep the symbol model honest about capability by treating it as lexical evidence, not semantic resolution.
- Persist symbols in the existing local snapshot artifact.
- Support exact symbol-name lookup from stored snapshot data.
- Expose the lookup through the Rust CLI.

## Non-Goals

- Semantic symbol resolution
- Cross-file call graph extraction
- Build/test graph inference
- Query rewriting
- Fuzzy or ranked symbol retrieval

## Constraints

- Technical:
  Keep the first symbol extractor dependency-light and deterministic.
- Product:
  Do not imply semantic certainty from syntax-level symbol extraction.
- Time / Team:
  Reuse the existing snapshot path and keep the slice narrow enough for one bounded pass.

## Workload And Performance Shape

- Expected input size / scale path:
  symbol extraction over repo-scale source files with repeated exact symbol lookups from maintained snapshots
- Hot, warm, or cold path:
  extraction is a cold-to-warm maintenance path; exact symbol lookup is an interactive read path
- Latency / throughput / memory sensitivity:
  linear extraction cost is acceptable during scan maintenance; lookup latency matters more than extraction speed for interactive use
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; lexical line-based extraction is preferred over heavier parsing in this first bounded slice

## Success Metrics

- A scan produces a persisted symbol inventory alongside the file inventory.
- Exact symbol lookup returns all snapshot hits for a symbol name.
- Symbol extraction stays stable across Rust, Python, and TypeScript/JavaScript basics.

## Risks of Inaction

- The next retrieval layers would still lack symbol anchors.
- The sidecar would remain path-driven only, which is too weak for real coding questions.
- The repo would have serving-policy abstractions for exact retrieval without symbol-level substance.

## Research Scope

This slice follows the repo's current architecture:

- syntax-level extraction from [SDD-006](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)
- latency-safe maintained snapshots from [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)

No additional external research is required before this bounded implementation.

## Acceptance Shape

- Primary user-visible outcomes:
  A user can scan a repo and perform exact symbol-name lookup from stored snapshot data.
- Invariants that must remain true:
  Symbol lookup remains snapshot-bound and exact-match only.
- Verification targets:
  Ingest unit tests, CLI smoke checks, and repo-native validation.
