# Spec: Symbol Inventory And Exact Symbol Lookup

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Symbol inventory and exact symbol lookup intent](d:/RepoBrainOS/plans/2026-03-30-symbol-inventory-and-exact-symbol-lookup-intent.md)

## Problem Statement

RepoBrain needs a first symbol-aware retrieval slice that works from maintained snapshot data and does not overclaim semantic understanding.

## Scope

- Syntax-level symbol extraction during ingest
- Symbol persistence in the existing snapshot artifact
- Exact symbol-name lookup over stored snapshot data
- CLI command for exact symbol lookup
- Focused tests for extraction and lookup behavior

## Non-Goals

- Semantic adapters
- Symbol ranking or fuzzy search
- Reference resolution
- Call graph generation
- Schema changes

## Behavioral Requirements

1. Repository scans must extract a deterministic symbol inventory for supported languages.
2. Extracted symbols must record their path, name, kind, line number, language, and extraction capability.
3. The first extractor must label symbols as lexical evidence only.
4. Exact symbol lookup must be case-sensitive and return all snapshot matches for the requested symbol name.
5. Snapshot normalization must keep symbols sorted by name and location so exact lookup remains bounded and predictable.

## Acceptance Examples

1. Scanning a repo with `struct RepositoryScanner` in Rust and `class RepositoryScanner` in Python returns two exact hits for `RepositoryScanner`.
2. Looking up `scan` after snapshot load returns the stored Rust `scan` function hit.
3. Looking up an unknown symbol returns no match without widening or guessing.

## Invariants

- Must always hold:
  Symbol extraction capability is exposed as lexical-only in this slice.
- Must not regress:
  The feature must not move semantic resolution or heavy parsing onto the interactive path.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  New Rust ingest symbol types and a new CLI command.
- Illegal states to remove:
  Symbol snapshots should not remain unsorted after scan/load normalization.

## Workload And Complexity Notes

- Workload shape and expected scale:
  linear symbol extraction over stored source contents plus repeated exact-name lookups over snapshot-held symbols
- Hot, warm, or cold path:
  extraction is cold/warm maintenance; exact symbol lookup is interactive and read-heavy
- Chosen data structures and why:
  sorted `Vec<IndexedSymbol>` keeps snapshot storage compact and enables contiguous exact-name ranges through partition-based lookup
- Key operation costs:
  extraction is `O(total_lines)` for supported source files; exact symbol lookup is `O(log n + k)` where `k` is the number of exact matches
- Memory / allocation notes:
  symbol metadata is stored as snapshot-owned records in contiguous vectors; no heavyweight parser state is retained after scan
- Measured vs inferred performance claims:
  complexity claims are inferred from the sorted-vector design; verification measured correctness through tests and smoke commands, not dedicated benchmarks

## Test Strategy

- Unit:
  Rust/Python/TypeScript symbol extraction basics, duplicate-name lookup, and snapshot round-trip.
- Integration:
  CLI smoke checks for `scan` and `lookup-symbol`.
- Regression:
  Symbol lookup remains exact and snapshot-bound.
- Property / invariant:
  Symbol normalization groups exact-name hits contiguously.

## Verification Notes

- Commands:
  `cargo test -p repobrain-ingest`, `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`, `cargo run -p repobrain-cli -- lookup-symbol RepositoryScanner --repo-root d:\RepoBrainOS`, `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  Updated repo-local snapshot JSON and a validation record
