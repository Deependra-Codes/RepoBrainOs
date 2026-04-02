# SDD-006: Contract Compilation And Semantic Extraction

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

RepoBrain will treat JSON Schema as the canonical contract layer, generate language-native bindings from it, and use a tiered extraction stack where tree-sitter provides a universal syntax floor and language-specific semantic adapters provide higher-trust semantic facts.

## Why This SDD Exists

Two architecture mistakes can quietly poison the whole system:

- schema drift across Rust, Python, and TypeScript
- overtrusting syntax parsers for semantic truth

If we leave language bindings hand-maintained, the polyglot monorepo will drift.

If we treat tree-sitter as if it understands macro expansion, type resolution, or decorator behavior, RepoBrain will overclaim certainty in exactly the repos where developers most need help.

This SDD turns both risks into explicit architecture.

## Part A: Contract Compilation

## Problem

Manual mirrors of `ContextRequest`, `BriefingPack`, and related contracts will drift as soon as the schemas evolve faster than the language bindings.

That drift is especially dangerous because it can pass silently:

- Rust compiles against one shape
- TypeScript compiles against another
- Python experiments assume a third

## Decision

The contract flow is:

1. edit canonical schema in `schemas/`
2. generate language bindings
3. compile and validate against generated bindings
4. allow only thin handwritten wrapper modules above the generated layer

Handwritten language-native models are a bootstrap convenience only, not the long-term architecture.

## Generated Binding Strategy

### Rust

Generate idiomatic Rust types from JSON Schema using `typify`.

Recommended generation path:

- `xtask` or `build.rs` driven generation
- output into a dedicated generated module
- handwritten wrapper types only when ergonomics or domain-specific helper behavior is needed

Why:

- `typify` is purpose-built for JSON Schema to Rust compilation
- it supports generation through a macro, builder interface, `build.rs`, or persistent files

### TypeScript

Generate TypeScript declarations from JSON Schema using `json-schema-to-typescript`.

Recommended generation path:

- generated `.ts` or `.d.ts` files in a dedicated generated folder
- wrapper modules may re-export narrower stable surfaces

Why:

- it is focused on compiling JSON Schema into TypeScript declarations
- the generated surface is reviewable and cheap to diff

### Python

Generate Python contract bindings from JSON Schema using `datamodel-code-generator`.

Recommended generation path:

- generate dataclasses or `TypedDict`-style models for lightweight local/runtime use
- keep handwritten convenience adapters separate from the generated source

Why:

- it directly supports JSON Schema input
- it supports multiple Python output styles and a `--check` mode for CI drift detection

## Generated Code Boundaries

Generated code should live in clearly named generated modules.

Rules:

- do not hand-edit generated files
- do not hide business logic in generated modules
- do not let handwritten convenience models redefine the same fields independently
- keep wrappers thin and reversible

## Drift Prevention

RepoBrain should add two repo-native commands:

- `cargo xtask schema`
- `cargo xtask schema-check`

Expected behavior:

- `schema` regenerates bindings for Rust, Python, and TypeScript
- `schema-check` fails if generated outputs are stale relative to `schemas/`

This makes schema drift a build failure instead of a latent architecture bug.

## Versioning

Contracts must support evolution without chaos.

Rules:

- every canonical schema should have a stable `$id`
- breaking contract changes should be visible in diff and release notes
- snapshot artifacts should record the schema version used to generate their bindings

## Part B: Semantic Extraction

## Problem

Tree-sitter is excellent for incremental parsing, but it produces syntax trees, not full semantic truth.

That means it cannot, by itself, guarantee:

- macro-expanded Rust structure
- template or type-instantiated C++ behavior
- true symbol or type resolution in TypeScript
- full decorator or runtime import meaning in Python

## Decision

RepoBrain will use a layered extraction model:

### Tier 0: File And Lexical Facts

Examples:

- file existence
- path patterns
- string and comment anchors
- manifest/build file presence

### Tier 1: Syntax Facts

Produced by tree-sitter or equivalent incremental parsers.

Examples:

- declarations
- imports
- function bodies
- syntactic call sites
- structural nesting

These facts are fast, broad, and universal.

### Tier 2: Semantic Facts

Produced by language-specific semantic adapters.

Examples:

- resolved symbols
- expanded macro-aware references
- type-aware call targets
- semantic module relationships

High-value adapters should start with:

- Rust semantic extraction via compiler-aware or rust-analyzer-backed expansion and resolution
- TypeScript semantic extraction via the TypeScript compiler API or language service

Python should remain syntax-first in v1 unless and until a semantic adapter proves worth the complexity.

### Tier 3: Build And Compiler Facts

Examples:

- build target membership
- test target membership
- compile graph relationships
- generated-file provenance

These come from build tools and compiler frontends rather than source parsers.

## Evidence Classes

Every extracted fact should record the capability that produced it.

Suggested classes:

- `lexical_confirmed`
- `syntax_confirmed`
- `semantic_confirmed`
- `compiler_confirmed`
- `inferred`
- `unknown`

This is what prevents syntax-only evidence from masquerading as semantic truth.

## Claim Discipline

RepoBrain must not make claims stronger than the extractor capability allows.

Examples:

- tree-sitter can support “this file contains a call expression that names `foo`”
- it should not alone support “this call definitely resolves to symbol X across macro expansion”

That stronger claim needs semantic or compiler-backed evidence.

## Latency And Extraction

Semantic adapters are not free.

Therefore:

- tree-sitter remains the universal baseline
- semantic enrichment runs in the background or on deep requests
- interactive serving prefers cached semantic facts, not on-demand whole-project semantic analysis

## Failure Modes And Mitigations

### 1. Generated bindings are ugly or awkward

Mitigation:

- keep them behind wrapper modules
- optimize wrapper ergonomics, not contract ownership

### 2. Semantic adapters create setup friction

Mitigation:

- keep syntax extraction as the minimum viable baseline
- enable semantic enrichers per language and per repo profile
- cache semantic outputs by snapshot and toolchain version

### 3. Compiler- or language-service-backed facts drift with toolchain versions

Mitigation:

- store toolchain identity with extracted artifacts
- invalidate semantic facts when toolchain or config changes

## Decision

RepoBrain will be schema-compiled across languages and syntax-plus-semantic in extraction, so the polyglot repo stays coherent and the knowledge graph stays honest about what it truly knows.
