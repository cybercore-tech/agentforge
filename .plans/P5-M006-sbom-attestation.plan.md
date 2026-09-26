# Plan: P5-M006 — SBOM attestation for releases (agent-built)

Status: Complete
Milestone: P5-M006
Created: 2026-09-25
Owner: AgentForge project

## Goal

Every release carries a CycloneDX SBOM of `forge` and `forged`, attached to the release and
**attested** for every archive, next to the build provenance (P5-M004). Anyone can check it with
`gh attestation verify <archive> -R cybercore-tech/agentforge --predicate-type
https://cyclonedx.org/bom`.

The milestone is also the operator's option 2: **it is implemented by Claude Code through AgentForge
with the MCP task-tool gateway** (P3-M005), which is the gateway's first use on real work. The task
holds `use_mcp_tools`, so the agent has `task_contract`, `check_changes`, and `run_gate`.

## Operator decisions (2026-09-25)

- Option 2 with option 4: a real agent-built milestone using the MCP gateway, with SBOM attestation
  as its task.
- SBOM tool: `cargo-cyclonedx`, pinned to 0.5.9 and installed with `cargo install --locked` in the
  release job (no third-party Action). Approved as a release-pipeline dependency.

## Non-goals

- No SBOMs for `v0.1.0` to `v0.3.0`; no new release in this milestone.
- No vulnerability scanning or license policy (the SBOM makes them possible later).
- No change to the Rust workspace or its dependencies.

## Context

`cargo cyclonedx --format json --spec-version 1.5 --describe binaries --no-build-deps --target
<target>` (0.5.9) writes one CycloneDX file per binary next to its crate's `Cargo.toml`, named
`<binary>_bin.cdx.json`: `crates/agentforge-cli/forge_bin.cdx.json` and
`crates/agentforge-daemon/forged_bin.cdx.json`, plus files for test fixtures and `xtask`, which must
not be released. Tried by the operator on a scratch clone: `forge` has 54 components (including
`serde_json` from the MCP crate) and `forged` has 43. `--target all` includes the dependencies of
every platform. `SOURCE_DATE_EPOCH` makes the output reproducible. `actions/attest@v4` accepts
`sbom-path` (SPDX or CycloneDX JSON) and creates an SBOM attestation for the given subjects.

## Architecture placement

- **`scripts/sbom-merge`** (new, Python 3, standard library only) merges the two per-binary
  CycloneDX 1.5 documents into one:
  - `sbom-merge --name agentforge --version <v> <in.json>... > out.json`;
  - components deduplicated by `bom-ref` (or `purl`), with dependency edges merged and sorted;
  - a metadata component `agentforge` at the version, whose components are the two binaries;
  - a deterministic `serialNumber` (a UUID derived from the content), and `SOURCE_DATE_EPOCH`
    honoured for the timestamp, so the same inputs give byte-identical output;
  - `--self-test` on built-in fixtures: dedupe, dependency merge, determinism, and malformed or
    wrong-format input refused.
- **`.github/workflows/release.yml`**, in the publish job after the release files are selected:
  - install Rust stable and `cargo install --locked cargo-cyclonedx@0.5.9`;
  - checkout is already present (P5-M003); with `SOURCE_DATE_EPOCH` set to the commit time, run
    `cargo cyclonedx` for `--target all` and merge `forge_bin` and `forged_bin` into
    `release/agentforge-<version>.cdx.json`. The version is the tag's for tags and
    `snapshot-<sha12>` for dispatch, as in the build job;
  - a second `actions/attest@v4` step: `subject-path: release/*.tar.gz` and `sbom-path` set to that
    file;
  - the "Select release files" guard still requires exactly four archives, and now also exactly one
    `*.cdx.json`. `SHA256SUMS` also covers the SBOM.
- **`scripts/publish-release`**, with `ATTESTATION_SIGNER_WORKFLOW` set: `verify` additionally runs
  `gh attestation verify <archive> --predicate-type https://cyclonedx.org/bom --signer-workflow ...`
  for every archive, when the release has a `*.cdx.json`. The self-test's fake `gh` models SBOM
  attestations (attested and missing), with cases for both.
- **`.github/workflows/ci.yml`**: the repository job runs `./scripts/sbom-merge --self-test`.
- **ADR-0054:** a CycloneDX SBOM from `cargo-cyclonedx`, one per release, attested per archive.
  Plus its registry row.
- **Docs:** RELEASE (the SBOM, how it is made, how to verify it) and OPERATIONS (the workflow row).
  The CHANGELOG `[Unreleased]` gets an entry.

## Invariants

- The release archives are unchanged; the SBOM is an added file and an added attestation.
- The SBOM is generated from the tagged commit's own `Cargo.lock` (`--locked` install, repository
  metadata), never from the network at large.
- A release whose archives lack a valid SBOM attestation fails its publish job, as provenance
  already does.

## ADRs

ADR-0054 (new), with its registry row (enforced by `xtask validate`).

## Public API / CLI

Maintainer scripts only.

## Compatibility analysis

Additive to releases. `publish-release` still verifies older releases without an SBOM (the SBOM
check applies only when the release has one).

## Dependency analysis

`cargo-cyclonedx` 0.5.9, installed in the release job with `--locked` (MSRV 1.85; the job uses
stable). Nothing in the Rust workspace changes.

## Agent task

One task, `P5-M006-T0001`, for Claude Code via `scripts/agents/claude-code-bridge`:
- capabilities: `read_repository`, `write_owned_paths`, `run_local_commands`, and `use_mcp_tools`;
- gate: `workspace` (`./scripts/gate.sh full`), with approval `merge_protected_branch` (after
  review);
