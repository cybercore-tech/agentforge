# ADR-0018: Deterministic multi-agent scheduling and serialized integration

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Phase 0 now supplies durable task state, isolated worktrees, policy checks, execution evidence, gates,
and audit records. Phase 1 needs concurrency without shared dirty state or ambiguous integration.

## Proposed decision

Derive deterministic runnable batches from task dependencies and explicit path ownership. Permit
concurrency only for disjoint tasks, reject conflicts conservatively, and serialize all integration
through one explicit Integrator boundary. Scheduling is orchestration evidence, not task acceptance.

## References

- `.plans/P1-M001-scheduling-integration.plan.md`
- `docs/GOVERNANCE.md`
- `docs/WORKTREE_ISOLATION.md`
