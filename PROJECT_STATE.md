# AgentForge Project State

## Current release

- Workspace version: `0.0.1`
- Release line: Phase 0 foundation

## Current phase

Phase 0 — reliable local orchestration foundation.

## Active milestone

- `P0-M003` — Plan-first workflow enforcement.
- Active plan: `.plans/P0-M003-plan-first-workflow-enforcement.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `ebe5d12608e233f87645b12e574981e173b145a7`.

## Recently completed milestones

- `P0-M001` — Repository bootstrap.
- `P0-M002` — Governance and agent contract.

## Current capability

AgentForge has local deterministic gates and a documented plan-first workflow, but implementation
authority is not yet mechanically checked and the GitHub repository has no Actions workflow.

## P0-M003 architecture boundary

P0-M003 may enforce repository plan policy through `xtask`, hooks, and GitHub Actions.

It does not add task scheduling, durable task state, worktree orchestration, model adapters, MCP
integration, or deployment automation.

## Bootstrap validation rule

Because P0-M003 creates the first GitHub Actions workflow, the plan-only checkpoint is validated by
the existing local full gate.

Once CI exists, exact-head remote CI is mandatory for the remaining P0-M003 checkpoints.

## Next exact action

Validate this exact Approved plan-only checkpoint with the local full gate.

Do not begin implementation until that checkpoint is locally green.
