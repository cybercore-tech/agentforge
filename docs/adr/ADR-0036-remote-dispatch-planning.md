# ADR-0036: Deterministic local remote-dispatch planning

- Status: Accepted
- Date: 2026-09-21
- Milestone: P4-M003

## Context

AgentForge now has a transport-neutral worker/lease domain contract and a separate durable lease
snapshot. A future transport still needs a local admission decision before any worker could be
contacted: which ready tasks are requested, which worker owns each lease, and what happens when a
batch cannot fit the available worker capacity.

That decision must not be hidden inside a transport adapter. It needs to preserve existing task
path ownership and lease invariants, remain deterministic across input ordering, and avoid
partially consuming leases when a later assignment fails.

## Decision

Add `agentforge_scheduler::plan_remote_dispatch`. It accepts a validated task graph, supplied
worker descriptors, caller-owned lease state, typed lease requests, and an explicit observation
time. It validates readiness, duplicate workers and request identities, and path ownership. It
then considers requests by task ID and workers by worker ID, expires due leases at the supplied
time, and delegates ownership, concurrency, window, and generation checks to `LeaseBook::grant`.

Planning happens on a cloned lease book. The original is replaced only after every requested
assignment succeeds. The result contains task, worker, lease, and generation evidence. Persistence
is still explicit through `FileLeaseStore`; the planner has no process, socket, cloud, credential,
or remote execution side effect.

## Consequences

Positive:

- equivalent inputs produce the same assignment order;
- worker capacity and lease generations remain enforced by the core state machine;
- failed batches are safe to inspect and retry because the caller's book is unchanged;
- future transport work receives a narrow, already-authorized local admission boundary.

Trade-offs:

- the caller must supply valid lease identities and timestamps;
- remote capability negotiation is not yet modeled in task contracts;
- a successful plan is an admission decision, not proof that a worker is reachable or trusted.
