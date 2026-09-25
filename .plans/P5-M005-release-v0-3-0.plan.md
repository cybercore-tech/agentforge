# Plan: P5-M005 — Release v0.3.0

Status: Approved
Milestone: P5-M005
Created: 2026-09-25
Owner: AgentForge project

## Goal

Publish `v0.3.0` from a green `main`: the first release whose archives carry keyless build-provenance
attestations (P5-M004), and the first real tag run through the rehearsed upload path (P5-M003). The
release must publish **from the workflow itself**, and every downloaded archive must pass
`sha256sum -c SHA256SUMS` and `gh attestation verify` against `release.yml`.

## Operator decision (2026-09-25)

The operator asked to cut `v0.3.0`, "the first signed release".

## Non-goals

- No functional change: version metadata, CHANGELOG, and docs only.
- No crates.io publication.
- No attestations for `v0.1.0` or `v0.2.0`.

## Context

Unreleased since `v0.2.0`:
- P4-M009: the worker-host doctor;
- P0-M015: wall-clock audit timestamps;
- P2-M034: closure and docs-index integrity;
- P5-M003: the provable release upload;
- P4-M010: remote-worker visibility and supervision, and the fixes for findings 16 and 17. Finding
  16 means any `forged` started with `forge daemon start` in `v0.2.0` stops expiring leases after
  the first expiry, so upgrading is recommended;
- P3-M005: the MCP task-tool gateway;
- P5-M004: keyless release provenance.

These are new capabilities on the `0.x` line, so this is a minor bump. `ToolInvoked` (audit code
13) means `v0.2.0` cannot read audit logs written by `v0.3.0` gateway use. This goes in the
CHANGELOG.

## Architecture placement

Release engineering only:
- `Cargo.toml` workspace version `0.3.0`, the internal requirements in
  `crates/agentforge-cli/Cargo.toml`, and `Cargo.lock`;
- `CHANGELOG.md`: `[Unreleased]` becomes `[0.3.0] - 2026-09-25` with a summary, plus a new empty
  `[Unreleased]` and compare links;
- README status, the site status pills, `docs/RELEASE.md` (the current line and the example tag),
  `docs/REGISTRY.md`, and `PROJECT_STATE.md`.

## Release procedure

1. Implementation commit; push; push CI plus a dispatched repeat, both green.
2. A dispatched release rehearsal on that commit (attest, upload to a draft, verify, delete).
3. The annotated `v0.3.0` tag on that exact commit, pushed.
4. The tag run must succeed, **including publishing and verifying its own uploaded assets and
   attestations**. Then check it independently:
   - four archives, their `.sha256` files, and `SHA256SUMS`;
   - a fresh download passes `sha256sum -c SHA256SUMS`;
   - each archive passes `gh attestation verify --signer-workflow` (source ref `refs/tags/v0.3.0`);
   - `forge version` and `forged --version` print `0.3.0`.
5. Release notes: the CHANGELOG section, install and verification steps, and provenance.
6. Close, tag the milestone, and update the Wiki and darknotes.

## Invariants

- The tag version equals the workspace version (the workflow enforces this).
- The tag is on the exact commit CI verified, and a failed publish never moves it.

## ADRs

None; this follows RELEASE.md and ADR-0053.

## Public API / CLI

`forge version` and `forged --version` report `0.3.0`.

## Compatibility analysis

Version metadata only. The audit-kind compatibility note is in the CHANGELOG.

## Dependency analysis

None; `Cargo.lock` changes only for workspace members.

## Expected file boundary

- `.plans/P5-M005-release-v0-3-0.plan.md`, `.plans/ACTIVE`
- `Cargo.toml`, `Cargo.lock`, `crates/agentforge-cli/Cargo.toml`
- `CHANGELOG.md`, `README.md`, `site/index.html`, `docs/RELEASE.md`, `docs/REGISTRY.md`
- closure records: `docs/MILESTONES.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`
- Amendment 1: `.cargo/registry-preflight.toml`, `tools/xtask/src/main.rs`, `docs/RELEASE.md`

## Test-first matrix

- `cargo metadata` shows `0.3.0` for every workspace package, and the built binaries report
  `0.3.0`.
- `./scripts/package-preflight` produces `agentforge-platform-0.3.0.crate`.
- The rehearsal and then the tag run pass their own attestation verification. The independent
  checks are in the release procedure.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Version bump and docs; gate and preflight; commit; push; CI plus a repeat; rehearsal.
4. Tag; watch; verify; release notes.
5. Close; tag the milestone after the closure CI is green; Wiki and darknotes.

## Failure modes

- The publish or attestation step fails on the tag: follow RELEASE.md (re-run the job, or publish
  from the run's artifacts), never move the tag, and record it by amendment.

## Documentation impact

CHANGELOG, README, site, RELEASE, and REGISTRY; closure records.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- green CI on the release commit, a green rehearsal, and a verified, attested published release.

## Acceptance criteria

- [ ] `v0.3.0` is published by the workflow with four archives, `.sha256` files, and `SHA256SUMS`,
      every archive attested and verified.
- [ ] Version metadata, the CHANGELOG, and the docs agree on `0.3.0`.
- [ ] A downloaded binary reports `0.3.0`; closed and tagged.

## Amendment 1 (2026-09-25)

`./scripts/package-preflight` failed on the release preparation: `no matching package named
agentforge-mcp found`. Classification: workflow/governance (a latent release-tooling gap from
P3-M005). The preflight packages `agentforge-platform` offline, resolving its internal crates
through `[patch.crates-io]` in `.cargo/registry-preflight.toml`. P3-M005 added `agentforge-mcp` as
a CLI dependency but not to that list. Neither the gate nor CI runs the preflight, so nothing
noticed. This is the same class as P2-M034: correctness that depends on a remembered step.

Fix:

1. Add `agentforge-mcp` to `.cargo/registry-preflight.toml`.
2. `xtask validate` (run by the gate and CI) checks that every `path` dependency in
   `crates/agentforge-cli/Cargo.toml` has a matching `[patch.crates-io]` entry with the same path,
   with unit tests. A new internal crate can then no longer break the release preflight
   unnoticed.
3. RELEASE.md notes the check.

The release preparation (stashed) resumes after this fix.
