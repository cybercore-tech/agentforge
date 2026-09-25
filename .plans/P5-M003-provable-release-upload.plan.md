# Plan: P5-M003 — Provable release upload

Status: Complete
Milestone: P5-M003
Created: 2026-09-25
Owner: AgentForge project

## Goal

The release upload step can be proven without cutting a release. A manually dispatched `AgentForge
Release` run uploads the release files through the same `gh release create` path a tag uses, into a
**draft** release that creates no tag. It downloads every asset back, checks it against the
selected files and `SHA256SUMS`, then deletes the draft. Real tag runs perform the same read-back
check after publishing, so a published release is verified by the workflow, not by hand.

## Non-goals

- No new release and no version change.
- No signing or provenance attestations (a later P5 milestone).
- No change to what is released: four archives, their `.sha256` files, and `SHA256SUMS`.

## Context

The upload step has failed on both real tags so far (P5-M001: no repository for `gh`; P5-M002: a
staging directory in `dist/*`). Each fix was verified only up to the upload, because the upload was
tag-only, and P5-M002 closed with "the upload step itself is proven only by the next real tag". A
defect in that step is found only when a release is already tagged, which forces the manual
recovery in `docs/RELEASE.md`. This is the same class as P2-M034: correctness that depends on a
remembered step (watching the next tag).

## Architecture placement

- **`scripts/publish-release`** (bash, new), holding all upload logic:
  - `publish-release tag <vX.Y.Z> <dir>`: `gh release create` with `--verify-tag`, generated notes
    and the release title (as today), then `verify`.
  - `publish-release rehearse <name> <sha> <dir>`: `gh release create <name> --draft --target <sha>`
    with the same files, then `verify`, then deletes the draft **always** (an exit trap), and
    finally checks that no tag `<name>` exists.
  - `verify <name> <dir>`: `gh release download` into a fresh directory; the set of downloaded asset
    names must equal the set of files in `<dir>`, every file must be byte-identical, and
    `sha256sum -c SHA256SUMS` must pass there.
  - `--self-test`: runs both modes against a fake `gh` on `PATH` that keeps releases in a temporary
    directory, covering success, a missing asset, a corrupted asset, and cleanup after a failure.
  - The repository comes from `GITHUB_REPOSITORY` (`--repo`), as today.
- **`release.yml`**: the publish job checks out the repository (for the script only; `gh` still
  gets `--repo`). The final step calls `publish-release tag` on tags and `publish-release rehearse
  rehearsal-<run id>-<attempt> ${GITHUB_SHA}` on dispatch.
- **`ci.yml`**: the repository job runs `./scripts/publish-release --self-test` next to the other
  self-tests.

## Invariants

- A dispatched run never publishes a release and never leaves a draft or a tag behind, including
  when verification fails.
- A tag run fails if the published assets differ from the selected files in name set or content.
- The files released are unchanged.

## ADRs

None; `docs/RELEASE.md` records the procedure.

## Public API / CLI

`scripts/publish-release` (a maintainer script).

## Compatibility analysis

Workflow and scripts only. The tag path gains a post-publish check; a failure there happens after a
release exists and is handled by the existing recovery (inspect, fix assets, never move the tag).

## Dependency analysis

None: `gh`, `sha256sum`, and `cmp`, already on GitHub runners.

## Expected file boundary

- `.plans/P5-M003-provable-release-upload.plan.md`, `.plans/ACTIVE`
- `scripts/publish-release`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`
- `docs/RELEASE.md`, `docs/OPERATIONS.md`
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- Self-test: tag mode publishes and verifies; rehearse mode uploads, verifies, deletes, and leaves
  no tag; a dropped asset fails verification; a corrupted asset fails verification; a failed
  rehearsal still deletes its draft.
- Real proof: a dispatched release run on `main` whose log shows the draft upload, the read-back
  verification of nine assets, and the deletion; afterwards `gh release list` shows no draft and
  `gh api` finds no `rehearsal-*` tag.
- The full gate, and push CI plus a dispatched repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Script with its self-test (failure cases proven to bite), then the workflows, then docs.
4. Gate; commit (exit checked); push; CI plus a repeat; a dispatched release rehearsal, checked.
5. Close; tag after the dry run names the "close ..." commit.

## Failure modes

- `gh` cannot download draft assets: the rehearsal fails loudly and the trap still deletes the
  draft. The fix is recorded by amendment.
- A runner is cancelled between create and delete: a draft named `rehearsal-*` remains, visible
  only to maintainers, and is deleted by hand (`docs/RELEASE.md`). It creates no tag.

## Documentation impact

RELEASE (the rehearsal, the post-publish check, stale-draft cleanup) and OPERATIONS (the workflow
row). CHANGELOG at closure.

## Quality gates

- `./scripts/gate.sh full` and `./scripts/publish-release --self-test`;
- push CI plus a dispatched repeat, and a dispatched release rehearsal.

## Acceptance criteria

- [x] A dispatched release run uploads, verifies, and deletes a draft release, leaving nothing.
- [x] Tag runs verify the published assets after upload.
- [x] The script's self-test runs in CI and covers the failure cases.
- [x] Docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `8353548`
CI run: `36107774174` (push) and `36108002066` (dispatched repeat); release rehearsal
`36107796647`
CI result: green on all seven jobs in both runs; the rehearsal is green on all five jobs
Completed: 2026-09-25
Notes:
- **The upload is proven without a release.** Rehearsal run `36107796647` built all four targets,
  uploaded the nine release files to the draft `rehearsal-36107796647-1` through the same `gh
  release create` a tag uses, downloaded them back, and verified names, bytes, and `SHA256SUMS`
  ("verified 9 assets"), then deleted the draft. Checked independently afterwards: `gh release
  list` shows only `v0.1.0` and `v0.2.0` (with `v0.2.0` still Latest), and the GitHub API finds no
  `rehearsal-*` tag. The check P5-M002 carried forward to the next real tag is closed.
- The self-test (20 checks, run in CI's repository job) was mutation-tested: removing the
  verification, the draft deletion, the regular-file check, or the `sha256sum -c` check each fails
  it. The `sha256sum -c` mutant first survived, because the byte comparison caught every corruption
  case first; a stale-`SHA256SUMS` case now covers it.
- The plan was approved on the second attempt: the first approval commit wrote the milestone ID
  instead of the plan path to `.plans/ACTIVE`, and the pre-commit validation refused it. The
  commit's exit code was checked, so nothing proceeded on it.
