# AgentForge Agent Handoff

## Repository state

- Active milestone: `P0-M002`.
- Active plan: `.plans/P0-M002-governance-agent-contract.plan.md`.
- Plan status: Approved.
- Implementation status: not started.
- Base main commit: `1910967c9989d7f705f71be7039e6877493e3aa7`.

## Resume checklist

1. Read `PROJECT_SPEC.md`.
2. Read `PROJECT_STATE.md`.
3. Read `AGENTS.md`.
4. Read `.plans/ACTIVE`.
5. Read the P0-M002 plan.
6. Read the governance, roles, task-contract, and approval-boundary documents.
7. Run `./scripts/project-status`.
8. Require the exact plan-only checkpoint to pass `./scripts/gate.sh full`.
9. Classify any failure before editing.
10. Do not begin Rust implementation on a partially validated plan checkpoint.

## Current work

P0-M002 defines:

- canonical agent roles;
- explicit capability grants;
- human approval boundaries;
- task/result contract semantics;
- concurrent write ownership rules;
- reviewer independence;
- escalation behavior.

## Next exact action

Run the full gate on the plan-only checkpoint.

Only after it is green may P0-M002 add provider-neutral domain types to `agentforge-core`.
