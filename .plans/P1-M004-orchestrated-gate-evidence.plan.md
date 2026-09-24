# Plan: P1-M004 — Orchestrated gate evidence in the task run path

Status: Approved
Milestone: P1-M004
Created: 2026-09-23
Owner: AgentForge project

## Goal

Make quality gates a real stage of every task run instead of caller-supplied evidence. Projects
declare gates as reviewed local profiles; after an agent exits successfully, the orchestrator runs
every configured gate inside the task's verified worktree, records bounded `GateFinished` audit
evidence per gate, and fails the task when any gate does not pass. This connects the P0-M007 gate
engine, which no other crate currently uses, to `forge run`, `forge task launch`, and the matching
daemon commands.

## Non-goals

- No change to the gate engine's process model, bounds, or cleared-environment policy.
- No shell execution, interpolation, or gate discovery outside `.forge/gates/`.
- No automatic acceptance: a task whose gates pass remains `Running` until an explicit operator
  decision, exactly as today.
- No CI observation or failure classification wiring (P1-M005) and no concurrent batches
  (P1-M006).
- No change to the legacy caller-evidence `run`/`execute_once` vertical-slice API.

## Context

`agentforge-gate` provides bounded, direct-argument process gates, but no crate depends on it. The
orchestrator's only gate concept is `SliceEvidence::gates_passed: bool`, supplied by the caller,
and the persisted run/launch paths never run a gate. The audit log already defines
`AuditEventKind::GateFinished` and nothing emits it. As a result the Phase 0 success condition
"run deterministic gates" holds only for library composition, not for the operator paths.

## Architecture placement

- `agentforge-gate` gains `GateProfileStore`, which loads `.forge/gates/<id>.conf` into
  `GateDefinition` values using the same bounded `key=value` format as agent profiles.
- `agentforge-orchestrator` depends on `agentforge-gate` (an internal workspace crate; no external
  dependency) and runs gates inside the persisted execution attempt.
- `agentforge-platform` (`forge`) gains the read-only `forge gate list <root>` command and gate
  result output. It depends on `agentforge-gate` with the same path-plus-version requirement form
  as its other internal crates.
- `agentforge-daemon` includes the gate summary in its run/launch responses.

## Data flow

1. Preflight loads and validates every gate profile before the task transitions to `Running`. A
   malformed, unsafe, or oversized profile fails closed before any agent process or worktree
   mutation.
2. The agent runs as today and `AgentFinished` is recorded.
3. If the agent exited normally with status zero, gates run in lexical ID order in the report's
   verified worktree path. Otherwise gates are skipped: the existing "nonzero exit is evidence,
   not completion" behavior is unchanged.
4. Each gate appends one `GateFinished` event with `gate`, `outcome`, `exit_code`, and
   `output_truncated` fields.
5. If any gate did not pass, the task transitions to `Failed` and one `FailureClassified` event
   records `stage=gates` and the first failing gate. Otherwise the task stays `Running`.
6. The returned `ProcessExecution` carries the gate reports; the CLI prints one line per gate and
   exits non-zero when a gate failed. A project with no `.forge/gates/` directory behaves exactly
   as before.

## Invariants

- Gate configuration is validated before any side effect of the run.
- Gates execute only in the task's own verified worktree, with a cleared environment plus explicit
  profile values.
- Gate evidence is persisted in the same attempt log as the agent evidence.
- A failing gate can never leave the task in a state from which it could be accepted.
- Gate order is deterministic and independent of directory iteration order.

## ADRs

- ADR-0037: gates are orchestrator-run evidence, and a failing gate fails the task.

## Public API / CLI

- `agentforge_gate::{GateProfileStore, GateProfileError}`, plus accessors on `GateDefinition`.
- `ProcessExecution::gates` and `ProcessExecution::gates_passed()`.
- `forge gate list <root>`.
- `forge run` / `forge task launch` print `gate <id> outcome=<...> exit=<...>` lines and exit 1
  on a gate failure. Daemon responses append `gates=<passed>/<total>`.

## Compatibility analysis

Projects without `.forge/gates/` see identical behavior and output apart from a `gates=0/0`
summary. Existing task snapshots, audit records, and daemon protocol framing are unchanged; the
daemon response message gains fields inside its existing free-text payload.

## Dependency analysis

No new external dependency. New internal edges: `agentforge-orchestrator -> agentforge-gate` and
`agentforge-platform -> agentforge-gate`.

## Expected file boundary

- `.plans/P1-M004-orchestrated-gate-evidence.plan.md`
- `.plans/ACTIVE`
- `Cargo.lock`
- `crates/agentforge-gate/src/lib.rs`
- `crates/agentforge-gate/tests/gate_profiles.rs`
- `crates/agentforge-orchestrator/Cargo.toml`
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-orchestrator/tests/vertical_slice.rs`
- `crates/agentforge-cli/Cargo.toml`
- `crates/agentforge-cli/src/main.rs`
- `crates/agentforge-cli/tests/gate_commands.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs` (portable failing-gate mode)
- `crates/agentforge-daemon/src/lib.rs`
- `docs/GATES.md`
- `docs/adr/ADR-0037-orchestrated-gate-evidence.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- Gate profiles parse the documented keys, reject unknown/duplicate keys, bound sizes and limits,
  reject symlinks and unsafe IDs, and list in lexical order; a missing directory is an empty list.
- A passing gate after a successful agent records `GateFinished` and leaves the task `Running`.
- A failing gate records `GateFinished` plus `FailureClassified` and transitions the task to
  `Failed`.
- A malformed gate profile fails preflight before the task becomes `Running` and before the agent
  runs.
- A nonzero agent exit skips gates.
- A project without gates behaves as before.
- `forge gate list` is read-only and deterministic; `forge task launch` with a failing gate exits 1
  and prints the gate evidence.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Add `GateProfileStore` and its tests.
4. Run gates in the orchestrator attempt with audit evidence and state transitions; add tests.
5. Surface gate results in the CLI and daemon; add `forge gate list` and CLI tests.
6. Update `docs/GATES.md`, README, and ADR-0037.
7. Run the full gate, push, and record exact-SHA CI evidence before closure.

## Failure modes

- Gate profile errors are preflight errors; no state or audit mutation occurs.
- Gate process I/O errors are recorded as a failed gate and fail the task rather than being
  ignored.
- If a gate writes into the worktree, later retirement still detects the dirty state; gates are
  documented as read-only checks.

## Documentation impact

`docs/GATES.md` documents the profile format, execution point, evidence, and failure semantics.
README lists `forge gate list`. ADR-0037 records the decision.

## Quality gates

- `./scripts/gate.sh full`;
- exact-SHA CI green on all seven jobs.

## Acceptance criteria

- [ ] Gates run automatically in the direct and daemon run/launch paths.
- [ ] Gate evidence is durable in the audit log.
- [ ] A failing gate fails the task; a malformed gate profile fails closed before execution.
- [ ] Projects without gates are unaffected.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
