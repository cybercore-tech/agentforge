# ADR-0031: Keep registry identity explicit before crates.io publication

- Status: Accepted
- Date: 2026-09-20
- Decision owners: AgentForge project

## Context

The crates.io name `agentforge` is already used by an unrelated package, and related names such as
`agentforge-core` and `agentforge-cli` are also occupied by another AgentForge-branded project.
This repository is an unpublished `0.0.x` workspace whose crates are currently private. An
unqualified `cargo add agentforge` instruction would therefore be ambiguous and could install the
wrong project.

## Decision

Keep all current workspace crates private and continue distributing the `forge` and `forged`
binaries through the existing GitHub release workflow and local source builds. Do not publish,
rename, yank, or request transfer of any occupied crates.io package as part of this milestone.

Any future crates.io publication or package rename requires a new approved plan with fresh global
name inventory, package-role decisions, dependency/version ordering, and explicit documentation
updates.

## Consequences

The current release path remains stable and cannot accidentally resolve to an unrelated crate.
Users receive a clear warning about the occupied name. A future package release will require
deliberate naming work before publication, but it will not silently create an ownership or
dependency collision.
