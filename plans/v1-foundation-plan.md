# V1 Foundation Plan

Status: Draft
Date: 2026-03-30

## Goal

Build the first end-to-end vertical slice of RepoBrain OS as a fast repo sidecar for coding agents, without overcommitting to heavyweight infrastructure.

## Phase 1. Deterministic Local Baseline

Deliver:

- repo scan
- file inventory
- symbol extraction
- dependency extraction
- SQLite-backed metadata store
- JSON/Parquet artifacts
- snapshot-bound serving baseline

Exit criteria:

- can scan a repo locally
- can persist a snapshot
- can answer a basic structural question
- does not require heavyweight services for the first useful answer

## Phase 2. Evidence And Briefing Core

Deliver:

- evidence receipts
- fact/claim/note records
- first context compiler
- readiness-aware response envelope
- first `get_brief` flow

Exit criteria:

- can return a briefing pack from stored snapshot data
- can include evidence references, readiness state, and scaffolding level
- can answer a real coding-agent question without dumping the repo

## Phase 3. Scoped Impact And Freshness

Deliver:

- direct change detection
- bounded impact propagation
- impact summary output
- freshness labels

Exit criteria:

- can mark stale regions after a change
- can emit an accuracy-labeled impact summary
- can stay usable during ordinary local edit churn

## Phase 4. MCP And Agent Surface

Deliver:

- MCP tool surface
- `get_brief`
- `explain_flow`
- `blast_radius`
- `list_invariants`
- verification planner outputs for edit-facing queries

Exit criteria:

- an external coding agent can query RepoBrain interactively
- the tool feels like a sidecar in the coding loop, not a separate product to babysit
- the agent receives concrete verification targets before risky edits

## Phase 5. Eval Harness

Deliver:

- baseline prompt comparisons
- uplift delta measurement
- false-fresh rate tracking
- unnecessary rebuild rate tracking

Exit criteria:

- we can prove whether RepoBrain is actually helping
- we can decide whether the sidecar shape is good enough for wider productization
