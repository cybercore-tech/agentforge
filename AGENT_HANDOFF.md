# AgentForge Agent Handoff

## Repository state

- Active milestone: `P0-M005`.
- Active plan: `.plans/P0-M005-worktree-isolation-manager.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `93f88b1af2f129c63b95bb0d9f7ca72677c847af`.
- P0-M004 post-repair main CI: `35477705487` — success.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/TASK_STATE.md`.
5. Read `docs/WORKTREE_ISOLATION.md`.
6. Read ADR-0002 and ADR-0010.
7. Run `./scripts/project-status`.
8. Confirm `.plans/ACTIVE` points to the Approved P0-M005 plan.
9. Never bypass hooks or gates.
10. Classify failures before repair.
11. Never use destructive Git cleanup as routine recovery.

## P0-M005 objective

Establish a safe task-owned Git worktree lifecycle:

- deterministic task branch;
- deterministic managed path;
- exact base commit;
- ownership verification;
- porcelain inspection;
- dirty-state detection;
- non-destructive retirement;
- preserved task branch.

## Next exact action

Commit this plan-only checkpoint and require the exact plan head to pass local and remote gates
before implementation begins.
