# ADR-0039: Concurrent batch launch with a single state coordinator

- Status: Accepted
- Date: 2026-09-23
- Milestone: P1-M006

## Context

P1-M001 promised that disjoint work can run concurrently, but its scheduler was only used by tests,
and every operator path ran one task. The persisted single-task path loads the task snapshot, runs
the agent, and saves the snapshot afterwards. Running several copies of that path in parallel would
lose updates, because each would save its own copy of the graph. `plan_batch` also rejects the whole
graph on any overlap, which suits a validity check but not a launcher.

## Decision

Add `forge task launch-batch`, backed by `plan_launch_batch` and `launch_batch_persisted`.

- Planning defers, rather than rejects, any ready task that overlaps a task already in the batch or
  still `running`, plus any task past the `--max` limit.
- One coordinating thread owns the task snapshot and audit log. It prepares tasks sequentially and
  persists them as `running`, runs agents and gates on scoped threads that only run processes, then
  applies all outcomes in task-ID order and persists once.
- Per-task results match the single-task path. A task that fails its own preflight is skipped
  without blocking its siblings.

## Consequences

Positive:

- disjoint tasks run at the same time without lost state updates;
- marking tasks `running` before execution stops a parallel single-task launch of the same task;
- the scheduler's ownership rules now govern a real execution path.

Trade-offs:

- a crash between phases leaves tasks `running` without finish evidence and needs operator cleanup;
- the batch waits for its slowest agent before finalizing;
- the daemon does not offer batch launch yet.
