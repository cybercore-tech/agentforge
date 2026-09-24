# Plan: P2-M033 — HUD view of agent runs

Status: Draft
Milestone: P2-M033
Created: 2026-09-24
Owner: AgentForge project
Implementer: Claude Code through AgentForge, integrated with `forge task integrate`

## Goal

Show recent agent runs in `forge hud` (one-shot and `--watch`): for each run, the task, the agent's
exit code and termination, where its full output is, and how its gates went. Operators can then see
at a glance which runs failed and where to look, without reading the raw audit log.

## Non-goals

- No change to the audit format, evidence files, or orchestration.
- No reading of evidence log contents; the HUD shows paths only and stays bounded.
- No interactivity beyond the existing watch commands.

## Context

Since P2-M029 each agent run appends an `AgentFinished` audit event with `exit_code`,
`termination`, `output_truncated`, and `stdout_log`/`stderr_log` (or `evidence_error`), followed by
one `GateFinished` event per gate (`gate`, `outcome`) and, on failure, `FailureClassified`
(`stage`). `forge hud` shows only the last 8 audit event labels (`#N Kind task=...`), so none of
that is visible without inspecting the log.

This milestone is also the first agent-built milestone to go through AgentForge's own integration:
the task is created with the `merge_protected_branch` capability and approval, the operator records
the approval after review, and `forge task integrate` fast-forwards `main` (dogfooding finding 7).

## Architecture placement

`agentforge-hud` only, plus the CLI test and docs:

- A new `AgentRunSummary` per `AgentFinished` event: sequence, task ID, exit code, termination,
  output-truncated flag, and stdout/stderr log paths (or the evidence error). It also records a gate
  summary built from the `GateFinished` events for the same task that follow it, before that task's
  next `AgentStarted`: passed/total, plus the first non-passing gate name and outcome.
- `HudSnapshot` gains `agent_runs`: the most recent runs, capped by a new `MAX_AGENT_RUNS` (5),
  newest last, consistent with `audit_recent`.
- `render` adds an `agent_runs:` section, one line per run, for example:
  `  - #12 task=P1-M007-T0003 agent-exit=0 termination=exited gates=1/1 stdout=.forge/evidence/... stderr=...`.
  A failing run shows `agent-exit=<n>` and, when a gate failed, `failed-gate=<name>:<outcome>`.
  With no runs, the section reads `  - none`.
- Everything stays within the existing `MAX_RENDERED_BYTES` bound, and output stays deterministic.

## Invariants

- The HUD remains read-only and deterministic.
- Runs recorded before P2-M029 (with no exit fields) render with `agent-exit=unknown` instead of
  failing.
- Output stays bounded.

## ADRs

None; this extends the P2-M001/P2-M002 HUD.

## Public API / CLI

- `agentforge_hud::{AgentRunSummary, MAX_AGENT_RUNS}` and `HudSnapshot::agent_runs`.
- A new `agent_runs:` section in `forge hud` output.

## Compatibility analysis

Additive output section. Existing sections and their order are unchanged, and the new section comes
after `audit_recent`.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M033-hud-agent-runs.plan.md`
- `.plans/ACTIVE`
- `crates/agentforge-hud/src/lib.rs`
- `crates/agentforge-cli/tests/hud_commands.rs`
- `docs/HUD.md`
- closure records: `docs/MILESTONES.md`, `docs/DOGFOODING.md`, `CHANGELOG.md`, `README.md`,
  `PROJECT_STATE.md`, `AGENT_HANDOFF.md`

The agent's task boundary is `crates/agentforge-hud/src/lib.rs`,
`crates/agentforge-cli/tests/hud_commands.rs`, and `docs/HUD.md`.

## Test-first matrix

- Unit: a synthetic audit sequence with two runs (one passing with one gate, one failing with a
  failed gate) yields two summaries with the right exit codes, gate counts, failed gate, and log
  paths; more than `MAX_AGENT_RUNS` runs keeps only the newest; a pre-P2-M029 `AgentFinished` with
  no fields renders `agent-exit=unknown`.
- Rendering stays deterministic and within `MAX_RENDERED_BYTES`, and an empty history renders
  `  - none`.
- CLI: after a real `forge task launch` with the fixture agent, `forge hud` shows an `agent_runs:`
  line with that task, `agent-exit=0`, and its evidence paths.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Create a task with merge authority and launch Claude Code through the bridge.
4. The operator reviews `forge task diff`, records the `merge_protected_branch` approval, accepts,
   runs `forge task integrate --target main`, and only then retires the worktree.
5. Push, verify CI, close, and tag.

## Failure modes

- Agent failure: the evidence logs and `agent-exit` explain it; the operator retries or amends.
- Integration refusal: classify it; never bypass by hand unless recorded as a finding.

## Documentation impact

`docs/HUD.md` (agent-runs section). README, CHANGELOG, and DOGFOODING are updated at closure.

## Quality gates

- `./scripts/gate.sh full` (the agent's pre-commit hook and the `workspace` project gate);
- push-triggered CI green, plus a dispatched repeat.

## Acceptance criteria

- [ ] `forge hud` shows recent agent runs with exit status, gates, and evidence paths.
- [ ] Implemented by Claude Code through AgentForge.
- [ ] Integrated through `forge task integrate` after a recorded approval.
- [ ] CI evidence recorded, closed, and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
