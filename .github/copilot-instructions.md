# Copilot Instructions

## RepoBrain Defaults

- Read `AGENTS.md` first for the full repo contract.
- Prefer research, design, and bounded reasoning before broad implementation.
- Keep changes small, modular, and easy to verify.
- Treat `schemas/` and `docs/sdd-*.md` as the architecture source of truth.

## Before Writing Code

For non-trivial tasks:

1. inspect the existing code and docs
2. research primary sources when the task touches architecture, algorithms, performance, or external claims
3. update intent, research, or spec artifacts when the change is significant
4. decide the owning boundary before adding code

## Code Shape

- prefer the smallest correct change
- prefer stronger types over wider branching logic
- prefer explicit data structure choices over defaulting to lists and maps everywhere
- treat time, space, and data-structure quality as release criteria for scale-sensitive work
- avoid avoidable quadratic behavior on repo-scale paths
- if a path is hot or repo-scale, state dominant operations, key costs, and memory tradeoffs
- do not put production logic into scripts or Markdown
- do not add dependencies without a reason tied to correctness, leverage, maintainability, or performance

## Verification

Before finalizing non-trivial work, run:

- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask quality`
- `cargo xtask check`

Final responses should mention:

- what changed
- what was verified
- any residual risks