- allowed paths: `scripts/sbom-merge`, `scripts/publish-release`, `.github/workflows/release.yml`,
  `.github/workflows/ci.yml`, `docs/adr/ADR-0054-release-sbom.md`, `docs/adr/README.md`,
  `docs/RELEASE.md`, `docs/OPERATIONS.md`, and `CHANGELOG.md`;
- the goal points at this plan.

The operator reviews the diff, accepts, approves, and integrates it, then proves it with a
dispatched release rehearsal. The rehearsal's run is the only place the workflow half can execute.

## Expected file boundary

- `.plans/P5-M006-sbom-attestation.plan.md`, `.plans/ACTIVE`
- the task's allowed paths above
- `docs/DOGFOODING.md` (the run log, by the operator)
- closure records: `docs/MILESTONES.md`, `CHANGELOG.md`, `README.md`, `PROJECT_STATE.md`,
  `AGENT_HANDOFF.md`

## Test-first matrix

- `scripts/sbom-merge --self-test`, run in CI.
- `scripts/publish-release --self-test` with the SBOM cases.
- The agent runs both, plus the full gate through the gateway's `run_gate`, and calls
  `check_changes` before finishing.
- **Real:** a dispatched release rehearsal attests provenance and the SBOM for all four archives and
  verifies both on the uploaded draft assets. Then, from this machine, `gh attestation verify
  --predicate-type https://cyclonedx.org/bom` passes on a downloaded archive, and the SBOM lists
  `serde_json` and the workspace crates.
- The full gate, and push CI plus a dispatched repeat.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Create the task, and launch Claude Code through `forge task launch` with the `claude-code`
   profile (with `--forge`, so the gateway is wired in).
4. Review: diff, gate result, and the agent's `ToolInvoked` audit trail. Accept, approve, and
   integrate.
5. Push; CI plus a repeat; a release rehearsal; independent verification.
6. Close, with the run log in DOGFOODING, and tag after the closure CI is green.

## Failure modes

- The agent stops early or goes out of bounds: the bridge refuses to commit, the attempt is kept
  as evidence, and a retry goes by amendment (as in P1-M007).
- The workflow half can only be tested by a rehearsal after integration. If the rehearsal fails,
  it is fixed forward by amendment.

## Documentation impact

RELEASE, OPERATIONS, ADR-0054, DOGFOODING (the agent run log); CHANGELOG and README at closure.

## Quality gates

- `./scripts/gate.sh full` (the agent through `run_gate`, and the bridge's pre-commit hook);
- push CI plus a dispatched repeat, and a dispatched release rehearsal.

## Acceptance criteria

- [x] Releases carry an attested CycloneDX SBOM per archive, verified by the workflow on the uploaded
      assets.
- [x] The implementation was produced by Claude Code through AgentForge with the MCP gateway, and its
      tool calls are in the audit log.
- [x] A release rehearsal and an independent `gh attestation verify --predicate-type` pass.
- [x] Docs; CI evidence; closed and tagged correctly.

## Completion record

Implementation commit: `14efd83` (by Claude Code through AgentForge; task `P5-M006-T0001`)
CI run: `36206311396` (push) and `36206485625` (dispatched repeat); release rehearsal `36206324690`
CI result: green on all seven jobs in both runs; the rehearsal is green on all five jobs
Completed: 2026-09-25
Notes:
- **Agent run:** 12 min 40 s, `agent-exit=0`, committed by the bridge (pre-commit gate passed),
  and the orchestrator's `workspace` gate passed 1/1. The agent used the MCP gateway as intended:
  `ToolInvoked` #38 `task_contract`, #39 `run_gate` (workspace), and #40 `check_changes` (9 changed
  paths, all in scope). It is the gateway's first use on real work.
- **Operator review:** the agent reported that it could not run its own self-tests (see finding
  22). The operator ran both: `sbom-merge` passed, and `publish-release` (with the new SBOM cases)
  passed. The merge was checked on real cargo-cyclonedx 0.5.9 output: 54 components, `serde_json`
  present, and byte-identical output across input order. The workflow diff was read before
  integration.
- **Integration:** `forge task integrate` refused, correctly, because the task lacked the
  `merge_protected_branch` capability. The operator created it with `--approval
  merge_protected_branch` but without the capability (finding 21). Accept and approve were recorded
  against `14efd83`, and `main` (the task's base, `6ee9463`) was fast-forwarded to that exact
  reviewed commit, the same route as the operator's own commits. The worktree is retired and the
  branch kept.
- **Rehearsal `36206324690`:** generated `agentforge-snapshot-14efd83ac09d.cdx.json` (76 KB),
  included in `SHA256SUMS`; "Attestation created for 4 subjects" twice (provenance, then SBOM); and
  "verified 10 assets ... (names, bytes, SHA256SUMS, 4 attestations, 4 SBOM attestations)".
  Checked from this machine: `gh attestation verify --predicate-type https://cyclonedx.org/bom`
  passes on the Linux archive. The attested SBOM (64 components with `--target all`) includes
  `serde_json` and `agentforge-mcp`, and a tampered copy fails.
- **Open findings from this run:** 21 (task creation should refuse an approval boundary without its
  capability) and 22 (the bridge's tool allow-list keeps the agent from running project scripts,
  including self-tests it writes).
- `scripts/sbom-merge` is committed without the executable bit, and CI calls it with `python3`,
  consistently. Left as is.
