# Local daemon

`forged` is the optional local runtime for one project. It listens only on a loopback TCP address,
serializes one mutating task execution at a time, and reuses the same task, approval, policy,
worktree, adapter, audit, and acceptance boundaries as `forge run`.

## Start

Run the daemon in the foreground from any shell:

```bash
forged serve --root /path/to/project
```

The daemon validates that the root is a Git repository and records a versioned endpoint under
`.forge/daemon/endpoint`. The endpoint is loopback-only and the instance lock is
`.forge/daemon/lock`. A second daemon for the same project is rejected.

The optional bind argument must remain loopback-only:

```bash
forged serve --root /path/to/project --bind 127.0.0.1:0
```

Port `0` lets the operating system choose an available local port.

For bounded background supervision, use the forge operator commands:

    forge daemon start /path/to/project
    forge daemon status /path/to/project
    forge daemon restart /path/to/project
    forge daemon stop /path/to/project

start waits for a verified loopback endpoint. restart performs a cooperative stop followed by
start. Neither command kills a PID merely because it appears in daemon metadata.

## Prepare a task

Worktrees are explicit operator actions. Create and inspect the deterministic task worktree before
starting a run:

```bash
forge worktree create /path/to/project P2-M006-T0001 HEAD
forge worktree inspect /path/to/project P2-M006-T0001
forge worktree list /path/to/project
```

The manager verifies the repository root, exact base commit, task branch, and managed path. It will
not adopt an unrelated worktree or silently repair an unsafe state.

For a foreground pilot, `forge task launch` composes those preparation checks with the direct
persisted process path. It is intentionally not a daemon request and does not change daemon
serialization or lifecycle behavior:

```bash
forge task launch /path/to/project P2-M015-T0001 /absolute/path/to/agent --base HEAD
forge task launch /path/to/project P2-M015-T0001 --profile local-agent
```

The command creates or verifies the task-owned worktree, records the observation, and then runs one
foreground task. Acceptance, review, integration, and retirement remain explicit. Use the daemon
commands below when detached captured execution is preferred.

## Operate

```bash
forge daemon status /path/to/project
forge daemon run /path/to/project P2-M005-T0001 /absolute/path/to/agent
forge daemon run /path/to/project P2-M005-T0001 --profile local-agent
forge daemon launch /path/to/project P2-M016-T0001 /absolute/path/to/agent --base HEAD
forge daemon launch /path/to/project P2-M016-T0002 --profile local-agent --base HEAD
forge daemon stop /path/to/project
```

`run` requires an existing task snapshot, a prepared managed worktree, and verified task-linked
approval evidence. Successful agent execution remains `Running` until an independent operator
accepts it:

```bash
forge task accept /path/to/project P2-M005-T0001 --actor operator
```

`launch` is the detached counterpart to the foreground pilot. It validates the ready task,
capabilities, approvals, repository, and exact base before creating or reusing the deterministic
managed worktree. It records a `WorktreeObserved` event and then executes through the same bounded
persisted process path. Repeated launch requests verify and reuse an owned clean worktree; dirty,
unresolved, ambiguous, or unrelated worktrees fail closed. `run` retains its existing prepared-
worktree behavior for compatibility.

The daemon-backed CLI dogfooding path uses a direct executable fixture and is covered by the
Linux, macOS, and Windows platform matrix. It does not depend on shell syntax or ambient process
environment behavior.

After an accepted task is no longer needed, retire its clean managed worktree explicitly:

```bash
forge worktree retire /path/to/project P2-M005-T0001
```

Stopping is cooperative. The daemon does not force-kill an active child, remove worktrees, delete
task branches, or accept tasks.

## Recovery and limits

Requests and responses use versioned, newline-framed records with bounded size. Unknown commands,
malformed endpoint metadata, non-loopback addresses, invalid task IDs, and non-absolute executable
paths fail closed.

Project-local profiles are validated before a daemon run and use direct executable arguments; see
AGENT_PROFILES.md for the bounded configuration format.

