# Plan: P1-M007 — Enforce task-declared required gates

Status: Approved
Milestone: P1-M007
Created: 2026-09-24
Owner: AgentForge project
Implementer: Claude Code through AgentForge (`forge task launch --profile claude-code`), the P2-M027 dogfooding proof

## Goal

Make `AgentTask.required_gates` meaningful. Today the field is parsed, stored, and passed to agents,
but orchestration ignores it: P1-M004 runs every project gate for every task. After this milestone:

1. A task that declares required gates runs **exactly those gates** after a successful agent, in
   lexical gate-ID order.
2. A task that declares no required gates runs **every project gate**, which is the current P1-M004
   behavior, unchanged.
3. A required gate name with no matching `.forge/gates/<name>.conf` profile fails preflight with
   `SliceError::Preflight` naming the missing gate, **before** the task becomes `running` and before
   any agent starts. The batch path reports that task as `Skipped` with the same reason and keeps
   launching its siblings.

## Non-goals

- No change to the gate profile format, `GateProfileStore`, the gate runner, or the task contract
  shape or its validation.
- No change to gate evidence (`GateFinished` / `FailureClassified` fields) beyond running the
  selected set.
- No CLI flag changes; `forge task create --gate <name>` already populates the field.

## Context

`AgentTask.required_gates: Vec<String>` exists in `agentforge-core`. `forge task create --gate`
fills it, and the adapter prompt includes it. `agentforge-orchestrator` loads all gates with
`GateProfileStore::list()` in `execute_process_attempt` and in `launch_batch_persisted`, and runs
them all through `run_gates`. The field was noted as unenforced in PROJECT_STATE.md known issues.

## Architecture placement

`agentforge-orchestrator` only. Add one selection helper, used by both the single-task attempt
(`execute_process_attempt`) and the batch prepare phase (`launch_batch_persisted`). It takes the
loaded gate definitions and the task, returns the definitions to run, or returns a preflight error
naming the first missing required gate (in lexical order). Selection happens before any state
transition; the batch path applies it per task inside Prepare.

## Invariants

- Missing required gates are detected before any side effect of that task.
- Tasks without required gates behave exactly as in P1-M004.
- Gate execution order stays lexical by gate ID.
- Duplicate names in `required_gates` do not run a gate twice.

## ADRs

None; this refines ADR-0037. `docs/GATES.md` records the rule.

## Public API / CLI

No public signature changes.

## Compatibility analysis

Backward compatible for tasks without `required_gates`. Tasks that already declare gates now run
only those, and fail preflight if a declared gate is not configured.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P1-M007-task-required-gates.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-orchestrator/src/lib.rs`
- `crates/agentforge-orchestrator/tests/vertical_slice.rs`
- `crates/agentforge-orchestrator/tests/batch_launch.rs`
- `docs/GATES.md`
- `docs/TASK_CONTRACT.md`
- closure records: `docs/MILESTONES.md`, `docs/DOGFOODING.md`, `CHANGELOG.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

The agent's task boundary is the three orchestrator files plus `docs/GATES.md` and
`docs/TASK_CONTRACT.md`; closure records are updated by the operator.

## Test-first matrix

- A single-task run with `required_gates = ["b-check"]` and gates `a-check` and `b-check`
  configured runs only `b-check` (one `GateFinished`).
- A single-task run with no required gates still runs every configured gate.
- A single-task run requiring an unconfigured gate fails preflight: task stays `pending`, no audit
  records, and the agent never runs.
- In a batch, a task requiring an unconfigured gate is `Skipped` with the missing gate named, and a
  sibling still launches.
- A duplicate required gate name runs once.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Implement through AgentForge: `forge task launch --profile claude-code` on a task whose allowed
   paths are the agent boundary above; the bridge commits and the pre-commit gate runs.
4. Project gates run; the operator reviews `forge task diff`, then accepts, integrates, and retires.
5. Push, verify CI, and close with evidence and dogfooding findings.

## Failure modes

- Agent failure, path violation, or gate failure: nothing is committed or integrated. The operator
  classifies the failure, retries the task, or implements by hand, and records the finding in
  `docs/DOGFOODING.md`.

## Documentation impact

`docs/GATES.md` (selection rule and preflight failure) and `docs/TASK_CONTRACT.md` (what
`required_gates` means).

## Quality gates

- `./scripts/gate.sh full` (the pre-commit hook on the agent's commit, and the project gate);
- push-triggered CI green on all seven jobs.

## Acceptance criteria

- [ ] Required gates select exactly the declared gates; empty keeps the P1-M004 behavior.
- [ ] Missing required gates fail preflight before any side effect, in both run paths.
- [ ] Implemented by a real agent through AgentForge and integrated through `forge task integrate`.
- [ ] CI evidence recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
