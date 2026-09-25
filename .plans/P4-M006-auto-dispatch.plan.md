# Plan: P4-M006 — Opt-in automatic dispatch to registered workers

Status: Approved
Milestone: P4-M006
Created: 2026-09-24
Owner: AgentForge project
Implementer: operator-authored (automatic granting moves authority from the operator to the daemon)

## Goal

Let a project opt in to having AgentForge grant leases for ready tasks to registered workers
without a manual `forge lease grant`. The daemon's idle tick dispatches under a reviewed, bounded
policy, and `forge lease dispatch` runs one pass by hand. With a polling `forge worker run`, a task
then goes from created to executed and awaiting review with no per-task operator command. Review,
acceptance, approval, and integration stay manual.

## Non-goals

- No transport and no remote workers (P4-M007 and later).
- No automatic review, accept, approve, or integrate.
- No re-dispatch of a task that already had a lease. A failed, expired, or refused attempt returns
  to the operator.
- No matching of task requirements to worker capabilities. Capabilities stay descriptive.

## Context

P4-M004 made leases operable and P4-M005 added `forge worker run`, but each grant is a manual
operator command. The daemon already runs an idle-only 5 s lease tick that holds the execution
slot. Automatic granting changes who exercises lease authority, so it must be opt-in, reviewed,
scoped, and bounded, and it must not loop on tasks that cannot run.

## Architecture placement

- **Policy file:** `.forge/dispatch.conf`, bounded `key=value`, fail-closed like worker profiles:
  - `enabled=true|false`, required;
  - `milestone=<ID>`, repeatable, at least one required: only tasks in these milestones are
    dispatched;
  - `ttl_ms=<ms>`, default 15 minutes, bounded by `MAX_LEASE_DURATION_MS`;
  - `max_per_tick=<n>`, from 1 to 64, default 4.

  A missing file means dispatch is off. An invalid file is an error that the daemon reports on
  each tick, and it dispatches nothing.
- **`agentforge-operator::dispatch`:**
  - `DispatchPolicy::load(root)`;
  - `dispatch_ready(root, now, actor) -> DispatchPass { granted, skipped }`.
  - Candidates are `pending`, ready tasks, in task-ID order, in a listed milestone, that have never
    held a lease, and whose pre-execution approvals are all recorded (the same rule as launch,
    P1-M008).
  - Each candidate is granted through `grant_lease` (the planner, the lock, audit, and path
    ownership). A path conflict skips that task with its reason; running out of worker capacity
    ends the pass; the pass also stops after `max_per_tick` grants.
  - Grants are ordinary `LeaseRecorded action=granted` events with actor `forged` (daemon) or the
    operator (CLI), plus a `dispatch=auto` field.
- **CLI:** `forge lease dispatch <root> --actor <actor>` runs one pass. It prints the grants and the
  skipped tasks with reasons, or "dispatch is disabled" when there is no policy.
- **Daemon:** the existing tick expires due leases, then, when a policy is enabled, runs one
  dispatch pass as `forged`. It now also runs when there is no lease snapshot yet but a policy
  exists. It is still idle-only and still holds the execution slot. There is no protocol change.

## Invariants

- Without an enabled policy, nothing is granted automatically.
- A task is dispatched at most once automatically.
- A dispatched task satisfies every rule a manual grant does, plus recorded pre-execution
  approvals.
- Every automatic grant is audited and attributable (actor and `dispatch=auto`).

## ADRs

ADR-0049 "Opt-in automatic dispatch": the policy file, milestone scoping, the dispatch-once rule,
the approval precheck, and running on the idle tick.

## Public API / CLI

`dispatch::{DispatchPolicy, dispatch_ready, DispatchPass}`, `forge lease dispatch`, and
`.forge/dispatch.conf`.

## Compatibility analysis

Additive. Projects without `.forge/dispatch.conf` behave as before. `dispatch=auto` is a new
optional field on `LeaseRecorded`.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P4-M006-auto-dispatch.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-operator/src/lib.rs`, `crates/agentforge-operator/src/leases.rs`,
  `crates/agentforge-operator/src/dispatch.rs`, `crates/agentforge-operator/tests/*.rs`
- `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-daemon/tests/daemon.rs`
- `crates/agentforge-cli/src/main.rs`, `crates/agentforge-cli/tests/*.rs`
- `docs/adr/ADR-0049-auto-dispatch.md`, `docs/REMOTE_WORKERS.md`, `docs/DAEMON.md`,
  `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- **Policy:** valid files load. Each of these fails closed: missing `enabled`, no milestone,
  unknown key, a bad TTL, `max_per_tick` 0 or 65, a symlink, and an oversize file. A missing file
  means off, and `enabled=false` means off.
- **`dispatch_ready`:**
  - grants only tasks in listed milestones, in task-ID order;
  - skips a task whose pre-execution approval is missing, with the reason, and grants it once
    approved;
  - never re-dispatches a task that had a lease (released or expired);
  - stops at worker capacity and at `max_per_tick`;
  - skips a path-overlapping task with the reason and grants the next;
  - grants carry `dispatch=auto` and the actor.
- **CLI:** `forge lease dispatch` with and without a policy. End to end: dispatch, then
  `forge worker run --once` runs the task.
- **Daemon:** a running daemon with an enabled policy grants a ready task within the tick, with
  actor `forged`.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Policy and `dispatch_ready`, with tests; then the CLI; then the daemon tick.
4. ADR and docs. Full gate plus package preflight. Dogfood on a scratch repo: `forged` plus a
   polling worker plus a policy. Create a task and watch it get dispatched, claimed, run, and
   released with no grant command.
5. Push and verify CI (push plus two repeats); close; tag.

## Failure modes

- An invalid policy: nothing is dispatched, and the error is reported each tick and by the CLI.
- A worker cannot run a dispatched task (for example, a missing gate profile): the worker releases
  the lease and the task stays `pending`. Dispatch-once means it is not retried automatically; the
  operator fixes the cause and grants it manually.
- A dead worker holding auto-granted leases: they expire. The tasks are not re-dispatched, and the
  operator decides.

## Documentation impact

REMOTE_WORKERS ("Automatic dispatch"), DAEMON (the tick), OPERATIONS (the command, the policy, and a
recovery row), and ADR-0049. README, CHANGELOG, and MILESTONES at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- push-triggered CI plus two dispatched repeats.

## Acceptance criteria

- [ ] With an enabled policy, ready tasks are granted to registered workers by the daemon and by
      `forge lease dispatch`, audited as automatic.
- [ ] Scoping, the approval precheck, dispatch-once, capacity, and per-tick limits hold.
- [ ] A created task runs end to end through a polling worker with no manual grant (dogfood).
- [ ] ADR-0049 and docs; CI evidence; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
