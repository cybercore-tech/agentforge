# Release process 📦

AgentForge uses a small, explicit release process for its early `0.x` line.

## Distribution boundary

The current release workflow publishes GitHub release archives for the `forge` and `forged`
binaries. AgentForge does not currently publish crates to crates.io. The names `agentforge`,
`agentforge-core`, and `agentforge-cli` are already occupied by unrelated published packages, so
`cargo add agentforge` is not a supported installation path and must not be added to release
instructions.

The selected future end-user package identity is `agentforge-platform`. It is not published yet;
the current source-install path is still `cargo install --path crates/agentforge-cli --locked`.

Any future crates.io package requires a separate approved registry-identity plan with fresh name
availability evidence, an explicit package naming scheme, dependency publication order, and
verification of the resulting install instructions. Do not rename or publish workspace crates as
part of an ordinary binary release.

Before discussing publication, maintainers may run the offline package-shape check from the
repository root:

```bash
CARGO_TARGET_DIR=/tmp/agentforge-cargo-target ./scripts/package-preflight
```

This uses the checked-in `.cargo/registry-preflight.toml` only to resolve private workspace
dependencies locally while constructing an archive. It does not contact crates.io or publish
anything, and a passing archive check does not authorize registry publication.

`scripts/package-preflight` resolves the workspace's internal crates through the
`[patch.crates-io]` list in `.cargo/registry-preflight.toml`. Since P5-M005, `xtask validate` (in
the gate and CI) requires an entry there for every path dependency of `crates/agentforge-cli`,
because a new internal crate without one broke the `v0.3.0` preflight, and only the release
itself ran it.

## Version policy

The workspace version is the release version. Tags must use `vMAJOR.MINOR.PATCH` and must match the
workspace version exactly. The current line is `0.3.x` (latest release `v0.3.1`, the first with an attested SBOM; `v0.3.0` was the first with attested archives, and the first tag was `v0.1.0`) because APIs and operator workflows are still
evolving.

- Patch releases contain compatible fixes and documentation corrections.
- Minor releases may add capabilities and non-breaking interfaces.
- Breaking changes are called out in `CHANGELOG.md`; before `1.0.0`, compatibility remains an
  explicit best-effort promise rather than a long-term guarantee.

Update `Cargo.toml` and `CHANGELOG.md` together before creating a release tag. The release workflow
rejects a tag whose version does not match the workspace metadata.

## Artifacts

The tagged release workflow builds `forge` and `forged` for:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Each target produces an `agentforge-<version>-<target>.tar.gz` archive and a sibling SHA-256 file.
The GitHub release also contains a consolidated `SHA256SUMS` file.

Since P5-M004 every archive also has a **build-provenance attestation** (ADR-0053). It is a signed
SLSA statement that the archive, by its SHA-256, was built by `.github/workflows/release.yml` in
`cybercore-tech/agentforge` from a specific commit. It is signed keylessly with a short-lived
Sigstore certificate issued to the workflow, so there is no key to hold, and it is stored by
GitHub, not as a release file. Verify a download:

```bash
gh attestation verify agentforge-<version>-<target>.tar.gz -R cybercore-tech/agentforge
# stricter: pin the workflow that must have built it
gh attestation verify agentforge-<version>-<target>.tar.gz -R cybercore-tech/agentforge \
  --signer-workflow cybercore-tech/agentforge/.github/workflows/release.yml
```

A modified archive, or one built anywhere else, fails. `v0.1.0` and `v0.2.0` predate attestations
and are verifiable by checksum only.

### SBOM

Since P5-M006 each release also contains `agentforge-<version>.cdx.json`, a CycloneDX 1.5 **software
bill of materials** for `forge` and `forged` (ADR-0054). It lists every crate the two binaries are
built from, at its exact version, including those used on only some of the four targets.
`SHA256SUMS` covers it.

The publish job makes it from the commit's own `Cargo.lock`. It installs `cargo-cyclonedx` 0.5.9
with `cargo install --locked`, sets `SOURCE_DATE_EPOCH` to the commit time, and runs:

```bash
cargo cyclonedx --format json --spec-version 1.5 --describe binaries --no-build-deps --target all
scripts/sbom-merge --name agentforge --version <version> \
  crates/agentforge-cli/forge_bin.cdx.json crates/agentforge-daemon/forged_bin.cdx.json
```

`scripts/sbom-merge` merges the two per-binary documents into one, deduplicates components by
`bom-ref`, merges the dependency edges, and derives the serial number from the content, so the same
commit gives a byte-identical SBOM. It refuses malformed input, anything other than CycloneDX 1.5, and
components that conflict. Its `--self-test` runs in CI. `cargo cyclonedx` writes `*_bin.cdx.json`
files next to every `Cargo.toml`; do not commit them.

Every archive also carries an **SBOM attestation**: the same keyless signature as the provenance,
binding the SBOM to the archive's SHA-256. Verify it and print the SBOM it attests:

```bash
gh attestation verify agentforge-<version>-<target>.tar.gz -R cybercore-tech/agentforge \
  --predicate-type https://cyclonedx.org/bom \
  --signer-workflow cybercore-tech/agentforge/.github/workflows/release.yml
gh attestation verify agentforge-<version>-<target>.tar.gz -R cybercore-tech/agentforge \
  --predicate-type https://cyclonedx.org/bom --format json \
  | jq '.[0].verificationResult.statement.predicate.components[].name'
```

