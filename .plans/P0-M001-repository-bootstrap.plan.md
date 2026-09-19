# Plan: P0-M001 — Repository bootstrap

Status: Approved
Milestone: P0-M001
Created: 2026-09-19

## Goal

Create the smallest reliable AgentForge repository that establishes permanent project identity,
workspace boundaries, command names, project-control documents, milestone numbering, and a strict
local bootstrap gate.

## Non-goals

- No SQLite state engine.
- No task DAG implementation.
- No Git worktree manager.
- No external coding-agent adapter.
- No MCP integration.
- No CI-provider monitor.
- No permissions engine.
- No TUI/HUD.
- No daemon background loop.
- No production deployment support.

## Context

AgentForge is intended to orchestrate multiple interchangeable software-development agents. The
repository must establish its own durable engineering contract before orchestration features are
implemented.

## Architecture placement

P0-M001 creates `agentforge-core`, the `forge` CLI, the `forged` daemon placeholder, `xtask`,
project-control files, ADR/milestone registries, and deterministic bootstrap gates.

## Data flow

No orchestration data flow is implemented in P0-M001. The only executable behavior is
version/identity output and local validation.

## Invariants

- Models are replaceable workers.
- Durable state does not depend on model memory.
- Agents do not share dirty working trees.
- Plans precede implementation.
- Failures are repaired forward.
- Gate failures are never bypassed.

## ADRs

- ADR-0001
- ADR-0002
- ADR-0003
- ADR-0004
- ADR-0005

## Public API / CLI

Initial commands are `forge version`, `forge doctor`, and `forged --version`.

No stable public Rust API is promised yet.

## Compatibility analysis

Workspace uses Rust edition 2024 and Rust 1.85 as the initial minimum toolchain.

## Dependency analysis

Standard library only.

## Expected file boundary

- root control documents;
- `.plans/**`;
- `docs/**`;
- `scripts/**`;
- `.githooks/**`;
- root `Cargo.toml` and `Cargo.lock`;
- `crates/agentforge-core/**`;
- `crates/agentforge-cli/**`;
- `crates/agentforge-daemon/**`;
- `tools/xtask/**`.

## Test-first matrix

| Behavior | Expected |
| --- | --- |
| `cargo check --workspace --all-targets` | success |
| `cargo test --workspace` | success |
| `cargo clippy --workspace --all-targets -- -D warnings` | success |
| `cargo fmt --all --check` | success |
| `cargo run -p agentforge-cli -- version` | prints AgentForge version |
| `cargo run -p agentforge-cli -- doctor` | reports bootstrap environment |
| `cargo run -p agentforge-daemon -- --version` | prints daemon version |
| `cargo run -p xtask -- validate` | validates required repository files |

## Implementation sequence

1. Commit Approved plan/control records.
2. Create Rust workspace and package skeletons.
3. Add minimal CLI/daemon/core behavior.
4. Add `xtask validate`.
5. Add text and Rust gates.
6. Run full gate.
7. Commit implementation separately.
8. Record exact implementation commit.
9. Mark milestone complete only after validation.

## Failure modes

- Mixing plan approval and implementation in one commit.
- Creating external dependencies before they are needed.
- Building orchestration features before repository rules exist.
- Adding daemon behavior before durable state exists.
- Treating a partial green gate as full validation.
- Bypassing format, Clippy, tests, or text policy.

## Documentation impact

This milestone creates the initial project documentation.

## Quality gates

- `./scripts/check-text-files`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo run -p xtask --locked -- validate`

## Acceptance criteria

- [ ] Repository initializes cleanly.
- [ ] Permanent Phase 0 milestone IDs exist.
- [ ] Project-control files exist.
- [ ] ADR registry exists.
- [ ] Rust workspace contains core, CLI, daemon, and xtask packages.
- [ ] `forge version` works.
- [ ] `forge doctor` works.
- [ ] `forged --version` works.
- [ ] `xtask validate` works.
- [ ] Text policy passes.
- [ ] Rust format/check/clippy/test pass.
- [ ] No external dependency is introduced.
- [ ] Plan and implementation are separate commits.

## Completion record

Implementation commit:
CI run: local bootstrap gate
CI result:
Completed:
Notes:
