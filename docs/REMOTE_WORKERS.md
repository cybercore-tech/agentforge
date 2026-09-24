# Remote workers

P4-M001 establishes the domain contract needed before AgentForge can support distributed
execution. P4-M002 adds durable local lease state, P4-M003 adds deterministic local dispatch
planning, and P4-M004 makes leases operable from `forge` and `forged` (see
[Operating leases](#operating-leases)). The contract remains transport-neutral and lives in `agentforge-core::remote`; it does
not open a network connection or grant a cloud service authority over a local project.

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

## Local dispatch planning

`agentforge_scheduler::plan_remote_dispatch` is the local admission seam between ready task
records and remote-worker leases. The caller supplies worker descriptors, lease requests, the
current `LeaseBook`, and `observed_at_ms`. The planner then:

1. validates readiness, duplicate identities, and requested path ownership;
2. orders tasks by task ID and workers by worker ID;
3. expires due leases using only the supplied observation time;
4. uses `LeaseBook::grant` to enforce worker capacity, task ownership, lease windows, and
   generations; and
5. commits the cloned lease book only after every request succeeds.

The returned assignment evidence identifies the task, worker, lease, and generation. A capacity,
conflict, readiness, or lease failure leaves the caller's original lease book unchanged. The
planner does not start a process, persist state automatically, contact a worker, or infer remote
identity from reachability. A caller may persist the committed book with `FileLeaseStore` after a
successful decision.

## Operating leases

P4-M004 (ADR-0046) makes leases an audited operator action. Nothing contacts a worker yet: until an
authenticated transport exists, the operator acts for the worker.

### Register a worker

Create one reviewed profile per worker. The file name is the worker ID:

```text
# .forge/workers/builder-1.conf
platform=linux-x86_64
capability=rust
capability=docker
max_leases=2
```

`platform`, at least one `capability`, and `max_leases` (1–256) are required. Unknown or repeated
keys, symlinks, and files over 4 KiB fail closed. `forge worker list <root>` shows each worker and
its active lease count.

### Lease commands

```bash
forge lease grant <root> <task-id> [--worker <id>] [--ttl-ms <ms>] --actor <you>
forge lease list <root>
forge lease renew <root> <lease-id> [--ttl-ms <ms>] --actor <you>
forge lease release <root> <lease-id> --actor <you>
forge lease expire <root> --actor <you>
```

- `grant` accepts only a ready, `pending` task and goes through `plan_remote_dispatch`: the first
  worker (in ID order) with capacity, or the named one. The default TTL is 15 minutes, and the
  maximum is 24 hours. The lease ID is `<task-id>.L<n>`, and generations increase on each re-grant.
- A grant is refused when the task's paths overlap a running task or another actively leased
  task.
- `renew` sets the expiry to now + TTL. `renew` and `release` of a past-due lease are refused, and
  the expiry is recorded.
- `list` shows the effective state at the current time. An active lease past its expiry reads
  `expired` before a sweep records it.
- Every change is a `LeaseRecorded` audit event (`action`, `lease_id`, `worker_id`, `generation`,
  `expires_at_ms`). Changes are serialized by `.forge/state/remote-leases.lock`. If a crash leaves
  that file behind, remove it only when no `forge` or `forged` process is running for the project.

### Leases block local runs

A task with an active lease, or whose owned paths overlap one, cannot run locally.

- `forge run`, `forge task launch`, and daemon `run` and `launch` refuse it before any worktree,
  state, or audit change.
- `forge task launch-batch` skips it with the reason and runs the rest.

Release the lease, or let it expire, to run the task locally.

### Daemon expiry

A running `forged` expires due leases every 5 seconds and audits them with actor `forged`. It
sweeps only while no execution holds its slot, so it never appends to the audit log during an
execution.

## Running a same-host worker

P4-M005 (ADR-0048) adds the worker side. A registered worker runs the tasks leased to it as a
separate process on the same host, sharing the project directory. There is no network transport
yet.

```bash
forge lease grant . <task-id> --worker builder-1 --actor <you>
forge worker run . builder-1 --profile claude-code --once     # or an absolute executable
forge worker run . builder-1 --profile claude-code --poll-ms 2000   # keep polling
```

For each of its active leases (in lease-ID order) whose task is `pending` and ready, the worker:

1. claims the lease (`LeaseRecorded action=claimed`, actor `worker:<id>`);
2. runs the task through the standard launch path (managed worktree, agent, gates, evidence);
3. renews the lease every third of its window while the agent runs;
4. releases the lease, whatever the outcome.

Only the exact lease holder (lease, worker, and generation) can run a leased task. Every other run
path still refuses it, and a claim never overrides another lease's paths.

The result is an ordinary task result, reviewed the usual way: `forge task diff`, `accept`, the
post-review merge approval, and `integrate`. `--once` exits 0 when idle or successful, and 1 when
the task failed. Stopping the process stops the worker: its lease expires and the task keeps its
evidence.
