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

## Version policy

The workspace version is the release version. Tags must use `vMAJOR.MINOR.PATCH` and must match the
workspace version exactly. The current line is `0.2.x` (latest release `v0.2.0`; the first tag was `v0.1.0`) because APIs and operator workflows are still
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
The GitHub release also contains a consolidated `SHA256SUMS` file. Artifacts are unsigned in this
phase; signing and provenance attestations are future release-engineering work.

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
   git tag -a v0.2.0 -m "AgentForge v0.2.0"
   git push origin v0.2.0
   ```

6. Check the run before announcing the release. Since P5-M003, the publish step downloads every
   uploaded asset back and fails unless the asset names equal the selected files, each asset is
   byte-identical, and `SHA256SUMS` passes `sha256sum -c`. Still download one archive yourself and
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

If the upload succeeded but its verification failed, the release already exists. Do not move the
tag: compare the release's assets with the run's artifacts, and replace the wrong assets (`gh release
upload vX.Y.Z <file> --clobber`) from those artifacts.

### Upload rehearsal (manual dispatch)

A manually dispatched `AgentForge Release` run is a full rehearsal. It builds and packages all four
targets, selects the release files, and checks their checksums, as a tag run does. It then uploads
them through the same `gh release create` into a **draft** release named
`rehearsal-<run id>-<attempt>` (`scripts/publish-release rehearse`). A draft creates no tag and is
visible only to maintainers. The rehearsal verifies the uploaded assets exactly as a tag run does,
then deletes the draft, even when verification fails, and fails if a tag with the draft's name
exists afterwards. Run one before tagging a release after any change to the release workflow:

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
and a staging directory is refused before anything is uploaded.

## Support expectations

Releases are currently source-compatible only within the documented `0.x` policy. The project is
local-first and does not promise a hosted service, automatic deployment, or a running daemon in this
phase. Report reproducible failures with the exact release version, target, command, and evidence.
