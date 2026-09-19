# AgentForge Task Graph and Durable State

P0-M004 defines the first durable orchestration state owned by AgentForge.

## Domain ownership

`agentforge-core` owns task identity, lifecycle semantics, dependencies, graph validation, and
readiness.

Persistence is not a core-domain concern.

`agentforge-state` stores and restores valid core graphs.

## Deterministic task identity

AgentForge-generated task IDs use:

```text
<milestone>-TNNNN
```

For example:

```text
P0-M004-T0001
```

The sequence is explicit and non-zero. The graph never silently creates random IDs.

## Dependency semantics

A valid graph:

- has unique task IDs;
- has no self dependency;
- resolves every dependency to another graph task;
- has no dependency cycle.

Task iteration is ordered by task ID so diagnostics, persistence, and tests are reproducible.

## Lifecycle

P0-M004 lifecycle states are expected to include:

- Pending
- Running
- Succeeded
- Failed
- Blocked
- Cancelled

Ready is deliberately derived rather than stored.

A pending task is ready only when all dependencies succeeded.

Terminal states in this milestone are Succeeded and Cancelled. Failed and Blocked can explicitly
return to Pending for retry/resume.

## Revision

Each accepted lifecycle mutation increments the task record revision exactly once.

This gives later schedulers and audit logic a deterministic local change counter without requiring
timestamps for correctness.

## Persistence

P0-M004 uses a versioned binary snapshot format behind a storage abstraction.

The initial project-local file backend stores task state beneath:

```text
.forge/state/
```

The exact binary encoding is private implementation detail. Its schema version is public persistence
metadata and is independent from the AgentTask semantic contract version.

Decoding is bounded and fail-closed: bad magic, unknown versions, malformed lengths, truncation,
trailing corruption, or domain-invalid restored graphs are errors.

## Future evolution

A future SQLite or remote backend may implement the same storage boundary.

Changing the persistence engine must not require changing task-graph semantics.
