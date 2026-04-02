# Docs, Intent, and Research Pattern

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Core Rule

No important architecture claim should exist without a traceable path back to intent, research, or verification.

## Lifecycle

1. `Intent`
   Clarify the problem, goals, constraints, and success criteria.
2. `Research`
   Gather sources, options, tradeoffs, risks, and recommendation.
3. `Decision`
   Update an SDD or ADR with the actual choice.
4. `Implementation`
   Build it in the correct repo boundary.
5. `Verification`
   Record what was actually checked and what remains uncertain.

Short version:

**No Decision without Research. No Confidence without Verification.**

## Repo Mapping

- high-level design: `docs/`
- research packets: `research/`
- plans and sequencing: `plans/`
- standards and templates: `docs/standards/`, `docs/templates/`
- verification evidence: `docs/validation/`

## When To Write What

Write intent/research when:

- a choice has multiple plausible options
- a claim depends on outside sources
- architecture or market positioning could be questioned later

Write verification when:

- a scaffold was validated
- a benchmark or evaluation was run
- a contract or architecture claim was tested

## Quality Rules

- Prefer primary sources.
- Separate direct evidence from inference.
- Keep architecture docs honest about uncertainty.
- Link related docs whenever a decision depends on earlier reasoning.
