# ADR-0053: Keyless build provenance for releases

- Status: Accepted
- Date: 2026-09-25
- Milestone: P5-M004

## Context

Releases shipped checksums only (`SHA256SUMS` and per-archive `.sha256`). A checksum proves an
archive matches a listed value, not who built it: anyone able to replace the release assets can
replace the checksums too. Release signing was deferred in P5-M001 because it needed a decision on
key custody. On 2026-09-25 the operator chose between keyless GitHub artifact attestations, a held
minisign key, and deferring again.

## Decision

- **Keyless attestations.** The release workflow's publish job attests every archive with
  `actions/attest@v4`: a signed SLSA build-provenance statement binding the archive's SHA-256 to
  this repository, `.github/workflows/release.yml`, the commit, and the trigger. The signature
  uses a short-lived Sigstore certificate issued to the workflow's OIDC identity. The repository
  is public, so the public-good Sigstore instance and its transparency log are used, and GitHub
  stores the attestation against the digest.
- **No held key.** Nothing to generate, store as a secret, back up, rotate, or leak. The trust root
  is GitHub's OIDC identity for this repository's workflow, checked by `gh attestation verify
  --signer-workflow`.
- **Verified as uploaded.** `scripts/publish-release` verifies the attestation of every archive it
  downloads back from the release (or rehearsal draft), with the signer workflow pinned, so a
  release whose uploaded archives do not verify fails its publish job.
- **Rehearsed.** Dispatched release runs attest their snapshot archives and verify them through the
  same path (P5-M003), so attestation defects surface before a tag.

## Consequences

- Users verify with `gh attestation verify <archive> -R cybercore-tech/agentforge` (or any
  Sigstore/in-toto verifier), with no key to distribute.
- Provenance proves where and from what an archive was built, not that the code is correct.
- `v0.1.0` and `v0.2.0` predate this and are verifiable by checksum only; they are not rebuilt.
- Verification depends on GitHub's attestation API and Sigstore. The checksums remain as an
  offline integrity check.
- An SBOM attestation can later use the same action.

## References

- `docs/RELEASE.md`
- `.plans/P5-M004-keyless-release-provenance.plan.md`
- ADR-0044 (milestone and release tags)
