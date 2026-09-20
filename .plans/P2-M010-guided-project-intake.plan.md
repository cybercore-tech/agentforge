# Plan: P2-M010 — Guided project intake surface

Status: Approved
Milestone: P2-M010
Created: 2026-09-20

## Goal

Add a bounded, line-oriented operator workflow for authoring and reviewing AgentForge project
intake from the terminal. Operators should be able to create or revise the structured blueprint,
guidelines, and an explicit task draft without hand-editing multiple files or accidentally
mutating durable state. The existing `.forge/` documents and task snapshot remain the source of
truth, and the workflow must be suitable for a future TUI to call or replace.

## Non-goals

- Do not build a raw-terminal TUI, alternate dashboard, or persistent interactive session.
- Do not add network services, provider integrations, natural-language task decomposition, or
  automatic capability/approval grants.
- Do not change daemon, scheduler, worktree, audit, policy, or task-transition semantics.
- Do not replace the existing explicit `init`, `blueprint validate`, or `task create` commands;
  the guided surface composes the same validated boundaries.
- Do not introduce a second mutable configuration or task-state store.

## Context

P1-M003 established versioned, bounded `.forge/blueprint.conf` and `.forge/guidelines.md` files,
plus explicit command-line task creation. P2-M001 through P2-M009 added observation, controlled
actions, daemon operation, and persisted audit continuity, but project intent is still authored by
manually editing files and assembling a long task command. Real operator use and the planned HUD
direction call for a small guided intake surface before a full TUI. The workflow must preserve
the current fail-closed validation and non-overwriting guarantees.

## Architecture placement

Keep document parsing, validation, bounded serialization, and atomic file replacement in
`agentforge-intake`. Keep prompt sequencing, terminal I/O, cancellation, preview rendering, and
exit-code mapping in the CLI. Reuse `load`, `initialize`, `build_task`, and `FileTaskStore`; no
new authority boundary is introduced.

## Data flow

1. The CLI accepts a project root and guided-intake mode, then checks that input is an interactive
   line source (or uses an explicitly supplied scripted input for tests).
2. It loads existing documents when present, displays bounded current values and safe defaults,
   and collects blueprint, guideline, and optional task-draft fields in a fixed order.
3. Draft values are parsed into typed intake structures and validated before any write occurs.
4. The operator receives a deterministic preview and must explicitly confirm the proposed change;
   cancellation or EOF leaves every existing file and task snapshot byte-for-byte unchanged.
5. Confirmed document writes use temporary files in `.forge/` followed by same-directory rename,
   with no overwrite of unrelated paths. A task draft is compiled through the existing task
   builder and persisted through the existing snapshot store only after document validation and
   confirmation succeed.
6. The command reports created/updated paths and task identity without claiming approval or
   execution; later lifecycle commands remain explicit.

## Invariants

- Durable `.forge/blueprint.conf`, `.forge/guidelines.md`, and task snapshots are the only source
  of truth.
- All existing byte, field, list, enum, path, task-ID, and UTF-8 limits remain enforced.
- Invalid input, duplicate task IDs, interrupted input, failed validation, failed rename, and
  declined confirmation are side-effect free.
- Existing files are never silently overwritten; an explicit edit mode must identify each target
  and preserve unrelated content.
- Guideline prose cannot grant capabilities, approvals, paths, gates, or execution authority.
- No shell interpolation, shell evaluation, routine cleanup, or hidden process execution is used.
- Output and diagnostics are deterministic and bounded for redirected/scripted input.

## ADRs

- Existing ADR-0020 remains authoritative for durable intake documents and task authority.
- Existing ADR-0021/0022 remain authoritative for read-only HUD behavior; this command is a
  separate, explicit mutation path.
- Add an ADR only if atomic edit semantics or the guided-input contract cannot be captured by the
  existing decisions; otherwise document the extension in `docs/BLUEPRINT.md` and
  `docs/ORCHESTRATION.md`.

## Public API / CLI

Add one explicit guided-intake command with a stable synopsis (for example,
`forge intake <root> [--task ...]`); the final spelling and option set must be documented and
tested within this milestone. It must support:

- create-or-edit blueprint and guidelines through bounded prompts;
- optional task-draft collection using the existing role, milestone, goal, path, capability,
  approval, gate, output, evidence, and dependency vocabulary;
