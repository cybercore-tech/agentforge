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
workspace version exactly. The current line is `0.1.x` (first tagged release `v0.1.0`) because APIs and operator workflows are still
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
   git tag -a v0.1.0 -m "AgentForge v0.1.0"
   git push origin v0.1.0
   ```

6. Verify the four build jobs, checksums, and generated GitHub release before announcing it:
   download `SHA256SUMS` and one archive, run `sha256sum -c --ignore-missing SHA256SUMS`, and check
   that the extracted `forge version` prints the release version.

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

Then fix the workflow for the next release. `v0.1.0` was published this way: its publish job had no
checkout, so `gh` could not find the repository. Its `SHA256SUMS` step would also have written
`dist/`-prefixed paths. Both defects were fixed in P5-M001.

The workflow also supports manual dispatch for packaging validation. Manual runs build artifacts but
do not publish a release.

## Support expectations

Releases are currently source-compatible only within the documented `0.x` policy. The project is
local-first and does not promise a hosted service, automatic deployment, or a running daemon in this
phase. Report reproducible failures with the exact release version, target, command, and evidence.
