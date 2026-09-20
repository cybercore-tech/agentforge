# Plan: P2-M003 — Controlled operator actions

Status: Completed
Milestone: P2-M003
Created: 2026-09-20

## Goal

Give operators an explicit, auditable command surface for inspecting a task, recording required
human approvals, and applying narrow lifecycle decisions without turning the HUD into an implicit
control channel. The existing `forge run` path must consume only verified approval evidence and
continue to require the existing policy, worktree, adapter, and gate boundaries.

## Non-goals

- No mutation commands embedded in `forge hud` or its watch mode.
- No automatic task creation, agent launch, worktree creation/retirement, gate execution, merge,
  deployment, or repair from a HUD frame.
- No approval inferred from guideline prose, terminal presence, command history, or an unverified
  event.
- No approval deletion or destructive audit rewriting; the audit log remains append-only.
- No new remote service, daemon protocol, cloud dependency, terminal UI framework, or third-party
  Rust dependency.
- No changes to task contract, snapshot, audit, or worktree formats without an explicit amendment.

## Context

P1-M003 already provides explicit task creation, P1-M002 provides `forge run`, and P2-M001/P2-M002
provide read-only one-shot and live HUD views. Operators can observe and launch a task, but there
is not yet a narrow, durable interface for human approval or independent lifecycle decisions. This
milestone adds that control boundary as a separate operator service and CLI command group while
keeping the HUD observational.

## Architecture placement

```text
forge task inspect|approve|accept|cancel|retry <root> ...
       |
       v
controlled operator action service
       |
       +--> agentforge-state (verified graph load/save)
       +--> agentforge-audit (append-only approval/transition evidence)
       +--> agentforge-core (task and lifecycle invariants)
       +--> agentforge-orchestrator (verified approvals for run)
```

The service owns no process spawning or Git mutation. CLI parsing and exit codes stay in
`agentforge-cli`; durable task and audit stores remain the sources of truth.

## Data flow

1. `task inspect` loads the snapshot read-only and renders one bounded task contract, lifecycle
   state, revision, dependencies, capabilities, approvals, and readiness result in stable order.
2. `task approve` validates the task and requested approval boundary, requires an explicit bounded
   actor identity, verifies that the boundary is actually required by the task, then appends one
   `ApprovalRecorded` audit event without changing task state.
3. `task accept`, `task cancel`, and `task retry` load the graph, validate the requested core
   transition, save the next snapshot atomically, and append a matching `TaskTransition` event.
4. `forge run` loads verified approval events for the selected task and passes only those approved
   boundaries to the existing orchestrator preflight. Missing approvals fail closed.
5. Every action reports the resulting revision/state or a bounded source-specific diagnostic. No
   action silently creates missing task state or audit evidence.

## Invariants

- HUD and watch mode remain strictly read-only.
- Every mutating operator action has explicit command syntax, actor identity, source validation,
  durable audit evidence, and deterministic exit behavior.
- Approval evidence is accepted only when the audit chain verifies, the event targets the task, and
  its boundary is required by that task; duplicate approvals are idempotent and do not create
  ambiguous state.
- Lifecycle transitions use `TaskGraph::transition`; invalid, terminal, missing, or not-ready
  transitions fail without partial snapshot writes.
- Snapshot and audit updates are ordered so a failed append cannot be reported as committed state.
- User-controlled values are direct path/argument data; no shell interpolation is used.
- Existing `forge run` policy, worktree, adapter, and gate checks remain authoritative.
- Output and actor/boundary inputs are bounded; no raw audit payload is printed unbounded.

## ADRs

- Add `docs/adr/ADR-0023-controlled-operator-actions.md` and register it in `docs/adr/README.md`.
- Explain why actions are separate from the read-only HUD, why approvals are append-only evidence,
  and why lifecycle mutation remains behind core transition validation.

## Public API / CLI

Add a focused standard-library-only `agentforge-operator` crate with provider-neutral operations:

- bounded `TaskInspection` projection;
- `inspect_task`, `approve_task`, `accept_task`, `cancel_task`, and `retry_task` operations;
- verified approval extraction for orchestrator callers;
- source-specific action errors with stable display text.

Add explicit commands:

```text
forge task inspect <root> [<task-id>]
forge task approve <root> <task-id> <approval-boundary> --actor <actor-id>
forge task accept <root> <task-id> --actor <actor-id>
forge task cancel <root> <task-id> --actor <actor-id>
forge task retry <root> <task-id> --actor <actor-id>
```

`task inspect <root>` lists all tasks; supplying an ID selects one. Existing `task create`,
`forge run`, and HUD commands remain compatible. Approval boundary names use the existing stable
`ApprovalBoundary::as_str` identities.

