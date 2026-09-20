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

## Operate

```bash
forge daemon status /path/to/project
forge daemon run /path/to/project P2-M005-T0001 /absolute/path/to/agent
forge daemon stop /path/to/project
```

`run` requires an existing task snapshot, a prepared managed worktree, and verified task-linked
approval evidence. Successful agent execution remains `Running` until an independent operator
accepts it:

```bash
forge task accept /path/to/project P2-M005-T0001 --actor operator
```

Stopping is cooperative. The daemon does not force-kill an active child, remove worktrees, delete
task branches, or accept tasks.

## Recovery and limits

Requests and responses use versioned, newline-framed records with bounded size. Unknown commands,
malformed endpoint metadata, non-loopback addresses, invalid task IDs, and non-absolute executable
paths fail closed.

If a daemon crashes, its endpoint and lock metadata are intentionally not adopted automatically.
Inspect the project and confirm no daemon process owns it before removing stale metadata manually.
This preserves evidence and avoids accidentally starting two runtimes for one project.

Adapter failures, timeouts, and interrupted requests persist the task transition and audit evidence
before the error is returned whenever the persistence boundary remains available. A successful run
does not grant authority to accept, merge, or clean up the task.

The direct command remains available when no daemon is wanted:

```bash
forge run /path/to/project P2-M005-T0001 /absolute/path/to/agent
```
