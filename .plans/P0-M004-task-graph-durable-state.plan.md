# Plan: P0-M004 — Task graph and durable state

Status: Approved
Milestone: P0-M004
Created: 2026-09-19

## Goal

Give AgentForge a deterministic task graph whose tasks, dependencies, lifecycle state, and revisions
survive process restarts.

P0-M004 establishes the first durable orchestration state owned by AgentForge itself.

The milestone must provide:

- stable deterministic task identifiers;
- a dependency graph with duplicate, missing-dependency, self-dependency, and cycle rejection;
- explicit task lifecycle transitions;
- derived readiness based on dependency completion;
- deterministic graph iteration order;
- a versioned durable snapshot format;
- a storage abstraction separating domain semantics from file persistence;
- a project-local file backend under `.forge/state/`;
- exact round-trip and corruption tests.

## Non-goals

- No scheduler or concurrent executor.
- No worktree creation or cleanup.
- No external coding-agent adapter.
- No MCP integration.
- No GitHub task synchronization.
- No database server.
- No SQLite dependency in P0-M004.
- No audit/event-log replacement for P0-M009.
- No distributed locking.
- No production migration framework.
- No CLI task-management UX beyond minimal smoke/debug support if required by tests.

## Context

P0-M002 introduced provider-neutral `AgentTask` contracts, including stable task identifiers and
dependency identifiers.

P0-M003 made the development workflow mechanically enforceable and added independent remote CI.

The next orchestration primitive is durable task state. AgentForge needs to know what work exists,
which tasks depend on which other tasks, which tasks are runnable, and what happened after a daemon
restart without consulting a model conversation.

`PROJECT_SPEC.md` requires durable state to live in files/databases rather than model memory and
defines project-local `.forge/` state as part of the product shape.

## Architecture placement

```text
agentforge-core
├── AgentTask / AgentResult
└── task graph domain
    ├── TaskId
    ├── TaskState
    ├── TaskRecord
    ├── TaskGraph
    └── graph validation / transitions

agentforge-state
├── TaskStore trait implementation boundary
├── versioned binary snapshot codec
└── FileTaskStore
    └── .forge/state/tasks.snapshot
```

`agentforge-core` owns meaning.

`agentforge-state` owns persistence mechanics.

No persistence-specific type may become required by the core task-graph API.

## Data flow

### Create graph

1. Construct or receive an `AgentTask`.
2. Validate its contract.
3. Convert its task identifier into validated `TaskId`.
4. Add the task to `TaskGraph`.
5. Validate dependency references and graph acyclicity.
6. Persist the graph through a `TaskStore`.

### Restore graph

1. Open the project-local state file.
2. Validate snapshot magic and schema version.
3. Decode bounded length-prefixed records.
4. Reconstruct `AgentTask` values and lifecycle state.
5. Rebuild `TaskGraph`.
6. Re-run graph invariants before returning the restored state.

### Readiness

`Ready` is not persisted as an independent lifecycle state.

A pending task is ready when every dependency exists and is in `Succeeded`.

This prevents durable state from containing contradictory combinations such as "ready" while a
dependency is failed or pending.

## Invariants

- Task IDs are stable values, never random UUIDs generated implicitly by the graph.
- AgentForge-provided deterministic IDs use the form `<milestone>-TNNNN`, for example
  `P0-M004-T0001`.
- Sequence zero is invalid.
- Duplicate task IDs are rejected.
- A task may not depend on itself.
- Every dependency must refer to another task in the graph before the graph is accepted/persisted.
- Dependency cycles are rejected.
- Iteration and persistence order are deterministic by task ID.
- Readiness is derived, not stored.
- A task can run only when all dependencies succeeded.
- `Succeeded` and `Cancelled` are terminal in P0-M004.
- Retry is explicit through a transition back to `Pending` from `Failed` or `Blocked`.
- Every accepted lifecycle mutation increments a per-task revision.
- Snapshot schema version is independent of `AGENT_CONTRACT_VERSION`.
- Snapshot decoding has explicit size/count bounds before allocation.
- Unsupported snapshot versions fail closed.
- Truncated, malformed, or trailing-corruption snapshots fail with controlled errors.
- A loaded snapshot is not trusted until domain validation passes.
- `agentforge-core` has no filesystem dependency.
- `agentforge-state` depends on core, never the reverse.
- Saving uses a same-directory temporary file, file sync, then rename on the supported local
  platform path.
- No external Rust dependency is introduced in P0-M004.

## ADRs

Existing:

- ADR-0001 — model-agnostic orchestration.
- ADR-0003 — durable project state outside model memory.
- ADR-0008 — versioned structured task and result contracts.

New:

- ADR-0009 — versioned task-state snapshots behind a storage boundary.

## Public API / CLI

Expected core API shape:

```text
TaskId
TaskState
TaskRecord
TaskGraph
TaskGraphError
TaskTransitionError
```

Expected storage API shape:

```text
TaskStore
FileTaskStore
StateError
SNAPSHOT_VERSION
```

Names may vary modestly during implementation if Rust ergonomics require it, but responsibilities
must remain separate.

No user-facing stable CLI contract is required in P0-M004.

## Compatibility analysis

- Workspace MSRV remains Rust 1.85.0.
- Snapshot format starts at schema version 1.
- The decoder must reject unknown future versions rather than guessing.
- `AgentTask.contract_version` remains separately validated.
- Snapshot schema evolution must not silently reinterpret older task semantics.
- Path format remains project-local and platform-neutral at the API layer.

## Dependency analysis

P0-M004 is intentionally standard-library-only.

Reasons:

- the current state model is small;
- a compact versioned binary codec is straightforward to bound and test;
- no serde/SQLite dependency is needed to prove the persistence contract;
- keeping storage behind `TaskStore` preserves the option to add SQLite or another backend later
  without changing core graph semantics.