`v0.1.0` to `v0.3.0` have no SBOM.

## Milestone tags versus release tags

`milestone/<ID>` tags mark milestone closure commits (ADR-0044, `scripts/tag-milestone`). They do
not match the release workflow's `v*.*.*` trigger and never produce artifacts. Only a `vX.Y.Z` tag,
created as an explicit release decision below, starts a release.

## Creating a release

1. Update the workspace version and `CHANGELOG.md`.
2. Run `./scripts/gate.sh full`.
3. Commit and push the versioned release preparation.
4. Confirm exact-head CI is green.
5. Create and push the matching tag, for example:

   ```bash
   git tag -a v0.3.1 -m "AgentForge v0.3.1"
   git push origin v0.3.1
   ```

6. Check the run before announcing the release. Since P5-M003, the publish step downloads every
   uploaded asset back and fails unless the asset names equal the selected files, each asset is
   byte-identical, and `SHA256SUMS` passes `sha256sum -c`. Since P5-M004 each downloaded archive
must also pass `gh attestation verify` against `release.yml`, and since P5-M006 its SBOM attestation
must pass too. Still download one archive yourself and
   check that the extracted `forge version` prints the release version.

Tag the exact commit that passed CI and the packaging dry run. Never move a published release tag;
fixes ship as a new patch release.

### If the publish job fails

The build jobs and the publish job are separate. When every build succeeded but publishing
failed, publish the release from that run's own CI-built artifacts instead of rebuilding or moving
the tag:

```bash
for t in x86_64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc; do
  gh run download <run-id> -R cybercore-tech/agentforge -n "$t" -D dist
done
rm -rf dist/*/                                   # keep only the archives and .sha256 files
(cd dist && for f in *.sha256; do sha256sum -c "$f"; done && sha256sum *.tar.gz > SHA256SUMS)
gh release create vX.Y.Z dist/* --repo cybercore-tech/agentforge --verify-tag \
  --title "AgentForge vX.Y.Z" --notes-file <notes>
```

Then fix the workflow for the next release. Two releases were published this way:

- `v0.1.0` (P5-M001): the publish job had no checkout, so `gh` could not find the repository, and
  `SHA256SUMS` would have had `dist/`-prefixed paths.
- `v0.2.0` (P5-M002): the artifacts also contain each target's staging directory, and `gh release
  create dist/*` failed on a directory. The recovery above already removed those directories, but
  the P5-M001 workflow fix did not.

A release published this way has neither attestations nor an SBOM. If the run got as far as
**Generate the release SBOM**, you can publish `agentforge-<version>.cdx.json` too, but it would not
be attested.

If **Generate the release SBOM**, **Attest build provenance**, or **Attest the SBOM** fails (for
example a crates.io or Sigstore outage), nothing was uploaded: re-run
the failed job, and do not move the tag. If the upload succeeded but its verification failed, the
release already exists. Do not move the
tag: compare the release's assets with the run's artifacts, and replace the wrong assets (`gh release
upload vX.Y.Z <file> --clobber`) from those artifacts.

### Upload rehearsal (manual dispatch)

A manually dispatched `AgentForge Release` run is a full rehearsal. It builds and packages all four
targets, selects the release files, and checks their checksums, as a tag run does. It then uploads
them through the same `gh release create` into a **draft** release named
`rehearsal-<run id>-<attempt>` (`scripts/publish-release rehearse`). A draft creates no tag and is
visible only to maintainers. The rehearsal verifies the uploaded assets exactly as a tag run does,
then deletes the draft, even when verification fails, and fails if a tag with the draft's name
exists afterwards. Since P5-M004 a rehearsal also attests its snapshot archives and verifies those
attestations on the downloaded draft assets, so the signing path is rehearsed too. Since P5-M006
it also generates, attests, and verifies an `agentforge-snapshot-<sha12>.cdx.json` SBOM. The snapshot
attestations stay on GitHub. They are harmless: they describe snapshot archives that were never
released. Run one before tagging a release after any change to the release workflow:

```bash
gh workflow run release.yml -R cybercore-tech/agentforge --ref main
```

Before P5-M003, the upload was tag-only, and it failed on both real tags (`v0.1.0` and `v0.2.0`)
after the fixes to everything else had passed their dry runs.

If a rehearsal run is cancelled between upload and deletion, a `rehearsal-*` draft remains. Delete
it by hand; it has no tag:

```bash
gh release list -R cybercore-tech/agentforge | grep rehearsal-
gh release delete rehearsal-<run id>-<attempt> -R cybercore-tech/agentforge --yes
```

`scripts/publish-release --self-test` (run in CI) checks both modes against a fake `gh`: a missing,
corrupted, or stale-checksum asset fails verification, a failed rehearsal still deletes its draft,
and a staging directory is refused before anything is uploaded. With a signer workflow, an archive
without a provenance attestation fails, and so does one without an SBOM attestation when the release
has an SBOM.

## Support expectations

Releases are currently source-compatible only within the documented `0.x` policy. The project is
local-first and does not promise a hosted service, automatic deployment, or a running daemon in this
phase. Report reproducible failures with the exact release version, target, command, and evidence.
