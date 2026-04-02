# Shared Repo-Root Normalization Boundary

Date: 2026-03-30
Status: Draft

## Research Question

Where should RepoBrain own repo-root canonicalization and Windows-specific normalization so crates do not duplicate platform boundary logic and drift over time?

## Main Conclusion

RepoBrain should treat repo-root canonicalization as one shared boundary API in the lowest Rust crate that both callers already depend on.

That means:

- keep the behavior in `repobrain-domain`
- give it one repo-specific API instead of a generic helper bucket
- prove the Windows `\\?\` stripping behavior with tests
- require higher-layer crates to import that API instead of re-implementing it

## What The Sources Suggest

### 1. Rust canonicalization on Windows is not a neutral detail

The Rust standard library documents that `std::fs::canonicalize` returns extended-length paths on Windows and warns that these paths can be awkward or incompatible when reused outside that API boundary.

Implication:

- RepoBrain does need a normalization step for repo roots on Windows
- that rule is platform-specific knowledge and should not be re-copied in each caller

### 2. Duplication should be removed at the knowledge boundary

Kent Beck's design rules, as summarized by Martin Fowler, keep "no duplication" as a first-class design force and explicitly warn about duplicated logic.

Implication:

- the duplicated thing here is not just a few lines of Rust
- it is the knowledge of how RepoBrain defines a canonical repo root

### 3. RepoBrain's own boundary doctrine points to the lowest shared crate

`SDD-003` defines `repobrain-domain` as the lowest Rust layer, while the engineering standards say to prefer boundary-first growth and to reject AI-generated code that duplicates nearby logic instead of extracting the owning boundary.

Implication:

- the fix should not live only in `repobrain-cli`
- the fix should not create a vague new utility crate
- the right move is a narrow repo-root boundary API in `repobrain-domain`

## Design Conclusions For RepoBrain

### 1. Use one explicit repo-root boundary API

Add:

- `canonicalize_repo_root(path: &Path) -> io::Result<PathBuf>`
- `normalize_repo_root(path: &Path) -> PathBuf`

and keep both names scoped to repo-root behavior, not general path manipulation.

### 2. Centralize the platform rule, not every path helper

This change should not promote `repobrain-domain` into a grab bag of unrelated filesystem helpers.

The stable shared knowledge is:

- how RepoBrain canonicalizes repo roots
- how RepoBrain removes Windows verbatim prefixes from repo-root identity and display

### 3. Guard the boundary with tests and doctrine

The durable safeguards should be:

- unit tests in the owning crate
- higher-layer callers importing the shared API
- an engineering-quality rule that cross-crate normalization logic must have one owner

## Recommendation

Implement a narrow shared repo-root normalization boundary in `repobrain-domain`, remove the duplicate local helpers, and encode the rule in the engineering baseline rather than answering this with a general-purpose utility layer.

## Sources

- Rust `std::fs::canonicalize`: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
- Martin Fowler, Beck design rules: https://martinfowler.com/bliki/BeckDesignRules.html
- Repo layout and boundaries: [SDD-003](d:/RepoBrainOS/docs/sdd-003-repo-layout-and-boundaries.md)
- Engineering quality baseline: [Engineering Quality Baseline](d:/RepoBrainOS/docs/standards/ENGINEERING_QUALITY_BASELINE.md)
