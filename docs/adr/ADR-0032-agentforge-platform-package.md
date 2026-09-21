# ADR-0032: Use `agentforge-platform` for the future end-user Cargo package

- Status: Accepted
- Date: 2026-09-20
- Decision owners: AgentForge project

## Context

The unqualified `agentforge` and the related `agentforge-cli` names are occupied on crates.io by
unrelated projects. AgentForge's current end-user workspace package is private and named
`agentforge-cli`, while its installed binaries are `forge` and `forged`.

## Decision

Use `agentforge-platform` as the future public end-user Cargo package identity. Keep the existing
source directory, `forge`/`forged` binary names, internal library crate names, and GitHub release
archive names unchanged in this increment. Keep the package private until a separate approved plan
makes its dependency graph publishable.

## Consequences

The public package identity is distinct from occupied crates.io names while retaining the
AgentForge brand. Existing operator commands and release artifacts remain stable. A later
publishability plan must decide whether to publish a coordinated library graph or consolidate
private path dependencies before any `cargo publish`; the current package dry-run fails because
those private dependencies do not specify registry version requirements.
