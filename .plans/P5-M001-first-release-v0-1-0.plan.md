# Plan: P5-M001 — First release, v0.1.0

Status: Complete
Milestone: P5-M001
Created: 2026-09-24
Owner: AgentForge project

## Goal

Cut AgentForge's first published release, `v0.1.0`, from a green `main`: a version bump, a CHANGELOG
release section, a verified packaging dry run, the `v0.1.0` tag, and a GitHub release with
checksummed `forge`/`forged` archives for Linux, macOS (Intel and Apple silicon), and Windows.

## Non-goals

- No crates.io publication (the P2-M019 to P2-M022 policy stands).
- No signing or provenance attestations (later P5 work).
- No compatibility promise beyond the documented `0.x` best-effort policy.

## Context

The workspace has carried version `0.0.1` since P2-M004, but no release tag was ever created: the
repository had no tags until P2-M030, and the CHANGELOG's `[0.0.1]` link points at a release that
does not exist. Since then AgentForge gained gates as evidence, CI observation, concurrent batch
launch, long-running daemon executions, a real-agent bridge, audited agent evidence, hermetic
gates, milestone tags, and a HUD view of agent runs. It has built its own milestones through a real
agent. The operator asked for important points to be tagged and approved cutting `v0.1.0`. This is
a minor bump (new capabilities, `0.x` line).

## Architecture placement

Release engineering only:

- `Cargo.toml` workspace version `0.1.0`; the `agentforge-platform` internal dependency version
  requirements `0.1.0`; `Cargo.lock` refreshed.
- `CHANGELOG.md`: `[Unreleased]` becomes `[0.1.0] - <date>` with a new empty `[Unreleased]`
  section. `[0.0.1]` is kept as history, marked as never tagged, with no release link. The compare
  links are fixed.
- `docs/RELEASE.md`: the current line becomes `0.1.x`, and the example tag becomes `v0.1.0`.
- The README status line and the site's status pills read `0.1.0-alpha`.

## Release procedure

1. Implementation commit (version and docs); push; push-triggered CI plus a dispatched repeat are
   green.
2. Packaging dry run: dispatch `AgentForge Release` on `main` (manual runs build all four targets but
   never publish) and confirm four archives with checksums.
3. Close the milestone and tag `milestone/P5-M001` per AGENTS.md rule 14.
4. Create the annotated `v0.1.0` tag on the closure commit, which has the same code as the verified
   implementation commit, and push it. The release workflow verifies that the tag matches the
   workspace version, builds, and publishes.
5. Verify the GitHub release: four archives, per-archive `.sha256` files, `SHA256SUMS`, generated
   notes. Download one Linux archive, check its checksum, and run `forge version`.

## Invariants

- The tag version equals the workspace version (enforced by the workflow).
- The release is cut only from a commit whose code passed green CI.
- Milestone tags and release tags stay distinct.

## ADRs

None; this follows `docs/RELEASE.md` and ADR-0044.

## Public API / CLI

`forge version` and `forged --version` report `0.1.0`.

## Compatibility analysis

Version metadata and documentation only. The internal crates stay unpublished.

## Dependency analysis

None; `Cargo.lock` changes only for the workspace members' own versions.

## Expected file boundary

- `.plans/P5-M001-first-release-v0-1-0.plan.md`
- `.plans/ACTIVE`
- `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-cli/Cargo.toml`
- `CHANGELOG.md`
- `docs/RELEASE.md`
- `docs/REGISTRY.md` (Amendment 1)
- `scripts/site-updates`, `site/script.js`, `docs/SITE.md` (Amendment 2)
- `.github/workflows/release.yml`, `docs/RELEASE.md`, `docs/OPERATIONS.md` (Amendment 3)
- `README.md`
- `site/index.html`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- `cargo metadata` reports `0.1.0` for every workspace package, and `forge version` prints `0.1.0`.
- `./scripts/package-preflight` produces `agentforge-platform-0.1.0.crate`.
- The dispatched release dry run builds four archives with checksums and publishes nothing.
- After tagging, the release exists with the expected assets, and a downloaded Linux binary reports
  `0.1.0`.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Bump the version and update the docs; run the full gate and package preflight; commit and push;
   check CI.
4. Run the packaging dry run.
5. Close; tag the milestone; tag and push `v0.1.0`; verify the release.

## Failure modes

- A release workflow failure after tagging leaves no published release. Classify the failure; a
  fix ships as `v0.1.1` rather than moving `v0.1.0`.

## Documentation impact

