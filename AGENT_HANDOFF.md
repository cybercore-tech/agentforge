# AgentForge Agent Handoff

## Repository state

- Active milestone: none.
- Active plan: none.
- P0-M001 status: Complete.
- P0-M002 status: Complete.
- P0-M003 status: Complete.
- P0-M003 validated implementation head: `e095d3e8fddcd35d0ca1803b0a62cf0aae7781b0`.
- P0-M003 implementation CI run: `35472661178` — all four jobs green.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `docs/CI.md`.
5. Run `./scripts/project-status`.
6. Confirm there is no active plan before starting new implementation.
7. Confirm P0-M003 closure CI and post-merge main CI completed successfully.
8. Never bypass repository hooks or gates.
9. Classify failures before repair.
10. Repair forward rather than destroying repository state.

## Completed P0-M003 work

P0-M003 established:

- mechanical active-plan validation;
- Approved-plan authority checks for implementation;
- plan/implementation commit separation;
- local policy enforcement through `scripts/gate.sh`;
- GitHub Actions remote validation;
- independent Repository policy, Stable code gate, MSRV, and CLI smoke jobs;
- exact-head CI evidence;
- documented target protection rules for `main`.

The first implementation run exposed a rustfmt-only failure. It was classified before repair and
fixed with a formatting-only commit. The next exact-head run passed all four jobs.

## Next milestone

The next planned milestone is:

`P0-M004 — Task graph and durable state`

Do not start it until the P0-M003 closure and post-merge `main` checks are green.
