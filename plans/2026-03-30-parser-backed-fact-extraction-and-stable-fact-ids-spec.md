# Spec: Parser-Backed Fact Extraction And Stable Fact IDs

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Parser-backed fact extraction and stable fact IDs intent](d:/RepoBrainOS/plans/2026-03-30-parser-backed-fact-extraction-and-stable-fact-ids-intent.md)

## Problem Statement

RepoBrain needs a stronger syntax-fact floor before graph and broker upgrades can compound safely.

The current lexical extractor is useful but too brittle to remain the default maintained layer for supported languages.

## Scope

- add parser-backed syntax extraction for symbols and imports in `repobrain-ingest`
- add deterministic fact IDs for extracted symbols and imports
- add `syntax_confirmed` evidence classes for syntax-backed facts
- preserve lexical fallback for unsupported or failed parses
- preserve old snapshot deserialization by backfilling missing fact IDs during normalization

## Non-Goals

- semantic adapters
- call graph extraction
- build graph extraction
- query planning changes
- storage engine migration

## Behavioral Requirements

1. Supported Rust, Python, TypeScript, and JavaScript files should attempt syntax-backed extraction first.
2. A single syntax parse pass per file should produce both symbol and import facts.
3. Parse-backed facts must be labeled `syntax_confirmed`; fallback facts must remain `lexical_confirmed`.
4. Extracted symbols and imports must carry deterministic stable IDs derived from source-stable fields.
5. Legacy snapshots missing new fact ID fields must normalize into valid current snapshots.

## Acceptance Examples

1. A Rust file with declarations inside string literals should not emit false symbol facts from those strings.
2. A TypeScript import statement should produce an import fact with a stable ID and `syntax_confirmed` evidence.
3. A legacy snapshot artifact without fact IDs should still load and expose backfilled deterministic IDs.

## Invariants

- Must always hold:
  - syntax-backed extraction must not claim semantic certainty
  - snapshot normalization must deterministically backfill empty fact IDs
  - exact symbol lookup remains name-based and snapshot-sorted
- Must not regress:
  - current snapshot write/load behavior
  - direct import lookup by importer path
  - current CLI scan and get-brief flows

## Contract And Type Changes

- Schema changes:
  - none in canonical JSON schemas yet; this remains an internal Rust-snapshot evolution
- Public interface changes:
  - add stable ID fields to extracted symbol and import records
  - add `syntax_confirmed` evidence variants
- Illegal states to remove:
  - empty fact IDs after snapshot normalization

## Workload And Complexity Notes

- Workload shape and expected scale:
  - sequential file scanning over repo-local source inventories, then repeated read-heavy lookup from the stored snapshot
- Hot, warm, or cold path:
  - warm at scan time, downstream-hot through blast radius and briefing reuse
- Chosen data structures and why:
  - keep vectors for stored facts because snapshot sorting and binary partitioning already match the read-heavy workload
  - add one syntax parse pass per file instead of separate parse passes per fact family to keep constant factors bounded
- Key operation costs:
  - scan remains roughly linear in scanned file count and file size
  - syntax extraction becomes one parse plus one tree walk per supported source file
  - snapshot normalization remains sort-plus-dedup over vectors
- Memory / allocation notes:
  - fact IDs add small per-fact string cost; acceptable because they remove future string recomputation and enable stable references
- Measured vs inferred performance claims:
  - inferred that a single parse pass per file is a better constant-factor trade than repeated lexical rescans once parser-backed extraction becomes the default

## Test Strategy

- Unit:
  - parser-backed extraction for Rust, Python, and TypeScript / JavaScript shapes
  - deterministic fact ID generation and legacy backfill
- Integration:
  - repository scan snapshot roundtrip
- Regression:
  - string/comment false positives do not reappear for supported syntax-backed languages
- Property / invariant:
  - normalization backfills empty IDs deterministically

## Verification Notes

- Commands:
  - `cargo test -p repobrain-ingest`
  - `cargo run -p repobrain-cli -- scan --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated snapshot artifact under `.repobrain/snapshots/`
  - future validation record after implementation
