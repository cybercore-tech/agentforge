# ADR-0019: Explicit durable single-task orchestration loop

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge has independent contracts for task state, policy, worktrees, adapters, gates, audit, and
review handoff, but `forged` does not yet compose them into an executable runtime.

## Proposed decision

Introduce one provider-neutral, revision-checked orchestration loop whose ordered stages are task
selection, policy, worktree, adapter, gates, audit, and review handoff. Each stage is explicit and
fail-closed; no stage accepts a task, grants authority, or performs protected integration implicitly.

## References

- `.plans/P1-M002-orchestration-loop.plan.md`
- `docs/GOVERNANCE.md`
- `docs/TASK_CONTRACT.md`
