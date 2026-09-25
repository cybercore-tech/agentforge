# ADR-0051: Remote execution and exact-SHA import

- Status: Accepted
- Date: 2026-09-24
- Milestone: P4-M008

## Context

P4-M007 let workers on other machines claim, renew, and release leases and receive the exact
contract and base commit over GhostPort. Completing remote execution means accepting work produced
elsewhere into the coordinator's repository. That is the most sensitive step: the coordinator must
not trust the worker's environment, its gate results, or its description of what changed.

## Decision

- **Transfer.** The worker returns the result in one bounded `RESULT` request:
  - the exit code and termination;
  - stdout and stderr (at most 1 MiB each);
  - a `git bundle` of `base..agentforge/task/<task>` (at most 32 MiB).

  Before sending, the worker commits any in-bounds changes itself
  (`agentforge: remote result for <task>`), so simple agents work. It refuses to send anything
  that touches a path outside the contract, and releases the lease instead.
- **Verify before any side effect** (`import_remote_result`), in this order:
  1. the task is `pending` and ready;
  2. the claim equals the active lease (lease, worker, and generation);
  3. required gates are configured, and no task worktree exists;
  4. the SHAs are well-formed, and the base commit is known;
  5. `git bundle verify` passes;
  6. the bundle's task branch is fetched into the quarantine ref `refs/agentforge/remote/<lease>`,
     and it must equal the reported head **exactly**;
  7. the head descends from the base;
  8. every changed path is allowed and not forbidden.

  Any failure deletes the quarantine ref and changes nothing else.
- **Import.**
  - The task worktree and branch are created at that exact commit.
  - The evidence is `WorktreeObserved mode=remote`, the running transition, `AgentStarted
    remote:<worker>`, and `AgentFinished`, with the remote logs saved and `channel=remote`,
    `worker`, `base_commit`, and `head_commit` recorded.
  - **The task's gates run on the coordinator**, and gate results reported by the worker are never
    used.
  - Everything is persisted as one audit batch. A result with no commits fails the task with "no
    changes".
- **Serialization.** An import writes the task snapshot, so in `forged` it holds the daemon's
  execution slot; while an execution runs, `RESULT` answers `BUSY` and the worker retries. The
  worker API handles each connection on its own thread, so a long import never stalls other
  workers' renewals.
- **Claim hardening.** `CLAIM` refuses a task whose pre-execution approvals are not recorded (the
  coordinator vouches for them, and the worker passes them to its adapter). It also returns the
  lease window, so renewals never depend on the worker's clock.
- **Review is unchanged.** Diff, accept, the post-review merge approval bound to the imported SHA
  (P1-M008), and integrate. The commit that lands is byte-for-byte the commit that was verified.

## Consequences

Positive:

- a worker on another machine runs a task end to end, and the coordinator keeps full control over
  what enters its repository;
- the remote agent's environment is irrelevant to correctness: exact SHA, path boundary, and local
  gates decide.

Negative:

- results are limited to 32 MiB bundles; larger work would need a shared remote;
- the worker must hold a clone that has, or can fetch, the base commit;
- a rejected result is not retried automatically. The task stays `pending`, and the operator
  decides.

## Alternatives considered

- **Push to a shared remote and have the coordinator fetch.** Rejected for now: it needs Git
  credentials on workers and a remote both sides trust. The bundle stays inside the authenticated
  channel.
- **Trust the worker's gate results.** Rejected: gate outcomes decide the task state and must be
  produced where the coordinator can vouch for them.
