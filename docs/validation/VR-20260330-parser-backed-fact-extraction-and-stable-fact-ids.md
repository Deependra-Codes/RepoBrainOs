# Verification Record: Parser-Backed Fact Extraction And Stable Fact IDs

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the first research-driven ingest upgrade covering:

- parser-backed syntax extraction for supported Rust, Python, TypeScript, and JavaScript files
- stable deterministic fact IDs for extracted symbols and imports
- honest `syntax_confirmed` evidence classes for syntax-backed facts
- backward-compatible loading of legacy snapshots without fact IDs

## Intent / Spec References

- Intent: [Parser-backed fact extraction and stable fact IDs intent](d:/RepoBrainOS/plans/2026-03-30-parser-backed-fact-extraction-and-stable-fact-ids-intent.md)
- Spec: [Parser-backed fact extraction and stable fact IDs spec](d:/RepoBrainOS/plans/2026-03-30-parser-backed-fact-extraction-and-stable-fact-ids-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-ingest`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- get-brief --goal "understand RepositoryScanner before editing" --scope RepositoryScanner --token-budget 4096 --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-ingest` now attempts parser-backed syntax extraction first for the currently supported source languages and falls back to the existing lexical path only when parser-backed extraction is unavailable.

The maintained snapshot now stores deterministic `fact_id` values on:

- `symbols`
- `imports`

and marks parser-backed facts as `syntax_confirmed`.

Smoke validation on `d:\RepoBrainOS` produced a snapshot with:

- 156 files
- 353 symbols
- 123 imports
- 23 verification targets

The stored snapshot also verified that:

- all 353 symbols carried non-empty `fact_id`
- all 123 imports carried non-empty `fact_id`
- all current extracted symbol facts were `syntax_confirmed`
- all current extracted import facts were `syntax_confirmed`

Running `get-brief` for `RepositoryScanner` still succeeded and now reflected the new ingest adjacency, including the new `src/rust/crates/repobrain-ingest/src/syntax.rs` module in the relevant structural neighborhood.

## Pass / Fail Against Expectations

Pass.

This slice raises the maintained fact floor from lexical-first to syntax-first for supported languages without widening into semantic overclaiming.

## Performance / Complexity Validation

- Measured:
  unit tests passed, CLI smoke commands passed, and the full repo-native validation loop passed
- Inferred:
  one parse plus one tree walk per supported file is a better constant-factor trade than repeated fact-family rescans once syntax extraction is the default maintained path
- Why no benchmark was needed, if applicable:
  this slice changes ingest internals and evidence quality, but it does not yet introduce a new interactive retrieval phase or wider graph search policy

## Residual Risks

- this is still syntax-backed, not semantic or compiler-backed extraction
- stable fact IDs currently cover symbols and imports, not the full future fact model
- parser-backed extraction currently improves supported languages only; unsupported languages still rely on the older fallback path or no structural facts

## Related

- Research: [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)
- SDD / ADR: [SDD-001](d:/RepoBrainOS/docs/sdd-001-repository-cognition-engine.md), [SDD-006](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md), [ADR-002](d:/RepoBrainOS/docs/adr-002-reality-check-corrections.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
