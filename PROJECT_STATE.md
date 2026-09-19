# AgentForge Project State

## Current release

- Workspace version: `0.0.1`
- Release line: Phase 0 foundation

## Current phase

Phase 0 — reliable local orchestration foundation.

## Active milestone

- `P0-M004` — Task graph and durable state.
- Active plan: `.plans/P0-M004-task-graph-durable-state.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `ff05435bf3ecaa3aba3b8e0281a2e67c038a34bb`.

## P0-M003 final evidence

- Closure head: `6f483e5a4b82010fe0fc958a1cca92d4a1dd2f3e`.
- Closure CI run: `35472745941` — all four jobs green.
- Merged main: `ff05435bf3ecaa3aba3b8e0281a2e67c038a34bb`.
- Post-merge CI run: `35472774527` — all four jobs green.

## Current capability

AgentForge now has mechanically enforced plan-first development and independent GitHub Actions.

P0-M004 will add the first durable orchestration graph/state while preserving provider and storage
separation.

## P0-M004 architecture boundary

Core owns task-graph semantics.

A new state crate may own project-local persistence.

P0-M004 does not implement scheduling, worktrees, external agents, MCP, CI classification, or
deployment.

## Next exact action

Require exact GitHub Actions green on this Approved plan-only checkpoint.

Do not begin implementation until the plan head is green.
