# ADR-0046: Operator lease operations

- Status: Accepted
- Date: 2026-09-24
- Milestone: P4-M004

## Context

P4-M001 to P4-M003 built the remote-worker descriptor, the `LeaseBook` state machine, its durable
snapshot, and a deterministic dispatch planner. All of them were library-only. Nothing could
register a worker or grant a lease, and nothing stopped a leased task from also running locally.
Transport and authentication are not designed yet, so leases must become operable without
implying that any remote party has authority.

## Decision

- **Registration is a reviewed file.** A worker is `.forge/workers/<worker-id>.conf` with
  `platform=`, repeated `capability=`, and `max_leases=`. It is bounded and fail-closed, like
  agent and gate profiles. A worker ID is a label, not an identity. Registration grants nothing.
- **The operator stands in for the worker until transport exists.** `forge lease grant` goes
  through `plan_remote_dispatch`. `renew`, `release`, and `expire` act for the recorded owner and
  generation. Every change runs under an exclusive, never-stolen
  `.forge/state/remote-leases.lock`, and is recorded as a `LeaseRecorded` audit event (code 12).
- **Leases own paths.** A grant is refused when the task's paths overlap a running or actively
  leased task. Local execution is refused for a leased task, or for a task overlapping a lease, on
  every path (`run`, `task launch`, `launch-batch`, and daemon `run` and `launch`). The canonical
  check is `agentforge_orchestrator::remote_lease_conflict`.
- **The daemon expires leases only while idle.** `forged` sweeps every 5 s, holding the execution
  slot for the few milliseconds a sweep takes. An execution keeps its own audit handle, so a
  concurrent append would reuse a sequence number. `stop` and new executions wait out an
  in-progress sweep rather than being refused.
- A leased task stays `pending`. The lease, not the task state, records the reservation. That
  keeps releasing or expiring a lease a pure lease change.

## Consequences

Positive:

- leases are visible (`forge lease list`, `forge worker list`), audited, and enforced against local
  runs before any worker can connect;
- the transport milestone can replace "operator acts for the worker" without changing lease
  semantics.

Negative:

- a crash while holding the lease lock leaves a stale lock that the operator removes by hand, the
  same policy as the daemon lock;
- lease expiry depends on wall-clock time. Windows are bounded, and the sweep only ever expires.

## Alternatives considered

- **Move leased tasks to `running`.** Rejected: task transitions carry acceptance semantics, and
  releasing a lease would then need a lifecycle rollback.
- **Serve lease commands through the daemon protocol.** Rejected for now: the lease lock
  serializes CLI and daemon writers without a protocol change. The transport milestone can add
  remote requests.
