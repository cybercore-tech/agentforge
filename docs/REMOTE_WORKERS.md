# Remote workers

P4-M001 establishes the domain contract needed before AgentForge can support distributed
execution. P4-M002 adds durable local lease state. The contract remains transport-neutral and
lives in `agentforge-core::remote`; it does not open a network connection or grant a cloud service
authority over a local project.

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

## Durable lease state and restart recovery

`agentforge_state::FileLeaseStore` persists the validated `LeaseBook` at:

```text
.forge/state/remote-leases.snapshot
```

Lease state is deliberately separate from `.forge/state/tasks.snapshot`. The lease snapshot has
its own magic and schema version, bounded record decoding, a checksum, and atomic replacement.
Missing lease state is valid and returns `None`; corrupt, incompatible, oversized, truncated, or
semantically invalid state fails closed without changing the task snapshot.

Restart recovery is explicit and does not read the system clock during load:

```text
load -> expire_due(observed_at_ms) -> continue scheduling or renewal
```

Loading preserves each lease's worker identity, task identity, generation, timestamps, and
terminal/active state exactly as recorded. The local authority supplies `observed_at_ms` and calls
`expire_due` before treating a restored active lease as no longer valid. No network, identity
verification, authentication, or remote command authority is implied by this storage API.

## Authority boundary

The remote-worker milestones intentionally stop before transport and execution authority:

- no remote worker can request execution through AgentForge;
- no cloud or Mission Control component can mutate local task state;
- durable lease state is local recovery data, not a remote source of authority;
- no identity or authentication mechanism is implied by a worker ID.

Future milestones may add authenticated transport, scheduling integration, and operator-visible
remote-worker actions. Each must preserve local approval and evidence boundaries and must be
introduced through a separate approved plan.

## Core API

The initial API is exposed from `agentforge_core::remote`:

- `RemoteWorkerId`, `WorkerCapability`, and `RemoteWorkerDescriptor`;
- `LeaseId`, `TaskLease`, and `LeaseState`; and
- `LeaseBook` with `grant`, `renew`, `release`, `expire_due`, deterministic read methods, and
  validated restoration support;
- `LeaseStore` and `FileLeaseStore` for the separate durable lease snapshot.

All invalid values fail closed with `RemoteWorkerError` before lease state changes.
