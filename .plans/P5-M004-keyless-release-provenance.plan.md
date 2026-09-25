# Plan: P5-M004 — Keyless release provenance

Status: Draft
Milestone: P5-M004
Created: 2026-09-25
Owner: AgentForge project

## Goal

Every release archive carries a signed SLSA build-provenance attestation made by the release
workflow itself, with no key for anyone to hold. Anyone can verify an archive with
`gh attestation verify <archive> -R cybercore-tech/agentforge`, which proves it was built by
`release.yml` in this repository from a specific commit. The workflow verifies the attestations of
the assets it actually uploaded, and the upload rehearsal (P5-M003) proves the whole path without a
release.

## Operator decision (2026-09-25)

Keyless GitHub artifact attestations (Sigstore, SLSA provenance) were chosen over a held signing
key (minisign) and over deferring signing.

## Non-goals

- No held keys, no minisign/cosign key pairs, no signature files in the release.
- No attestations for `v0.1.0` or `v0.2.0`; they are already published and are not rebuilt.
- No release is cut in this milestone (a release is an explicit, separate decision).
- No SBOM attestation (possible later with the same action).

## Context

Releases ship checksums (`SHA256SUMS` and per-archive `.sha256`). Checksums prove integrity
against the listed values, not where the archive came from: anyone who can replace the assets can
replace the checksums. Attestations bind each archive's digest to the workflow run that built it
(repository, workflow file, commit, and trigger), signed with a short-lived Sigstore certificate
from the workflow's OIDC identity and recorded in the public transparency log. The repository is
public, so the public-good Sigstore instance is used and attestations are available on the current
plan.

## Architecture placement

- **`release.yml` publish job:**
  - permissions gain `id-token: write`, `attestations: write`, and `artifact-metadata: write`
    (with `contents: write`);
  - a new step after the release files are selected: `actions/attest@v4` with `subject-path:
    release/*.tar.gz`. It runs on tags and on dispatch, so every rehearsal attests its snapshot
    archives too;
  - the publish and rehearse steps set `ATTESTATION_SIGNER_WORKFLOW=<repo>/.github/workflows/
    release.yml`.
- **`scripts/publish-release`:** when `ATTESTATION_SIGNER_WORKFLOW` is set, `verify` also runs
  `gh attestation verify <asset> --repo <repo> --signer-workflow <workflow>` on every downloaded
  `.tar.gz`, so the check covers the assets as uploaded, not the local copies. The self-test's fake
  `gh` gains `attestation verify` (an attested-digest list), with cases for a verified release, an
  unattested archive, and verification switched off.
- **ADR-0053** records keyless provenance as the release signing model (and why no held key).
- **Docs:** RELEASE (how attestations are made and how to verify one; recovery when attestation
  fails), README install steps (verify with `gh attestation verify`), and OPERATIONS.

## Invariants

- A release (or rehearsal) whose uploaded archives do not verify against this repository's
  `release.yml` fails its publish job.
- No secret or key is added to the repository or its settings.
- The released files are unchanged.

## ADRs

ADR-0053 (new): keyless build provenance for releases. Registry row (enforced by `xtask validate`).

## Public API / CLI

None in the product. `scripts/publish-release` gains `ATTESTATION_SIGNER_WORKFLOW`.

## Compatibility analysis

The release assets are unchanged. Attestations are stored by GitHub against each archive's digest,
not as release files. Old releases stay verifiable by checksum only.

## Dependency analysis

`actions/attest@v4` (GitHub-owned) in the release workflow; nothing in the Rust workspace.

## Expected file boundary

- `.plans/P5-M004-keyless-release-provenance.plan.md`, `.plans/ACTIVE`
- `.github/workflows/release.yml`, `scripts/publish-release`
- `docs/RELEASE.md`, `docs/OPERATIONS.md`, `docs/adr/ADR-0053-keyless-release-provenance.md`,
  `docs/adr/README.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- Self-test: with a signer workflow set, attested archives pass, and an unattested one fails
  verification (and a rehearsal still deletes its draft); without it, no attestation check runs.
  Mutation: removing the attestation check fails the self-test.
- Real: a dispatched release rehearsal on `main` attests its four archives, uploads them to the
  draft, and verifies the downloaded assets' attestations, then deletes the draft. Independently,
  the run's artifacts downloaded to this machine verify with `gh attestation verify`, and a
  modified archive does not.
- The full gate, and push CI plus a dispatched repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Script and self-test; workflow; ADR and docs.
4. Gate; commit (exit checked); push; CI plus a repeat; a dispatched rehearsal, checked from this
   machine too.
5. Close; tag after the closure CI is green and the dry run names the "close ..." commit.

## Failure modes

- Attestation creation fails (for example a Sigstore outage) on a tag run: the publish job stops
  before uploading, so no unattested release appears. Re-run the job; the tag is not moved.
- Verification of uploaded assets fails after a real upload: the release exists; treat it like a
  failed asset verification (RELEASE.md recovery) and do not move the tag.

## Documentation impact

RELEASE, README, OPERATIONS, ADR-0053; CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/publish-release --self-test`;
- push CI plus a dispatched repeat, and a dispatched release rehearsal.

## Acceptance criteria

- [ ] Release archives are attested by the release workflow, keylessly.
- [ ] The workflow verifies the attestations of the uploaded assets; the self-test covers it.
- [ ] A rehearsal proves it end to end, and an independent local verification passes (and a
      tampered archive fails).
- [ ] ADR, docs; CI evidence; closed and tagged correctly.
