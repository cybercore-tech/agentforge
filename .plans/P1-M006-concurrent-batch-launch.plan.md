# Plan: P1-M006 — Concurrent batch launch of disjoint ready tasks

Status: Complete
Milestone: P1-M006
Created: 2026-09-23
Owner: AgentForge project

## Goal

Let an operator launch every ready task whose owned paths are disjoint in one command, with the
agents running concurrently in their own worktrees. This connects the P1-M001 scheduler, which only
tests use today, to a real execution path, and is the first time AgentForge runs more than one
agent at once.

## Non-goals

- No daemon batch command; the daemon keeps its one-task-per-request protocol.
- No concurrent integration. Integration stays serialized through `forge task integrate`.
- No automatic retry, re-planning, or follow-up batches after tasks finish.
- No remote workers or leases; this is local concurrency only.
- No change to single-task `forge run` or `forge task launch` behavior.

## Context

P1-M001's acceptance signal was "disjoint work can run concurrently", but `plan_batch` is only
exercised by unit tests, and every operator path runs one task. `plan_batch` rejects the whole graph
when any two ready tasks overlap, which is right for a validity check but not for launching: an
overlapping task can wait for a later batch without risk.

The persisted single-task path loads the task snapshot, runs the agent, and saves the snapshot at
the end. Running that path in parallel threads would lose updates, because each thread would save
its own copy of the graph. Concurrent execution therefore needs one coordinator that owns state and
audit persistence, with the worker threads only running processes.

## Architecture placement

- `agentforge-scheduler`: `plan_launch_batch(graph, max)` returns the ready tasks to launch and the
  deferred ones, each with a reason. It is deterministic, in task-ID order, and defers a task on
  path overlap (with an earlier batch task or with any task still `running`) or when `max` is
  reached.
- `agentforge-orchestrator`: `launch_batch_persisted` runs three phases:
  1. **Prepare (sequential):** validate policy, approvals, and gate profiles; resolve the base; create
     or verify worktrees; record `WorktreeObserved`, `TaskTransition`, and `AgentStarted`; persist.
  2. **Execute (parallel):** one scoped thread per task runs the adapter and, after a zero exit,
     the project gates. Threads touch no shared state.
  3. **Finalize (sequential):** apply outcomes in task-ID order (agent evidence, gate evidence,
     failure transitions) and persist once.
  The worktree preparation in `launch_process_persisted` becomes a shared helper.
- `agentforge-platform`: `forge task launch-batch`.

## Data flow

1. `forge task launch-batch <root> (<absolute-executable> | --profile <id>) [--base <ref>]
   [--max <n>]` loads the graph and approvals.
2. `plan_launch_batch` picks disjoint ready tasks (default `max` 4, allowed range 1-16).
3. Gate profiles are loaded once. A malformed profile aborts the batch before any side effect.
4. A task that fails its own preflight (missing capability or approval, dirty or foreign worktree)
   is reported as skipped with the reason, and the other tasks continue.
5. Prepared tasks are persisted as `running` before any agent starts, so a concurrent single-task
   launch of the same task is rejected as not ready.
6. Agents and gates run concurrently. Outcomes are persisted in one final step.
7. The CLI prints one line per launched, skipped, and deferred task, then a summary. It exits 0 when
   every launched task's adapter ran and all gates passed, and 1 otherwise. With no ready tasks it
   prints `no ready tasks` and exits 0.

## Invariants

- Tasks in one batch never have overlapping owned paths, and never overlap a task that is still
  `running`.
- Only the coordinator thread reads or writes the task snapshot and audit log.
- Worktree creation is sequential, so Git worktree metadata is never written concurrently.
- Per-task outcomes match the single-task path: adapter error means `failed`; a gate failure means
  `failed` with gate evidence; success stays `running` until explicit acceptance.
- Planning is deterministic for a given graph.

## ADRs

- ADR-0039: concurrent batch launch uses a single state coordinator with parallel process
  execution.

## Public API / CLI

- `agentforge_scheduler::{plan_launch_batch, LaunchBatch, DeferredTask, DeferReason}`.
- `agentforge_orchestrator::{launch_batch_persisted, BatchLaunch, BatchTaskOutcome}`.
- `forge task launch-batch <root> (<absolute-executable> | --profile <id>) [--base <ref>]
  [--max <n>]`.

