# Plan: P5-M007 — Release v0.3.1

Status: Approved
Milestone: P5-M007
Created: 2026-09-25
Owner: AgentForge project

## Goal

Publish `v0.3.1` from a green `main`. It is the first release that ships an **attested CycloneDX
SBOM** (P5-M006) next to the build provenance, and the first with the remote-worker fixes found by
the two-host rehearsal (findings 19 and 20). The workflow must publish it by itself. Every archive
must pass `sha256sum -c`, provenance verification, and SBOM verification. Then the two-host
rehearsal runs against the published release.

## Operator decision (2026-09-25)

"Fix 21 and 22 then cut v0.3.1." Findings 21 and 22 were resolved in P2-M035.

## Non-goals

- No functional change: version metadata, the CHANGELOG, and docs, plus pointing the rehearsal's
  default at the new release after it is published.
- No crates.io publication.

## Context

Unreleased since `v0.3.0`:
- P4-M011: worker-host setup scripts;
- P4-M012: the two-host rehearsal. `v0.3.0` workers abandon finished work when a result upload is
  lost (finding 19), and lose a lease claimed late (finding 20). Both are fixed on `main`, and
  GhostPort `v0.1.2` is required;
- P5-M006: the attested SBOM;
- P2-M035: agent task hygiene (findings 21 and 22).

The operator named the version: a patch, `v0.3.1`. The product changes are fixes. The additions are
release tooling and maintainer scripts, not new product interfaces, which fits RELEASE.md's patch
policy.

## Architecture placement

Release engineering only:
- `Cargo.toml` workspace version `0.3.1`, the internal requirements in
  `crates/agentforge-cli/Cargo.toml`, and `Cargo.lock`;
- `CHANGELOG.md`: `[Unreleased]` becomes `[0.3.1] - 2026-09-25` with a summary, plus a new empty
  `[Unreleased]` and compare links;
- README status, the site status pills, and RELEASE (latest `v0.3.1`, example tag);
- after publication, `scripts/rehearse-two-hosts` defaults to `v0.3.1`.

## Release procedure

1. Implementation commit; push; push CI plus a dispatched repeat, both green; a dispatched release
   rehearsal on that commit (provenance and SBOM attested and verified on the draft).
2. The annotated `v0.3.1` tag on that exact commit, pushed.
3. The tag run must publish by itself and verify its own uploaded assets and attestations
   (provenance and SBOM). Then check it independently from this machine:
   - four archives, their `.sha256` files, `agentforge-0.3.1.cdx.json`, and `SHA256SUMS`;
   - `sha256sum -c SHA256SUMS` passes (it now covers the SBOM too);
   - every archive passes `gh attestation verify` for provenance (`--source-ref refs/tags/v0.3.1`)
     and for the SBOM (`--predicate-type https://cyclonedx.org/bom`);
   - `forge version` and `forged --version` print `0.3.1`.
4. Release notes: the CHANGELOG section, install and verification steps (provenance and SBOM), and
   provenance.
5. `scripts/rehearse-two-hosts` against the published `v0.3.1` passes every check. It is the first
   release rehearsed with findings 19 and 20 fixed.
6. Close, tag the milestone after the closure CI is green, and update the Wiki and darknotes.

## Invariants

- The tag version equals the workspace version, on the exact commit CI and the rehearsal verified.
- A failed publish never moves the tag.

## ADRs

None; this follows RELEASE.md, ADR-0053, and ADR-0054.

## Public API / CLI

`forge version` and `forged --version` report `0.3.1`.

## Compatibility analysis

Version metadata only.

## Dependency analysis

None.

## Expected file boundary

- `.plans/P5-M007-release-v0-3-1.plan.md`, `.plans/ACTIVE`
- `Cargo.toml`, `Cargo.lock`, `crates/agentforge-cli/Cargo.toml`
- `CHANGELOG.md`, `README.md`, `site/index.html`, `docs/RELEASE.md`, `scripts/rehearse-two-hosts`
- closure records: `docs/MILESTONES.md`, `PROJECT_STATE.md`, `AGENT_HANDOFF.md`

## Test-first matrix

- `cargo metadata` shows `0.3.1` for every package, and the binaries report `0.3.1`.
- `./scripts/package-preflight` produces `agentforge-platform-0.3.1.crate`.
- The rehearsal and then the tag run pass their own verification. The independent checks are in the
  release procedure.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Version bump and docs; gate and preflight; commit; push; CI plus a repeat; release rehearsal.
4. Tag; watch; verify; release notes; the two-host rehearsal against the release.
5. Close; tag the milestone.

## Failure modes

- The publish, attestation, or SBOM step fails on the tag: RELEASE.md's recovery applies, the tag
  is never moved, and it is recorded by amendment.

## Documentation impact

CHANGELOG, README, site, RELEASE; closure records.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/package-preflight`;
- green CI on the release commit, a green rehearsal, a verified published release, and a passing
  two-host rehearsal against it.

## Acceptance criteria

- [ ] `v0.3.1` is published by the workflow with archives, `.sha256` files, the SBOM, and
      `SHA256SUMS`, with every archive's provenance and SBOM attested and verified.
- [ ] Version metadata, the CHANGELOG, and the docs agree on `0.3.1`.
- [ ] The two-host rehearsal passes against the published release; closed and tagged.
