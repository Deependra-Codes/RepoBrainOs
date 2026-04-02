# Research: Architecture Reality Check

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Question

What failure modes would make RepoBrain look architecturally impressive but fail in real use, and how should the design change before we harden the wrong assumptions?

## Primary Sources Reviewed

- Quicktype README and docs
- Typify README
- json-schema-to-typescript README
- datamodel-code-generator README and docs
- tree-sitter docs
- Rust compiler macro expansion guide
- TypeScript compiler API docs
- Watchman docs on settle, subscriptions, clocks, and SCM-aware queries
- Anthropic Claude Code memory and rules docs
- OpenAI harness engineering
- OpenAI practical guide to building agents

## Findings

### 1. Schema drift is real unless code generation becomes a build concern

The strongest common recommendation from schema-codegen tools is simple:

- commit the schema
- generate language bindings from it as part of the build process

Quicktype explicitly recommends generating code from committed schema as part of the build process. Typify explicitly supports generation in `build.rs` or `xtask`. datamodel-code-generator includes a `--check` mode for CI.

Conclusion:

- RepoBrain should not keep handwritten mirrors as the steady-state architecture
- schema compilation should be repo-native and fail fast on drift

### 2. Tree-sitter is foundational, but not enough for semantic truth

Tree-sitter describes itself as an incremental parsing library that builds concrete syntax trees quickly enough for editor use.

That is exactly why it should stay in the stack.

But Rust’s own compiler guide shows macro expansion is a separate compiler process that expands to a complete AST. TypeScript’s compiler API similarly exposes a program and type checker that can retrieve symbols and types beyond raw syntax.

Conclusion:

- tree-sitter is the syntax floor
- semantic truth needs compiler- or language-service-backed enrichers for high-value languages

### 3. Background maintenance needs batching, clocks, and bulk-change modes

Watchman’s docs are unusually relevant here:

- `settle` exists specifically to avoid firing on unstable trees
- clients are expected to track clocks and use them for race-free incremental change views
- subscriptions can be deferred during version-control operations
- SCM-aware queries exist because large repos sometimes need minimized change sets instead of raw file churn

Conclusion:

- RepoBrain cannot schedule maintenance file-by-file without strong coalescing
- it needs a maintenance budget controller, not just a file watcher

### 4. Coverage audit should be about observable completeness, not hidden truth

The risky mistake is asking retrieval to prove that nothing important is missing in an absolute sense.

That is not realistic on an interactive path.

The safer framing is:

- define observable evidence slots per task class
- measure whether those slots are satisfied, missing-but-retrievable, not-applicable, not-observable, or stale
- trigger one bounded second pass only for missing-but-retrievable required slots

Conclusion:

- coverage audit stays deterministic
- it becomes a slot completeness check, not a pseudo-semantic oracle

### 5. The discipline model must be strict internally and lightweight externally

Anthropic’s guidance is that project instructions work best when they are specific, concise, and well-structured. OpenAI’s recent harness-engineering write-up says the leverage is in scaffolding, feedback loops, and encoded invariants rather than expecting the model to “try harder.”

That means the correct UX is not:

- force the user through a visible eight-step ritual on every task

The correct UX is:

- let the harness absorb the sequence
- keep durable artifacts for architecture-visible work
- keep trivial work lightweight

Conclusion:

- execution policy should remain strong
- enforcement should be progressive by task class

## Additional Risks Noted

### 1. Semantic adapters can themselves become a new portability bottleneck

Correction:

- treat them as enrichers behind a stable interface
- cache by snapshot and toolchain version
- never make the whole product unusable without them

### 2. Generated bindings can become unpleasant to use directly

Correction:

- keep wrappers thin
- keep generated code isolated
- avoid writing business logic in generated files

### 3. Maintenance throttling increases the chance of serving stale information

Correction:

- expose freshness explicitly
- allow cheap local overlays for open files
- show when the system is in coarse refresh mode

## Recommended Corrections

1. Add a contract-compilation SDD and make generated bindings the target architecture.
2. Change extraction language from “deterministic AST extraction” to “syntax floor plus semantic enrichers.”
3. Add a maintenance budget controller to the latency SDD.
4. Redefine coverage audit in observable slot terms.
5. Clarify that the execution loop is a harness model, not a mandatory user-visible halt sequence.

## Sources

- Quicktype: https://github.com/glideapps/quicktype
- Typify: https://github.com/oxidecomputer/typify
- json-schema-to-typescript: https://github.com/bcherny/json-schema-to-typescript
- datamodel-code-generator: https://github.com/koxudaxi/datamodel-code-generator
- tree-sitter: https://github.com/tree-sitter/tree-sitter
- Rust macro expansion guide: https://rustc-dev-guide.rust-lang.org/macro-expansion.html
- TypeScript compiler API: https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API
- Watchman configuration: https://facebook.github.io/watchman/docs/config
- Watchman subscriptions: https://facebook.github.io/watchman/docs/cmd/subscribe
- Watchman clockspec: https://facebook.github.io/watchman/docs/clockspec
- Watchman file queries: https://facebook.github.io/watchman/docs/file-query
- Watchman SCM-aware queries: https://facebook.github.io/watchman/docs/scm-query
- Claude Code memory and rules: https://code.claude.com/docs/en/memory
- OpenAI harness engineering: https://openai.com/index/harness-engineering
- OpenAI practical guide to building agents: https://openai.com/business/guides-and-resources/a-practical-guide-to-building-ai-agents/
