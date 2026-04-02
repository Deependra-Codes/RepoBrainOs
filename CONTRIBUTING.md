# Contributing

RepoBrain OS is being built as an accuracy-first, contract-first system. Contributions should strengthen clarity, evidence, and maintainability rather than just add files.

## Contribution Flow

1. Clarify intent for non-trivial work.
2. Add or update research if the change depends on tradeoffs, external evidence, or market/technical claims.
3. Write or update a spec for non-trivial behavior, contract, or workflow changes.
4. Record important architecture corrections in an ADR or relevant SDD update.
5. Start from contracts, types, and tests before broad implementation when the change touches behavior.
6. Implement within the correct repo boundaries.
7. Add or update verification evidence when behavior, contracts, or architecture meaningfully change.

Use [AGENTS.md](d:/RepoBrainOS/AGENTS.md) as the default working contract for AI-assisted work.

## Local Validation Gates

Rust:

```powershell
cargo xtask doctor
cargo xtask fmt
cargo xtask policy
cargo xtask sync
cargo xtask quality
cargo xtask check
```

## Rules

- Do not bypass quality gates for AI-generated code.
- Do not hide canonical contracts in prompts.
- Do not add heavyweight local dependencies without updating `SDD-002`.
- Do not introduce architecture claims without evidence or uncertainty labels.
- Do not move production logic into scripts or research packages.
- Do not merge new files that mix unrelated responsibilities just because an AI produced them.
- Do not disable lint/type gates without a written reason in docs or an ADR.
- Do not implement non-trivial behavior without a written intent and spec.
- Do not skip research before making algorithm, data structure, or performance claims.
- Do not skip failing tests or executable examples on behavior-changing work unless the limitation is documented.
- Do not weaken types when a narrower contract would remove ambiguity.
- Do not let schemas, bindings, examples, or repo indexes drift without updating the sync surface.

## When Docs Must Change

Update docs when you change:

- architecture boundaries
- canonical schemas
- briefing/output contracts
- validation expectations
- local-first vs scale-up assumptions

## Preferred Artifacts

- standards: `docs/standards/`
- templates: `docs/templates/`
- validation records: `docs/validation/`
- plans: `plans/`

## Quality Workflow

Use this baseline for all non-trivial changes:

1. write or update intent, research, and spec artifacts as needed
2. shape contracts and types first
3. add or update failing tests, examples, or verification targets
4. run `cargo xtask fmt`
5. run `cargo xtask policy`
6. run `cargo xtask sync`
7. run `cargo xtask quality`
8. run `cargo xtask check` before finalizing behavior-changing work

The repo bootstrap also configures a pre-commit hook that runs `cargo xtask quality`.
