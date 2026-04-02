# Repo Sync Policy

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

The biggest long-term repo killer is silent drift.

That means:

- schema says one thing
- Rust types say another
- TypeScript integrations assume a third
- Python experiments keep an old shape
- indexes and docs stop reflecting the actual repo

If that drift is not machine-detected, people stop trusting the repo.

## One-Line Policy

Every important repo surface must have:

- one canonical source of truth
- explicit derived surfaces
- a repo-native sync check that fails when they drift

## Canonical To Derived Map

### Contracts

Canonical:

- `schemas/`

Derived:

- Rust domain bindings
- TypeScript contract bindings
- Python contract bindings
- examples and generated outputs

### Architecture Documents

Canonical:

- `docs/sdd-*.md`
- `docs/adr-*.md`

Derived:

- `docs/README.md`
- `README.md` architecture anchors
- agent-facing summaries when needed

### Standards And Validation

Canonical:

- files inside `docs/standards/`
- files inside `docs/validation/`
- files inside `research/`

Derived:

- their respective index README files

## Repo-Native Enforcement

Use:

- `cargo xtask sync`

The sync gate should fail when:

- canonical schemas and language bindings drift
- docs indexes stop listing actual files
- validation indexes drift
- standards or template indexes drift

## Design Rules

### 1. One source of truth per concern

Do not allow two handwritten canonical definitions for the same contract.

### 2. Derived surfaces must be disposable

If a file is derived, it should be cheap to regenerate or recheck.

### 3. Sync checks must be low-noise

The gate should catch real drift without forcing constant manual babysitting.

### 4. Drift should fail locally before review

Sync should be part of the normal repo-native workflow, not a reviewer-only surprise.

### 5. Complete sync is a staged program

Today:

- sync checks verify contract surfaces and repo indexes

Next:

- generated bindings
- schema regeneration commands
- stronger artifact lineage

## Decision

RepoBrain will treat synchronization as a first-class engineering concern and enforce it through repo-native automation instead of hoping humans notice drift in review.
