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

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/WORKTREE_ISOLATION.md`.
5. Read ADR-0002 and ADR-0010.
6. Run `./scripts/project-status`.
7. Confirm P0-M005 closure CI and post-merge `main` CI completed successfully.
8. Confirm there is no active plan before starting P0-M006.
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

Do not start it until the P0-M005 closure and post-merge `main` checks are green.
