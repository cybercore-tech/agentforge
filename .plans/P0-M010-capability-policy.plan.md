# Plan: P0-M010 — Capability and permission policy

Status: Completed
Milestone: P0-M010
Created: 2026-09-19

## Goal

Add a standard-library-only, provider-neutral policy engine that evaluates explicit task capability
grants, path ownership, and approval prerequisites using deterministic deny-by-default decisions.

## Non-goals

- No OS sandbox, syscall interception, credential broker, network proxy, scheduler, task transition,
  process launcher, worktree mutation, automatic approval, or remote policy service.
- A policy decision is validation evidence; it does not itself enforce arbitrary child processes.

## Context and architecture

P0-M002 defines roles, capabilities, scopes, and approvals. P0-M005 isolates worktrees, P0-M006
defines adapter boundaries, and P0-M009 records evidence. P0-M010 supplies a pure policy boundary
between those contracts: callers provide an `AgentTask`, requested operation, paths, and observed
approvals; the engine returns a deterministic allow or explicit denial/escalation reason.

Create `agentforge-policy` with no external dependencies. Capabilities remain independent of roles;
missing grants, forbidden paths, unknown contract versions, duplicate/conflicting grants, and unmet
approval boundaries fail closed. Canonical ordering must not depend on map iteration or environment.

## Expected file boundary

- `.plans/P0-M010-capability-policy.plan.md`
- `.plans/ACTIVE` (only after approval)
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/POLICY.md`
- `docs/adr/ADR-0015-capability-policy.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-policy/Cargo.toml`, `crates/agentforge-policy/src/**`, `crates/agentforge-policy/tests/**`

No changes to adapters, worktrees, gates, audit storage, CLI/daemon, hooks, or workflows without a
plan amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Role independence | Same role with different grants yields different decisions |
| Least privilege | Missing capability is denied; no ambient environment grants authority |
| Path scope | Owned paths allow, forbidden or outside paths deny; normalization is deterministic |
| Approvals | Required approval is explicit and unmet approval yields escalation/denial |
| Grant integrity | Duplicate, conflicting, unsupported-version, and malformed inputs fail closed |
| Determinism | Equivalent inputs produce stable decision/error ordering |
| Boundary claims | Engine never spawns, mutates worktrees, reads secrets, or claims OS enforcement |

## Implementation sequence

1. Review this Draft plan and ADR-0015; obtain approval and green plan CI.
2. Activate the plan in a separate approved-plan commit.
3. Implement policy types, validation, canonical decisions, and isolated tests.
4. Document semantics and enforcement limits in `docs/POLICY.md`.
5. Run the full gate, focused tests, exact CI, closure, and post-merge verification.

## Quality gates and acceptance

Run `./scripts/gate.sh full`, stable tests, MSRV 1.85.0, and CLI smoke. Acceptance requires
explicit deny-by-default behavior, deterministic results, no external dependencies, and green exact
SHA CI for approved-plan, implementation, closure, and post-merge commits.

## Completion record

Implementation commit: `1ece087f25ccb6ce8fda96867e9f2502b91e4c86`
Implementation CI: `35490199374` — all four jobs green
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