## Compatibility analysis

Additive only. `plan_batch`, the single-task paths, the daemon, and on-disk formats are unchanged.

## Dependency analysis

No new external crate. New internal edge `agentforge-orchestrator -> agentforge-scheduler`. The
adapter must be `Sync` to be shared by the scoped threads; `ProcessAdapter` already is.

## Expected file boundary

- `.plans/P1-M006-concurrent-batch-launch.plan.md`
- `.plans/ACTIVE`
- `Cargo.lock`
- `crates/agentforge-scheduler/src/lib.rs`
- `crates/agentforge-orchestrator/Cargo.toml`
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-orchestrator/tests/batch_launch.rs`
- `crates/agentforge-cli/src/main.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs` (rendezvous mode)
- `crates/agentforge-cli/tests/batch_launch_commands.rs`
- `docs/SCHEDULING.md`
- `docs/adr/ADR-0039-concurrent-batch-launch.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- `plan_launch_batch` defers overlapping tasks, respects `max`, excludes non-ready tasks, and is
  deterministic.
- Two disjoint tasks run concurrently. The proof is a rendezvous fixture: each agent waits, with a
  bound, until both agents have started, so serial execution would time out and fail.
- An overlapping third task is deferred and stays `pending`.
- A task missing its approval is skipped, and the others still launch.
- Adapter failure and gate failure in one task produce the same states and evidence as the
  single-task path, without affecting its batch siblings.
- A malformed gate profile aborts the batch before any worktree is created.
- The task snapshot after the batch contains every task's transition, with no lost updates.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Add `plan_launch_batch` with tests.
4. Extract worktree preparation; add `launch_batch_persisted` with orchestrator tests.
5. Add `forge task launch-batch`, the rendezvous fixture mode, and portable CLI tests.
6. Update docs and ADR-0039.
7. Run the full gate, push, and record exact-SHA CI evidence before closure.

## Failure modes

- A crash after Prepare leaves the prepared tasks `running` with `AgentStarted` evidence and no
  finish record. The operator inspects them and cancels or retries them as for any interrupted run.
- A thread panic is converted into an adapter-failure outcome for that task, not a batch abort.

## Documentation impact

`docs/SCHEDULING.md` documents batch planning, deferral, the three phases, and crash behavior.
README lists the command. ADR-0039 records the decision.

## Quality gates

- `./scripts/gate.sh full`;
- exact-SHA CI green on all seven jobs, including the portable concurrency test on Windows and
  macOS.

## Acceptance criteria

- [x] Disjoint ready tasks launch concurrently from one command.
- [x] Overlapping and over-limit tasks are deferred deterministically.
- [x] State and audit persistence has a single owner, with no lost updates.
- [x] Per-task failures are isolated and match single-task semantics.
- [x] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit: `b54f47acbd01ac1fb40da3302cb53b02fbeefda3`
CI run: `35962553757` (push) plus dispatched `35962575356`, `35962581675`, `35962587949`
CI result: green across all seven jobs in all four runs, including the portable rendezvous
concurrency test on Ubuntu, macOS, and Windows.
Completed: 2026-09-23
Notes:
`plan_launch_batch` defers ready tasks that overlap a task already in the batch or still `running`,
and tasks beyond `--max` (default 4, allowed 1-16). `launch_batch_persisted` prepares and persists
tasks sequentially, runs each agent and its gates on a scoped thread, and finalizes all outcomes in
task-ID order under a single coordinator. `forge task launch-batch` exposes it. Concurrency is proven
by rendezvous fixtures: each agent waits for the other to start, so serial execution fails. New tests:
four scheduler tests, six orchestrator batch tests (concurrency, deferral, skipped approval,
isolated adapter failure, gate failure, malformed-gate abort, capacity), and two portable CLI tests.
One amendment (`01b1c6b`) added deferral for overlap with running tasks. This is the first use of
`agentforge-scheduler` outside its own tests.

Found during closure verification, outside this milestone: `forge daemon launch` and `daemon run`
use the 2-second client read timeout for requests that execute the agent synchronously. With a
3-second agent the client reports "stale daemon metadata ... remove it" while the live daemon is
still running the task. Recorded as a known issue for a separate milestone.
