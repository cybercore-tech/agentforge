# ADR-0033: Prepare package metadata without publishing the private workspace

- Status: Accepted
- Date: 2026-09-20
- Decision owners: AgentForge project

## Context

`agentforge-platform` is the selected future end-user package, but its implementation crates remain
private and are not available from crates.io. Cargo also requires registry version requirements and
complete package metadata before it can construct a registry-compatible manifest. A package-shape
check is useful now, but it must not be confused with publication readiness or registry authority.

## Decision

Keep every workspace crate `publish = false`, add explicit version requirements alongside local
paths for the platform package, and provide an opt-in offline package preflight. The preflight uses
`.cargo/registry-preflight.toml` to patch the private workspace crates locally, builds the
`agentforge-platform` archive without verification or upload, and checks its bounded contents.

## Consequences

The package manifest and archive shape can be reviewed deterministically before any registry change.
Local builds continue to resolve the workspace paths normally. A normal registry resolution still
fails closed until a future approved plan decides whether and in what order any internal crates
should become public; no package name is reserved and no crates.io state is changed by this ADR.
