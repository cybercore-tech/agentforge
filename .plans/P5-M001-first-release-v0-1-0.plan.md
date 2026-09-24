# Plan: P5-M001 — First release, v0.1.0

Status: Approved
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

- [ ] `v0.1.0` is published with four checksummed archives.
- [ ] Version metadata, the CHANGELOG, and the docs agree on `0.1.0`.
- [ ] Milestone closed and tagged, and the release tag verified.

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

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
