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

## Operate

```bash
forge daemon status /path/to/project
forge daemon run /path/to/project P2-M005-T0001 /absolute/path/to/agent
forge daemon run /path/to/project P2-M005-T0001 --profile local-agent
forge daemon stop /path/to/project
```

`run` requires an existing task snapshot, a prepared managed worktree, and verified task-linked
approval evidence. Successful agent execution remains `Running` until an independent operator
accepts it:

```bash
forge task accept /path/to/project P2-M005-T0001 --actor operator
```

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

Adapter failures, timeouts, and interrupted requests persist the task transition and audit evidence
before the error is returned whenever the persistence boundary remains available. A successful run
does not grant authority to accept, merge, or clean up the task.

The direct command remains available when no daemon is wanted:

```bash
forge run /path/to/project P2-M005-T0001 /absolute/path/to/agent
```
