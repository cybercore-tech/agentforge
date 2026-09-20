# ADR-0017: Explicit single-agent vertical slice

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Existing milestones provide isolated contracts but no composed execution path proving their ordering.

## Proposed decision

Add one bounded orchestration flow that validates authority, re-inspects a task worktree, runs the
configured adapter, executes gates, emits audit evidence, and hands off for review. It never accepts
results, schedules peers, merges, or deploys automatically.

## References

- `.plans/P0-M012-single-agent-vertical-slice.plan.md`
- `docs/GOVERNANCE.md`
