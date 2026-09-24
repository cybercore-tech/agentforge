# ADR-0048: Same-host worker process

- Status: Accepted
- Date: 2026-09-24
- Milestone: P4-M005

## Context

P4-M004 made leases operable and made every local run path refuse a leased task. That guard also
refused the lease holder, so no leased task could run. A worker process has to run exactly the
tasks leased to it, through the same launch path and evidence as local runs, without gaining any
authority beyond the task contract. There is no transport or authentication yet; the first worker
shares the project directory on the same host.

## Decision

- **Claims.**
  - A worker presents a `LeaseClaim` (lease, worker, and generation).
  - `launch_leased_process_persisted` lets a leased task through only when its active lease matches
    the claim exactly. Overlap with any other active lease is still refused, and every other path
    still refuses leased tasks.
  - `claim_lease` verifies the claim under the lease lock (active, owned, task `pending`) and
    records `LeaseRecorded action=claimed`. This is an audit-only action: the lease book does not
    change.
- **`forge worker run`.** It checks that the worker is registered, then picks its claimable leases
  in lease-ID order. For each lease it:
  - claims it;
  - runs the task through the standard launch path, with the task's recorded approvals;
  - renews the lease for its original window every third of that window while the agent runs
    (actor `worker:<id>`);
  - releases the lease on every outcome.

  `--once` runs at most one task; otherwise the worker polls until the process stops.
- **After the run the task is an ordinary result**: `running` (awaiting review) or `failed`. Review,
  accept, the post-review merge approval (ADR-0045), and integration are unchanged.
- **A renewal failure does not stop the run.** Once launched, the task is `running`, so it cannot be
  re-leased or claimed again. The lease is about liveness from then on, not exclusivity.
- **Stopping is process exit.** Renewals stop, the lease expires (daemon sweep or `lease expire`),
  and the task keeps its evidence for the operator, like any interrupted launch.

## Consequences

Positive:

- leases now lead to execution with one evidence chain: granted, claimed, the execution records,
  renewed, and released;
- the worker loop is transport-independent. A remote worker needs only to replace "shares the
  project directory" with an authenticated channel that presents the same claim.

Negative:

- a same-host worker is only as isolated as the local user. It demonstrates the protocol, not a
  security boundary;
- without signal handling, a stopped worker leaves its lease active until expiry.

## Alternatives considered

- **Let the daemon run leased tasks itself.** Rejected: the lease would then describe no separate
  worker, and the path to remote workers would not be exercised.
- **Transition the task to `running` at claim time.** Rejected: the launch path already performs
  and audits that transition, and a second transition source would split the evidence.