The cost is maintaining a small explicit codec. That codec is intentionally private to
`agentforge-state` and covered by round-trip/corruption tests.

## Expected file boundary

Plan checkpoint:

- `.plans/ACTIVE`
- `.plans/P0-M004-task-graph-durable-state.plan.md`
- `docs/MILESTONES.md`
- `docs/TASK_STATE.md`
- `docs/adr/README.md`
- `docs/adr/ADR-0009-versioned-task-state-snapshots.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

Implementation checkpoint:

- workspace `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-core/src/lib.rs`
- `crates/agentforge-core/src/task.rs`
- `crates/agentforge-state/Cargo.toml`
- `crates/agentforge-state/src/lib.rs`
- focused tests under those crates
- documentation/state files only when required for exact implementation evidence.

No existing CLI, daemon, workflow, hook, or policy file should require semantic changes.

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| Deterministic ID for P0-M004 sequence 1 | `P0-M004-T0001` |
| Sequence zero | reject |
| Empty/invalid task ID | reject |
| Duplicate task ID | reject |
| Missing dependency | reject graph |
| Self dependency | reject graph |
| Two-node cycle | reject graph |
| Longer cycle | reject graph |
| Acyclic graph | accept |
| Pending task with no dependencies | ready |
| Pending task with all dependencies succeeded | ready |
| Pending task with pending/failed/blocked dependency | not ready |
| Transition Pending -> Running when ready | accept |
| Transition Pending -> Running when dependency incomplete | reject |
| Running -> Succeeded | accept |
| Running -> Failed | accept |
| Failed -> Pending | accept retry |
| Blocked -> Pending | accept resume |
| Succeeded -> Running | reject |
| Cancelled -> Pending | reject |
| Accepted transition | revision increments exactly once |
| Graph iteration | deterministic task-ID order |
| Snapshot save/load | exact semantic round trip |
| Snapshot task order | deterministic bytes for same graph |
| Bad magic | controlled failure |
| Unsupported schema version | controlled failure |
| Truncated snapshot | controlled failure |
| Oversized declared string/count | controlled failure before allocation |
| Trailing unexpected bytes | controlled failure |
| Restored graph with invalid dependencies | controlled failure |
| Re-save existing state | latest valid snapshot loads |
| Save creates parent state directory | success |
| Core crate filesystem usage | none |

## Implementation sequence

1. Commit this Approved plan-only checkpoint.
2. Require exact plan-head GitHub Actions green.
3. Add task-graph domain tests and types in `agentforge-core`.
4. Add `agentforge-state` workspace crate and path dependency on core.
5. Implement private version-1 binary codec with bounded reads.
6. Implement same-directory temporary-file save, sync, and replace semantics.
7. Add round-trip, corruption, determinism, and transition tests.
8. Run local full gate when available.
9. Inspect exact implementation diff.
10. Push one implementation checkpoint.
11. Require all four CI jobs green on that exact head.
12. Classify every failure before repair and allow independent jobs to finish.
13. Repair only the classified category.
14. Close P0-M004 in docs/state only.
15. Require exact closure CI green.
16. Merge with history preserved.
17. Require post-merge `main` CI green.
18. Do not begin P0-M005 before post-merge evidence is green.

## Failure modes

- Persisting derived readiness and allowing it to drift from dependency state.
- Allowing dependencies that do not exist.
- Checking cycles only when tasks are inserted and missing cycles after restore.
- Reusing `String` everywhere without validating task identity.
- Implicit random ID generation.
- Parsing untrusted lengths directly into allocations.
- Treating partial/truncated snapshots as valid.
- Letting storage types leak into the core graph.
- Coupling P0-M004 to SQLite or a model provider.
- Saving nondeterministic map iteration order.
- Mutating state without incrementing revision.
- Allowing terminal states to silently restart.
- Weakening CI or policy checks to land the milestone.

## Documentation impact

- Add `docs/TASK_STATE.md` as the semantic description of graph/state behavior.
- Add ADR-0009 for the persistence boundary and snapshot choice.
- Update project state/handoff during milestone activation and closure.

## Quality gates

Local:

- `./scripts/check-text-files`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo run -p xtask --locked -- validate`
- `cargo run -p xtask --locked -- validate-plan-policy`

Remote:

- Repository policy
- Stable code gate
- MSRV 1.85.0
- CLI smoke

## Acceptance criteria

- [ ] Deterministic task IDs are implemented and tested.
- [ ] Task graph rejects duplicate IDs.
- [ ] Task graph rejects missing dependencies.
- [ ] Task graph rejects self dependencies.
- [ ] Task graph rejects cycles.
- [ ] Graph ordering is deterministic.
- [ ] Readiness is derived from dependency completion.
- [ ] Lifecycle transitions are explicit and validated.
- [ ] Accepted lifecycle transitions increment task revision.
- [ ] Core graph has no filesystem/storage dependency.
- [ ] A storage abstraction separates domain and persistence.
- [ ] Version-1 durable snapshot format is implemented.
- [ ] Snapshot decoding is bounded and fail-closed.
- [ ] Snapshot save/load round trip is exact.
- [ ] Snapshot bytes are deterministic for the same graph.
- [ ] Corruption/truncation tests exist.
- [ ] File store writes project-local state under `.forge/state/` by default.
- [ ] No external Rust dependency is introduced.
- [ ] Exact implementation CI is green.
- [ ] Exact closure CI is green.
- [ ] Post-merge `main` CI is green.

## Completion record

Implementation commit:
Implementation CI:
Closure commit:
Closure CI:
Post-merge main:
Post-merge CI:
Completed:
Notes:
