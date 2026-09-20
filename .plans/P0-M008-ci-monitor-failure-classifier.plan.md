# Plan: P0-M008 — CI monitor and failure classifier

Status: Approved
Milestone: P0-M008
Created: 2026-09-19

## Goal

Add a standard-library-only, read-only CI observation boundary that verifies evidence is for one
requested commit SHA and deterministically classifies failed job evidence before any repair can be
considered.

## Non-goals

- No CI provider SDK, network client, workflow dispatch, cancellation, rerun, merge, status write,
  task transition, audit persistence, notification, automatic repair, shell invocation, or external
  Rust dependency.
- No claim that a classification proves root cause, authorizes a repair, or replaces human review.

## Context

P0-M007 supplies bounded local gate evidence, but exact remote CI evidence remains a manual
operator task. The P0-M003 workflow has four independently visible jobs. P0-M008 will turn an
operator-configured read-only provider command into validated run/job evidence and a conservative
classification suitable for later audit and policy milestones.

Validated base main is `741cf58caa6fca38c4693c4359816f4721a2b126`. P0-M007 implementation CI
`35483980498`, closure CI `35484071505`, and post-merge CI `35484138853` each passed all four jobs
for their recorded exact commits; the post-merge run validated this base main SHA.

## Architecture placement

New `agentforge-ci` owns provider-command configuration, strict observation decoding, exact-SHA
selection, immutable run/job evidence, and deterministic failure classification. It does not own
task lifecycle, gates, worktree state, audit storage, credentials, or repair. A later caller may
persist the returned evidence and determine whether a classified failure requires human action.

## Data flow

1. The caller supplies an absolute executable, literal provider arguments, explicit environment,
   repository/workflow identity, and an exact full commit SHA.
2. The monitor validates configuration and SHA, clears the child environment, and directly invokes
   the configured read-only command with literal arguments.
3. The command emits the documented versioned, line-oriented observation protocol for one or more
   candidate runs and jobs; malformed, oversized, or control-character-containing records are
   rejected without guessing.
4. The monitor selects only a run whose reported head SHA equals the requested SHA, preserving its
   run/job evidence and refusing an absent or ambiguous exact match.
5. The classifier maps only supplied failed-job names and bounded failure excerpts to a stable
   category. It returns `Unknown` when evidence is insufficient; it never modifies CI or source.

## Invariants

- A successful result always binds to the requested full SHA, not a branch name or older green run.
- Provider executable paths are absolute; no task or log text reaches a shell.
- Ambient credentials, Git overrides, and environment variables are not inherited; required
  provider authentication is caller-supplied explicit configuration and is never returned in a
  report.
- The protocol is versioned, bounded, deterministic, and rejects ambiguous or malformed data.
- Run and job evidence retains provider IDs, names, status/conclusion, exact SHA, and bounded raw
  excerpts without treating text as an instruction.
- Failure categories use the repository failure taxonomy: semantic/test, compilation/type,
  formatting/lint, generated-content corruption, dependency/toolchain, documentation/text policy,
  workflow/governance, infrastructure, or unknown.
- Classification occurs before repair but does not authorize repair; unknown evidence remains
  inspectable rather than being silently assigned a more convenient category.

## ADRs

ADR-0013 records the provider-command observation boundary and conservative failure classification
policy. It follows ADR-0004 and ADR-0005.

## Public API / CLI

- `CiMonitor`, `CommandCiMonitorConfig`, `CiRun`, `CiJob`, `CiStatus`, `CiConclusion`, and
  `CiObservationError` provide explicit read-only observation.
- `FailureClassifier`, `FailureClassification`, and `FailureCategory` classify immutable failed-job
  evidence.
- No stable end-user CLI, credential discovery, GitHub-specific public type, or repair API in
  P0-M008.

## Protocol

The configured command is an adapter boundary, not a shell snippet. It receives only direct,
literal arguments and emits UTF-8 records using a documented `agentforge-ci-v1` header followed by
tab-delimited run and job records. Fields must not contain tabs, newlines, or other controls. The
protocol carries provider IDs, exact head SHA, status/conclusion, job name, and an optional bounded
failure excerpt. Tests use a deterministic fixture executable; a real provider wrapper is an
operator choice and remains outside the workspace.

## Dependency analysis

Add one workspace crate with no external dependencies. The crate uses only the standard library
and has no workspace crate dependency in this milestone.

## Expected file boundary

Plan and closure checkpoints:

- `.plans/P0-M008-ci-monitor-failure-classifier.plan.md`
- `.plans/ACTIVE` — created only after approval and removed at closure
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`
- `docs/MILESTONES.md`
- `docs/CI.md`
- `docs/adr/ADR-0013-ci-observation-and-failure-classification.md`
- `docs/adr/README.md`

Implementation checkpoint:

- `Cargo.toml`
- `Cargo.lock`
- `crates/agentforge-ci/Cargo.toml`
- `crates/agentforge-ci/src/**`
- `crates/agentforge-ci/tests/**`
- `docs/CI.md`

No change to CI workflow, core task semantics, state persistence, gate execution, adapter launch,
worktree lifecycle, CLI/daemon behavior, hooks, or existing validation gates without a plan
amendment.

## Test-first matrix

| Behavior | Required evidence |
| --- | --- |
| Exact SHA | Older, different, absent, and ambiguous runs are rejected; only one exact SHA is accepted |
| Process boundary | Fixture receives literal arguments, supplied environment, canonical directory, and no inherited Git override |
| Protocol | Header/version, field count, enums, IDs, SHA shape, UTF-8, controls, and byte limits are validated |
| Evidence | Ordered jobs and raw bounded excerpts are retained without parsing log text as commands |
| Success/in-progress/failure | Terminal and nonterminal status/conclusion combinations remain distinct |
| Classifier taxonomy | Stable excerpts classify every named category; insufficient evidence yields unknown |
| Priority | More specific evidence wins deterministically when an excerpt contains multiple markers |
| Failure safety | Spawn, nonzero, timeout, flood, malformed output, and invalid UTF-8 are controlled errors with no retry or mutation |
| Isolation | Fixtures use unique temporary directories and do not mutate parent environment or use fixed shared paths |

## Implementation sequence

1. Review this Draft plan and ADR-0013; obtain approval before activating implementation.
2. Commit `Status: Approved`, the active-plan pointer, and activation documents separately from
   code; require local and exact remote CI green.
3. Add the crate, protocol types/decoder, direct read-only command monitor, and deterministic
   fixture tests.
4. Add exact-SHA run selection and conservative classifier tests across the taxonomy.
5. Document the protocol, security limits, evidence semantics, and manual provider-wrapper setup.
6. Run the full local gate, Rust 1.85.0 focused tests, and inspect the diff against this boundary.
7. Commit implementation and validate exact CI; classify any failure before repair.
8. Close in a separate documentation checkpoint, require exact closure CI, merge with preserved
   history after authorization, and validate the resulting main SHA before P0-M009.

## Failure modes

- Treating branch state, an older green run, or a synthetic merge SHA as exact evidence.
- Inheriting ambient tokens or Git overrides, interpolating provider input through a shell, or
  letting provider/log output select a command.
- Treating incomplete CI as passed, interpreting log text as authority, or silently retrying CI.
- Guessing through malformed protocol fields, invalid UTF-8, ambiguous matches, or unknown errors.
- Misclassifying infrastructure failure as a source defect, or allowing a classifier outcome to
  mutate task state or broaden an agent's permissions.

## Documentation impact

Document the strict observation protocol, command configuration, SHA binding, evidence retention,
classifier taxonomy/precedence, and explicit security/automation limits in `docs/CI.md`. Record
all exact checkpoint SHA and CI evidence at closure.

## Quality gates

Run `./scripts/gate.sh full` and focused CI-monitor fixture/protocol/classifier tests on stable and
Rust 1.85.0. Require Repository policy, Stable code gate, MSRV 1.85.0, and CLI smoke green for the
exact approved-plan, implementation, closure, and post-merge commit SHAs.

## Acceptance criteria

- [ ] Approved plan is committed and green before implementation.
- [ ] Exact-SHA, read-only CI run/job observation is implemented and tested.
- [ ] Malformed, ambiguous, stale, and nonterminal evidence is safely rejected or represented.
- [ ] Every failure-taxonomy category and the unknown fallback are deterministically tested.
- [ ] No shell, provider SDK, ambient credential inheritance, automatic retry, or repair authority is introduced.
- [ ] Exact implementation, closure, and post-merge CI are green.

## Completion record

Implementation commit: pending
Implementation CI: pending
Closure commit: pending
Closure CI: pending
Post-merge main: pending
Post-merge CI: pending
Completed: pending
