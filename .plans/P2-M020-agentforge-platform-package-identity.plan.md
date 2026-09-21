# Plan: P2-M020 — AgentForge platform package identity

Status: Complete
Milestone: P2-M020
Created: 2026-09-20
Owner: AgentForge project

## Goal

Adopt `agentforge-platform` as the public-facing Cargo package identity for the AgentForge
operator distribution while preserving the existing `forge` and `forged` binaries, internal crate
boundaries, and GitHub artifact release path.

## Context

The package name `agentforge` and related names such as `agentforge-cli` are occupied on crates.io
by unrelated projects. The project has selected `agentforge-platform` as a clearer public identity.
The current workspace package is named `agentforge-cli`, is private, and is referenced by CI,
release workflows, documentation, and Cargo metadata. A controlled package-identity migration is
needed before any future registry publication discussion.

## Scope

- Rename the end-user CLI workspace package from `agentforge-cli` to `agentforge-platform` while
  keeping its directory, source modules, fixture target, and `forge` binary stable.
- Update workspace package selectors, Cargo lock metadata, CI smoke commands, release packaging
  commands, and operator documentation to use the new package identity.
- Keep all implementation library crates private and retain the existing `forge`/`forged` GitHub
  release archives and local source-install behavior.
- Update the registry/release policy and add an ADR recording the selected identity and the decision
  to defer crates.io publication until a separately approved publishability plan resolves private
  path dependencies.
- Record P2-M020 state and completion evidence in the milestone, project-state, and handoff files.

## Non-goals

- No `cargo publish`, crates.io ownership transfer, package reservation, or release announcement.
- No rename of internal library crates such as `agentforge-core`, no dependency changes, and no
  public API or runtime behavior changes.
- No directory move for `crates/agentforge-cli`; the source path remains a stable implementation
  detail for this increment.
- No rename of the `forge` or `forged` binaries and no change to archive names or workflow
  permissions.

## Expected file boundary

- `.plans/ACTIVE`
- `.plans/P2-M020-agentforge-platform-package-identity.plan.md`
- `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-cli/Cargo.toml`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `README.md`
- `docs/BLUEPRINT.md`
- `docs/REGISTRY.md`
- `docs/RELEASE.md`
- `docs/adr/ADR-0032-agentforge-platform-package.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Validation

- `cargo metadata --no-deps` reports exactly one end-user package named `agentforge-platform` and
  retains the `forge` binary target.
- CI, release, README, and blueprint command examples no longer select `agentforge-cli` as a
  package identity.
- All workspace crates remain `publish = false`; a package dry-run may document private path
  dependency limitations but must not publish anything.
- The full local gate passes, including package identity and documentation checks.
- Exact implementation and closure CI evidence is recorded before the milestone is closed.

## Acceptance criteria

- [x] The end-user package is named `agentforge-platform` in Cargo metadata.
- [x] `forge` and `forged` behavior, binary names, tests, and release archive layout are unchanged.
- [x] Workspace, CI, release, README, and blueprint references use the selected package identity.
- [x] Internal crates remain private and no crates.io publication occurs.
- [x] ADR, registry policy, milestone state, and handoff records describe the new identity and the
      deferred publishability gate.
- [x] Full local validation and exact-SHA CI evidence are recorded in the completion record.

## Implementation sequence

1. Change the package identity and Cargo/workflow selectors; refresh the lockfile.
2. Update documentation, registry policy, and ADR-0032.
3. Run metadata, package dry-run, documentation, and full-gate checks.
4. Commit implementation, verify exact-head CI, then perform separate closure evidence commits.

## Completion record

Implementation commit:
`02cf453ab778c4c2aa9e44d0c83d9378b8ac142e`
Exact CI run:
`35562749505` (rerun of failed macOS job; all seven jobs green)
Exact CI result:
green
Closure commit:
pending
Closure CI run:
pending
Closure CI result:
pending
Completed:
2026-09-20
Notes:
The selected package identity is private and not published. The source directory remains
`crates/agentforge-cli`; `forge`, `forged`, and GitHub release archives are unchanged. A future
publishability plan must add registry-compatible version requirements for private path dependencies
before any package publication is considered.
