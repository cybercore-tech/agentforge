# Plan: P5-M002 — Release v0.2.0

Status: Approved
Milestone: P5-M002
Created: 2026-09-24
Owner: AgentForge project

## Goal

Publish `v0.2.0` from a green `main`. It ships everything since `v0.1.0`, including the audit-log
corruption fix, and is the first real tag run of the publish job repaired in P5-M001
(`--repo`, bare `SHA256SUMS` names). The release must publish **from the workflow itself**, with
four checksummed archives and `SHA256SUMS` that verifies directly with `sha256sum -c`.

## Non-goals

- No signing or provenance attestations (a later P5 milestone that needs key-custody decisions).
- No crates.io publication.
- No functional change: version metadata, CHANGELOG, and docs only.

## Context

Unreleased since `v0.1.0`:
- P1-M008: merge approvals after review, bound to the reviewed SHA;
- P0-M013: coordinated audit appends. Concurrent writers could corrupt the audit chain in
  `v0.1.0`;
- P4-M004 to P4-M008: remote-worker leases, same-host workers, opt-in dispatch, the GhostPort
  channel, and remote execution with exact-SHA import;
- the release-workflow fix.

These are new capabilities on the `0.x` line, so this is a minor bump. The P5-M001 closure noted
that the next tag is the first real exercise of the fixed publish job. If it fails again, the
documented recovery applies (`docs/RELEASE.md`), and the defect is recorded rather than hidden.

## Architecture placement

Release engineering only:

- `Cargo.toml` workspace version `0.2.0`; the internal dependency requirements in
  `crates/agentforge-cli/Cargo.toml` become `0.2.0`; `Cargo.lock` is refreshed.
- `CHANGELOG.md`: `[Unreleased]` becomes `[0.2.0] - 2026-09-24` with a one-paragraph summary, a new
  empty `[Unreleased]`, and compare links (`v0.1.0...v0.2.0`, `v0.2.0...HEAD`).
- The README status, the site status pills, `docs/RELEASE.md` (current line `0.2.x`, example tag),
  `docs/REGISTRY.md` (`0.2.x`), and `PROJECT_STATE.md` (current release).

## Release procedure

1. Implementation commit; push; push CI plus a dispatched repeat, both green.
2. A packaging dry run (`AgentForge Release` dispatched on `main`) builds four archives.
3. The annotated `v0.2.0` tag on the implementation commit (the exact commit CI verified), pushed.
4. The tag release run must succeed **including "Publish GitHub release"**. Verify:
   - four `.tar.gz` archives, each with its `.sha256`;
   - `SHA256SUMS` with bare names;
   - a fresh download passes `sha256sum -c SHA256SUMS` with no `--ignore-missing` workaround for
     path prefixes;
   - `forge version` and `forged --version` print `0.2.0`.
5. Release notes: the CHANGELOG `[0.2.0]` section, install steps, and provenance (edited after
   publishing, as for `v0.1.0`).
6. Close, tag `milestone/P5-M002`, and update the Wiki and darknotes.

## Invariants

- The tag version equals the workspace version (enforced by the workflow).
- The release is cut from the exact commit that passed CI.
- A failed publish never moves the tag (`docs/RELEASE.md`).

## ADRs

None; this follows `docs/RELEASE.md`, ADR-0044, and the P5-M001 amendments.

## Public API / CLI

`forge version` and `forged --version` report `0.2.0`.

## Compatibility analysis

Version metadata only. The P4-M008 protocol notes stay as documented.

## Dependency analysis

None; `Cargo.lock` changes only for workspace members.

## Expected file boundary

- `.plans/P5-M002-release-v0-2-0.plan.md`, `.plans/ACTIVE`
- `Cargo.toml`, `Cargo.lock`, `crates/agentforge-cli/Cargo.toml`
- `CHANGELOG.md`, `README.md`, `site/index.html`, `docs/RELEASE.md`, `docs/REGISTRY.md`
- closure records: `docs/MILESTONES.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`

## Test-first matrix

- `cargo metadata` shows `0.2.0` for every package, and the built binaries report `0.2.0`.
- `./scripts/package-preflight` produces `agentforge-platform-0.2.0.crate`.
- The dry run builds four archives.
- The tag run publishes by itself, and the downloaded `SHA256SUMS` verifies with plain
  `sha256sum -c`.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Version bump and docs; gate and preflight; commit; push; CI.
4. Dry run; tag; watch the publish; verify.
5. Close; tag the milestone; Wiki and darknotes.

## Failure modes

- The publish job fails again: classify it, publish from the run's artifacts per `RELEASE.md`
  without moving the tag, and fix the workflow in an amendment with a finding recorded.

## Documentation impact

CHANGELOG release section, README, site, RELEASE, and REGISTRY; closure records.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- green CI on the release commit, a dry run, and a verified published release.

## Acceptance criteria

- [ ] `v0.2.0` is published **by the workflow**, with four checksummed archives and a
      directly verifiable `SHA256SUMS`.
- [ ] Version metadata, the CHANGELOG, and the docs agree on `0.2.0`.
- [ ] A downloaded binary reports `0.2.0`; closed and tagged.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
