# Plan: P0-M014 — EINTR-safe pipe readers (remote real-agent dogfood)

Status: Draft
Milestone: P0-M014
Created: 2026-09-24
Owner: AgentForge project
Implementer: Claude Code as remote worker `remote-dogfood`, through `forge worker remote run` over
GhostPort, imported with exact-SHA verification and integrated with `forge task integrate`

## Goal

1. **The work (dogfooding finding 11).** Every blocking `read()` loop on a process pipe or stdin
   retries on `io::ErrorKind::Interrupted` instead of failing. These are the eight sites left after
   P5-M002 fixed the socket readers:
   - `agentforge-adapter`: stdin forwarding, plus three output capture loops;
   - `agentforge-gate`: gate output capture;
   - `agentforge-ci`: the provider output reader;
   - `agentforge-cli`: two bounded input readers.
2. **The dogfood.** A real coding agent (Claude Code) builds it as a remote worker. It uses a
   separate clone of the repository, reached only through the P4-M007/P4-M008 channel (GhostPort
   Noise KK tunnel, AFW1 worker API, git-bundle result, exact-SHA import, coordinator-side gates).
   Everything that happens is recorded, and every real problem becomes a dogfooding finding.

## Non-goals

- No change to socket readers (fixed in P5-M002) or to any other I/O behavior.
- No new retry for other error kinds.
- No second physical machine: both ends run on this host, with separate clones, key sets, and
  loopback ports. This exercises everything except a real network hop, and the closure says so.

## Context

P2-M027 and P1-M007 put a real agent through the local path, and that run found 8 real problems.
The remote path has more moving parts, so it deserves the same honest test:
- a second clone and base-commit availability;
- the bridge and pre-commit gate inside the worker's clone;
- the worker's auto-commit (or the bridge's own commit);
- bundle size;
- lease renewals over the tunnel during a long agent run;
- the RESULT timeout while the coordinator runs its full gate;
- import, review, and integrate.

Finding 11 is real, open, small, and testable, so it is a good first job.

## Architecture placement

- **Code (by the agent):** in each listed loop, `Err(e) if e.kind() == io::ErrorKind::Interrupted`
  retries the read. Existing error mapping and output limits stay unchanged. A small shared helper
  per crate is acceptable. Unit tests use a reader that returns `Interrupted` once and then data,
  and prove the loops keep the data.
- **Agent task contract** (`P0-M014-T0001`):
  - allowed paths: the four source files and each crate's `tests/`;
  - capabilities: `run_local_commands`, `write_owned_paths`, `merge_protected_branch`;
  - approval: `merge_protected_branch` (post-review);
  - required gate: `workspace` (`./scripts/gate.sh full`, run by the coordinator on import).
- **Dogfood setup** (operator-local, nothing committed):
  - the coordinator is this checkout, running the current `forged` build with
    `.forge/worker-api.conf bind=127.0.0.1:47420`, worker `remote-dogfood` registered and enrolled,
    and a GhostPort server;
  - the worker host is a fresh clone in the scratch area with hooks installed, its own
    `claude-code` profile (pointing at the clone's bridge), a GhostPort client listening on
    127.0.0.1:47500, and the secret file with mode 600;
  - the operator grants the lease; `forge worker remote run --once` runs in the background with
    logs.
- **Review:** `forge task diff`, reading the code, `accept`, `approve` (bound to the imported SHA),
  `integrate --target main`, and `retire` only after that succeeds.

## Invariants

- Nothing is committed to `main` while the remote task runs (integration is fast-forward only).
- The imported commit is exactly the agent's commit, gated on the coordinator.
- Operator-local configuration stays under ignored `.forge/` and the scratch area.

## ADRs

None expected. A finding that needs a design change gets its own milestone.

## Public API / CLI

None.

## Compatibility analysis

Behavior only improves: an interrupted read no longer aborts the operation.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P0-M014-eintr-safe-pipe-readers.plan.md`, `.plans/ACTIVE`
- agent: `crates/agentforge-adapter/src/lib.rs`, `crates/agentforge-gate/src/lib.rs`,
  `crates/agentforge-ci/src/lib.rs`, `crates/agentforge-cli/src/main.rs`, and
  `crates/agentforge-{adapter,gate,ci,cli}/tests/*.rs`
- closure: `docs/DOGFOODING.md`, `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`,
  `PROJECT_STATE.md`, `AGENT_HANDOFF.md`

## Test-first matrix

- For each crate, a reader that yields `Interrupted` once and then bytes: the loop returns the full
  data, and other errors still propagate.
- The full gate in the agent's worktree (pre-commit hook), and again on the coordinator after
  import (the `workspace` gate).
- CI: the push run plus a dispatched repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Set up the coordinator and worker host; create the task; grant the lease.
4. Claude Code runs remotely; the coordinator imports it.
5. Review, accept, approve, integrate, and retire.
6. Push, CI, and a closure with the remote run log and findings; tag.

## Failure modes

- The agent fails, or the import is rejected: classify it, record it as a finding, and retry with a
  new attempt or fix the tool in its own milestone. Never land the work by hand without recording
  it.
- The tool fails: the same. Findings are the point of this milestone.

## Documentation impact

DOGFOODING: a remote run log and findings. CHANGELOG, README, and MILESTONES at closure.

## Quality gates

- the pre-commit gate in the worker's clone, and the `workspace` gate on the coordinator;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] All eight pipe readers retry on `Interrupted`, with tests.
- [ ] Implemented by Claude Code as a remote worker through GhostPort, imported at its exact SHA,
      gated on the coordinator, and integrated with `forge task integrate`.
- [ ] Remote run log and findings recorded; CI evidence; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
