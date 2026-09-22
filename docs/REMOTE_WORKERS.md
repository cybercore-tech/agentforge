# Remote workers

P4-M001 establishes the domain contract needed before AgentForge can support distributed
execution. The contract is transport-neutral and lives in `agentforge-core::remote`; it does not
open a network connection or grant a cloud service authority over a local project.

## Worker descriptor

`RemoteWorkerDescriptor` identifies one worker with:

- protocol version;
- a bounded stable worker ID;
- a bounded platform label;
- a deterministic, duplicate-free capability list; and
- a bounded maximum number of active task leases.

Capability values are descriptive contract labels. They do not grant authority by themselves.
Local task policy, approvals, worktree ownership, gates, and integration remain authoritative.

## Lease lifecycle

`LeaseBook` is an explicit in-memory state machine. A local authority supplies timestamps and must
explicitly perform each transition:

```text
grant -> active -> renew -> active
                   |
                   +-> release

active --observed after expiry--> expired
expired -> later grant with a higher task generation
```

The state machine enforces one active lease per task, worker concurrency limits, owner and
generation checks for renewal/release, bounded lease windows, and deterministic lease ordering.
It never starts a process, creates a worktree, writes a task snapshot, or sends a network request.

## Authority boundary

This milestone intentionally stops before transport and persistence:

- no remote worker can request execution through AgentForge;
- no cloud or Mission Control component can mutate local task state;
- no lease survives process loss because there is no durable lease store yet; and
- no identity or authentication mechanism is implied by a worker ID.

Future milestones may add authenticated transport, durable recovery, scheduling integration, and
operator-visible remote-worker actions. Each must preserve local approval and evidence boundaries
and must be introduced through a separate approved plan.

## Core API

The initial API is exposed from `agentforge_core::remote`:

- `RemoteWorkerId`, `WorkerCapability`, and `RemoteWorkerDescriptor`;
- `LeaseId`, `TaskLease`, and `LeaseState`; and
- `LeaseBook` with `grant`, `renew`, `release`, `expire_due`, and deterministic read methods.

All invalid values fail closed with `RemoteWorkerError` before lease state changes.
