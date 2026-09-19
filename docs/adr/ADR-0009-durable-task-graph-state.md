# ADR-0009: Durable task graph state

- Status: Accepted
- Date: 2026-09-19

## Context

AgentForge requires task dependencies and lifecycle state to survive process restart.

Process memory and model context are not durable project state.

## Decision

Task graph semantics live in agentforge-core.

Persistence lives in a separate agentforge-state crate.

agentforge-core owns deterministic task identity, lifecycle state, task records, dependency graph
behavior, cycle detection, validation, and readiness.

agentforge-state owns snapshots, serialization, versioning, and local filesystem persistence.

agentforge-core performs no persistence I/O.

## Consequences

Task semantics remain storage-independent and independently testable.

Future storage backends can be introduced without rewriting the graph model.

Database selection, remote persistence, scheduling, and distributed coordination remain deferred.