CHANGELOG release section, RELEASE.md, README, and site status.

## Quality gates

- `./scripts/gate.sh full`, `./scripts/package-preflight`;
- green CI on the release commit;
- a successful packaging dry run and a verified published release.

## Acceptance criteria

- [x] `v0.1.0` is published with four checksummed archives.
- [x] Version metadata, the CHANGELOG, and the docs agree on `0.1.0`.
- [x] Milestone closed and tagged, and the release tag verified.

## Amendment 1 (2026-09-24)

A pre-implementation grep found `docs/REGISTRY.md` stating "AgentForge is a `0.0.x` alpha". It is
added to the file boundary so the release line reads `0.1.x` everywhere. Historical records
(`PROJECT_STATE.md` evidence for earlier milestones, ADRs) keep their `0.0.1` references
unchanged.

## Amendment 2 (2026-09-24)

Moving the `[Unreleased]` entries into `[0.1.0]` leaves the section empty, so the project site's
*Coming in the next release* panel (P2-M026) would render as a blank block. The feed generator gains
a `latest_release` object (`version`, `date`) parsed from the first versioned CHANGELOG heading.
When `unreleased` has no entries, the site shows "Nothing unreleased since vX.Y.Z (date)" with a
link to that GitHub release instead of an empty panel. `docs/SITE.md` documents the new field. The
feed stays at `version: 1` because the change is additive.

Test: `./scripts/site-updates` on the release commit emits `latest_release` `0.1.0` with an empty
`unreleased`, and the site renders the fallback line (checked in a local browser preview of
`site/` against the generated feed).

## Amendment 3 (2026-09-24)

`v0.1.0` was tagged on `b1f1cab`, the exact commit with green push CI `36012325272`, dispatched CI
`36012648916`, and packaging dry run `36012653332`, rather than on the closure commit. The code is
the same, and the release commit is the one CI verified.

The tag-triggered release run `36013033041` built all four targets, but **Publish GitHub release**
failed: `gh release create` exited with `failed to run git: fatal: not a git repository`. The
publish job downloads artifacts without checking out the repository, so `gh` cannot infer the
target repository. The job has never run before because no release had ever been published.
Classification: a latent release workflow defect. The code, the builds, and the tag are good.

Recovery, without moving the tag:

1. Publish `v0.1.0` from run `36013033041`'s own CI-built artifacts, with the workflow's exact
   steps: consolidated `SHA256SUMS` over the `.tar.gz` files, then `gh release create v0.1.0 dist/*
   --verify-tag --generate-notes --title "AgentForge v0.1.0"`, adding `--repo
   cybercore-tech/agentforge`. Verify each archive's `.sha256`, `SHA256SUMS`, and a downloaded
   Linux binary.
2. Fix `release.yml`: the publish step passes `--repo "${GITHUB_REPOSITORY}"`, so future tags
   publish without a checkout. `docs/RELEASE.md` and `docs/OPERATIONS.md` record the incident and
   the manual recovery procedure.

This supersedes the "fix ships as `v0.1.1`" failure mode for this case. The tagged code is
correct, and a patch release would only work around a publish-job defect.

## Completion record

Implementation commit: `b1f1cab` (release preparation, tagged `v0.1.0`); `0eb777b` (release
workflow fix, Amendment 3)
CI run: `36012325272` (push) and `36012648916` (dispatched) on `b1f1cab`; `36013626245` on `0eb777b`.
Packaging dry runs: `36012653332` (`b1f1cab`) and `36013843718` (`0eb777b`). Tag release run:
`36013033041`.
CI result: green on all seven jobs in every CI run, and all four targets built in every release run.
The tag run's publish job failed on a workflow defect (Amendment 3).
Completed: 2026-09-24
Notes: `v0.1.0` is published at https://github.com/cybercore-tech/agentforge/releases/tag/v0.1.0.
It has four `.tar.gz` archives (Linux x86_64, macOS x86_64 and aarch64, Windows x86_64), each with
a `.sha256` file, plus a consolidated `SHA256SUMS`. The assets are the tag run's own CI-built
artifacts, published with the workflow's command plus `--repo`. The release notes are the
CHANGELOG `[0.1.0]` section with install steps and provenance. A fresh download passed `sha256sum -c
--ignore-missing SHA256SUMS`, and the extracted binaries report `AgentForge 0.1.0` and `AgentForge
daemon 0.1.0`. The workflow now passes `--repo` and writes bare checksum names, so the next tag
publishes automatically. That path is proven by reproduction and the dry run, not yet by a tag
run.
