# ADR-0054: An attested CycloneDX SBOM for every release

- Status: Accepted
- Date: 2026-09-25
- Milestone: P5-M006

## Context

Since P5-M004 every release archive carries a keyless build-provenance attestation (ADR-0053). It
proves where and from what commit an archive was built, not what went into it. Users who need to
answer "does this release contain crate X at version Y?" had to read `Cargo.lock` at the tag and
work out which crates reach the shipped binaries. ADR-0053 noted that an SBOM attestation could later
use the same action. On 2026-09-25 the operator chose `cargo-cyclonedx`, pinned to 0.5.9 and
installed with `cargo install --locked` in the release job, as the SBOM tool.

## Decision

- **One CycloneDX 1.5 SBOM per release**, `agentforge-<version>.cdx.json`, attached to the release
  next to the archives. `cargo cyclonedx --describe binaries --no-build-deps --target all` writes one
  document per binary. `scripts/sbom-merge` (Python standard library) merges the `forge` and
  `forged` documents into one, whose root component is `agentforge` at the release version. Test
  fixtures and `xtask` are never included.
- **From the commit's own metadata.** The tool is pinned and installed with `--locked`, and it reads
  the tagged commit's `Cargo.lock` through cargo metadata. It builds nothing and resolves nothing new.
  `--target all` lists the dependencies of every platform, because the SBOM covers all four archives.
- **Deterministic.** `SOURCE_DATE_EPOCH` is the commit time. The merge deduplicates components by
  `bom-ref`, merges and sorts dependency edges, and derives `serialNumber` from the content, so the
  same commit gives a byte-identical SBOM. A component that appears twice with different content is
  refused, as is an edge to a component the document does not list.
- **Attested per archive.** A second `actions/attest@v4` step, with `sbom-path`, signs an SBOM
  attestation (predicate type `https://cyclonedx.org/bom`) for every archive, with the same
  keyless identity as the provenance.
- **Verified as uploaded.** When the release has a `*.cdx.json`, `scripts/publish-release` requires
  each downloaded archive's SBOM attestation from `release.yml`, alongside its provenance. A
  release whose archives lack one fails its publish job. The release-file guard requires exactly
  one SBOM, and `SHA256SUMS` covers it.

## Consequences

- Users verify with `gh attestation verify <archive> -R cybercore-tech/agentforge --predicate-type
  https://cyclonedx.org/bom`, and can read the SBOM with any CycloneDX tool.
- Vulnerability scanning and license policy become possible on the SBOM; neither is part of this
  decision.
- `v0.1.0` to `v0.3.0` have no SBOM and are not rebuilt; `publish-release` still verifies releases
  without one.
- The release job depends on `cargo-cyclonedx` 0.5.9 building from crates.io. An upgrade is an
  explicit change to the pinned version.
- Workspace crates appear with `path+file://` references to the runner's checkout path, as
  `cargo-cyclonedx` writes them.
- The workflow half runs only in release runs, so a dispatched rehearsal proves it before a tag.

## References

- `docs/RELEASE.md`
- `.plans/P5-M006-sbom-attestation.plan.md`
- ADR-0053 (keyless build provenance)
