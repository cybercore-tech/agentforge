# Plan: P0-M006 — Agent adapter interface

Status: Draft
Milestone: P0-M006
Created: 2026-09-19

## Goal

Define a provider-neutral execution interface and a configurable local-process adapter that can
invoke an operator-selected coding-agent executable inside a verified task worktree.

Preserve AgentForge's existing task/result contracts and keep process completion distinct from
acceptance of an agent's work. Prove the interface offline with a deterministic fixture executable.

## Non-goals

- No bundled provider-specific integration, cloud API, credentials, or paid agent execution.
- No scheduler, concurrent dispatcher, daemon integration, or stable end-user CLI.
- No task-graph transitions or durable state writes.
- No gate engine, CI monitoring, audit/event storage, or automatic retry.
- No worktree creation, retirement, branch deletion, commits, pushes, merges, or deployment by
  the adapter infrastructure.
- No OS sandbox or claim that task path/capability declarations constrain arbitrary child code.
- No detached/background agent processes or process-tree supervision.
- No third-party Rust dependencies or changes to P0-M004 persistence semantics.

## Context

P0-M002 supplied versioned `AgentTask` and `AgentResult` semantic contracts. P0-M004 supplied
validated task identity and durable state. P0-M005 supplied managed worktree inspection.

Validated base main: `069c058f7d39ab39a3267f5893d20b963d4f5397`.
P0-M005 closure CI: `35481797256` for `259f4c06844898c7cd99aeb29b38f23f5688de3a`.
P0-M005 post-merge CI: `35481855409` for the base main SHA; both runs passed all four jobs.

The existing `AgentTask.task_id` is a string. Validate it with `TaskId` at the adapter boundary;
do not change the existing core contract or task-state storage format.

## Architecture placement

- `agentforge-core`: existing task identity and agent task/result semantics.
- `agentforge-worktree`: existing worktree ownership and state inspection.
- New `agentforge-adapter`: synchronous adapter trait, request validation, prompt rendering,
  process configuration, bounded process execution, and execution reports.

The adapter crate depends only on the two existing workspace crates and the standard library.
Keep process management out of `agentforge-core`. Use public worktree APIs; report discovered
worktree defects separately rather than broadening this implementation silently.

## Data flow

1. Caller supplies an `AgentTask`, repository manager, explicit approval acknowledgements, and
   operator-controlled executable configuration with timeout and output limits.
2. Validate the task contract, parse task identity, and require its milestone to match the ID.
3. Require `RunLocalCommands`. Require acknowledgements for every declared human approval;
   caller-supplied acknowledgements represent operator authorization, never agent output.
4. Re-inspect the task through `WorktreeManager`; require matching ownership, a clean worktree,
   and no unresolved Git operation. Do not accept an arbitrary caller-supplied working directory.
5. Render every task-contract field deterministically as a versioned, length-delimited UTF-8
   prompt document. Include explicit counts and byte lengths for repeated or multiline values.
   This is adapter input, not a new persistent task-state encoding.
6. Invoke the configured executable directly with an argument vector, set the verified worktree
   as its working directory, and deliver the rendered task through stdin.
7. Drain stdout and stderr concurrently, apply a shared output-byte budget, and enforce a
   deadline covering stdin delivery and child execution. Avoid serial pipe I/O deadlocks.
8. Return an execution report containing task/adapter identity, starting worktree identity,
   termination reason, optional exit code, captured bytes, and truncation indicators.
9. The caller reviews process evidence and any separately reported `AgentResult`. Validate
   result contract version and task binding before accepting it as an adapter result. Neither a
   result claiming `Completed` nor a successful exit grants authority or transitions task state.

## Invariants

- A failed preflight never spawns the executable.
- Task text never selects the executable, changes arguments, or passes through a shell.
- Configuration uses an absolute executable path and explicit arguments and environment values.
- Child environment is cleared and rebuilt from explicit operator configuration; no ambient
  Git index, repository override, credential, or token variables are inherited implicitly.
- Environment configuration is not evidence of permission to use secrets or network access.
- Every required approval acknowledgement must come from the caller before launch.
- Capture remains bounded even if the child floods both output streams.
- Timeout/output-limit/error paths terminate and reap the direct child and preserve captured
  evidence; no unbounded stdin writer or output-reader join is permitted for supported children.
- Supported executables must stay in the foreground and must not leave descendants holding
  adapter pipes. This is an explicit process model, not a process-tree containment guarantee.
- Zero exit status means only that the process exited successfully; raw stdout is not implicitly
  parsed or trusted as an `AgentResult`.
- Adapter execution never claims enforcement of filesystem scope or network isolation.
- Tests do not mutate the parent process environment or use fixed shared temporary paths.
- Failure leaves the task worktree and branch available for inspection.

## ADRs

Follow ADR-0001, ADR-0002, ADR-0007, ADR-0008, and ADR-0010.
Proposed ADR-0011 records the adapter/process boundary and the distinction between process
evidence, agent claims, and orchestrator acceptance.

## Public API / CLI

Expected API shape, with final names settled within this boundary:

- `AgentAdapter`: object-safe synchronous execution interface and stable adapter identity.
- `AdapterRequest`: borrowed task, repository manager, and explicit approval acknowledgements.
- `ProcessAdapterConfig`: executable, argument vector, explicit environment, timeout, byte limits.
- `ProcessAdapter`: foreground executable implementation of `AgentAdapter`.
- `ExecutionReport` / `ExecutionTermination`: bounded evidence and exit/timeout/output-limit state.
- `AdapterError`: invalid request/configuration, ownership/approval failure, spawn and I/O errors.
- Task-prompt rendering and result-binding validation helpers.

