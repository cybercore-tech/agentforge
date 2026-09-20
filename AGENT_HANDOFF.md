# AgentForge Agent Handoff

## Repository state

- Active milestone: none.
- Active plan: none.
- P0-M001 status: Complete.
- P0-M002 status: Complete.
- P0-M003 status: Complete.
- P0-M004 status: Complete.
- P0-M005 status: Complete.
- P0-M005 validated implementation head: `e302cdae8a6610be04ec244bcefa2eab7c768864`.
- P0-M005 implementation CI: `35481590769` — all four jobs green.
- P0-M005 closure: `259f4c06844898c7cd99aeb29b38f23f5688de3a`; CI `35481797256` green.
- P0-M005 main merge: `069c058f7d39ab39a3267f5893d20b963d4f5397`; CI `35481855409` green.
- Current planning branch: `feat/p0-m006-agent-adapter-interface`.
- P0-M006 plan: `.plans/P0-M006-agent-adapter-interface.plan.md` — Draft.
- Implementation has not started; `.plans/ACTIVE` remains absent pending approval.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/WORKTREE_ISOLATION.md`.
5. Read ADR-0002 and ADR-0010.
6. Run `./scripts/project-status`.
7. Read the P0-M006 Draft plan and proposed ADR-0011.
8. Require human approval and green CI for the committed Approved plan before implementation.
9. Never bypass repository hooks or gates.
10. Classify failures before repair.

## Completed P0-M005 work

P0-M005 established the task-owned Git worktree lifecycle:

- deterministic `agentforge/task/<task-id>` branches;
- deterministic `.forge/worktrees/<task-id>` paths;
- exact base commit resolution;
- authoritative `git worktree list --porcelain -z` inspection;
- task, path, and branch ownership checks;
- managed-only deterministic listing;
- tracked and untracked dirty-state detection;
- unresolved merge, rebase, cherry-pick, and revert detection;
- conservative non-forced retirement;
- task-branch preservation;
- no shell interpolation or routine destructive cleanup;
- isolated concurrent integration fixtures;
- ambient `GIT_INDEX_FILE` isolation for Git subprocesses.

Exact implementation CI run `35481590769` passed all four jobs for
`e302cdae8a6610be04ec244bcefa2eab7c768864`.

## Next milestone

The next planned milestone is:

`P0-M006 — Agent adapter interface`

P0-M005 closure and post-merge checks are verified green. Next, review and approve the P0-M006
plan. Activation requires a separate Approved plan commit and `.plans/ACTIVE`, followed by local
and remote validation of that exact commit. Draft-plan CI does not authorize implementation.
