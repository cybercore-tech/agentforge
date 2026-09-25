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

## Automatic dispatch

P4-M006 (ADR-0049) lets a project opt in to automatic granting. Create a reviewed policy:

```text
# .forge/dispatch.conf
enabled=true
milestone=P4-M006        # repeatable; only these milestones are dispatched
ttl_ms=900000            # optional, default 15 minutes
max_per_tick=4           # optional, 1–64
```

`forge lease dispatch <root> --actor <you>` runs one pass and prints the grants, the skipped tasks
with reasons, and why the pass stopped. A running `forged` runs a pass on its idle tick every
5 seconds, as actor `forged`. With a polling worker (`forge worker run ... --poll-ms 2000`), a
created task then runs without any grant command.

A pass grants only `pending`, ready tasks in the listed milestones, in task-ID order, whose
pre-execution approvals are recorded. It uses the same checks as `forge lease grant`: capacity, one
lease per task, and path ownership. It stops when every worker is full or after `max_per_tick`
grants. **Dispatch runs once per task:** a task that already had a lease is skipped, so a failed or
abandoned attempt waits for you (grant it manually to retry). Automatic grants are audited with
`dispatch=auto`.

## Workers on other machines (GhostPort)

P4-M007 (ADR-0050) adds an authenticated, encrypted channel for workers on other hosts. AgentForge
still listens only on loopback. [GhostPort](https://github.com/cybercore-tech/ghostport) carries
the traffic (Noise KK, pinned keys), and every request is also authenticated with the worker's own
AgentForge secret. A remote worker claims, renews, and releases its leases and receives the exact
contract and base commit (P4-M007). It then runs the task and returns the result for exact-SHA
import ([Running tasks remotely](#running-tasks-remotely), P4-M008).

### 1. On the coordinator

```bash
# Register and enroll the worker (prints the secret once; copy it to the worker host, mode 600).
printf 'platform=linux-x86_64\ncapability=rust\nmax_leases=1\n' > .forge/workers/remote-1.conf
forge worker enroll . remote-1

# Turn on the worker API (loopback only) and restart forged.
echo 'bind=127.0.0.1:47420' > .forge/worker-api.conf
forge daemon restart .
```

GhostPort server config on the coordinator. There is one `[[peers]]` entry per worker host, and
each is allowed only its own link:

```toml
role = "server"
private_key_path = "/home/you/.config/ghostport/identity.key"
listen_control = "0.0.0.0:9000"
listen_data = "0.0.0.0:9001"

[[peers]]
name = "remote-1-host"
public_key = "<worker host's public key>"
links = ["agentforge-remote-1"]

[[links]]
id = "agentforge-remote-1"
mode = "forward"
target = "127.0.0.1:47420"      # forged's worker API
```

### 2. On the worker host

```toml
role = "client"
private_key_path = "/home/you/.config/ghostport/identity.key"
peer_public_key = "<coordinator's public key>"
server_control_addr = "coordinator.example.com:9000"
server_data_addr = "coordinator.example.com:9001"

[[links]]
id = "agentforge-remote-1"
mode = "forward"
listen = "127.0.0.1:47500"      # the worker talks to this local end of the tunnel
```

```bash
forge worker remote claim   --endpoint 127.0.0.1:47500 --worker remote-1 --secret-file ~/.config/agentforge/remote-1.secret --contract-out contract.txt
forge worker remote renew   --endpoint 127.0.0.1:47500 --worker remote-1 --secret-file ... --lease <lease> --ttl-ms 3600000
forge worker remote release --endpoint 127.0.0.1:47500 --worker remote-1 --secret-file ... --lease <lease>
```

A claim returns the lease, the coordinator's exact base commit, and the task contract (the
`agentforge-task-prompt-v1` document a local agent would receive). A renewal must extend the
current expiry. Expiry is judged on the coordinator's clock.

### Security properties (verified with GhostPort v0.1.1)

- A host without a pinned GhostPort key cannot connect: the server logs `no configured peer
  matched`, and repeated attempts are rate-limited.
- A wrong or missing AgentForge secret is refused with `unauthorized`, even through a valid tunnel.
  Nothing is written to state or audit.
- A worker can only claim, renew, or release its own leases.
- On the wire, only ciphertext: a capture of the tunnel's data path contained no protocol text,
  secret, contract, or IDs.
- The worker API and its client refuse non-loopback addresses, so the plaintext protocol never
  leaves the machine.

**Rotating a secret:** delete `.forge/workers/<id>.secret`, run `forge worker enroll` again, and
update the worker host.

**Rate limiting (GhostPort v0.1.1):** GhostPort limits repeated failed handshakes *per source
address*, not per pinned peer. Failed attempts from one address (for example a shared NAT) can
briefly block a legitimate worker behind the same address. Retry after a short wait.

## Running tasks remotely

P4-M008 (ADR-0051) completes remote execution. On the worker host, with a clone of the project that
has (or can fetch) the coordinator's commits:

```bash
forge worker remote run --endpoint 127.0.0.1:47500 --worker remote-1 \
  --secret-file ~/.config/agentforge/remote-1.secret --repo ~/src/project \
  --profile claude-code            # or --executable /abs/agent; add --once for a single task
```

For each claim, the worker:

1. decodes the contract and makes sure the exact base commit is in the clone (one `git fetch` if
   it is missing);
2. runs the agent in a managed worktree at the base, renewing the lease every third of its window
   (the window comes from the coordinator's clock);
3. commits in-bounds changes (`agentforge: remote result for <task>`), or refuses to send anything
   that touches a path outside the contract, and releases the lease;
4. sends `RESULT`: the exit status, logs (at most 1 MiB each), and a `git bundle` (at most 32 MiB).
   It retries while the coordinator answers `BUSY`.

The coordinator imports the result **only** if the bundle verifies, the fetched commit is exactly
the reported SHA, it descends from the base, and every changed path is allowed. It then creates the
task worktree at that commit, records the remote agent evidence (`channel=remote`), runs the task's
gates **locally**, and releases the lease. A rejected result changes nothing. Review as usual:

```bash
forge task diff . <task-id>
forge task accept . <task-id> --actor <you>
forge task approve . <task-id> merge_protected_branch --actor <you>   # bound to the imported SHA
forge task integrate . <task-id> --target main --actor <you>
```

`CLAIM` only hands out tasks whose pre-execution approvals are recorded on the coordinator.
Verified end to end through GhostPort v0.1.1: grant, remote claim and run, import with the
coordinator gate 1/1, then accept, approve, and integrate. The exact remote commit landed on
`main`.

## Setting up and checking a worker host

A worker host needs (P4-M009, from the P0-M014 real-agent run):

1. **A clone** of the project with commits and a remote, so it can fetch a base commit it lacks.
2. **A Git identity** in that clone (`user.name` and `user.email`). Remote results are committed
   there.
3. **The project's hooks**. For AgentForge that means `./scripts/install-hooks`
   (`core.hooksPath=.githooks`), so the agent's own pre-commit gate runs.
4. **An agent profile for this host.** Profiles hold absolute paths: point project scripts such as
   the Claude Code bridge at *this clone's* copy, not another checkout's.
5. **The worker secret** from `forge worker enroll`, in a file with mode 600.
6. **A working channel**: the GhostPort client is up, and the worker is registered and enrolled on
   the coordinator.

Check all of it:

```bash
forge worker remote doctor --endpoint 127.0.0.1:47500 --worker remote-1 \
  --secret-file ~/.config/agentforge/remote-1.secret --repo ~/src/project --profile claude-code
```

Each check prints `ok`, `warn`, or `fail`, with a fix: `repo`, `git-identity`, `hooks`, `agent`,
`secret`, `endpoint`, and `origin`. The `endpoint` check uses the authenticated, side-effect-free
`PING`, so it proves the credentials without claiming anything. **`forge worker remote run` runs the
same checks first and refuses to start on any `fail`,** so a missing hook can no longer silently skip
the agent's gate. After each task, the runner also reports how many times it renewed the lease.
