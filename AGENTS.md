# AgentForge Agent Rules

These rules apply to every human or automated agent changing this repository.

## Required workflow

1. Read `PROJECT_SPEC.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `.plans/ACTIVE`.
2. Implementation requires an Approved plan already committed in `HEAD`.
3. A plan-only checkpoint must pass validation before implementation begins.
4. Keep changes inside the plan's declared file boundary.
5. Run the required gate before commit.
6. Never bypass hooks or validation with `--no-verify`.
7. Never use `git reset --hard` or `git clean` as routine failure recovery.
8. Repair failures forward after inspecting repository state.
9. Do not weaken tests, lint, docs checks, or CI to make a change pass.
10. Record exact implementation and validation evidence before milestone closure.

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
