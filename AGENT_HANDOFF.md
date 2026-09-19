# AgentForge Agent Handoff

## Repository state

- Active milestone: `P0-M004`.
- Active plan: `.plans/P0-M004-task-graph-durable-state.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `ff05435bf3ecaa3aba3b8e0281a2e67c038a34bb`.

## Previous milestone evidence

P0-M003 is fully complete:

- closure CI `35472745941` — all four jobs green;
- merge commit `ff05435bf3ecaa3aba3b8e0281a2e67c038a34bb`;
- post-merge CI `35472774527` — all four jobs green.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `.plans/ACTIVE`.
5. Read the P0-M004 plan.
6. Read `docs/TASK_STATE.md`.
7. Read ADR-0009.
8. Confirm the exact plan-only CI head is green.
9. Do not begin implementation on a failed plan checkpoint.
10. Classify any CI failure before editing.

## Current work

P0-M004 establishes:

- deterministic task identity;
- graph/dependency validation;
- explicit lifecycle transitions;
- derived readiness;
- per-task revisions;
- storage/domain separation;
- a versioned project-local durable snapshot.

## Next implementation boundary

Expected implementation files are limited to the workspace manifest/lockfile, task-domain code in
`agentforge-core`, and the new `agentforge-state` crate unless the Approved plan is amended first.