- deterministic preview, explicit confirmation, and cancellation/EOF;
- a non-interactive scripted-input path for automation and integration tests without granting
  extra authority.

The existing commands retain their current output and compatibility behavior.

## Compatibility analysis

The Rust 1.85 standard-library-only baseline remains unchanged. Existing documents, CLI commands,
snapshot formats, HUD output, daemon behavior, and audit sequence rules remain readable and
unchanged. New prompts must not depend on terminal-specific escape sequences, so redirected input
and Windows/macOS/Linux CI behave consistently.

## Dependency analysis

No new external dependencies. Use standard-library buffered I/O, bounded line reads, temporary
files, and rename semantics already available to the supported platforms. `Cargo.lock` should not
change unless an already-approved implementation need proves otherwise.

## Expected file boundary

Implementation may modify only:

- `.plans/P2-M010-guided-project-intake.plan.md` (status/completion record during approval/closure);
- `crates/agentforge-intake/src/lib.rs`;
- `crates/agentforge-intake/Cargo.toml` only if existing metadata must be extended (no dependency
  additions without plan amendment);
- `crates/agentforge-cli/src/main.rs`;
- `crates/agentforge-cli/tests/intake_commands.rs` and focused intake unit tests;
- `docs/BLUEPRINT.md`, `docs/ORCHESTRATION.md`, and a focused ADR only when required to document
  the accepted CLI contract;
- `docs/MILESTONES.md`, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md` only during closure evidence.

No daemon, scheduler, worktree, audit, policy, HUD implementation, release workflow, or unrelated
workspace files may change.

## Test-first matrix

| Case | Expected evidence |
| --- | --- |
| Guided creation from empty project | Valid bounded documents are created and validate through existing loader. |
| Guided edit of existing files | Only selected targets change; unrelated bytes remain intact. |
| Preview declined or EOF | No document or task snapshot mutation. |
| Invalid version, enum, path, oversized, or malformed input | Source-located diagnostics and no mutation. |
| Optional task draft | Existing `build_task`/`TaskGraph` validation and snapshot persistence are used. |
| Duplicate task ID or invalid dependency | Failure is side-effect free. |
| Scripted input and bounded lines | Same deterministic result on Linux, macOS, and Windows. |
| Existing CLI regression | Current init, validate, and task-create integration tests remain green. |

## Implementation sequence

1. Finalize the command synopsis, prompt schema, bounds, preview format, and confirmation rules in
   tests and docs.
2. Add pure intake helpers for bounded prompt parsing, draft validation, deterministic rendering,
   and atomic document serialization.
3. Wire the CLI command to those helpers and existing task/snapshot APIs, preserving explicit
   lifecycle boundaries.
4. Add unit and integration coverage for success, cancellation, invalid input, duplicate state,
   and cross-platform scripted input.
5. Run formatting, unit/integration tests, full gate, and exact CI evidence before closure.

## Failure modes

- Missing or corrupt existing intake source: fail closed with the existing source-labelled error;
  do not replace it automatically.
- Input exceeds a bound or contains an unknown enum: report the field and leave state untouched.
- Atomic write or snapshot save fails: report the filesystem error and retain the prior state.
- Concurrent external edit detected between preview and commit: reject the commit using a stable
  source fingerprint rather than overwriting newer content.
- Operator declines or disconnects: exit successfully as a cancellation with no mutation.

## Documentation impact

Update `docs/BLUEPRINT.md`, `docs/ORCHESTRATION.md`, and README command examples with the final
guided workflow, its scripted-input contract, preview/confirmation behavior, and the distinction
between intake authoring and task approval. Record milestone evidence in the normal closure files.

## Quality gates

- `cargo fmt --all -- --check`.
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`.
- CLI intake integration tests plus the complete workspace test suite.
- `git diff --check` and repository policy checks.
- Exact-commit remote CI green across the required stable, MSRV, policy, smoke, and platform jobs.

## Acceptance criteria

- [ ] Operators can create or edit validated blueprint/guideline documents through one documented,
      bounded guided command.
- [ ] Operators can optionally compile and persist an explicit task draft through the existing
      task contract path.
- [ ] Preview, confirmation, cancellation, EOF, invalid input, and concurrent-edit safeguards are
      deterministic and side-effect safe.
- [ ] Existing intake, task, HUD, daemon, audit, and policy behavior remains compatible.
- [ ] Tests, full local gate, documentation, and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
