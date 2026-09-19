# Plan: P0-M004 — Task graph and durable state

Status: Approved
Milestone: P0-M004
Created: 2026-09-19

## Goal

Implement AgentForge's first deterministic durable task graph.

P0-M004 establishes deterministic task IDs, lifecycle state, dependency relationships, DAG
validation, cycle detection, deterministic readiness queries, versioned local persistence, and
restart-safe state loading.

## Non-goals

P0-M004 does not implement scheduling, concurrent worker execution, Git worktree management,
external model adapters, CI monitoring, audit/event logging, MCP, deployment, or remote/distributed
storage.

## Architecture

Task-domain behavior remains in agentforge-core.

Persistence is isolated in a new agentforge-state crate.

Flow:

    AgentTask
        |
        v
    TaskRecord
        |
        v
    TaskGraph
        |
        +-- dependency validation
        +-- cycle detection
        +-- lifecycle state
        +-- readiness
        |
        v
    StateSnapshot
        |
        v
    StateStore
        |
        v
    local project state

agentforge-core must perform no filesystem I/O.

## Deterministic task identity

Task identity derives from:

- algorithm version;
- milestone ID;
- stable task key.

It must not depend on randomness, timestamps, process IDs, insertion order, model/provider identity,
or mutable goal prose.

Canonical IDs have this shape:

    task-0123456789abcdef

Fixed known-vector tests must prevent accidental identity drift.

## Lifecycle

Initial durable task states:

- Pending
- Ready
- Running
- Blocked
- Completed
- Failed
- Cancelled

Terminal states:

- Completed
- Failed
- Cancelled

Lifecycle state does not grant authority or capabilities.

## Dependency graph

If B depends on A:

    A -> B

B is graph-ready only when A is Completed.

The graph must reject duplicate tasks, self dependencies, missing dependencies, cycles, and
deterministic-ID mismatches.

Traversal and readiness results must be deterministic.

## Readiness

A task is graph-ready only when the graph is valid, the task is not terminal or already running,
all dependencies exist, and all dependencies are Completed.

P0-M004 does not schedule or execute ready tasks.

## Persistence

P0-M004 introduces:

    crates/agentforge-state

The state crate owns snapshots, serialization, deserialization, format validation, and local
filesystem persistence.

Project-local state lives under:

    .forge/state/

Writes must publish atomically through temporary-file creation followed by rename.

Malformed or incompatible authoritative state must fail explicitly.

Loaded state must be graph-validated before use.

## Expected implementation boundary

- Cargo.toml
- Cargo.lock
- crates/agentforge-core/src/lib.rs
- crates/agentforge-core/src/task_graph.rs
- crates/agentforge-state/**
- focused tests
- project-state and handoff evidence

## Test matrix

Identity:

- same milestone/key produces the same ID;
- different milestone produces a different ID;
- different key produces a different ID;
- mutable goal text does not change identity;
- fixed vectors remain stable.

Graph:

- valid DAG succeeds;
- duplicate task fails;
- self dependency fails;
- missing dependency fails;
- two-node cycle fails;
- longer cycle fails;
- traversal is deterministic.

Readiness:

- root task can become ready;
- incomplete dependency blocks readiness;
- completed dependency permits readiness;
- failed dependency does not satisfy readiness;
- cancelled dependency does not satisfy readiness;
- running and terminal tasks are not returned as newly ready.

Persistence:

- empty graph round-trips;
- populated graph round-trips;
- lifecycle state round-trips;
- dependencies round-trip;
- equivalent state serializes deterministically;
- malformed input fails;
- unsupported format version fails;
- loaded graph is revalidated.

## Implementation sequence

1. Add deterministic task identity.
2. Add task lifecycle state.
3. Add task records.
4. Implement DAG insertion and validation.
5. Implement cycle detection.
6. Implement deterministic traversal.
7. Implement readiness queries.
8. Add core regression tests.
9. Add agentforge-state.
10. Add state snapshot format.
11. Add deterministic serialization.
12. Add atomic local persistence.
13. Add restart and malformed-state tests.
14. Run full repository gate.
15. Record exact implementation evidence.
16. Close P0-M004.

## Acceptance criteria

- [ ] P0-M003 is complete.
- [ ] Plan commit is separate from implementation.
- [ ] Exact plan checkpoint passes full gate.
- [ ] Deterministic task IDs exist.
- [ ] Fixed identity vectors are tested.
- [ ] Task lifecycle exists.
- [ ] Invalid dependency graphs are rejected.
- [ ] Cycles are rejected.
- [ ] Readiness semantics are deterministic.
- [ ] agentforge-core performs no persistence I/O.
- [ ] agentforge-state exists separately.
- [ ] Durable state is versioned.
- [ ] Complete graph state survives save/load.
- [ ] Malformed state fails safely.
- [ ] Loaded state is revalidated.
- [ ] Exact implementation head passes full validation.
- [ ] Closure removes .plans/ACTIVE.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
