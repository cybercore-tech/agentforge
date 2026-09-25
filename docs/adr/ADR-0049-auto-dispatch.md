# ADR-0049: Opt-in automatic dispatch

- Status: Accepted
- Date: 2026-09-24
- Milestone: P4-M006

## Context

After P4-M005 a worker process can run its leased tasks, but every lease is a manual `forge lease
grant`. Granting leases automatically moves that authority from the operator to AgentForge, so it
must be something a project chooses, reviews, scopes, and bounds. It must also never loop on a task
that cannot run.

## Decision

- **Opt-in, reviewed policy.** `.forge/dispatch.conf` contains:
  - `enabled=true|false`;
  - one or more `milestone=`;
  - an optional `ttl_ms` (default 15 minutes);
  - an optional `max_per_tick` (1–64, default 4).

  A missing file means off. An invalid file is reported, and nothing is dispatched.
- **One rule set with manual grants.** `dispatch_ready` grants through `grant_lease`: the dispatch
  planner, the lease lock, path ownership against running and leased tasks, and audit. Candidates
  are `pending`, ready tasks in listed milestones, in task-ID order, with every pre-execution
  approval recorded.
- **Dispatch-once.** A task that ever held a lease is never dispatched automatically again. A
  refused, failed, or abandoned attempt returns the decision to the operator, who can grant
  manually.
- **Bounded passes.** A pass stops when no registered worker has capacity, or after
  `max_per_tick` grants.
- **Attributable.** Automatic grants are ordinary `LeaseRecorded action=granted` events with
  `dispatch=auto`, carrying the actor `forged` (daemon) or the operator (`forge lease dispatch`).
- **Where it runs.** On the daemon's existing idle-only tick, after expiry, holding the execution
  slot. `forge lease dispatch` runs one pass by hand.

## Consequences

Positive:

- with `forged` and a polling `forge worker run`, a created (and pre-approved) task runs without
  any per-task command. The review, accept, approve, and integrate boundaries stay manual;
- dispatch never weakens a manual-grant check.

Negative:

- dispatch-once needs operator attention after any failed attempt, by design;
- milestone scoping is coarse; per-task opt-in would need a task-contract change.

## Alternatives considered

- **Dispatch every ready task.** Rejected: the operator may intend some tasks to run locally.
- **Retry with backoff.** Rejected for now: a task that cannot run (a missing gate profile, or a
  broken base) would consume worker capacity indefinitely, and failed attempts deserve a human
  look.