## Compatibility analysis

Existing task creation, one-shot/live HUD, `forge run`, task snapshot encoding, audit encoding, and
worktree behavior remain compatible. New approval events use the existing audit event kind and
fields. Existing tasks with no required approvals continue to run unchanged; tasks requiring an
approval fail closed until an explicit matching event exists.

## Dependency analysis

Use the Rust standard library and existing workspace crates only. A new local
`agentforge-operator` crate is permitted by this plan; no third-party dependency is permitted.

## Expected file boundary

- `.plans/P2-M003-controlled-operator-actions.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `docs/HUD.md`, `docs/ORCHESTRATION.md`
- `docs/adr/ADR-0023-controlled-operator-actions.md`, `docs/adr/README.md`
- `Cargo.toml`, `Cargo.lock`
- `crates/agentforge-operator/**`
- `crates/agentforge-orchestrator/**`
- `crates/agentforge-cli/**`
- focused unit and integration tests for inspection, approvals, transitions, audit ordering,
  failure atomicity, run preflight, CLI exit behavior, and read-only HUD compatibility

No changes to scheduler, adapter internals, worktree lifecycle, gate definitions, workflow, hooks,
or deployment behavior without an amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Inspection | One/all-task projection is bounded, deterministic, and read-only |
| Approval | Required-boundary validation, actor bounds, duplicate idempotence, audit integrity |
| Lifecycle | Accept/cancel/retry use core transitions and preserve revisions |
| Atomicity | Failed transition or audit append leaves snapshot/evidence unchanged |
| Run authorization | Required approvals are extracted only from verified task-linked events |
| Diagnostics | Missing/corrupt state or audit fails closed with source context |
| CLI | Syntax, actor, boundary, and exit behavior are deterministic |
| Compatibility | Existing HUD/watch, task creation, run, and workspace tests remain green |

## Implementation sequence

1. Approve this plan separately and activate it only after the pointer is present in `HEAD`.
2. Define operator action models, approval evidence rules, and atomic update ordering in ADR-0023
   and the operator documentation.
3. Implement the operator service and unit tests for inspection, approvals, transitions, and
   failure atomicity.
4. Wire the CLI command group and verified approval consumption into `forge run`.
5. Add isolated temporary-repository CLI/integration fixtures proving audit and snapshot behavior.
6. Run the full local gate and exact-head CI.
7. Close the milestone with exact implementation, closure, and mainline CI evidence.

## Failure modes

- Missing/corrupt task snapshot: fail closed without creating state.
- Missing/corrupt audit log: fail closed without recording a guessed approval or transition.
- Boundary not required by the task: reject approval without mutation.
- Unknown actor, task, boundary, or malformed syntax: deterministic non-success exit.
- Invalid lifecycle transition or unmet dependency: preserve prior snapshot and audit state.
- Audit append failure after a proposed transition: do not report the transition as committed.
- Unverified or task-unrelated approval event: exclude it from `forge run` authorization.

## Documentation impact

Document the command syntax, actor requirement, stable approval names, lifecycle semantics, audit
ordering, failure behavior, and the continued separation between read-only HUD observation and
explicit operator mutation.

## Quality gates

- `./scripts/gate.sh full`
- focused operator-service, orchestrator, HUD-compatibility, and CLI tests
- MSRV 1.85.0 compatibility
- exact-head Repository policy, Stable code gate, MSRV, and CLI smoke CI
- no bypassed hooks or weakened validation

## Acceptance criteria

- [x] Operators can inspect one or all tasks through a bounded deterministic command.
- [x] Required approvals can be recorded only with explicit actor/boundary/task validation.
- [x] Accept/cancel/retry actions use core transitions and append durable evidence atomically.
- [x] `forge run` consumes only verified task-linked approval evidence.
- [x] HUD and watch mode remain read-only and backward-compatible.
- [x] Tests prove authorization, failure atomicity, bounded output, and CLI behavior.
- [x] Exact implementation, closure, and mainline CI evidence is recorded.

## Completion record

Implementation commit: `381afc6d0342ad12b36ce9574dd6ffb1a1c34a93`
CI run: `35498108446`
CI result: all four jobs green for the exact implementation SHA
Completed: 2026-09-20
Notes: Added the standard-library-only operator service and explicit `forge task` inspect,
approval, accept, cancel, and retry commands. Approval evidence is verified and task-linked before
`forge run`; HUD and watch mode remain read-only. Closure and post-merge evidence will be recorded
by the separate documentation closure commit.
