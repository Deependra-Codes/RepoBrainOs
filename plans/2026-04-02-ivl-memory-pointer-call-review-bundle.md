# IVL Memory Pointer Call Review Bundle

## Purpose

This file is a downstream review artifact for humans.

It exists so the full L3 IVL feature can be inspected in one place without collapsing the runtime implementation back into one oversized source file.

Canonical source still lives in:

- `src/rust/crates/repobrain-cli/src/semantic_pipeline/ivl.rs`
- `src/rust/crates/repobrain-cli/src/semantic_pipeline/ivl/parse.rs`
- `src/rust/crates/repobrain-cli/src/semantic_pipeline/ivl/smt.rs`
- `src/rust/crates/repobrain-cli/src/semantic_pipeline/ivl/tests.rs`

## What Shipped

The L3 LLVM SSA or IVL lane now handles bounded:

- pointer summaries
- memory summaries
- call summaries
- aggregate extraction
- local stack allocation tracking
- effect-aware equivalence checking

The pipeline is wired by default.

This is not behind an environment toggle.

The semantic-equivalence backlog now exposes a default 5-iteration hardening roadmap across L0, L1, L2, and L3.

## End To End Flow

The implemented flow is:

1. Parse compiler-emitted LLVM IR into a small typed IVL.
2. Symbolically summarize each function into path guards, return values, writes, and call events.
3. Compare reference and target signatures.
4. Encode semantic differences into QF_BV.
5. Ask the configured SMT solver for a counterexample.
6. Return `proved`, `refuted`, or `unavailable` with explicit bounded assumptions.

## Runtime Entry Point

The entry point is `ivl_evaluate_obligation` in `ivl.rs`.

It:

- parses reference and target LLVM IR
- summarizes both functions
- rejects signature mismatches as observable semantic differences
- builds the equivalence SMT script
- runs the solver
- emits theorem-stage assumptions that explicitly mention the active bounds

## Core IVL Data Model

The feature is built around these internal concepts:

- `Sort`
  - `Bool`
  - `BitVec(u16)`
  - `Ptr`
  - `Aggregate(Vec<Sort>)`
  - `Void`
- `LayoutType`
  - bounded layout reasoning for integer, pointer, array, struct, and void layout tokens
- `ValueRef`
  - local SSA value
  - global symbol
  - boolean constant
  - bitvector constant
  - null
- `Rhs`
  - scalar values and binary ops
  - comparisons
  - `select`
  - casts
  - `getelementptr`
  - `load`
  - `call`
  - `extractvalue`
  - `alloca`
- `SymbolicValue`
  - scalar symbolic expression
  - pointer summary
  - aggregate symbolic value
- `PointerValue`
  - `base`
  - `offset_bytes`
  - `local_only`
- `MemoryCell`
  - memory identity keyed by pointer base, byte offset, access sort, and observability
- `PathSummary`
  - path guard
  - optional return value
  - observable writes
  - ordered call events
- `SummaryFeatures`
  - pointer use
  - memory use
  - call use
  - call havoc
  - local allocas

## Supported LLVM Surface

The parser and executor currently support:

- `ret`
- `br`
- `switch`
- `icmp`
- `phi`
- `select`
- integer binary ops
- `zext`
- `sext`
- `trunc`
- `bitcast`
- `load`
- `store`
- `getelementptr`
- `call`
- `invoke`
- `extractvalue`
- `landingpad`
- `alloca`
- `resume`
- `unreachable`

The lane explicitly rejects or remains unavailable for unsupported instructions such as:

- unsupported aggregate constants
- unsupported layout patterns beyond the bounded subset

## Pointer Model

Pointer reasoning is intentionally bounded and structural.

Each pointer is summarized as:

- a base identity string
- a constant byte offset
- a `local_only` flag

Examples of pointer bases:

- `param:arg0`
- `global:@symbol`
- `alloca:%tmp`
- `freshptr:...`
- `null`

The current equality model is:

- same base plus same offset means equal
- different bases are compared through symbolic base vars plus exact offsets
- non-equality pointer predicates other than `eq` and `ne` are currently treated conservatively
- struct and array `getelementptr` offsets are resolved with bounded constant-index layout math and ABI-like field alignment

## Memory Model

Memory is modeled as typed cells:

- key: `(base, offset_bytes, sort, local_only)`
- value: a `SymbolicValue`

Loads:

- return the last known cell value if present
- otherwise synthesize a fresh symbolic value tied to the cell and havoc epoch

Stores:

- write into the typed cell map
- record the written cell for the eventual path summary

Observability:

- `local_only` cells are tracked for intra-procedural reasoning
- only non-local writes are emitted into observable path summaries by default

## Call Model

Calls are bounded and structural, not interprocedural proofs.

