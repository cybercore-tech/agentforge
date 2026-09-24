# Plan: P2-M028 — Hermetic repository gate under Git hooks and in worktrees

Status: Approved
Milestone: P2-M028
Created: 2026-09-24
Owner: AgentForge project

## Goal

Make `./scripts/gate.sh` hermetic when it runs from a Git hook or inside a linked worktree, which is
exactly what happens when anything commits inside an AgentForge task worktree:

1. the test suite must never act on the enclosing repository through inherited Git variables; and
2. a worktree's gate must never test binaries built from a different checkout.

## Non-goals

- No changes to the Rust test fixtures in this milestone. The gate is the boundary where hook
  environment meets the test suite, so it is fixed there.
- No change to the plan-policy check itself, which legitimately reads the hook's index.

## Context

Found by the first real-agent run (P2-M027, task P1-M007-T0001, 2026-09-24). In a linked worktree,
Git exports absolute `GIT_DIR` and `GIT_INDEX_FILE` to hooks. In the main checkout, `GIT_DIR` is not
set and `GIT_INDEX_FILE` is the relative `.git/index`; this was verified in a scratch repository.
The pre-commit hook runs `./scripts/gate.sh precommit`, which runs `cargo test`. Test fixtures such
as `operator_actions.rs` shell out to `git init`/`config`/`add`/`commit` in temporary directories
without clearing those variables, so under a worktree hook they acted on the real repository:

- `git init` set `core.bare = true` in the shared `.git/config`, which broke every Git command in
  the main checkout;
- `git config` appended a `[user]` section (`AgentForge Test <agentforge@example.invalid>`);
- a fixture commit (`initial`, README replaced with `fixture`) landed on the task branch together
  with the agent's staged work.

The operator repaired the config (backup kept at `~/agentforge-git-config.damaged`). Nothing was
pushed. Every commit before this was made from the main checkout, which is why the leak never
appeared.

A second leak surfaced while committing this plan. This operator machine sends every Cargo build to
one shared target directory (`~/.cargo/config.toml`, `/home/raven/.cargo-target`). The agent's
worktree build wrote its own `forge` binary to `~/.cargo-target/debug/forge`. The main checkout's
next gate run then considered its own build fresh, did not copy its binary back over that shared
path, and the CLI integration tests ran the worktree's binary (P1-M007 behavior) against
main-branch expectations. They failed with "required gate is not configured: full".

Classification: semantic/test (non-hermetic test environment and shared build output), with
infrastructure impact.

## Architecture placement

`scripts/gate.sh`:

- The text-policy, repository, and plan-policy steps keep the inherited environment, because plan
  policy must inspect the hook's index. Before the cargo steps, the gate unsets every `GIT_*`
  variable, so tests and build scripts see only their own repositories.
- When the checkout is a linked worktree (its `--git-dir` differs from `--git-common-dir`), the
  gate sets `CARGO_TARGET_DIR` to `<configured target dir>/agentforge-worktrees/<worktree name>`.
  The main checkout keeps the configured target directory, so every checkout tests only its own
  binaries.
`AGENTS.md` and `CONTRIBUTING.md` are unchanged. `docs/DOGFOODING.md` records the incident.

## Invariants

- No cargo step of the gate inherits `GIT_DIR`, `GIT_INDEX_FILE`, `GIT_WORK_TREE`, or any other
  `GIT_*` variable.
- Plan-policy validation still sees the hook's staged changes.
- A linked worktree's gate builds into its own target directory; the main checkout's does not
  change.

## ADRs

None; the incident and rule are recorded in `docs/DOGFOODING.md` and `docs/GOVERNANCE.md`.

## Public API / CLI

None.

## Compatibility analysis

No behavior change outside hooks. Direct gate runs have no `GIT_*` variables to begin with.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P2-M028-hermetic-gate-under-hooks.plan.md`
- `.plans/ACTIVE`
- `scripts/gate.sh`
- `docs/GOVERNANCE.md`
- `docs/DOGFOODING.md`
- `CHANGELOG.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- Reproduce the incident in a disposable clone: create a linked worktree, stage a change, and commit
  so the real pre-commit hook runs the full gate. Before the fix, the clone's config gains
  `core.bare = true` and a `[user]` section, and the branch gains an `initial` commit. After the
  fix, the config is unchanged, the branch has exactly the intended commit, and the gate passes.
- The main-checkout gate still passes, and a commit that violates plan policy is still rejected
  under the hook.
- In the reproduction, the worktree gate reports its own target directory, and the main checkout's
  CLI tests pass after a worktree build.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Reproduce the failure in a disposable clone (evidence), apply the gate fix, and re-run the
   reproduction.
4. Document it, run the full gate, push, and verify CI.
5. Close, then retry P1-M007 as task `P1-M007-T0002`.

## Failure modes

- If the reproduction still leaks after the fix, the change is not committed; classify and repair
  forward.

## Documentation impact

`docs/DOGFOODING.md` (incident and fix), `docs/GOVERNANCE.md` (the gate is hermetic under hooks),
CHANGELOG.

## Quality gates

- `./scripts/gate.sh full`;
- the linked-worktree hook reproduction before and after the fix;
- push-triggered CI green on all seven jobs.

## Acceptance criteria

- [ ] The reproduction shows the leak before the fix and none after it.
- [ ] Plan policy still works under the hook.
- [ ] CI evidence recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
