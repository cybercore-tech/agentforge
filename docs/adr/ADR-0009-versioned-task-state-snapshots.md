# ADR-0009: Versioned task-state snapshots behind a storage boundary

- Status: Accepted
- Date: 2026-09-19
- Decision owners: AgentForge project

## Context

AgentForge needs durable local task/dependency state before it can schedule work or recover after a
daemon restart.

The task graph is domain state. Filesystem/database mechanics are infrastructure.

Coupling the core graph directly to SQLite, JSON, or another storage technology would make a
persistence choice part of the orchestration contract.

## Decision

AgentForge separates task-graph semantics from persistence.

`agentforge-core` owns task identity, dependency validation, lifecycle transitions, revisions, and
derived readiness.

A separate `agentforge-state` crate owns persistence.

P0-M004 defines a `TaskStore`-style storage boundary and ships a project-local file backend using a
versioned binary snapshot format.

The version-1 codec is private infrastructure and uses only the Rust standard library.

Snapshots are deterministic, bounded during decode, and revalidated through the core graph before
being accepted.

Readiness is derived and is not persisted.

## Consequences

Core orchestration remains independent from any database implementation.

The initial backend stays dependency-free and locally usable offline.

A future SQLite or other backend can be introduced behind the storage boundary without changing
task-graph semantics.

AgentForge must maintain explicit snapshot-version compatibility behavior and corruption tests.

## References

- `PROJECT_SPEC.md`
- `docs/TASK_STATE.md`
- `ADR-0003`
- `ADR-0008`
