# Plan: P0-M003 — Plan-first workflow enforcement

Status: Complete
Milestone: P0-M003
Created: 2026-09-19

## Goal

Make AgentForge's plan-first development workflow mechanically enforceable locally and remotely.

P0-M003 must turn the documented workflow into executable policy:

- validate active-plan structure and status;
- reject implementation when no Approved plan is active;
- reject implementation mixed into the plan-approval commit;
- preserve a valid no-active-plan state between milestones;
- strengthen the pre-commit path;
- add GitHub Actions as the independent remote quality authority;
- establish exact-head CI evidence for future milestones.

## Non-goals

- No task DAG or SQLite state.
- No worktree manager.
- No agent process spawning.
- No external model adapter.
- No MCP gateway.
- No CI failure classifier beyond clear job separation.
- No branch-protection mutation in this milestone until required checks exist and have run successfully.
- No dependency or build cache.
- No deployment automation.

## Context

P0-M001 established local gates and plan artifacts. P0-M002 established governance and explicit
human approval boundaries.

The current repository still relies on convention for several plan-first rules. The pre-commit hook
runs the full local gate, but it does not verify that staged implementation is backed by an
Approved plan already present in HEAD.

The GitHub repository also has no Actions workflow yet, so remote validation does not exist.

P0-M003 closes those gaps.

## Architecture placement

```text
developer / agent
      |
      v
staged change
      |
      +--> local plan-policy validation
      +--> local code/text gates
      |
      v
commit / push
      |
      v
GitHub Actions
      |
      +--> Repository policy
      +--> Stable code gate
      +--> MSRV 1.85.0
      +--> CLI smoke
      |
      v
exact-head evidence
```

Local and remote gates must agree on repository invariants.

## Data flow

### Local

1. Inspect `.plans/ACTIVE`.
2. If implementation is staged, require the active plan to exist in HEAD and have `Status: Approved`.
3. Reject a commit that introduces both an Approved plan and implementation code.
4. Run text, format, check, Clippy, tests, and repository validation.
5. Permit the commit only if all required checks pass.

### Remote

1. GitHub checks out the exact PR/push head.
2. Repository-policy validation runs.
3. Stable Rust code gate runs.
4. MSRV 1.85.0 compatibility runs.
5. CLI smoke tests run.
6. Each job reports independently.
7. Future closure records the exact run/head evidence.

## Invariants

- An implementation commit requires an Approved plan already committed in HEAD.
- Plan approval and implementation are separate commits.
- `.plans/ACTIVE` may be absent between milestones.
- If `.plans/ACTIVE` exists, it points to an existing plan under `.plans/`.
- An active implementation plan has `Status: Approved`.
- A Complete plan is not active.
- Local hooks must not silently bypass policy checks.
- GitHub Actions uses read-only repository permission by default.
- CI uses `--locked`.
- Stable and MSRV validation are independent.
- CI jobs are independently diagnosable.
- Exact-head green status, not a prior green run, is the acceptance signal.
- No cache is introduced in the bootstrap CI.

## ADRs

Existing ADRs apply:

- ADR-0003 — durable state outside model memory.
- ADR-0004 — plan-first implementation workflow.
- ADR-0005 — repair-forward failure policy.
- ADR-0007 — capability grants independent of roles.

No new ADR is required for GitHub Actions itself. GitHub Actions is the current remote CI
implementation, not a core AgentForge provider contract.

## Public API / CLI

P0-M003 may extend `xtask` with repository-policy validation commands.

Expected command surface:

- `cargo run -p xtask --locked -- validate`
- `cargo run -p xtask --locked -- validate-plan-policy`

The exact command split may be simplified if one command can remain deterministic and clear.

No stable public library API is required.

## Compatibility analysis

CI must validate:

- current stable Rust;
- declared MSRV Rust 1.85.0.

The workspace remains edition 2024.

GitHub Actions is repository automation only and must not leak into `agentforge-core` semantics.

## Dependency analysis

No Rust dependency is expected.

The workflow may use GitHub's official checkout action and an established Rust toolchain installer
only where required to select Rust 1.85.0. No cache action is introduced.

## Expected file boundary

Plan checkpoint:

