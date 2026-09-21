# Package identity and distribution policy 📦

## Current status

AgentForge is a `0.0.x` alpha and is not published to crates.io. The workspace crates are
deliberately marked `publish = false`; the supported public distribution is the GitHub release
archive containing the `forge` and `forged` binaries. Local development may also install those
binaries with `cargo install --path` from a checked-out repository.

Do not use `cargo add agentforge` as an AgentForge installation command. The crates.io package
named `agentforge` is an unrelated Rust project. Its published metadata was checked on
2026-09-20:

| Name | Version | Description | Published repository |
| --- | --- | --- | --- |
| `agentforge` | `0.1.0` | A Rust crate for building multi-agentic applications. | [kinghuynh/agentforge](https://github.com/kinghuynh/agentforge) |
| `agentforge-core` | `0.1.10` | Shared types and data models for another AgentForge project. | [bhavinkotak/agentforge](https://github.com/bhavinkotak/agentforge) |
| `agentforge-cli` | `0.1.10` | CLI for another AgentForge project. | [bhavinkotak/agentforge](https://github.com/bhavinkotak/agentforge) |

The matching brand name does not imply affiliation, shared code, or shared ownership. Crates.io
names are global and cannot be reused by this project while those packages exist.

## Canonical AgentForge identities

- Project: **AgentForge**
- Repository: `https://github.com/darkstardevx/agentforge`
- Project site: `https://darkstardevx.github.io/agentforge/`
- CLI binary: `forge`
- Optional local daemon binary: `forged`
- Workspace crates: internal implementation units, not public crates at this time

The repository, plans, task state, approvals, audit records, worktrees, gates, and exact-head CI
evidence remain authoritative. The Pages site is an informational entry point, not a package
registry.

## Future crates.io gate

Publishing a Rust package is intentionally deferred. Before any package becomes public, a new
approved plan must:

1. inventory the then-current crates.io namespace with fresh `cargo search`/`cargo info` evidence;
2. select names that are globally available and clearly distinguishable from unrelated projects;
3. decide whether the package is an end-user binary, a reusable library, or an internal workspace
   unit;
4. define package metadata, dependency publication order, versioning, README links, and release
   verification; and
5. update this policy before the first `cargo publish`.

No owner-transfer request, package yank, or attempt to impersonate an existing project is part of
that future work. Until the gate is completed, use the GitHub release artifacts or build from the
official repository checkout.
