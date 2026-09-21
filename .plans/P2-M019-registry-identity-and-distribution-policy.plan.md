# Plan: P2-M019 — Registry identity and distribution policy

Status: Approved
Milestone: P2-M019
Created: 2026-09-20
Owner: AgentForge project

## Goal

Make AgentForge's public package identity unambiguous before any crates.io publication. Record
the existing crates.io namespace collision, define the authoritative installation channels, and
establish a release gate that prevents users from accidentally installing an unrelated
`agentforge` crate.

## Context

The crates.io name `agentforge` is already registered at version `0.1.0` by an unrelated project,
and related names such as `agentforge-core` and `agentforge-cli` are also occupied. This repository
currently avoids the collision because its workspace crates are private (`publish = false`) and
the release workflow distributes `forge` and `forged` as GitHub artifacts. The policy must preserve
that safe state while the project's public package names are decided deliberately.

## Scope

- Add a registry and distribution guide that distinguishes the AgentForge brand, the `forge` and
  `forged` binaries, private workspace crates, GitHub release archives, and any future registry
  packages.
- Document the verified occupied names and their unrelated ownership/repository metadata without
  claiming the names, requesting transfer, or implying affiliation.
- Update the release process and README installation guidance with an explicit warning that
  `cargo add agentforge` is not an AgentForge installation and that crates.io publication is not
  yet available.
- Add an ADR recording the namespace decision: keep current crates private, continue binary
  distribution, and require a separately approved naming/publication plan before changing package
  names or publishing.
- Record the milestone and evidence expectations in the milestone, project-state, and handoff
  documents.

## Non-goals

- No crates.io publication, ownership-transfer request, package yank, or contact with another
  crate owner.
- No Cargo package renames, dependency changes, binary renames, or API changes.
- No change to the GitHub release artifact format or release workflow permissions.
- No claim that AgentForge is release-ready; the project remains alpha and pre-release.

## Expected file boundary

- `.plans/ACTIVE`
- `.plans/P2-M019-registry-identity-and-distribution-policy.plan.md`
- `docs/REGISTRY.md`
- `docs/RELEASE.md`
- `docs/adr/ADR-0031-registry-identity-and-distribution.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Validation

- Registry evidence is refreshed with `cargo search`/`cargo info` and records the query date,
  package versions, descriptions, and repository links used for the collision decision.
- Documentation checks confirm that the `cargo add agentforge` warning, official GitHub release
  path, private-crate status, and future-publication gate agree across README, release guidance,
  and the registry guide.
- ADR and milestone/state/handoff records agree on the no-publication/no-rename decision.
- The full local gate passes; no Rust behavior or package metadata changes are introduced.
- Exact implementation and closure CI evidence is recorded before the milestone is closed.

## Acceptance criteria

- [ ] The occupied crates.io names and unrelated ownership are clearly documented with a dated
      evidence record and no implied affiliation.
- [ ] A user cannot mistake `cargo add agentforge` for the official AgentForge installation path.
- [ ] GitHub release archives and local source installation remain the only documented supported
      installation channels in this alpha phase.
- [ ] All workspace crates remain private and no publication or package rename occurs in this
      milestone.
- [ ] A future crates.io naming/publication change is explicitly gated behind a new approved plan.
- [ ] Full local validation and exact-SHA CI evidence are recorded in the completion record.

## Implementation sequence

1. Refresh registry evidence and write `docs/REGISTRY.md`.
2. Update README and release guidance, then add ADR-0031.
3. Update milestone/state/handoff records and run the full gate.
4. Commit implementation, verify exact-head CI, then perform the separate closure evidence commit.

## Completion record

Implementation commit:
Exact CI run:
Exact CI result:
Closure commit:
Closure CI run:
Closure CI result:
Completed:
Notes:
