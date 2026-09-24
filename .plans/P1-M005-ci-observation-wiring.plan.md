# Plan: P1-M005 — CI observation and failure classification in the operator path

Status: Approved
Milestone: P1-M005
Created: 2026-09-23
Owner: AgentForge project

## Goal

Give operators one command that observes the exact-SHA CI run for a commit, classifies every
failed job with the existing conservative classifier, and records the result as durable audit
evidence. This connects the P0-M008 CI monitor and `FailureClassifier`, which no other crate uses
today, to the CLI, the audit log, and therefore the HUD.

## Non-goals

- No automatic repair, retry, rerun, or task transition from CI evidence. Classification stays
  evidence only, as P0-M008 decided.
- No built-in network client or provider credentials inside AgentForge. Observation still runs
  through an explicit, reviewed provider command.
- No change to the `agentforge-ci-v1` protocol, the exact-SHA selection rules, or the classifier
  taxonomy and precedence.
- No polling loop or waiting for completion; one command records one observation.

## Context

`agentforge-ci` has a bounded direct-command `CommandCiMonitor`, strict protocol parsing,
exact-SHA selection that rejects stale, missing, and ambiguous evidence, and a deterministic
classifier. Nothing calls it. The audit schema defines `CiObserved` and nothing emits it.
AGENTS.md requires failures to be classified before repair, but the classifier is not reachable
from any operator path.

## Architecture placement

- `agentforge-ci` gains `CiProviderStore`, which loads `.forge/ci/provider.conf` into a
  `CommandCiMonitorConfig` using the bounded agent-profile `key=value` format, plus stable string
  labels for status, conclusion, and failure category.
- `agentforge-operator` gains `observe_ci`, which runs the monitor, classifies failed jobs, and
  appends audit evidence. It depends on `agentforge-ci` (internal crate only).
- `agentforge-platform` (`forge`) gains `forge ci observe`.
- A reference provider, `scripts/ci-provider-github`, adapts the GitHub CLI (`gh`) to the
  `agentforge-ci-v1` protocol. It lives outside the crates and is opt-in.

## Data flow

1. `forge ci observe <root> <repository> <workflow> <sha> [--task <task-id>]` validates arguments,
   the optional task ID (it must exist in the task snapshot), and the provider profile.
2. The monitor runs the provider command once, with the existing bounds, and selects exactly one
   run for the full SHA.
3. One `CiObserved` event records `repository`, `workflow`, `sha`, `run`, `status`, and
   `conclusion`, plus `task_id` when given.
4. For each job whose conclusion is `failure`, one `FailureClassified` event records `stage=ci`,
   `job`, `category`, and `marker`.
5. The CLI prints the run and one line per job (with category for failed jobs), then exits 0 for a
   successful completed run, 1 for any other completed conclusion, and 3 when the run is still
   queued or in progress. Observation errors (missing run, ambiguous run, provider failure) exit 1
   and record nothing.

## Invariants

- CI evidence is bound to one exact 40-character SHA; stale or ambiguous evidence is never
  recorded.
- Classification never changes task state and never starts a repair.
- The provider receives a cleared environment plus explicit profile values and the three
  `AGENTFORGE_CI_*` request variables.
- Audit field values are bounded and free of control characters.

## ADRs

- ADR-0038: CI observation is an explicit operator command with recorded, classified evidence.

## Public API / CLI

- `agentforge_ci::{CiProviderStore, CiProviderError}`; `as_str` on `CiStatus`, `CiConclusion`,
  and `FailureCategory`.
- `agentforge_operator::{observe_ci, CiObservation}`.
- `forge ci observe <root> <repository> <workflow> <sha> [--task <task-id>]`.
- `scripts/ci-provider-github` reference provider.

## Compatibility analysis

Additive only. Projects without `.forge/ci/provider.conf` are unaffected; `forge ci observe` fails
closed with a clear message when the profile is missing.

## Dependency analysis

No new external crate. New internal edge `agentforge-operator -> agentforge-ci`. The reference
provider needs `python3` and an authenticated `gh` at run time; neither is a build or test
dependency.

## Expected file boundary

- `.plans/P1-M005-ci-observation-wiring.plan.md`
- `.plans/ACTIVE`
- `Cargo.lock`
- `crates/agentforge-ci/src/lib.rs`
- `crates/agentforge-ci/tests/ci_provider.rs`
- `crates/agentforge-operator/Cargo.toml`
- `crates/agentforge-operator/src/lib.rs`
- `crates/agentforge-cli/Cargo.toml`
- `crates/agentforge-cli/src/main.rs`
- `crates/agentforge-cli/src/bin/agentforge-cli-fixture.rs` (fixture CI provider modes)
- `crates/agentforge-cli/tests/ci_commands.rs`
- `scripts/ci-provider-github`
- `docs/CI.md`
- `docs/adr/ADR-0038-operator-ci-observation.md`
- `README.md`
- `docs/MILESTONES.md`
- `PROJECT_STATE.md`
- `AGENT_HANDOFF.md`

## Test-first matrix

- Provider profiles parse documented keys and fail closed on unknown/repeated keys, relative
  executables, symlinks, oversize, and out-of-range limits.
- A successful run records one `CiObserved` event and no classification; exit 0.
- A failed run records `CiObserved` plus one `FailureClassified` per failed job with the expected
  category; exit 1.
- An in-progress run records `CiObserved` with no conclusion; exit 3.
- A provider reporting no run or two runs for the SHA records nothing and exits 1.
- `--task` with an unknown task fails before the provider runs.
- The CLI fixture provider is portable to Linux, macOS, and Windows.

## Implementation sequence

1. Commit this draft plan.
2. Approve and activate it in a separate checkpoint.
3. Add `CiProviderStore`, labels, and tests.
4. Add `observe_ci` in the operator crate and `forge ci observe` with fixture-provider CLI tests.
5. Add the reference GitHub provider and dogfood it against AgentForge's own CI history.
6. Update docs and ADR-0038.
7. Run the full gate, push, and record exact-SHA CI evidence before closure.

## Failure modes

- Provider or protocol errors are reported with the existing `CiObservationError` text and do not
  touch the audit log.
- An uninitialized project (no `.forge/` directory) fails closed. The audit log is created on
  first observation, as the run paths already do, because CI evidence is often a project's first
  audit record.

## Documentation impact

`docs/CI.md` documents the provider profile, command, recorded evidence, exit codes, and the
reference provider. README lists the command. ADR-0038 records the decision.

## Quality gates

- `./scripts/gate.sh full`;
- a real dogfooding observation of an AgentForge CI run through the reference provider;
- exact-SHA CI green on all seven jobs.

## Acceptance criteria

- [ ] `forge ci observe` records exact-SHA CI evidence and classified failures in the audit log.
- [ ] Stale, missing, and ambiguous evidence is never recorded.
- [ ] Classification never mutates task state.
- [ ] The reference provider observes a real AgentForge run.
- [ ] Full local validation and exact-SHA CI evidence are recorded before closure.

## Completion record

Implementation commit:
CI run:
CI result:
Completed:
Notes:
