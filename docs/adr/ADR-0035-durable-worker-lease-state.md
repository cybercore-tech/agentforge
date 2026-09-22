# ADR-0035: Durable remote-worker lease state

- Status: Accepted
- Date: 2026-09-21
- Milestone: P4-M002

## Context

P4-M001 introduced a validated, transport-neutral `LeaseBook` for remote-worker task leases.
The in-memory state machine protects ownership, generations, expiry, and one-active-lease-per-task
in a running process, but process loss otherwise discards that state. Reusing task persistence for
leases would couple two independent schemas and could allow a lease-format change to rewrite task
state.

Restart recovery also needs a deliberate time boundary. Reading the system clock inside a storage
adapter would make loading nondeterministic and would hide the authority decision about when a
restored lease is considered expired.

## Decision

Add `LeaseStore` and `FileLeaseStore` to `agentforge-state`. The default path is the separate
project-local file `.forge/state/remote-leases.snapshot`, with its own magic and schema version.
The format stores bounded, checksummed records for lease ID, task ID, worker ID, generation,
issue/expiry timestamps, and lifecycle state. Saves use the existing temporary-file, sync, and
atomic-replace pattern.

Lease records are reconstructed through the validators in `agentforge-core::remote`. Missing state
returns `None`; invalid bytes, unsupported versions, bounds violations, duplicate active tasks,
and invalid generations fail closed. Loading never reads a clock. Callers must explicitly perform:

```text
load -> expire_due(observed_at_ms) -> continue
```

The task snapshot remains independent, and this milestone adds no transport, authentication,
daemon protocol, CLI, scheduling, or remote execution behavior.

## Consequences

Positive:

- lease ownership and generations survive a process restart;
- recovery is deterministic, testable, and controlled by the local authority;
- task and lease corruption boundaries remain separate;
- atomic replacement avoids exposing partially written lease state.

Trade-offs:

- callers must remember to expire restored leases before scheduling or renewal;
- the snapshot is local recovery state, not proof of worker identity or distributed consensus;
- future format changes require an explicit schema/migration decision.
