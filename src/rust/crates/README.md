# Rust Crates

These crates form the deterministic production core.

Current crates:

- `repobrain-domain`
- `repobrain-ingest`
- `repobrain-graph`
- `repobrain-compiler`
- `repobrain-serving`
- `repobrain-cli`
- `repobrain-xtask`

Dependency shape:

- `repobrain-domain` sits at the bottom
- other crates build on top of domain contracts
- `repobrain-cli` is the local operator surface
- `repobrain-xtask` is the repo workflow surface
