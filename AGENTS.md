# AgentForge Agent Rules

These rules apply to every human or automated agent changing this repository.

## Required workflow

1. Read `PROJECT_SPEC.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `.plans/ACTIVE` when it exists.
2. Implementation requires an Approved plan already committed in `HEAD`.
3. A plan-only checkpoint must pass validation before implementation begins.
4. Plan approval and implementation must not be introduced by the same commit.
5. Keep changes inside the plan's declared file boundary.
6. Run the required gate before commit.
7. Never bypass hooks or validation with `--no-verify`.
8. Never use `git reset --hard` or `git clean` as routine failure recovery.
9. Repair failures forward after inspecting repository state.
10. Do not weaken tests, lint, docs checks, or CI to make a change pass.
11. Record exact implementation and validation evidence before milestone closure.
12. After P0-M003 CI bootstrap, remote green status applies only to the exact commit SHA that produced it.

## Mechanical plan policy

AgentForge treats Markdown and `.plans/ACTIVE` changes as control/documentation changes.

Other staged repository changes are implementation-sensitive and require the current `HEAD` to
already contain:

- `.plans/ACTIVE`;
- an active plan under `.plans/`;
- `Status: Approved` in that committed plan.

This makes a plan-only commit legal while preventing the same commit from both creating its
implementation authority and using that authority.

A no-active-plan state is valid between completed milestones.

## Failure classification

Before editing after a failed gate or CI run, classify the failure as one of:

- semantic/test;
- compilation/type;
- formatting/lint;
- generated-content corruption;
- dependency/toolchain;
- documentation/text policy;
- workflow/governance;
- infrastructure.

Repair only the identified category unless new evidence requires broader change.

## Agent boundaries

An agent must not modify files outside its declared task boundary, change dependencies without
explicit approval, access unnecessary secrets, push/merge without capability, or deploy production
without explicit human approval.
