# Intent: L3 Solver Execution And Replay

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

L3 theorem mode existed as obligation compilation only and returned placeholder inconclusive certificates, which blocked proof-grade progress and replayability.

## Why Now

The semantic equivalence ladder needed a concrete L3 execution slice that:

- emits real per-obligation outcomes
- persists replayable artifacts
- stays deterministic and bounded for trust

## Goals

- execute theorem obligations and produce bounded `proved`, `refuted`, or `inconclusive` certificates
- persist replay artifacts that include obligations, encoding hashes, solver metadata, and witnesses
- add a replay command path for deterministic inspection
- strengthen obligation decomposition and deterministic scheduling
- add an optional Alive2 translation-validation sidecar evidence lane

## Non-Goals

- unrestricted full-program theorem proving
- mandatory Alive2 execution for default flows
- replacing L0/L1/L2 baseline stages

## Constraints

- keep L3 claims contract-bounded and explicit
- maintain deterministic ordering for obligations, certificates, and replay artifacts
- keep interactive workflows safe by bounding policy knobs and timeouts

## Workload And Performance Shape

- Expected input size / scale path:
  bounded changed-scope obligations per semantic diff run
- Hot, warm, or cold path:
  warm/cold verification path
- Dominant operations and expected frequency:
  obligation scheduling, bounded per-obligation evaluation, artifact serialization
- Likely failure mode at 10x scale:
  timeout pressure and inconclusive growth without tighter decomposition or parallel execution

## Acceptance Shape

- L3 no longer emits placeholder-only theorem certificates
- replay artifact persistence and replay CLI path exist
- optional Alive2 sidecar emits explicit bounded evidence without over-claiming proof
- full repo quality and policy gates remain green
