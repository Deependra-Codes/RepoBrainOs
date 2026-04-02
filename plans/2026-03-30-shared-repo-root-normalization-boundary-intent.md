# Intent: Shared Repo-Root Normalization Boundary

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

Repo-root normalization currently exists in more than one Rust crate. That duplicates platform-specific boundary knowledge and risks divergence in CLI behavior, snapshot identity, and future path-handling code.

## Why Now

Windows canonicalization already requires a special normalization rule. Leaving that logic duplicated invites subtle drift as more repo-aware crates and commands are added.

## Goals

- move repo-root canonicalization and normalization to one shared Rust boundary
- remove duplicate local implementations from callers
- add a durable guardrail for cross-crate normalization logic

## Non-Goals

- a general filesystem utility module
- relative-path normalization refactors
- schema changes
- new dependencies

## Constraints

- Technical:
  stay `std`-only and preserve current behavior on Windows and non-Windows paths
- Product:
  repo-root identity must stay stable for snapshot ids, CLI output, and artifact lookup
- Time / Team:
  fix the repeated knowledge without inventing a utility framework

## Workload And Performance Shape

- Expected input size / scale path:
  single repo-root canonicalization calls at crate boundaries, reused by CLI and ingest entry points
- Hot, warm, or cold path:
  cold boundary path, not a repo-scale inner loop
- Latency / throughput / memory sensitivity:
  correctness and consistency matter more than micro-performance for this helper
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; a small `std`-only boundary helper is preferred over premature path-utility complexity

## Success Metrics

- only one Rust implementation of repo-root normalization remains
- `repobrain-ingest` and `repobrain-cli` both import the shared API
- tests lock in the Windows verbatim-prefix normalization rule

## Risks of Inaction

- path handling can drift between crates
- snapshot ids and operator-visible repo roots can diverge subtly
- AI-generated convenience helpers can spread without an owning boundary

## Research Scope

Use:

- Rust standard library documentation for canonicalization behavior
- design guidance on duplication and simple design
- RepoBrain boundary and quality doctrine

## Acceptance Shape

- Primary user-visible outcomes:
  CLI and ingest share one repo-root representation rule.
- Invariants that must remain true:
  repo-root normalization stays narrow, deterministic, and tested.
- Verification targets:
  domain tests, workspace checks, and a validation record.
