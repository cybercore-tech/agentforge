# ADR-0016: Read-only deterministic doctor and status diagnostics

- Status: Proposed
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

Operators need one bounded view of environment readiness, project contracts, task/worktree health,
and blockers without allowing diagnostics to change state.

## Proposed decision

Extend the CLI with deterministic `doctor` and `status` observations. Probes are read-only,
bounded, explicitly ordered, and report uncertainty or missing prerequisites as findings. They do
not repair, transition tasks, mutate worktrees, or infer acceptance.

## References

- `.plans/P0-M011-doctor-status.plan.md`
- `docs/GOVERNANCE.md`
- `docs/WORKTREE_ISOLATION.md`