For each call, the lane records a structural call key built from:

- callee identity
- return sort
- resolved argument summaries
- memory snapshot for reachable pointer bases
- the participating pointer bases

Call behavior:

- non-void calls produce fresh symbolic return values
- pointer-taking calls havoc memory for the touched pointer bases
- identical summarized call events are treated as observationally stable under the current contract
- `invoke` shares the same bounded call summary, then forks into normal and unwind successors behind a fresh symbolic success guard

This gives effect awareness without pretending to prove arbitrary callees.

## Aggregate And Extractvalue Model

Aggregates are represented as `SymbolicValue::Aggregate`.

This enables:

- aggregate-valued call returns
- `extractvalue` over bounded aggregate paths
- overflow-intrinsic style patterns such as `{ i32, i1 }`
- bounded exception payload tracking through `landingpad` and `resume`

## Control Flow And Path Exploration

Path exploration is deterministic and bounded.

Current caps:

- `IVL_MAX_PATHS = 24`
- `IVL_MAX_BLOCK_REVISITS = 1`

That means:

- acyclic or near-acyclic compiler IR is the intended target
- loops and backedges currently terminate as bounded unavailability rather than pretending to reason through them

Each completed path collects:

- path guard
- return value
- observable writes
- ordered call events

## SMT Encoding

The SMT encoding lives in `ivl/smt.rs`.

It:

- declares scalar and pointer-base variables
- defines per-path guards for both reference and target
- asserts that a counterexample exists when:
  - one side is defined and the other is not
  - or both are on compatible paths but return values differ
  - or writes differ
  - or call event sequences differ

The solver logic is `QF_BV`.

The encoding is intentionally about:

- symbolic return-state
- bounded observable memory-write effects
- bounded call-summary effects

It is not claiming unrestricted semantic equivalence.

## Result Semantics

`Proved` means:

- no bounded counterexample was found in the supported IVL subset

`Refuted` means:

- a bounded semantic-state or effect difference was found
- or the compared signatures differ

`Unavailable` means:

- parsing failed
- summarization exceeded current bounds
- script generation failed
- solver execution or solver interpretation was inconclusive

## Assumptions Emitted To Users

The theorem receipts now explicitly state when the proof depends on:

- compiler-emitted LLVM IR lowering into typed SSA or IVL
- opaque-pointer base plus constant-offset summaries
- typed load or store cell modeling
- bounded structural call summaries
- local `alloca` tracking
- bounded supported-instruction semantics

This keeps the claims accuracy-first and bounded.

## Hardening Roadmap Now Wired In

The default staged backlog now shows:

1. L0 hardening
2. L1 hardening
3. L2 hardening
4. L3 hardening with bounded memory, pointer, and call summaries plus stricter compiler-IR translation validation
5. Cross-stage hardening

This backlog is exposed by default.

It is not hidden behind a toggle.

## Tests Added

The new IVL tests cover:

- branching path summarization
- bitvector equivalence script generation
- aggregate `extractvalue` plus `unreachable`
- `load` or `store` or `getelementptr` or `call` effect summaries
- write-difference detection
- mismatched signatures refuting the obligation

## Validation Run

The feature passed:

- `cargo test -p repobrain-cli`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Current Bounds And Known Gaps

The current lane still intentionally does not claim:

- unrestricted interprocedural proof
- arbitrary loop reasoning
- dynamic GEP index reasoning
- full struct layout offset support
- unrestricted pointer-order reasoning
- unrestricted aggregate constant support
- unrestricted full-program semantic equivalence

That is deliberate.

The lane is now stronger than the old scalar-only slice, but still honest about where it stops.

## File Map

Use this when reviewing the real implementation:

- `ivl.rs`
  - shared IVL data model
  - symbolic execution
  - pointer and memory helpers
  - theorem result shaping
- `ivl/parse.rs`
  - LLVM IR subset parsing
  - type and token helpers
  - call, GEP, load, store, extractvalue, and alloca lowering
- `ivl/smt.rs`
  - signature comparison
  - effect-difference formulas
  - SMT script emission
  - solver execution
- `ivl/tests.rs`
  - regression coverage for the new lane

## Practical Read

If you want to sanity-check the whole feature quickly, read in this order:

1. `ivl.rs` for the runtime data model and executor
2. `ivl/parse.rs` for the accepted LLVM subset
3. `ivl/smt.rs` for what is actually proved or refuted
4. `ivl/tests.rs` for behavior examples

## Bottom Line

The feature result is:

- L3 no longer falls back immediately when supported LLVM IR contains pointer, memory, or call behavior
- the IVL lane now emits bounded semantic-state and effect summaries
- the proof receipts and roadmap reflect that reality explicitly
- the implementation is split for policy and maintainability, but the feature is complete as one coherent slice
