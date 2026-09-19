# ADR-0004: Plan-first implementation workflow

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Autonomous implementation without a frozen scope makes agent changes difficult to review and easy
to expand accidentally.

## Decision

Substantial implementation requires an Approved plan committed before implementation. Plan approval
and implementation remain separate commits.

## Consequences

Scope, expected files, tests, failure modes, and acceptance criteria exist before code changes.
