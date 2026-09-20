# AgentForge Project State

## Current release

- Workspace version: `0.0.1`
- Release line: Phase 0 foundation

## Current phase

Phase 0 — reliable local orchestration foundation.

## Active milestone

- `P0-M005` — Worktree isolation manager.
- Active plan: `.plans/P0-M005-worktree-isolation-manager.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `93f88b1af2f129c63b95bb0d9f7ca72677c847af`.

## Completed milestones

- `P0-M001` — Repository bootstrap.
- `P0-M002` — Governance and agent contract.
- `P0-M003` — Plan-first workflow enforcement.
- `P0-M004` — Task graph and durable state.

## P0-M004 final integration evidence

- Post-merge executable-mode repair implementation: `f3fa50f31f5cbc368e6a51e09acc7a2e106509da`.
- Post-repair main CI: `35477705487` — success.

P0-M004 is fully closed.

## Current capability

AgentForge has provider-neutral governance, deterministic task IDs, validated dependency graphs,
explicit lifecycle transitions, durable task-state persistence, plan-first enforcement, and green
post-merge CI.

P0-M005 is defining the Git worktree isolation boundary required before external coding agents can
execute tasks.

## P0-M005 architecture boundary

P0-M005 owns deterministic task branches, managed worktree paths, Git worktree inspection,
ownership validation, dirty-state detection, and conservative retirement.

It does not execute coding agents, schedule tasks, merge branches, monitor CI, deploy, or perform
destructive cleanup.

## Next exact action

Commit and validate this Approved P0-M005 plan-only checkpoint.

Do not begin Rust implementation until the exact plan checkpoint is green.

## Known blockers

None.