If a daemon crashes, its endpoint and lock metadata are intentionally not adopted automatically.
Inspect the project and confirm no daemon process owns it before removing stale metadata manually.
This preserves evidence and avoids accidentally starting two runtimes for one project.

A client disconnect does not stop the daemon. The request is bounded and the daemon remains
available for status or a later cooperative stop; any execution already started retains its durable
task and audit evidence.

The accept loop reads one request frame per connection, and each connection must deliver its frame
within one second. A client that connects and sends nothing, or stalls mid-frame, receives a
best-effort error and is dropped; it cannot block a later status request or cooperative stop.
For control requests (`status`, `stop`), an elapsed client socket timeout is classified as a stale
endpoint on every platform (Unix reports it as `WouldBlock`, Windows as `TimedOut`).

While a stop is observed, a status poll can connect into the listen backlog just as the daemon
drops its listener. macOS then rejects the client's socket-timeout call on the reset connection with
`EINVAL` (`Invalid argument`), where Linux accepts it. The client treats that, like a connection
reset, as the endpoint going away and keeps observing the stop (P2-M032).

## Remote-worker lease expiry

Since P4-M004 the daemon also expires due remote-worker leases every 5 seconds, recorded as
`LeaseRecorded` audit events with actor `forged`. A sweep holds the execution slot for the few
milliseconds it takes, and only when the slot is free. When this was added, an execution's own
audit handle made a concurrent append unsafe. Since P0-M013 audit appends are coordinated
(ADR-0047), and the slot rule is kept so sweeps stay out of executions' way. `stop` and new executions wait out an
in-progress sweep instead of being refused. Since P4-M006 the same tick also runs one automatic dispatch pass when
`.forge/dispatch.conf` enables it (ADR-0049). Daemon `run` and `launch` refuse leased tasks like the
direct commands do (see [REMOTE_WORKERS.md](REMOTE_WORKERS.md#operating-leases)).

## Long-running executions

`daemon run` and `daemon launch` run the agent, and since P1-M004 its gates, before they respond,
so they can take far longer than the 2-second control timeout. Since P2-M024:

- **One execution at a time.** The accept loop hands each execution to a worker thread that holds
  the daemon's single execution slot. The accept loop itself never waits on an agent, so
  `forge daemon status` answers immediately during a run.
- **Keepalives.** While the execution runs, the worker sends an `AFD1 OK PENDING` frame every
  second, then the final response. The client reads frames with a 10-second idle bound instead of a
  total deadline. The agent's own profile timeout bounds the run itself.
- **Busy requests.** A second `run` or `launch` during an execution is refused immediately with
  `daemon is busy: executing task <id>; retry after it finishes`. Nothing is changed for the refused
  task.
- **Stop during a run.** `forge daemon stop` during an execution is refused with `daemon is busy:
  executing task <id>; stop after it finishes`. The daemon never exits while an agent or its state
  writes are in progress. Stop again once the run completes.
- **Lost connection.** If the daemon stops responding after accepting an execution, the client
  reports `daemon stopped responding during execution` and points to `forge task inspect` and
  `forge daemon status`. It never suggests removing daemon metadata, because the daemon may still be
  running the task.
- **Client disconnects.** Closing the client (for example with Ctrl-C) does not stop the execution.
  The worker keeps running and still persists the task state and audit evidence.

`forge` and `forged` from the same build are required: an older `forge` rejects the `PENDING`
frame as malformed.

Teardown removes the endpoint before the lock. A cooperative `stop` (and therefore `restart`)
returns only after both files are gone, so an immediate restart never collides with the previous
daemon's lock.

Adapter failures, timeouts, and interrupted requests persist the task transition and audit evidence
before the error is returned whenever the persistence boundary remains available. A successful run
does not grant authority to accept, merge, or clean up the task.

Each persisted daemon execution appends to the existing verified audit chain, continuing its next
sequence and prior digest. Existing audit records are never rewritten or discarded; a corrupt or
incompatible audit file still fails closed.

The direct command remains available when no daemon is wanted:

```bash
forge run /path/to/project P2-M005-T0001 /absolute/path/to/agent
```
