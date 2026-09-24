# ADR-0041: cybercore-tech/agentforge is the canonical repository

- Status: Accepted
- Date: 2026-09-23
- Milestone: P2-M025

## Context

The Cybercore projects are moving from the personal `darkstardevx` GitHub account to the
`cybercore-tech` account. AgentForge's full history (13 branches, up to `976c4f9`) was pushed to the
new, previously empty `cybercore-tech/agentforge`. Pages was enabled there with workflow builds and
deployed, and development continued there from P2-M024. Metadata, links, and the public site still
named `darkstardevx`.

## Decision

- `https://github.com/cybercore-tech/agentforge` is the canonical repository, and
  `https://cybercore-tech.github.io/agentforge/` is the project site.
- Repository-owned metadata (workspace `Cargo.toml`), README, CHANGELOG, registry documentation,
  and the Pages site point at the canonical locations.
- `darkstardevx/agentforge` is the historical location. It is not updated and is not configured as
  a mirror by this repository. Historical plans and evidence records that name it are left
  unchanged.
- Local clones use `origin` for the canonical repository and may keep the old one as a named
  remote.

## Consequences

Positive:

- one authoritative location for code, CI evidence, releases, and Pages;
- no history rewrite; evidence commit SHAs stay identical in both repositories.

Trade-offs:

- CI run IDs recorded before P2-M024 refer to runs in the historical repository;
- until push-triggered workflows fire in the new repository, CI evidence is collected with
  `workflow_dispatch` on the same head.
