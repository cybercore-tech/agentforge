# AgentForge Agent Handoff

## Repository state

- Active milestone: none.
- Active plan: none.
- P0-M001 status: Complete.
- P0-M002 status: Complete.
- P0-M003 status: Complete.
- P0-M004 status: Complete.
- P0-M004 validated implementation head: `e64fe647206ed8a0a7be19e6246c4a9f73557b1f`.
- P0-M004 implementation CI: `35473456133` — all four jobs green.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/TASK_STATE.md`.
5. Read ADR-0009.
6. Run `./scripts/project-status`.
7. Confirm P0-M004 closure CI and post-merge main CI completed successfully.
8. Confirm there is no active plan before starting P0-M005.
9. Never bypass repository hooks or gates.
10. Classify failures before repair.

## Completed P0-M004 work

P0-M004 established the durable task-state foundation:

- deterministic `TaskId` values;
- BTreeMap-backed deterministic `TaskGraph`;
- dependency validation and cycle rejection;
- derived readiness;
- explicit lifecycle transition validation;
- per-task revisions;
- `agentforge-state` persistence boundary;
- dependency-free version-1 binary snapshot codec;
- bounded fail-closed decoding;
- payload checksum validation;
- project-local file store;
- round-trip, determinism, corruption, truncation, size-bound, restored-domain, and replacement tests.

Two implementation CI repair cycles were preserved in history:

- run `35473369262`: formatting/lint + compilation/type;
- run `35473425385`: final formatting/lint only;
- run `35473456133`: 4/4 green.

## Next milestone

The next planned milestone is:

`P0-M005 — Worktree isolation manager`

Do not start it until the P0-M004 closure and post-merge `main` checks are green.
