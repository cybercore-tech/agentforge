# Plan: P0-M004-R001 — Restore gate executable mode

Status: Complete
Milestone: P0-M004
Created: 2026-09-19

## Goal

Restore the executable bit on `scripts/gate.sh` after the completed P0-M004 merge introduced it as
mode `100644`.

## Non-goals

- No Rust changes.
- No task-graph changes.
- No durable-state changes.
- No policy weakening.
- No hook bypass.
- No P0-M005 implementation.

## Context

The merged P0-M004 main tree tracks `.githooks/pre-commit` as executable and the hook invokes
`./scripts/gate.sh precommit`.

The same merged tree tracks `scripts/gate.sh` as mode `100644`, causing the hook and direct gate
execution to fail with `Permission denied`.

The gate succeeds when explicitly invoked through Bash, confirming this is a file-mode regression.

## Architecture placement

Repository workflow infrastructure only.

## Data flow

No runtime data flow changes.

## Invariants

- `scripts/gate.sh` must remain content-identical.
- Its Git mode must change from `100644` to `100755`.
- No policy or gate semantics may change.
- No other implementation file may change.

## ADRs

- ADR-0004 — Plan-first implementation workflow.
- ADR-0005 — Repair failures forward.

## Public API / CLI

None.

## Compatibility analysis

No compatibility impact.

## Dependency analysis

No dependency changes.

## Expected file boundary

Plan checkpoint:

- `.plans/ACTIVE`
- `.plans/P0-M004-post-merge-gate-mode-repair.plan.md`

Repair checkpoint:

- `scripts/gate.sh` — mode only, `100644` -> `100755`

Closure checkpoint:

- `.plans/ACTIVE`
- `.plans/P0-M004-post-merge-gate-mode-repair.plan.md`

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| `git ls-files -s scripts/gate.sh` | mode `100755` after repair |
| `./scripts/gate.sh full` | executes normally |
| gate content | unchanged |
| repository full gate | green |

## Implementation sequence

1. Commit this Approved repair plan.
2. Stage only the executable-bit change.
3. Confirm the blob content is unchanged.
4. Commit the mode repair.
5. Run the exact repair-head full gate.
6. Mark this repair plan Complete and remove `.plans/ACTIVE`.
7. Run the exact closure-head full gate.
8. Push main and require post-merge CI green.
9. Only then begin P0-M005.

## Failure modes

- Changing gate contents while repairing mode.
- Bypassing hooks.
- Beginning P0-M005 before main is green.
- Combining repair implementation with plan activation.

## Documentation impact

Repair plan only.

## Quality gates

- `./scripts/check-text-files`
- `cargo run -p xtask --locked -- validate`
- `cargo run -p xtask --locked -- validate-plan-policy`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `./scripts/gate.sh full`

## Acceptance criteria

- [x] `scripts/gate.sh` is tracked as `100755`.
- [x] Script contents are unchanged.
- [x] Direct gate execution succeeds.
- [x] Exact repair-head full gate is green.
- [x] Exact closure-head full gate is green.

## Completion record

Implementation commit: f3fa50f31f5cbc368e6a51e09acc7a2e106509da
CI run: local exact-head full gate
CI result: success
Completed: 2026-09-19
Notes: Restored scripts/gate.sh executable mode from 100644 to 100755 with no content changes.

Evidence:

- repair was preceded by a separately committed Approved plan;
- scripts/gate.sh content remained unchanged;
- Git mode changed from 100644 to 100755;
- direct ./scripts/gate.sh execution works again;
- exact repair head passed the full local gate;
- no Rust code, dependency, or policy semantics changed.
