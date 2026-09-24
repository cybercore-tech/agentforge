# Plan: P2-M029 — Agent run visibility, evidence logs, and agent build isolation

Status: Approved
Milestone: P2-M029
Created: 2026-09-24
Owner: AgentForge project

## Goal

Make every agent run explain itself. An operator should always see how the agent exited, find its
full output later, and get a failing command status when the agent fails. Also close the last
build-isolation gap found while dogfooding: an agent running `cargo` directly in its worktree must
not share build output with other checkouts.

## Non-goals

- No change to the task state model: a non-zero agent exit stays evidence, and the task stays
  `running` for an explicit operator decision.
- No log shipping, rotation, or remote storage.
- No change to the adapter's capture bounds.

## Context

From `docs/DOGFOODING.md`:

- Finding 1: `forge task launch` printed only `termination=Exited` and exited 0 when the bridge
  exited 4 or 5.
- Finding 4: the adapter's captured stdout and stderr are neither printed nor persisted. Diagnosing
  attempt `P1-M007-T0001` needed forensics, and attempt `T0002` had to be rerun by hand with a
  `tee` wrapper to see why it stopped.
- Build isolation: P2-M028 isolates the gate's Cargo target directory in linked worktrees, but
  during attempt `T0002` Claude Code ran `cargo test` directly, wrote the worktree's binaries into
  the shared `~/.cargo-target`, and broke the main checkout's next gate again.

## Architecture placement

- `agentforge-orchestrator`: after each agent run (single-task and batch finalize), write the
  captured stdout and stderr to
  `.forge/evidence/<task-id>/<audit-sequence>-{stdout,stderr}.log` in the project root (not the
  worktree). `AgentFinished` records `exit_code`, `termination`, `stdout_log`, `stderr_log`, and
  `output_truncated`. Writing the evidence files is best-effort: a write failure is recorded in the
  event instead of aborting state persistence.
- `agentforge-platform`: `forge run`, `forge task launch`, and `forge task launch-batch` print the
  agent's exit code and termination, the evidence log paths, and on failure the last 20 lines of
  stderr (or stdout if stderr is empty). They exit 1 when the agent did not exit 0.
- `agentforge-daemon`: run/launch responses include `agent-exit=<code|none>` and the log paths.
- `scripts/agents/claude-code-bridge`: in a linked worktree, set `CARGO_TARGET_DIR` for the agent
  to the same `<target>/agentforge-worktrees/<worktree name>` that `gate.sh` uses, unless the
  caller already set it.
- `docs/OPERATIONS.md`: a single operations reference for the GitHub workflows, scripts, hooks,
  remotes and push method, CI evidence practice, local `.forge/` setup, agent bridge, and recovery
  procedures.

## Invariants

- Evidence logs are written outside the task worktree, so they never dirty it or reach commits.
- Audit records reference logs by project-relative path.
- CLI exit codes: 0 only when the agent exited 0 and all gates passed.

## ADRs

None; this extends ADR-0037 and ADR-0043 evidence practice.

## Public API / CLI

- New `AgentFinished` fields; `ProcessExecution` exposes the evidence log paths.
- `forge run` / `forge task launch` / `forge task launch-batch` now exit 1 on a non-zero agent exit.

## Compatibility analysis

Scripts that treated `forge task launch` exit 0 as "launched" now see 1 when the agent fails; this
is the intended correction and is documented. Audit readers ignore unknown fields.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M029-agent-run-visibility.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-orchestrator/tests/vertical_slice.rs`
- `crates/agentforge-orchestrator/tests/batch_launch.rs`
- `crates/agentforge-cli/src/main.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs`
- `crates/agentforge-cli/tests/task_launch_commands.rs`
- `crates/agentforge-cli/tests/run_commands.rs`
- `crates/agentforge-daemon/src/lib.rs`
- `scripts/agents/claude-code-bridge`
- `docs/OPERATIONS.md`
- `docs/AGENT_PROFILES.md`
- `docs/DOGFOODING.md`
- `README.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- A failing agent: `forge task launch` exits 1, prints `agent-exit=<n>` and a stderr tail, and
  evidence logs exist under `.forge/evidence/<task>/` and are referenced by `AgentFinished`.
- A successful agent: exit 0, logs written, and no tail printed.
- Batch: per-task exit codes and log paths are printed, and the command exits 1 if any agent
  failed.
- The bridge self-test covers the worktree target-directory rule.

## Implementation sequence

1. Commit this draft plan (after P1-M007 integrates, because integration is fast-forward only).
2. Approve and activate it in a separate checkpoint.
3. Implement evidence logs and audit fields, then CLI and daemon output, then the bridge rule, with
   tests.
4. Write `docs/OPERATIONS.md` and update the other docs.
5. Run the full gate, push, check CI, and close.

## Failure modes

- An evidence write failure is recorded in the audit event and printed, and state persistence
  continues.

## Documentation impact

`docs/OPERATIONS.md` (new), `docs/AGENT_PROFILES.md`, `docs/DOGFOODING.md` (findings resolved),
README, CHANGELOG.

## Quality gates

- `./scripts/gate.sh full`;
- push-triggered CI green on all seven jobs.

## Acceptance criteria

- [ ] Agent exit codes and output are visible and persisted for every run path.
- [ ] CLI exit status reflects agent failure.
- [ ] Agent builds in worktrees are isolated.
- [ ] `docs/OPERATIONS.md` documents workflows, scripts, hooks, remotes, evidence, and recovery.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
