# Scheduling and serialized integration

P1-M001 introduces deterministic runnable batches from the durable task graph. Tasks are ordered by
stable ID and only disjoint owned paths may share a batch. Overlapping ownership is rejected
conservatively. Integrator reservations are single-owner and serialized. Scheduling produces evidence
and reservations; it does not accept results, grant capabilities, merge protected branches, or deploy.

## Concurrent batch launch

P1-M006 connects scheduling to execution. `forge task launch-batch` launches every disjoint ready
task, up to a limit, with the agents running at the same time in their own worktrees:

```bash
forge task launch-batch /path/to/project --profile local-agent [--base HEAD] [--max 4]
```

### Planning

`plan_launch_batch` walks ready tasks in task-ID order. A task is **deferred** instead of launched
when one of its owned paths overlaps a task already chosen for this batch, or a task that is still
`running` (for example from an earlier batch). A task is also deferred when the batch already holds
`--max` tasks (default 4, allowed 1-16). Deferral never changes state; the task stays `pending` for
a later batch. Unlike `plan_batch`, an overlap does not reject the whole graph.

### Execution phases

1. **Prepare** (sequential). Gate profiles are loaded once; a malformed profile aborts the batch
   before any side effect. Each selected task is checked (contract, capabilities, approvals,
   repository) and its worktree is created or verified. A task that fails is reported as
   `skipped` with the reason, and its siblings continue. Prepared tasks are marked `running`, with
   `WorktreeObserved`, `TaskTransition`, and `AgentStarted` evidence, and persisted before any
   agent starts.
2. **Execute** (parallel). One scoped thread per task runs the agent and, after a zero exit, the
   project gates in that task's worktree. Threads never read or write the task snapshot or the audit
   log.
3. **Finalize** (sequential). Outcomes are applied in task-ID order and persisted in one step:
   `AgentFinished` and gate evidence, or `failed` with `FailureClassified` for an adapter error or
   failing gate. This matches the single-task path.

Only the coordinating thread writes state, so concurrent agents cannot lose each other's updates.
Worktree creation is sequential, so Git's worktree metadata is never written concurrently.

### Output and exit codes

The command prints one line per `launched`, `failed`, `skipped`, and `deferred` task, then a summary.
It exits 0 when every selected task launched and all of its gates passed. As with `forge task
launch`, a non-zero agent exit is recorded as evidence and does not by itself fail the command. It
exits 1 when any task was skipped, failed to launch, or failed a gate, and 2 for usage errors. With
no ready tasks it prints `no ready tasks` and exits 0.

### Interrupted batches

A crash after Prepare leaves the prepared tasks `running` with `AgentStarted` evidence and no
finish record. Inspect them with `forge task inspect` and use `forge task cancel` or retry them as
for any interrupted run. Integration stays serialized through `forge task integrate`.
