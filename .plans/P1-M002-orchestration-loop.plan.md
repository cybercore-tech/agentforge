# Plan: P1-M002 — Single-task orchestration loop

Status: Draft
Milestone: P1-M002
Created: 2026-09-19

## Goal

Wire the existing task state, policy, worktree, adapter, gate, audit, and review contracts into one
real operator-invocable single-task execution loop with explicit durable transitions and fail-closed
error handling.

## Non-goals

- No remote workers, TUI, deployment, automatic protected-branch merge, hidden retries, or bypass of
  approvals and gates.
- No replacement of existing crate boundaries or weakening of their independent contracts.

## Architecture

Add a provider-neutral orchestration service and a minimal CLI command path. The loop must load and
validate durable task state, derive a ready task, evaluate capability/approval policy, create and
re-inspect its managed worktree, invoke the configured adapter, run declared gates, append audit
events, and emit an explicit review handoff. Every transition is revision-checked and persisted;
failures stop later side effects. Acceptance remains a caller/operator decision.

## Expected file boundary

- `.plans/P1-M002-orchestration-loop.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/ORCHESTRATION.md`, `docs/adr/ADR-0019-orchestration-loop.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-orchestrator/**`, `crates/agentforge-cli/**`, focused integration tests

No workflow, hook, dependency, persistence-format, or unrelated scheduler changes without amendment.

## Test matrix

| Stage | Required evidence |
| --- | --- |
| Task selection | Durable ready task and revision are loaded deterministically |
| Policy | Missing capability/approval stops before worktree or process side effects |
| Worktree | Create, identity recheck, dirty/unresolved rejection, and safe retirement are exercised |
| Adapter | Real fixture process receives explicit task data and bounded evidence |
| Gates | Declared gates run after execution and before review handoff |
| Audit | Start, worktree, agent, gate, failure, and handoff events replay in order |
| State | Running/failed/blocked transitions persist with stale-revision rejection |
| CLI | Success, blocked, escalation, and failure exit behavior is deterministic |

## Acceptance

Approve separately, implement an isolated end-to-end fixture, run full gate and exact CI, close,
merge, and verify post-merge CI.
