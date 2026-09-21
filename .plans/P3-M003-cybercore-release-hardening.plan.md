# Plan: P3-M003 — Cybercore connector release hardening

Status: Approved
Milestone: P3-M003
Created: 2026-09-21
Owner: AgentForge project

## Goal

Move the Cybercore Mission Control connector from an experimental implementation to a
release-candidate quality package with explicit provenance, reproducible checks, supported
platform coverage, operator-safe shutdown/version behavior, and a gated artifact publishing path.

“Release-ready” means a user can obtain a named, checksummed artifact, verify what was built, read
the security/support limits, configure it without embedding a secret, run it on supported desktop
platforms, and stop or upgrade it predictably. It does not mean production Cloudflare deployment,
unreviewed crates.io publication, or remote execution authority.

## Context and current gaps

P3-M002 added the outbound-only `cybercore-agent` Rust connector and a green exact-SHA CI run, but
the standalone repository still lacks a license file, release metadata and policy, tag-driven
artifacts/checksums, a Rust platform matrix, a version command, graceful periodic shutdown, and a
release-candidate verification checklist.

## Scope

Implementation occurs only in the standalone `cybercore-mission-control` repository. AgentForge
changes are limited to this plan, `.plans/ACTIVE`, and closure evidence.

The implementation may include:

- MIT `LICENSE` and complete package metadata, repository links, and included documentation;
- a connector `--version`/`--help` surface and cooperative Ctrl-C shutdown for periodic mode;
- a release-safe version source shared by Cargo metadata and CLI output;
- a reproducible packaging check (`cargo package --locked` or equivalent) that verifies the exact
  files and excludes local configuration/credentials;
- a supported-platform CI matrix for Linux, macOS, and Windows plus Worker type-check coverage;
- a tag-gated GitHub release workflow that builds release binaries, archives, SHA-256 checksums,
  and a machine-readable manifest without deploying Cloudflare or publishing a crate;
- release notes, support matrix, upgrade/rollback guidance, and a pre-release checklist;
- dependency/security checks that fail closed on malformed workflow or missing provenance; and
- tests for version output, cooperative shutdown, package boundaries, and credential/config safety.

## Security and release invariants

- Release artifacts contain no credentials, local config, `.dev.vars`, or build workspace state.
- The release workflow runs only on explicit version tags or manual approval and requests the
  minimum GitHub permissions required to create a release; it never deploys production.
- Checksums are generated from the exact archived bytes and published beside the artifacts.
- The connector remains outbound-only and observation-only. No release work may add command,
  shell, file-transfer, reverse-tunnel, or cloud-to-local execution authority.
- Non-loopback HTTP remains rejected; TLS verification cannot be disabled by normal configuration.
- Version output, changelog, Cargo metadata, and release tag must agree exactly.
- Upgrade and rollback instructions must preserve credential revocation and never print secrets.

## Non-goals

- No production Cloudflare deployment, real operator credential, or public dashboard exposure.
- No automatic crates.io publication; `publish = false` remains until a separate registry decision.
- No multi-tenant IAM redesign, rate limiting, report upload, or remote task dispatch.
- No broad telemetry, raw command output, source-tree upload, or local AgentForge mutation.

## Expected standalone repository boundary

- `LICENSE`, `README.md`, `CHANGELOG.md`, and release/support documentation;
- root and connector Cargo metadata/lockfile as required;
- `connector/src/**` and connector tests for version/shutdown behavior;
- `.github/workflows/ci.yml` platform matrix and a tag-gated release workflow;
- release scripts/templates/manifests/checksum generation; and
- `.gitignore` updates needed to keep local configs and artifacts out of commits.

AgentForge files in this milestone:

- `.plans/P3-M003-cybercore-release-hardening.plan.md`;
- `.plans/ACTIVE`; and
- closure evidence in `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, and `docs/MILESTONES.md`.

## Validation matrix

Required before closure:

- `cargo fmt --all -- --check`;
- `cargo check --workspace --locked` and `cargo test --workspace --locked`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- connector tests for `--version`, help/argument failures, shutdown, credential/config safety,
  and existing heartbeat protocol behavior;
- `cargo package --locked --allow-dirty` or an equivalent inspected package archive proving no
  local secrets/config are included;
- local validation of release archive names, checksums, manifest, and supported target mapping;
- exact-SHA GitHub CI for Linux, macOS, Windows, Worker type-check, and packaging checks; and
- exact-SHA workflow validation for the tag-gated release definition (without creating a release
  or deploying production).

Infrastructure failures must be classified separately from connector semantics and rerun only at
the same exact SHA. No hook, test, lint, or security check may be bypassed.

## Acceptance criteria

1. A user can run `cybercore-agent --version` and identify the release version and commit/build
   provenance without a network call.
2. Periodic mode exits cooperatively on operator interrupt and does not leave an unbounded sleep,
   child process, socket, or credential-bearing log behind.
3. A clean package/archive inspection proves release artifacts contain only intended binaries,
   documentation, license, and checksums; local credentials/config are excluded.
4. CI exercises the connector on Linux, macOS, and Windows and retains the Worker type-check job.
5. A tag-gated workflow creates reproducible, checksummed release artifacts only after the full
   validation matrix succeeds and with no production deployment side effect.
6. README, changelog, security/support policy, and release checklist clearly label the support
   boundary and recovery procedures.
7. The implementation commit and closure commit are separate from this plan checkpoint and have
   exact-SHA validation evidence.

## Rollback and recovery

- Delete or disable a release draft without changing Mission Control data.
- Revoke the affected agent credential, replace the local credential file, and return to the prior
  verified artifact using the documented checksum.
- Revert the standalone implementation commit; AgentForge state remains local and durable.

## Completion record

Implementation commit: pending
Exact validation evidence: pending
Completed: pending
Notes: Plan-only approval checkpoint. Implementation must occur in the standalone
`cybercore-mission-control` repository after this plan is committed.
