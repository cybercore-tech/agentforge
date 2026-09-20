# Plan: P0-M011 — Doctor and status diagnostics

Status: Approved
Milestone: P0-M011
Created: 2026-09-19

## Goal

Provide deterministic, read-only operator diagnostics for environment, project state, active
agents, managed worktrees, and blockers through `forge doctor` and `forge status`.

## Non-goals

- No mutation, repair, scheduling, task transitions, process launch, network access, secret access,
  worktree creation/removal, or automatic remediation.
- Diagnostics are bounded observations and do not claim that an environment is secure or that a task
  is accepted.

## Architecture and boundary

Build on existing core/state/worktree/audit/policy contracts. Keep observation logic provider-neutral
and deterministic; direct Git inspection must use existing `WorktreeManager` boundaries. Human-readable
output is stable, while failures remain explicit and actionable. Environment probes must be injected
or bounded for tests and must not mutate the parent process environment.

## Expected file boundary

- `.plans/P0-M011-doctor-status.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/DOCTOR_STATUS.md`, `docs/adr/ADR-0016-doctor-status-diagnostics.md`, `docs/adr/README.md`
- `crates/agentforge-cli/src/**`, `crates/agentforge-cli/tests/**`

No changes to workflows, hooks, adapters, policy semantics, or persistence formats without amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Deterministic output | Stable ordering and bounded output for identical observations |
| Read-only boundary | Doctor/status never mutate files, Git state, environment, or task state |
| Environment checks | Missing tools/configuration are explicit findings, not panics |
| Project checks | Version, workspace, plan, and gate health are reported distinctly |
| Worktree checks | Managed worktrees use authoritative inspection and surface dirty/unresolved state |
| Blockers | Findings carry severity and actionable, non-mutating remediation text |
| CLI contract | Success/failure exit codes and unknown commands are stable |

## Sequence and acceptance

Review and approve this plan in a separate commit, then implement read-only diagnostics and tests,
run the full gate and exact CI, close, merge, and verify post-merge CI. No external dependency is
expected.

## Completion record

Implementation commit: pending
Implementation CI: pending
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
