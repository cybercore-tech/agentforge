# Plan: P2-M011 — Guided intake operator hardening

Status: Complete
Milestone: P2-M011
Created: 2026-09-20

## Goal

Make the new guided intake workflow dependable for repeatable operator use by exercising it as a
real first-run flow on disposable projects and adding one explicit, shell-independent scripted
input path. An operator should be able to replay a reviewed intake session, inspect the resulting
durable task contract, and hand it to the existing worktree/daemon path without ambiguity.

## Non-goals

- Do not change task authority, approval, audit, daemon, worktree, scheduler, or agent execution
  semantics.
- Do not add a raw-terminal TUI, JSON state store, network service, or provider integration.
- Do not add an automatic `--yes` bypass or any path that skips preview and confirmation.
- Do not modify a user project during tests; dogfooding uses disposable repositories and bounded
  fixtures only.
- Do not broaden the guided editor into natural-language decomposition or task scheduling.

## Context

P2-M010 delivered `forge intake <root>` and `forge intake <root> --task`, with bounded prompts,
deterministic preview, explicit confirmation, conflict detection, atomic document writes, and
task-snapshot rollback. The command accepts redirected stdin, but a repeatable operator session
currently depends on shell redirection and gives no named input artifact for review or replay. The
next increment should validate the complete intake-to-inspect handoff and harden only gaps observed
through disposable-project use.

## Architecture placement

Keep the prompt schema and validation in `agentforge-intake`/`agentforge-cli`. Add a direct file
reader at the CLI boundary that feeds the exact same prompt engine as stdin; it must not become a
second parser or source of authority. Keep dogfood fixtures in CLI integration tests and document
the workflow in existing intake/operator docs.

## Data flow

1. The operator prepares a bounded UTF-8 session file containing the same line-oriented answers
   accepted by stdin.
2. `forge intake <root> [--task] --input-file <path>` opens that file directly, rejects invalid,
   oversized, or unreadable input before mutation, and runs the existing prompt sequence.
3. The command renders the same preview and still requires explicit `y`/`yes` confirmation from
   the session input; no flag bypasses confirmation.
4. The resulting blueprint, guidelines, and optional task snapshot are validated with the existing
   loaders and then inspected through the existing read-only task command in the dogfood test.
5. A second replay against the same project demonstrates deterministic duplicate/cancellation
   behavior and leaves state unchanged when confirmation is declined.

## Invariants

- Stdin and `--input-file` produce identical parsing, validation, preview, and mutation behavior.
- Input files are read directly as data; no shell interpolation, command execution, or path search
  is introduced.
- Input is bounded before allocation and must be valid UTF-8; missing or malformed files fail
  closed without creating `.forge` state.
- Preview and confirmation remain mandatory; scripted input cannot grant capabilities or approvals.
- The durable blueprint, guidelines, and task snapshot remain the only source of truth.
- Existing commands and output remain compatible unless a test demonstrates a concrete defect.

## ADRs

- ADR-0020 remains authoritative for intake documents and task authority.
- ADR-0021/0022 remain authoritative for read-only HUD behavior.
- No new ADR is expected; document the named input-file extension in `docs/BLUEPRINT.md` and
  `docs/ORCHESTRATION.md`.

## Public API / CLI

Extend the guided command with:

```text
forge intake <root> [--task] [--input-file <path>]
```

`--input-file` is optional and mutually exclusive with any future alternate input mode. The path
is passed directly to the standard-library file reader. The command must preserve the current
interactive behavior and all existing diagnostics when the option is absent.

## Compatibility analysis

The standard-library-only Rust 1.85 baseline and all durable formats remain unchanged. The input
file is an operator convenience, not a persisted project artifact, and is never copied into `.forge`.
The direct reader must work on Linux, macOS, and Windows with ordinary filesystem paths.

## Dependency analysis

No new dependencies. Use `std::fs::File`, bounded reads, and the existing prompt reader.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M011-guided-intake-operator-hardening.plan.md` (approval/completion metadata);
- `crates/agentforge-cli/src/main.rs`;
- `crates/agentforge-cli/tests/intake_commands.rs`;
- `crates/agentforge-intake/src/lib.rs` only if a shared bounded-input helper is required;
- `docs/BLUEPRINT.md`, `docs/ORCHESTRATION.md`, and focused README examples;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No daemon, scheduler, audit, policy, worktree, release, or unrelated workspace files may change.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Empty disposable project via input file | Valid documents are created and validate through existing loader. |
| Input-file task flow | Task snapshot contains the requested contract and `task inspect` reports it. |
| Interactive stdin regression | Existing guided tests remain green and output is unchanged. |
| Missing, unreadable, invalid UTF-8, or oversized input file | Bounded diagnostic and no `.forge` mutation. |
| Declined confirmation and replay | No mutation; duplicate task behavior remains explicit and safe. |
| Cross-platform path handling | Direct file input works on Linux, macOS, and Windows fixtures. |
| Real handoff projection | Blueprint validation, task inspection, and HUD remain consistent after intake. |

## Implementation sequence

1. Run the guided flow against disposable projects and record any concrete operator friction or
   semantic defect; amend this plan before widening scope if evidence requires it.
2. Add bounded `--input-file` argument parsing and route it through the existing prompt reader.
3. Add failure-side-effect tests and a complete intake-to-validate/inspect/HUD integration fixture.
4. Update command documentation and replay guidance with the final input-file contract.
5. Run formatting, full local gate, exact implementation CI, and closure evidence.

## Failure modes

- Input path is missing, not a regular readable file, invalid UTF-8, or exceeds the bound: fail
  before prompting or creating project state.
- Input ends before confirmation: report cancellation and leave every durable source unchanged.
- A replay encounters an existing task ID: reject it through the existing task graph validation;
  never overwrite the existing record.
- A source changes after preview: retain the existing conflict diagnostic and rollback semantics.

## Documentation impact

Document `--input-file`, its bounds, replay/cancellation behavior, and the recommended disposable
project dogfood sequence in `docs/BLUEPRINT.md`, `docs/ORCHESTRATION.md`, and the README workflow.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- CLI intake integration tests and complete workspace tests.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and Windows.

## Acceptance criteria

- [x] `--input-file` reuses the stdin prompt engine without changing authority or confirmation rules.
- [x] Missing, malformed, and oversized input files fail before durable mutation.
- [x] Disposable-project dogfood proves intake → validation → task inspection/HUD consistency.
- [x] Existing interactive intake behavior and all prior lifecycle boundaries remain compatible.
- [x] Tests, documentation, local gate, and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit: `7b40c0a83cf022d66462270d4d4ca6fc66c40975`.
CI run: `35529284464`.
CI result: Green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and Windows.
Completed: 2026-09-20.
Notes: Added bounded UTF-8 `--input-file` replay through the existing guided prompt engine,
failure-side-effect coverage, and disposable intake → validate → inspect/HUD dogfooding. Preview
and explicit confirmation remain mandatory; no input file is persisted into project state.
