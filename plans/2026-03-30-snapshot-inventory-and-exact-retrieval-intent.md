# Intent: Snapshot Inventory And Exact Retrieval

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain already has strong architecture docs, shared contracts, and serving-policy types, but it does not yet have a real local snapshot baseline that can scan a repository, persist inventory state, and answer even the simplest exact retrieval request from stored snapshot data.

## Why Now

This is the first practical step in the v1 foundation plan. Without a real snapshot inventory and exact lookup path, later retrieval, freshness, and briefing layers have no trustworthy local substrate to build on.

## Goals

- Scan a repository root into a deterministic file inventory.
- Respect the v1 exclusion defaults for obviously unsafe or noisy paths.
- Persist the inventory as a local snapshot artifact.
- Support exact relative-path lookup from the stored snapshot.
- Expose the slice through the Rust CLI in the owning production boundary.

## Non-Goals

- Full symbol extraction
- Lexical or semantic retrieval
- SQLite-backed metadata
- Freshness propagation
- MCP wiring
- Full briefing compilation for edit-facing tasks

## Constraints

- Technical:
  The first slice must stay local-first, deterministic, and dependency-light.
- Product:
  Answers must be snapshot-bound and avoid pretending to know more than the stored snapshot contains.
- Time / Team:
  The slice should be narrow enough to verify in one bounded implementation pass.

## Workload And Performance Shape

- Expected input size / scale path:
  full repository scans over repo-sized file trees, with exact lookups reused after the snapshot exists
- Hot, warm, or cold path:
  scanning is a cold-to-warm maintenance path; exact lookup is a lightweight interactive path
- Latency / throughput / memory sensitivity:
  moderate throughput and memory sensitivity during scans; low-latency exact lookup matters more than scan speed in this slice
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; a full-rescan JSON baseline is acceptable before incremental maintenance exists

## Success Metrics

- A repository scan writes a snapshot artifact under the repo root.
- The artifact can be loaded back without losing inventory fidelity.
- Exact path lookup works against the stored snapshot with normalized path separators.
- Repo-native validation stays green.

## Risks of Inaction

- Later retrieval work would still be designing in the abstract.
- The CLI and MCP surfaces would remain disconnected from real repo state.
- Trust and readiness concepts would stay mostly conceptual instead of executable.

## Research Scope

This slice is already covered by the repo's current architecture and latency/trust SDDs. No additional external research is required before this bounded implementation.

## Acceptance Shape

- Primary user-visible outcomes:
  A user can scan a repo and perform an exact path lookup from stored snapshot data.
- Invariants that must remain true:
  Excluded paths stay out of the inventory, and exact lookups are snapshot-bound.
- Verification targets:
  Ingest unit tests, CLI compile checks, and repo-native `xtask` validation.
