# AgentForge Agent Handoff

## Repository state

- Active milestone: none; P0-M008 is in Draft planning.
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
- P0-M006 implementation head: `d277a9191421ae5f4018a4aaacf69f349e2b7974`.
- P0-M006 implementation CI: `35482922234` — all four jobs green.
- P0-M006 closure: `bfad41192a4376e1f36d7f2791ddba1e0dabeb2b`; CI `35483034282` green.
- P0-M006 main merge: `023a74969826f1de1bfb80c49e0dda125942336b`; CI `35483102850` green.
- P0-M007 implementation head: `bc115e2c8f12d3459287661e554f02f7adb7b3e4`.
- P0-M007 implementation CI: `35483980498` — all four jobs green.
- P0-M007 closure: `02242b2f3fe7eb82239e2dca43b1bd85aa239a86`; CI `35484071505` green.
- P0-M007 main merge: `741cf58caa6fca38c4693c4359816f4721a2b126`; CI `35484138853` green.
- Current planning branch: `feat/p0-m008-ci-monitor-classifier`.
- P0-M008 plan: `.plans/P0-M008-ci-monitor-failure-classifier.plan.md` — Draft.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/WORKTREE_ISOLATION.md`.
5. Read ADR-0002 and ADR-0010.
6. Run `./scripts/project-status`.
7. Read the P0-M008 Draft plan and proposed ADR-0013.
8. Require plan approval and green CI for the committed Approved plan before implementation.
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

## P0-M007 work

P0-M007 added a standard-library-only gate engine with direct explicit executable/argument
configuration, cleared child environments, bounded concurrent raw output capture, timeout and
output-limit termination, ordered batch reports, and duplicate-name rejection. Its implementation,
closure, and post-merge CI are green. P0-M008 will add exact-SHA CI observation and conservative
failure classification before later audit and policy milestones.
