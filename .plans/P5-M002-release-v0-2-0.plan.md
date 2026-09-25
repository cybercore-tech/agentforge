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
- Amendment 1: `crates/agentforge-daemon/src/lib.rs`, `crates/agentforge-daemon/src/worker_api.rs`,
  `docs/DOGFOODING.md`
- Amendment 2: `.github/workflows/release.yml`, `docs/RELEASE.md`, `docs/OPERATIONS.md`, `CHANGELOG.md`
- Amendment 3: `crates/agentforge-operator/tests/worker.rs`

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

## Amendment 1 (2026-09-24)

The release gate (`./scripts/gate.sh full`) failed
`claim_refuses_unapproved_tasks_and_reports_the_lease_window` with `response failed: Interrupted
system call (os error 4)`. Classification: a latent intermittent defect in code this release ships,
not related to the version bump. The worker API's `read_line` and the daemon's `read_frame` loop on
`TcpStream::read` and return on `ErrorKind::Interrupted` instead of retrying, which is the standard
handling. Both socket readers now retry on `Interrupted`. The fix is required before tagging,
because `v0.2.0` ships this code.

A sweep found eight more `read()` loops on process pipes (adapter capture, gates, CI provider, CLI
input) with the same pattern. They are recorded as dogfooding finding 11 for a dedicated follow-up,
and kept out of the release scope.

Verification: the full gate passes, and the worker API tests pass on repeated local runs.

## Amendment 2 (2026-09-24)

`v0.2.0` was tagged on `991eb60` (CI `36097122882` and `36097287895`, dry run `36097289496`). Tag
run `36097480845` built all four targets, but **Publish GitHub release** failed again:

```text
read dist/agentforge-0.2.0-aarch64-apple-darwin: is a directory
```

Classification: a third latent defect in the publish job. Each build job uploads its staging
directory together with the archive, so `dist/*` contains directories, and `gh release create`
cannot upload a directory. This is the same reason the P5-M001 manual recovery ran
`rm -rf dist/*/`. The P5-M001 workflow fix covered `--repo` and the checksum names, but missed this
case, although the recovery procedure already showed it. The run left no draft release.

Recovery, per `docs/RELEASE.md`, without moving the tag:

1. Publish `v0.2.0` from run `36097480845`'s own artifacts: archives, `.sha256` files, and a bare
   `SHA256SUMS`, with the directories removed.
2. Fix `release.yml` so publish uploads only the files: `dist/*.tar.gz`, `dist/*.tar.gz.sha256`,
   and `dist/SHA256SUMS`, with a guard that fails if any archive is missing. Update RELEASE.md and
   the operations notes, and add a CHANGELOG `[Unreleased]` Fixed entry.
3. The fixed publish path is proven only by the next real tag run. The closure records that
   honestly, as a carried-forward check.

## Amendment 3 (2026-09-24)

Push CI `36098012428` on `46d8480` (a workflow and docs change) failed the Stable code gate:
`a_slow_agent_keeps_its_lease_renewed` got 1 renewal and expected at least 3. Classification: a
timing flake in a P4-M005 test, not a product defect and unrelated to this change.
- The test used a 300 ms lease window with renewals every 100 ms.
- A renewal delayed past 300 ms (lease lock plus audit fsync on a slow runner) finds the lease
  expired, and the renewal thread stops by design.
- The product behaviour is acceptable: the task is already `running`, so the lease no longer guards
  exclusivity.
- The test asserted the count before the failure list, which hid the reason.

Fix: a 1.5 s window with renewals every 500 ms and a 3 s agent, which gives each renewal about 1 s
of slack. The failures are asserted first, so a future flake shows its cause. Verified with repeated
local runs.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
