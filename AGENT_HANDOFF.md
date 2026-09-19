# AgentForge Agent Handoff

## Repository state

- Active milestone: `P0-M003`.
- Active plan: `.plans/P0-M003-plan-first-workflow-enforcement.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `ebe5d12608e233f87645b12e574981e173b145a7`.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `.plans/ACTIVE`.
5. Read the P0-M003 plan.
6. Read `docs/CI.md`.
7. Run `./scripts/project-status`.
8. Validate the exact plan-only checkpoint with `./scripts/gate.sh full`.
9. Classify any failure before editing.
10. Do not begin implementation on a failed plan checkpoint.

## Current work

P0-M003 makes plan-first development mechanically enforceable and bootstraps the repository's first
GitHub Actions workflow.

The implementation is expected to:

- strengthen active-plan validation;
- require an Approved plan already committed before implementation;
- reject plan approval mixed with implementation;
- preserve closure/no-active-plan validity;
- strengthen pre-commit policy enforcement;
- add independent policy/stable/MSRV/smoke CI jobs;
- establish exact-head remote CI evidence.

## Bootstrap note

The P0-M003 plan checkpoint predates GitHub Actions and therefore uses the existing local full gate
as its checkpoint authority.

After CI is introduced, remote exact-head validation is mandatory.
