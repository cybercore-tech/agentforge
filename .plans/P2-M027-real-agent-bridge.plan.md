# Plan: P2-M027 — Real-agent bridge and AgentForge-on-AgentForge dogfooding

Status: Draft
Milestone: P2-M027
Created: 2026-09-23
Owner: AgentForge project

## Goal

Run a real coding agent through AgentForge end to end on AgentForge itself. Provide a bridge that
turns the `agentforge-task-prompt-v1` stdin contract into instructions a real agent understands,
runs Claude Code headless inside the task worktree, enforces the task's path boundary, and commits
the result so the normal gate, review, accept, and integrate flow applies. Then prove it by
implementing a real milestone (P1-M007) through `forge task launch`, and record the friction found.

## Non-goals

- No change to the adapter stdin contract, the process adapter, or orchestration semantics.
- No OS sandbox for the agent; the path check is a post-run verification, as documented today.
- No automatic acceptance or integration; the operator still reviews, accepts, and integrates.
- No bridges for other agents in this milestone. The design keeps the prompt decoding reusable.

## Context

Every run through AgentForge so far used fixture programs or `/usr/bin/true`. Real agents read
natural-language instructions, not the length-delimited v1 document, and a task's changes only
reach review and integration once they are committed on the task branch. Nothing in the repository
does that step. Claude Code 2.1.281 is installed on the operator machine and supports headless
runs (`-p`, `--permission-mode`, `--allowedTools`, `--append-system-prompt`,
`--no-session-persistence`).

## Architecture placement

`scripts/agents/claude-code-bridge` (Python 3, standard library):

1. Decodes the v1 prompt from stdin (strict length-delimited parsing; any malformed input exits
   non-zero before the agent starts).
2. Builds instructions: goal, non-goals, allowed and forbidden paths, expected outputs, evidence
   requirements, and required gates. It also tells the agent to read `AGENTS.md` and project
   docs, stay inside the allowed paths, run the repository gate, and not commit (the bridge
   commits).
3. Runs `claude -p` in the worktree with `--permission-mode acceptEdits`, a bounded tool
   allowlist (read/edit tools, `cargo`, read-only `git`, `./scripts/gate.sh`), and
   `--no-session-persistence`.
4. After the agent exits, lists changed files (`git status --porcelain`). Any change outside the
   allowed paths, or inside a forbidden path, fails with exit 4 and leaves the worktree for
   review.
5. Stages the changes and commits with a detailed message derived from the task. The repository's
   pre-commit hook runs the full gate; a gate failure exits 5 with the hook output.
6. Exit codes: 0 committed, 3 agent made no changes, 4 path violation, 5 commit or gate failure,
   anything else is the agent's own exit code.

`--dry-run` prints the instructions and command without running the agent; `--self-test` checks
the decoder against an encoded sample and malformed inputs.

## Data flow

`forge task launch --profile claude-code` → adapter (verified worktree, cleared environment plus
profile values) → bridge stdin → Claude Code edits the worktree → bridge path check → bridge
commit (pre-commit gate) → adapter evidence → project gates (`.forge/gates/`) →
`forge task diff` → operator review → `accept` → `integrate` → `retire`.

## Invariants

- The agent never commits; only the bridge does, after the path check passes.
- Changes outside the task boundary are never committed.
- The repository's own hooks and gates run on every agent commit; nothing is bypassed.
- The existing adapter trust boundary is unchanged: exit 0 is evidence, not acceptance.

## ADRs

- ADR-0043: real agents run through a bridge that owns path verification and commits.

## Public API / CLI

- `scripts/agents/claude-code-bridge [--dry-run | --self-test]`, used through an agent profile.

## Compatibility analysis

Additive. Existing profiles and fixtures are unaffected.

## Dependency analysis

No new Rust or Python dependency. Running the bridge requires an installed, authenticated
Claude Code CLI; CI only runs `--self-test`.

## Expected file boundary

- `.plans/P2-M027-real-agent-bridge.plan.md`
- `.plans/ACTIVE`
- `scripts/agents/claude-code-bridge`
- `.github/workflows/ci.yml` (self-test step)
- `docs/AGENT_PROFILES.md`
- `docs/DOGFOODING.md`
- `docs/adr/ADR-0043-real-agent-bridge.md`
- `README.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- `--self-test` decodes a sample prompt with multiline and Unicode fields and lists, and rejects a
  truncated document, a bad header, a non-numeric length, and a missing field.
- `--dry-run` on a real `forge`-rendered prompt produces complete instructions.
- The path check rejects changes outside the allowed paths and inside forbidden paths, leaves the
  worktree untouched, and exits 4.
- A real run: P1-M007 is implemented by Claude Code through `forge task launch --profile
  claude-code`, committed by the bridge with the gate passing, reviewed with `forge task diff`,
  accepted, and integrated. Its CI evidence is recorded.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Write the bridge with `--self-test` and `--dry-run`; add the CI self-test step.
4. Document it (`AGENT_PROFILES.md`, `DOGFOODING.md`, ADR-0043, README, CHANGELOG); commit and
   push.
5. Draft and approve P1-M007 through the normal plan checkpoints.
6. Launch P1-M007 through AgentForge with the bridge; review, accept, integrate, and push.
7. Record the dogfooding findings in `docs/DOGFOODING.md`, then close both milestones.

## Failure modes

- Agent timeout or crash: the adapter records it and nothing is committed; the operator inspects
  the worktree.
- Path violation: exit 4, nothing committed, and the worktree kept for review.
- Gate failure in the pre-commit hook: exit 5, nothing committed, and the hook output kept in the
  adapter evidence. The operator can relaunch after `forge task retry`.

## Documentation impact

Real-agent setup and the Claude Code profile in `AGENT_PROFILES.md`; a new `docs/DOGFOODING.md`
with the run log and findings; README and CHANGELOG; ADR-0043.

## Quality gates

- `./scripts/gate.sh full`;
- `scripts/agents/claude-code-bridge --self-test` in CI;
- dispatched or push-triggered CI green on all seven jobs;
- a completed real-agent run with evidence.

## Acceptance criteria

- [ ] A real agent completes an AgentForge milestone through `forge task launch`.
- [ ] Path boundaries are enforced before any agent commit.
- [ ] Repository gates and hooks run on the agent's commit.
- [ ] Findings are documented, with follow-ups identified.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
