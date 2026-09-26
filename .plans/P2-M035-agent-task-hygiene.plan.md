# Plan: P2-M035 — Agent task hygiene: integrable tasks, runnable scripts

Status: Draft
Milestone: P2-M035
Created: 2026-09-25
Owner: AgentForge project

## Goal

Close the two open findings from the P5-M006 agent run:

1. **Finding 21:** creating a task that requires a post-execution approval but lacks the capability
   that exercises it is refused at creation, with the fix in the message. Such a task can be
   approved but never integrated or deployed.
2. **Finding 22:** a Claude Code agent whose task holds `run_local_commands` may run the files in
   its own allowed paths, such as the self-tests it writes. Before, the bridge's fixed allow-list
   (`cargo`, `./scripts/gate.sh`, a few `git` commands) kept it from executing
   `scripts/sbom-merge --self-test`.

## Operator decision (2026-09-25)

"Fix 21 and 22 then cut v0.3.1."

## Non-goals

- No change to stored tasks: validation on load is unchanged, so existing snapshots (including this
  repository's `P5-M006-T0001`) still load. The rule applies where tasks are created.
- No new capability or approval kinds, and no `forge task` command to edit contracts.
- No change to the gateway's tools.

## Context

- **21:** the operator created `P5-M006-T0001` with `--approval merge_protected_branch` but
  without `--capability merge_protected_branch`. Accept and approve succeeded, then `forge task
  integrate` refused, correctly. The mismatch should be caught when it is cheapest to fix.
- **22:** the agent reported that permission settings blocked running `scripts/sbom-merge` and
  `scripts/publish-release`, so it could not run its own self-tests. With `run_local_commands`, a
  task can already execute arbitrary code through `cargo` (tests, build scripts), so letting it
  execute files inside its own allowed paths grants nothing new, and does grant it the ability to
  test what it writes.

## Architecture placement

- **`agentforge-core`:** `ApprovalBoundary::exercised_by(self) -> Option<Capability>` maps
  `merge_protected_branch` to `merge_protected_branch` and `deploy_production` to
  `deploy_production`. Every other boundary returns `None`: `publish_release` has no capability,
  and the pre-execution boundaries are approvals only.
- **`agentforge-intake::build_task`** (used by `forge task create` and guided intake): after
  defaults are applied, a required approval whose `exercised_by` capability the task lacks is
  refused with `IntakeError::Task`, and the message names the missing capability and the flag.
- **`scripts/agents/claude-code-bridge`:** when the task holds `run_local_commands`, it adds, for
  each allowed path `P`, `Bash(./P)`, `Bash(./P *)`, `Bash(python3 P *)`, and `Bash(bash P *)` to
  the allowed tools, and says so in the instructions. Without `run_local_commands`, nothing is
  added. Its `--self-test` covers both cases, and paths with spaces are left out, since Claude
  Code's patterns split on spaces.
- **Docs:** TASK_CONTRACT (the approval and capability rule), AGENT_PROFILES (what the bridge lets
  the agent run), and DOGFOODING (findings 21 and 22 resolved).

## Invariants

- No task created from now on can require a post-execution approval it can never exercise.
- The bridge never lets an agent execute files outside its task's allowed paths, and only for tasks
  holding `run_local_commands`.

## ADRs

None.

## Public API / CLI

`ApprovalBoundary::exercised_by`; `forge task create` refuses the mismatch.

## Compatibility analysis

Task creation is stricter (the refusal names the fix); stored tasks are unaffected. The bridge only
adds permissions within a task's allowed paths.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M035-agent-task-hygiene.plan.md`, `.plans/ACTIVE`
- `crates/agentforge-core/src/agent.rs`, `crates/agentforge-core/tests/*.rs`
- `crates/agentforge-intake/src/lib.rs`, `crates/agentforge-intake/tests/*.rs`
- `crates/agentforge-cli/tests/*.rs`
- `scripts/agents/claude-code-bridge`
- `docs/TASK_CONTRACT.md`, `docs/AGENT_PROFILES.md`, `docs/DOGFOODING.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- Core: `exercised_by` for every boundary.
- Intake: the merge approval without the capability is refused (with the flag in the message); with
  it, the task is accepted. `deploy_production` behaves the same. `publish_release` and the
  pre-execution approvals are unaffected. A stored task with the mismatch still loads (snapshot
  round trip).
- CLI: `forge task create ... --approval merge_protected_branch` without the capability exits 1,
  names `--capability merge_protected_branch`, and creates nothing.
- Bridge self-test: the allowed tools include `Bash(./scripts/x *)` and the others for a
  `run_local_commands` task, none without it, and the instructions mention it.
- The full gate, and push CI plus a repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Tests first; core, intake, then the bridge; docs.
4. Gate; commit (exit checked); push; CI plus a repeat.
5. Close; tag after the closure CI is green. Then `v0.3.1` (a separate release plan).

## Failure modes

- An existing workflow relies on creating such tasks: it gets a clear refusal with the flag to add.
  No stored data changes.

## Documentation impact

TASK_CONTRACT, AGENT_PROFILES, DOGFOODING; CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full`, and the bridge `--self-test`;
- push CI plus a dispatched repeat.

## Acceptance criteria

- [ ] Task creation refuses a post-execution approval without its exercising capability.
- [ ] The bridge lets a `run_local_commands` task run files in its allowed paths.
- [ ] Docs; CI evidence; closed and tagged correctly.
