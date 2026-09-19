# AgentForge Agent Handoff

## Repository state

- Active milestone: none.
- Active plan: none.
- P0-M001 status: Complete.
- P0-M002 status: Complete.
- P0-M003 status: Complete.
- P0-M004 status: Complete.
- P0-M004 implementation checkpoint: `266a67578c95bf7fd27652d4ce5701932fe1ca72`.
- P0-M004 validation: exact implementation head passed the full local gate.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Run `./scripts/project-status`.
5. Confirm there is no active plan before starting P0-M005.
6. Never bypass repository hooks or gates.
7. Classify failures before repair.
8. Repair forward rather than erasing repository history.

## Completed P0-M004 work

P0-M004 established:

- deterministic task identity;
- explicit task lifecycle state;
- deterministic DAG storage;
- dependency validation;
- cycle detection;
- readiness queries;
- versioned durable snapshots;
- deterministic encoding;
- bounded decoding;
- corruption detection;
- atomic local publication;
- generation monotonicity;
- graph validation after load.

## Architecture boundary

`agentforge-core` owns task semantics.

`agentforge-state` owns persistence.

## Next milestone

`P0-M005 — Worktree isolation manager`

P0-M005 builds task-scoped Git worktree isolation on the deterministic task identities introduced
by P0-M004.