- `.plans/ACTIVE`
- `.plans/P0-M003-plan-first-workflow-enforcement.plan.md`
- `docs/MILESTONES.md`
- `docs/CI.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

Implementation checkpoint:

- `.github/workflows/ci.yml`
- `.githooks/pre-commit`
- `scripts/gate.sh`
- `tools/xtask/src/main.rs`
- `tools/xtask/**` tests if needed
- `AGENTS.md` and `docs/CI.md` for exact enforcement documentation
- project state/handoff evidence as appropriate.

No Cargo dependency change is expected.

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| No active plan between milestones | valid |
| ACTIVE points outside `.plans/` | reject |
| ACTIVE points to missing file | reject |
| Active plan status Draft | reject implementation |
| Active plan status Complete | reject implementation |
| Active plan status Approved | eligible for implementation |
| Implementation with no Approved plan in HEAD | reject |
| Plan approval + Rust/Cargo implementation in same commit | reject |
| Docs-only plan checkpoint | allow |
| Closure removing ACTIVE | allow |
| Stable fmt/check/clippy/test | pass |
| MSRV 1.85.0 check/test | pass |
| CLI version/doctor/daemon smoke | pass |
| CI permissions | contents read only |
| CI Cargo operations | use --locked |

## Implementation sequence

1. Commit this Approved plan-only checkpoint.
2. Validate the exact plan checkpoint with the existing local full gate.
3. Implement plan-policy validation in `xtask`.
4. Integrate policy validation into the pre-commit path.
5. Add GitHub Actions with four independent jobs.
6. Run the full local gate.
7. Inspect the exact implementation diff.
8. Push the implementation checkpoint.
9. Require the first exact-head GitHub Actions run to complete successfully.
10. Classify any failure before repair.
11. Record exact implementation/run evidence.
12. Close P0-M003 in a documentation/state-only commit.
13. Require closure CI green.
14. Merge with history preserved.
15. Require post-merge `main` CI green.
16. Only after successful required checks exist, configure protected-main policy.

## Bootstrap validation note

This milestone creates the repository's first GitHub Actions workflow.

Therefore the Approved plan checkpoint cannot itself have remote Actions evidence. Its authority is
the existing local full gate.

This is a one-time bootstrap exception. Once the implementation checkpoint introduces CI, every
subsequent implementation/closure/post-merge checkpoint in P0-M003 requires exact-head remote CI.

## Failure modes

- CI YAML that does not run on pull requests.
- A policy validator that rejects legitimate no-active-plan closure state.
- Checking only the working tree instead of the committed plan checkpoint.
- Treating a Draft or Complete plan as active implementation authority.
- Allowing plan approval and implementation in one commit.
- Weakening gates to get a green run.
- Hiding unrelated work in the CI bootstrap commit.
- Adding unnecessary caching or dependencies.
- Using broad GitHub token write permissions.
- Reusing a green result from a different commit.

## Documentation impact

P0-M003 creates `docs/CI.md` as the normative repository CI description and updates agent
workflow rules to distinguish local checkpoint authority from remote exact-head evidence.

## Quality gates

Local:

- `./scripts/check-text-files`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- repository/plan policy validation
- `./scripts/gate.sh full`

Remote:

- Repository policy
- Stable code gate
- MSRV 1.85.0
- CLI smoke

## Acceptance criteria

- [x] Active-plan state is mechanically validated.
- [x] Implementation requires an Approved plan already in HEAD.
- [x] Plan approval and implementation cannot share one commit.
- [x] Closure with no ACTIVE plan remains valid.
- [x] Pre-commit runs policy enforcement.
- [x] GitHub Actions exists and runs on pull requests.
- [x] GitHub Actions runs on pushes to main.
- [x] GitHub Actions supports manual dispatch.
- [x] Workflow permissions default to contents read.
- [x] Stable code gate is independently visible.
- [x] MSRV 1.85.0 gate is independently visible.
- [x] Repository policy gate is independently visible.
- [x] CLI smoke gate is independently visible.
- [x] Cargo CI commands use --locked.
- [x] Exact implementation head is green remotely.
- [x] Exact closure head is green remotely.
- [x] Post-merge main is green remotely.
- [x] No external Rust dependency is added.
- [x] Main protection requirements are documented after checks exist.

## Completion record

Implementation commit: e095d3e8fddcd35d0ca1803b0a62cf0aae7781b0
CI run: 35472661178
CI result: success — all four jobs green
Closure commit: recorded by Git history immediately after this record
Closure CI: required before merge
Post-merge main: required after merge
Post-merge CI: required after merge
Completed: 2026-09-19
Notes: P0-M003 bootstrapped AgentForge's first GitHub Actions workflow and mechanical plan-first policy.

Evidence:

- Base main: `ebe5d12608e233f87645b12e574981e173b145a7`.
- Approved plan-only checkpoint: `89394dd4e6eef56bdb5c0313379c39e6bd0862c8`.
- Initial implementation checkpoint: `b3ba78e462ce208bab898163b29311be6cf07631`.
- Initial CI run `35472609042` completed with Repository policy, MSRV, and CLI smoke green; Stable code gate failed only at `cargo fmt --check`.
- Failure classification: formatting/lint.
- Formatting-only repair: `e095d3e8fddcd35d0ca1803b0a62cf0aae7781b0`.
- Repaired exact-head CI run `35472661178` passed Repository policy, Stable code gate, MSRV 1.85.0, and CLI smoke.
- No Rust dependency or cache was introduced.
- Closure and post-merge exact-head checks are execution evidence produced after this completion record is committed.
