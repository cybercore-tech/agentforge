# Plan: P2-M004 — Release readiness

Status: Approved
Milestone: P2-M004
Created: 2026-09-20

## Goal

Move AgentForge from a tested source checkout toward a repeatable alpha release process by adding
explicit project metadata, an MIT license, a changelog and versioning policy, tagged release
artifacts for the `forge` and `forged` binaries, and cross-platform CI coverage.

## Scope

- Add release metadata to the workspace manifest: repository, description, readme, and MIT license.
- Add a root `LICENSE` containing the approved MIT license text.
- Add `CHANGELOG.md` with Keep a Changelog structure and SemVer policy for the `0.x` release line.
- Add a tagged/manual GitHub release workflow that builds `forge` and `forged`, packages supported
  platform archives, emits SHA-256 checksums, and publishes a GitHub release for version tags.
- Extend CI with a stable cross-platform build/test matrix for Linux, macOS, and Windows while
  preserving the exact-head policy and existing stable/MSRV/policy/smoke jobs.
- Update README and release documentation with artifact names, tag syntax, installation guidance,
  support expectations, and the current alpha status.

## Non-goals

- No daemon implementation, provider marketplace, or new orchestration behavior.
- No crates.io publication; workspace crates remain `publish = false`.
- No automatic production deployment or signing-key management.
- No dependency additions.
- No claim that the release artifacts provide a long-term compatibility guarantee.

## Proposed license

Use the MIT License for this repository and record `license = "MIT"` in workspace metadata. This is
the proposed permissive default for the current open-source project; changing it requires an explicit
plan amendment before implementation.

## Release contract

- Release tags use `vMAJOR.MINOR.PATCH` (for example, `v0.1.0`).
- `0.x` follows SemVer intent: patch releases are compatible fixes, minor releases may add
  capabilities, and breaking changes are called out explicitly in the changelog.
- Release artifacts are named `agentforge-<version>-<target>.<ext>` and include both binaries.
- Each archive has a sibling `.sha256` checksum file; the workflow also emits a consolidated
  `SHA256SUMS` file.
- Supported release targets are Linux x86_64, macOS x86_64, macOS arm64, and Windows x86_64.
- Release workflow runs only for version tags or explicit manual dispatch; ordinary pushes never
  publish a release.

## Expected files

- `.plans/P2-M004-release-readiness.plan.md`, `.plans/ACTIVE`
- `PROJECT_STATE.md`, `AGENT_HANDOFF.md`, `docs/MILESTONES.md`
- `Cargo.toml`
- `LICENSE`, `CHANGELOG.md`, `docs/RELEASE.md`
- `README.md`
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`

## Acceptance criteria

- [ ] Workspace metadata declares the repository, readme, description, and MIT license.
- [ ] License and changelog/versioning policy are present and linked from the README.
- [ ] Release workflow builds both binaries for all four supported targets and packages deterministic
      archives with checksums.
- [ ] Release workflow cannot publish from an ordinary branch push and uses least-privilege token
      permissions.
- [ ] CI exercises stable workspace checks/tests on Linux, macOS, and Windows.
- [ ] Existing exact-head policy, MSRV coverage, CLI smoke, formatting, Clippy, and tests remain.
- [ ] Full local gate and exact-head CI pass for implementation and closure commits.

## Validation

- `./scripts/gate.sh full`
- GitHub Actions workflow syntax/policy checks
- Cross-platform matrix checks/tests
- Exact-head CI for the implementation and closure commits

## Implementation sequence

1. Commit this approved plan and active pointer as a plan-only checkpoint.
2. Add release metadata, MIT license, changelog, and release documentation.
3. Add the cross-platform CI matrix without weakening existing jobs.
4. Add the tagged/manual packaging workflow with deterministic archive/checksum steps.
5. Update README and record implementation evidence.
6. Run local gate and exact-head CI.
7. Close the milestone with separate documentation and CI evidence.
