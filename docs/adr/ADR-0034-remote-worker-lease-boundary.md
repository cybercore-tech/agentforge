# ADR-0034: Transport-neutral remote-worker lease boundary

## Status

Accepted

## Context

Phase 3 added an observation-oriented external control plane and an outbound local connector. Phase
4 is reserved for remote workers and distributed execution. Starting with transport or cloud
commands would make the authority boundary ambiguous and would couple the local domain model to a
deployment provider.

## Decision

Define remote-worker descriptors and task leases as provider-neutral domain values in
`agentforge-core::remote`. The first implementation includes:

- an explicit protocol version;
- bounded worker and lease identities;
- deterministic, duplicate-free capability advertisements;
- bounded worker concurrency;
- one active lease per task;
- owner and generation checks for renew/release; and
- explicit timestamp-driven expiry.

The lease book is an in-memory state machine. It has no socket, HTTP, cloud, process, worktree,
secret, or persistence dependency.

## Consequences

Positive consequences:

- future transports have a stable contract to encode and validate;
- stale, conflicting, and overlong lease operations fail closed before side effects;
- local tests can exercise distributed scheduling semantics without network or credentials; and
- local AgentForge policy remains the authority for execution and integration.

Trade-offs:

- leases are not durable across process loss yet;
- worker IDs are not authenticated identities; and
- no remote task execution is available until later milestones add transport, authentication,
  persistence, and explicit operator boundaries.

## Rejected alternatives

- Adding a cloud client first would couple core semantics to Mission Control and risk implying cloud
  execution authority.
- Reusing daemon loopback protocol would incorrectly treat a remote worker as a local process and
  would not define lease ownership or recovery semantics.
- Persisting leases in the existing task snapshot would expand the storage format and recovery
  contract before the lease lifecycle itself is stable.
