# AI Code Quality Guardrails

Date: 2026-03-30
Status: Draft

## Research Question

What quality baseline should RepoBrain use if the repo will be built with heavy AI assistance and we want to prevent fast but structurally weak code?

## Main Conclusion

The right answer is not "more review comments."

It is:

- one repo-native automation surface
- one formatter per language family
- strict lint and type gates on the critical path
- explicit modularity rules for what tools cannot judge reliably

## What The Research And Tooling Guidance Suggest

### 1. AI-generated code needs feedback beyond correctness

Recent work on static-analysis-guided code improvement argues that code can be functionally correct while still materially improved by static analysis feedback.

Implication:

- tests alone are not enough
- lint, type, and static analysis should be first-class gates

### 2. Robustness problems remain real in generated code

Recent robustness work on LLM-generated code emphasizes that generated code can miss important guards and structural safety even when it appears plausible.

Implication:

- quality gates should focus on preventing obvious footguns
- the baseline should reward explicitness and safe defaults

### 3. AI package hallucination is a real supply-chain risk

Recent work on package hallucinations shows that code-generating models can invent package names or weak dependency choices.

Implication:

- new dependencies should not be accepted casually
- dependency additions need human review and written intent

### 4. Use language-native tools where possible

Official tooling guidance points in a clean direction:

- Rust: `cargo fmt` plus `cargo clippy`
- Python: Ruff can unify formatting and linting with low friction
- TypeScript: strict compiler options remain a major correctness lever
- Biome provides one TS/JSON formatter-linter surface without the ESLint plus Prettier split

Implication:

- prefer fewer strong tools over many overlapping ones

## RepoBrain-Specific Design Conclusions

### 1. Keep the enforcement surface repo-native

Humans, CI, and AI agents should all use the same commands:

- `cargo xtask fmt`
- `cargo xtask quality`
- `cargo xtask check`

### 2. Make style automatic, not debatable

If a formatter can decide it, humans should not spend review time on it.

### 3. Make weak Python and TypeScript edges stronger

Rust already gives strong structure.

The weakest long-term drift risk in a mixed repo is usually:

- untyped Python helper growth
- TypeScript convenience shortcuts
- random config and script drift

So the baseline should add:

- `mypy --strict`
- stronger TypeScript compiler flags
- one TS/JSON formatter-linter

### 4. Separate hard gates from design rules

Tooling can reliably enforce:

- formatting
- linting
- type safety
- banned footguns

Tooling cannot reliably enforce:

- whether a file has too many responsibilities
- whether a new abstraction was actually the right abstraction
- whether code placement respects future architecture

So those must remain explicit review rules.

## Final Recommendation

RepoBrain should adopt:

- Rust native formatting and clippy gates
- Python Ruff plus mypy strict
- TypeScript strict mode plus stronger safety flags
- Biome for TS and JSON formatting/linting
- repo-native enforcement through `cargo xtask`
- pre-commit hook installation through repo bootstrap

This is the strongest low-chaos baseline for an AI-heavy repo.

## Sources

- Rust `cargo fmt`: https://doc.rust-lang.org/cargo/commands/cargo-fmt.html
- Clippy docs: https://doc.rust-lang.org/stable/clippy/
- Ruff configuration docs: https://docs.astral.sh/ruff/configuration/
- Ruff formatter docs: https://docs.astral.sh/ruff/formatter/
- TypeScript `strict`: https://www.typescriptlang.org/tsconfig/strict.html
- TypeScript `noUncheckedIndexedAccess`: https://www.typescriptlang.org/tsconfig/noUncheckedIndexedAccess.html
- TypeScript `exactOptionalPropertyTypes`: https://www.typescriptlang.org/tsconfig/exactOptionalPropertyTypes.html
- TypeScript `noImplicitOverride`: https://www.typescriptlang.org/tsconfig/noImplicitOverride.html
- Biome getting started: https://biomejs.dev/guides/getting-started/
- Biome file naming convention rule: https://biomejs.dev/linter/rules/use-filenaming-convention/
- Static Analysis as a Feedback Loop: https://arxiv.org/abs/2508.14419
- Enhancing the Robustness of LLM-Generated Code: https://arxiv.org/abs/2503.20197
- HFuzzer package hallucinations: https://arxiv.org/abs/2509.23835
