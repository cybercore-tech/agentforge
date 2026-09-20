# Plan: P1-M001 — Multi-agent scheduling and serialized integration

Status: Approved
Milestone: P1-M001
Created: 2026-09-19

## Goal

Add a provider-neutral scheduler that derives runnable tasks from the durable task graph, coordinates
concurrent read-only or disjoint write tasks, and serializes integration through an explicit boundary.

## Non-goals

- No remote workers, implicit privilege elevation, automatic merge into protected branches, deployment,
  unbounded retries, hidden shared worktrees, or provider-specific scheduling behavior.
- Scheduling does not accept task results; review, gates, approvals, and integration remain explicit.

## Architecture

Build on `agentforge-state` task dependencies, `agentforge-worktree` ownership, `agentforge-policy`
capability checks, `agentforge-orchestrator` evidence, and `agentforge-audit` events. A scheduler
produces deterministic runnable batches. Conflict detection rejects overlapping write ownership and
reserves a single Integrator boundary for merge-only operations. State mutation is serialized behind
an explicit store boundary; workers receive immutable task snapshots and cannot schedule themselves.

## Expected file boundary

- `.plans/P1-M001-scheduling-integration.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/SCHEDULING.md`, `docs/adr/ADR-0018-multi-agent-scheduling.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`, `crates/agentforge-scheduler/**`

## Test matrix

| Behavior | Required evidence |
| --- | --- |
| Readiness | Dependencies and task state derive deterministic runnable order |
| Concurrency | Disjoint ownership can share a batch; reads never grant writes |
| Conflicts | Overlapping paths, duplicate reservations, and missing capabilities fail closed |
| Integration | Exactly one serialized Integrator boundary is admitted |
| State safety | Revisions and transitions are serialized; stale snapshots are rejected |
| Fairness/bounds | Batch size, queue ordering, and retries are explicit and bounded |
| Isolation | Scheduler has no ambient process, network, secret, or environment authority |

## Acceptance

Approve this plan separately, implement the scheduler with deterministic isolated tests, run full gate
and exact CI, then close, merge, and verify post-merge CI.
