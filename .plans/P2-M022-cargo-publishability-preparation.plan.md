# Plan: P2-M022 — Cargo publishability preparation

Status: Approved
Milestone: P2-M022
Created: 2026-09-20
Owner: AgentForge project

## Goal

Prepare the `agentforge-platform` package for a future, explicitly approved crates.io decision by
making its manifest metadata and local dependency declarations registry-aware, producing a
verifiable package artifact, and documenting the remaining coordinated-publication gate.

## Context

P2-M020 selected `agentforge-platform` as the future end-user package identity but intentionally
left every workspace crate private. The current package dry-run fails before archive creation
because local dependencies such as `agentforge-adapter` do not specify version requirements, and
Cargo warns that the package manifest does not expose complete registry metadata. A package can be
prepared without publishing it; the project must not accidentally imply that private internal
crates are already available from crates.io.

## Scope

- Add complete package metadata to the end-user `agentforge-platform` manifest using the existing
  workspace identity, repository, README, license, and release-policy values.
- Add explicit version requirements alongside `path` for the end-user package's internal
  dependencies, keeping local development and the lockfile behavior unchanged.
- Add a deterministic package-preflight/check path that distinguishes archive construction from
  registry resolution and reports the remaining unpublished-private dependency gate clearly.
- Validate the generated package contents, manifest metadata, included README/license files, and
  binary targets without contacting crates.io or publishing anything.
- Update registry/release guidance and add an ADR describing the prepared-but-not-publishable
  state, dependency publication ordering, and the approval required before any package or internal
  crate becomes public.
- Record P2-M022 state and exact validation evidence in the milestone, project-state, and handoff
  documentation.

## Non-goals

- No `cargo publish`, `cargo owner`, package reservation, yank, transfer request, or registry write.
- No decision to publish `agentforge-core` or any other internal crate; no change from
  `publish = false` in this milestone.
- No consolidation of the workspace into one crate, directory moves, binary renames, or runtime/API
  behavior changes.
- No network-dependent validation requirement; offline/package-preflight evidence must remain
  useful in a clean checkout.

## Architecture placement

This is a distribution-metadata boundary around the existing `agentforge-platform` package. Cargo
manifests, package contents, release policy, and validation tooling are in scope; orchestration,
daemon, worktree, task, audit, and agent contracts are not.

## Invariants

- `forge` and `forged` binary names and the GitHub archive release path remain unchanged.
- All workspace crates remain explicitly private until a separate approved publication plan changes
  that policy.
- Local path resolution remains available for workspace builds; registry version requirements are
  additive metadata, not a switch to a different runtime dependency.
- Packaging never publishes, mutates registry state, or silently substitutes an unrelated crate.
- A failed registry-resolution check remains an explicit, actionable blocker rather than a green
  result inferred from `--no-verify` alone.

## Expected file boundary

- `.plans/P2-M022-cargo-publishability-preparation.plan.md`
- `.plans/ACTIVE`
- `Cargo.toml`
- `crates/agentforge-cli/Cargo.toml`
- `docs/REGISTRY.md`
- `docs/RELEASE.md`
- `docs/adr/ADR-0033-cargo-publishability-preparation.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`
- `scripts/` or `tools/xtask/` only if a bounded package-preflight command is needed

## Test-first matrix

- `cargo metadata --no-deps --format-version 1` confirms the package identity and binary targets.
- `cargo package -p agentforge-platform --allow-dirty --no-verify` creates the expected archive
  contents without registry writes.
- Package-content checks verify the README, license, manifest metadata, source files, and `forge`
  target are present and no `.forge` or unrelated workspace state leaks into the archive.
- A normal package verification run reports the still-private internal dependency gate explicitly;
  it must not be misclassified as a successful publishability result.
- Full local gate and exact-head cross-platform CI remain green.

## Implementation sequence

1. Capture current package metadata, dependency graph, and intentional `cargo package` failure as
   baseline evidence.
2. Add manifest metadata and additive path-plus-version requirements for the end-user package.
3. Implement or extend a bounded package-preflight check and archive-content assertions.
4. Update registry/release policy, ADR-0033, and operator-facing install guidance.
5. Run package checks, the full gate, and exact-head CI; record the remaining publication gate.
6. Perform the separate closure checkpoint without publishing any package.

## Failure modes

- If Cargo rejects a manifest or package file boundary, classify it as packaging/metadata and repair
  the manifest or preflight evidence only.
- If registry resolution cannot succeed while internal crates remain private, preserve that result
  as the documented publication blocker; do not make internal crates public implicitly.
- If package contents contain project-local state, fail closed and correct the include/exclude
  boundary before closure.

## Quality gates

- `cargo fmt --all --check`
- `cargo metadata --no-deps --format-version 1`
- package-preflight/archive-content checks
- `CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/gate.sh full`
- Exact-SHA GitHub CI green across repository policy, stable, MSRV, CLI smoke, Ubuntu, macOS, and
  Windows.

## Acceptance criteria

- [ ] `agentforge-platform` exposes complete registry/package metadata and retains `forge`.
- [ ] Its local dependencies carry explicit registry version requirements without changing local
      workspace resolution.
- [ ] A deterministic package archive can be built and inspected without publishing.
- [ ] The remaining private-dependency/publication gate is explicit and fail-closed.
- [ ] All workspace crates remain private and no registry state changes occur.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
