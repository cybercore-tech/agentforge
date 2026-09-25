# Plan: P5-M004 — Keyless release provenance

Status: Complete
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
- Amendment 1: `crates/agentforge-daemon/tests/worker_api.rs`

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

- [x] Release archives are attested by the release workflow, keylessly.
- [x] The workflow verifies the attestations of the uploaded assets; the self-test covers it.
- [x] A rehearsal proves it end to end, and an independent local verification passes (and a
      tampered archive fails).
- [x] ADR, docs; CI evidence; closed and tagged correctly.

## Amendment 1 (2026-09-25)

The pre-commit gate on this milestone's first closure attempt failed in a P4-M010 test,
`a_closed_port_is_unreachable_not_refused`: `Unreachable("response failed: Connection reset by
peer")` where the test expects the message to start with `cannot reach worker API`.
Classification: semantic/test, a race in the test. `free_port()` binds `127.0.0.1:0`, reads the
port, and releases it. Another test in the same binary, running in parallel, can be given that
port for its own server, so the "closed" port briefly had a live server that reset the connection.
The product classified it correctly (`Unreachable`). The closure commit was rejected, and the
tagger (P2-M034) refused to tag the uncommitted closure, as designed. The closure was set aside
(stashed) and will be redone after this fix.

Fix: the two tests that only need a refused connection (`a_closed_port_is_unreachable_not_refused`
and `a_run_once_worker_still_returns_the_outage`) connect to port 0, where nothing can listen, so
connecting fails at once on every platform, with no race. The outage-and-reconnect test must later
start a server on its port, so it keeps `free_port()`. Its residual risk (another test being given
the same port within the few hundred milliseconds before its server starts) is recorded rather
than hidden.

## Completion record

Implementation commit: `623be58` (attestations); `c228f23` (Amendment 1 test fix)
CI run: `36145286790` (push) and `36145728809` (dispatched repeat) on `623be58`; `36146684432`
(push) and `36147102170` (dispatched repeat) on `c228f23`; release rehearsal `36145288186`
CI result: green on all seven jobs in all four runs; the rehearsal is green on all five jobs
Completed: 2026-09-25
Notes:
- **Rehearsal `36145288186`:** "Attestation created for 4 subjects"
  (https://github.com/cybercore-tech/agentforge/attestations/50194895), then "verified 9 assets of
  rehearsal-36145288186-1 (names, bytes, SHA256SUMS, 4 attestations)" on the downloaded draft
  assets, then the draft was deleted. Afterwards there was no draft and no `rehearsal-*` tag, and
  `v0.2.0` is still Latest.
- **Independent check from this machine:** the run's Linux archive verified with `gh attestation
  verify --signer-workflow cybercore-tech/agentforge/.github/workflows/release.yml`. Its
  certificate was issued by `https://token.actions.githubusercontent.com` to
  `release.yml@refs/heads/main`, source digest `623be58...`, trigger `workflow_dispatch`. A copy
  with one byte appended failed (no attestation for its digest).
- The self-test's mutant without the attestation check fails it.
- **Amendment 1:** the first closure attempt's pre-commit gate caught a port-reuse race in a
  P4-M010 test. The commit was rejected, and the tagger refused the uncommitted closure, as
  P2-M034 intended. The closure was stashed, the plan amended, and the test fixed (port 0) and
  verified on all platforms before this closure was redone.
- The next real `vX.Y.Z` tag will carry attestations automatically. `v0.1.0` and `v0.2.0` stay
  checksum-only.
