# Source Layout

All product code lives under `src/` and is split by language responsibility:

- `src/rust/` for deterministic systems code
- `src/python/` for research and evaluation
- `src/ts/` for integration surfaces

This keeps the top-level repo clean while still preserving hard boundaries between concerns.
