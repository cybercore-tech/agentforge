# ADR-0021: Read-only operator HUD

Status: Accepted

## Decision

The first operator HUD is a standard-library-only, deterministic snapshot command exposed as
`forge hud <root>`. A focused `agentforge-hud` crate collects validated intake, durable task state,
verified audit records, and verified managed worktree status through existing public boundaries.
It renders bounded plain text and owns no mutation, process execution, terminal control, or second
source of truth.

Missing or corrupt sources fail closed with a source-specific diagnostic. In particular, the HUD
does not create a missing task snapshot or audit log merely to make a report appear healthy.

## Rationale

Operators need one reliable view of project intent and runtime evidence before an interactive TUI
can safely be added. Keeping collection and rendering separate makes output deterministic and
testable while preserving the existing intake, state, audit, and worktree contracts.

## Consequences

The report is intentionally a snapshot: it has no watch loop, ANSI requirement, key handling, or
editing affordance. Future interactive work may build on the same snapshot model but must retain
the existing durable stores as the authority.