No provider name or provider-specific flags belong in the trait. No new CLI command is required.
The versioned prompt layout must be documented before its implementation and pinned by fixtures.

## Compatibility analysis

Rust MSRV remains 1.85.0. Existing core types, snapshots, branches, and worktree paths stay intact.
Process capture preserves raw bytes, including invalid UTF-8. Provider executables may need an
operator-supplied wrapper to consume the stdin prompt and stay within the foreground-process model.
Native provider adapters can later implement the same trait without changing orchestration types.

## Dependency analysis

No external Rust dependency. Add only workspace path dependencies on `agentforge-core` and
`agentforge-worktree`. No provider installation or external service is needed for validation.

## Expected file boundary

Plan and closure checkpoints:

- `.plans/P0-M006-agent-adapter-interface.plan.md`
- `.plans/ACTIVE` — created only when the plan is approved; removed at closure
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`
- `docs/MILESTONES.md`
- `docs/AGENT_ADAPTERS.md`
- `docs/adr/ADR-0011-agent-adapter-execution-boundary.md`
- `docs/adr/README.md`

Implementation checkpoint:

- `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-adapter/Cargo.toml`
- `crates/agentforge-adapter/src/**`
- `crates/agentforge-adapter/tests/**`
- `crates/agentforge-adapter/examples/**` — deterministic fixture executable only
- `docs/AGENT_ADAPTERS.md` — precise prompt/API behavior and limitations

No edits to core, state, worktree, CLI/daemon, hooks, gates, or CI without a prior plan amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Adapter interchangeability | Fake and process adapters exercise the same trait through `dyn` |
| Invalid version/task ID/milestone | Rejected before fixture can create a spawn marker |
| Missing capability or approval acknowledgement | Rejected before spawn |
| Missing, dirty, mismatched, or unresolved worktree | Preflight refuses execution |
| Task prompt | All contract fields retained; deterministic framing handles multiline/Unicode text |
| Child working directory | Fixture reports the canonical task-owned worktree |
| Argument handling | Spaces and shell metacharacters reach fixture literally |
| Environment isolation | Fixture sees explicit values; inherited Git overrides are absent |
| Successful process exit | Report preserves task ID and bytes without asserting task completion |
| Nonzero exit or spawn failure | Controlled failure evidence; worktree remains available |
| Stdout and stderr flood | Bounded capture and termination without pipe deadlock |
| Timeout and blocked stdin | Direct child is reaped; caller receives bounded-time failure |
| Invalid UTF-8 output | Bytes retained without panic or lossy decoding |
| Result binding | Wrong task/version rejected; valid worker report remains an unaccepted claim |
| Concurrent tests | Unique atomically allocated fixture directories; no global environment mutation |

## Implementation sequence

1. Review this Draft plan and proposed ADR; obtain human approval to activate implementation.
2. Commit `Status: Approved`, the active-plan pointer, and activation documents separately from code.
3. Run the full local gate and require all four remote jobs green for that exact plan commit.
4. Implement adapter contracts, preflight, and deterministic prompt tests.
5. Add the foreground fixture executable and process-adapter integration tests.
6. Implement bounded execution and failure handling; validate each test-matrix row.
7. Run the full local gate and inspect the diff against the declared file boundary.
8. Commit implementation separately and validate that exact commit remotely.
9. Classify and repair failures forward; retain failure evidence.
10. Close the milestone in a separate documentation checkpoint, marking only observed checks done.
11. Require CI success for the closure commit, merge with history preserved after authorization,
    and verify CI for the resulting main commit before P0-M007.

## Failure modes

- Treating process exit or worker text as proof of accepted completion.
- Launching in the primary repository or using stale worktree evidence.
- Shell interpolation, inherited Git overrides, or implicit credentials.
- Blocking on stdin while the child blocks on a full output pipe.
- Unbounded capture or timeout cleanup that hangs waiting for inherited descendant pipes.
- Passing task policy text off as an OS security boundary.
- Silently adding a provider SDK, changing core semantics, or inventing approval grants.
- Checking closure or post-merge acceptance boxes before those runs finish.

## Documentation impact

Document the adapter contract, process model, prompt format, error behavior, and security limits.
Update durable milestone state with exact commit/run evidence at each checkpoint.

## Quality gates

Run `./scripts/gate.sh full` (text policy, repository validation, plan policy, formatting,
workspace check, Clippy with warnings denied, and locked workspace tests).
Run focused adapter tests and fixture stress cases for pipe, timeout, and isolation behavior.
Require Repository policy, Stable code gate, MSRV 1.85.0, and CLI smoke green for the exact
approved-plan, implementation, closure, and post-merge commit SHAs.

## Acceptance criteria

- [ ] Approved plan committed and validated before implementation.
- [ ] Provider-neutral trait and request/report types are implemented.
- [ ] Preflight validates task, capability, approval, and current worktree ownership/state.
- [ ] Configurable local executable receives complete deterministic task input via stdin.
- [ ] Child arguments and environment are explicit and shell-free.
- [ ] Output limits, concurrent draining, deadline, and direct-child reaping are tested.
- [ ] Process exit and task acceptance are distinct; result binding is validated.
- [ ] Offline fixture tests cover every row of the matrix.
- [ ] No external Rust dependency or core/persistence change is introduced.
- [ ] Exact implementation local gate and remote CI are green.
- [ ] Exact closure CI is green.
- [ ] Post-merge main CI is green.

## Completion record

Implementation commit: pending
Implementation CI: pending
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
Completed: pending
Notes: Draft planning checkpoint; implementation authority has not been activated.
