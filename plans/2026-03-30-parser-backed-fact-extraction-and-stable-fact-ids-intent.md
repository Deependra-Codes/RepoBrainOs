# Intent: Parser-Backed Fact Extraction And Stable Fact IDs

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

`repobrain-ingest` currently extracts symbols and imports through lexical line scanning.

That was the right bootstrap move, but it is now the main ceiling on extraction quality:

- symbols and imports can still be missed or misclassified in syntax-heavy files
- evidence cannot honestly rise above `lexical_confirmed`
- graph and broker layers still rebuild identities from path/name strings instead of stable extracted fact IDs

## Why Now

The repo now has real snapshot inventory, structural adjacency, verification planning, `get_brief`, evidence receipts, and flow capsules.

That means extraction quality has become the next highest-leverage layer. Improving ingest now compounds every downstream crate without forcing early semantic-adapter complexity.

## Goals

- move supported language extraction from lexical-first to syntax-first
- keep lexical fallback when parsing is unavailable or fails
- add stable fact IDs for extracted symbols and imports
- introduce `syntax_confirmed` evidence without overclaiming semantic truth
- preserve deterministic local snapshot artifacts and current CLI flows

## Non-Goals

- full semantic resolution
- compiler-backed extraction
- SCIP import in this slice
- graph redesign or broker query planning in this slice
- incremental delta ingest storage in this slice

## Constraints

- Technical:
  - must stay local-first and snapshot-bound
  - must not regress current exact symbol, import, and blast-radius flows
  - must preserve backward compatibility for previously written snapshot artifacts
- Product:
  - extraction claims must stay honest about syntax vs semantics
  - unsupported languages must continue to fail soft rather than disappear
- Time / Team:
  - first slice should be implementable as a bounded refactor, not a platform rewrite

## Workload And Performance Shape

- Expected input size / scale path:
  - repo-scale scans over hundreds to low-thousands of source files, with repeated interactive reuse of the stored snapshot
- Hot, warm, or cold path:
  - warm during scan, but its outputs feed hot interactive paths like `blast-radius` and `get-brief`
- Latency / throughput / memory sensitivity:
  - parse cost must remain bounded per file and avoid redundant work; a single syntax pass should produce both symbols and imports
- Acceptable simplicity-over-speed tradeoff, if any:
  - simpler fallback logic is acceptable for parse failures and unsupported languages, but repeated reparsing per fact family is not

## Success Metrics

- supported Rust, Python, TypeScript, and JavaScript files extract symbols/imports through parser-backed logic by default
- extracted syntax facts carry stable deterministic IDs
- new evidence class distinguishes syntax-confirmed from lexical-confirmed facts
- legacy snapshots continue to deserialize and normalize correctly

## Risks of Inaction

- RepoBrain keeps building higher-trust features on top of lower-trust extraction
- syntax-only evidence remains mislabeled as lexical-only, limiting downstream usefulness
- graph and broker surfaces keep depending on brittle string identity reconstruction

## Research Scope

- Tree-sitter docs
- SDD-006 extraction tiers and evidence classes
- research note: [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)

## Acceptance Shape

- Primary user-visible outcomes:
  - `scan` still succeeds, but the stored snapshot now contains parser-backed syntax facts with stable IDs
- Invariants that must remain true:
  - snapshot artifacts remain deterministic and serializable
  - parse-backed facts must not be labeled as semantic truth
  - exact symbol and import lookup semantics remain stable
- Verification targets:
  - `cargo test -p repobrain-ingest`
  - `cargo run -p repobrain-cli -- scan --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
