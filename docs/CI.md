# AgentForge Continuous Integration

P0-M003 bootstraps GitHub Actions as AgentForge's independent remote validation authority.

## Workflow

The repository workflow is `.github/workflows/ci.yml`.

It exposes independently visible policy, code, compatibility, smoke, and platform jobs:

1. `Repository policy`
2. `Stable code gate`
3. `MSRV 1.85.0`
4. `CLI smoke`
5. `Platform matrix`

Independent jobs keep failures classifiable instead of hiding all evidence behind one monolithic
status.

## Triggers

The workflow runs on:

- pull requests;
- pushes to `main`;
- manual workflow dispatch.

Pull-request jobs explicitly check out the pull-request head SHA so exact-head policy checks inspect
the real branch commit rather than only GitHub's synthetic merge commit.

Merge-queue support may be added later if the repository enables a merge queue.

## Repository policy

The repository-policy job validates:

- tracked text policy;
- required repository structure;
- active-plan state;
- plan-first implementation authority.

For staged local changes, implementation-sensitive paths require an Approved plan already in HEAD.

For committed branch heads, an implementation-sensitive commit requires its parent commit to
contain the Approved active plan.

Merge commits are integration boundaries; repository-state validation still runs, while
single-commit sequencing is enforced on the branch commits before merge.

## Permissions

Workflow-level GitHub token permissions are:

```yaml
permissions:
  contents: read
```

A later job may receive broader authority only through an explicit reviewed requirement.

## Dependency and cache policy

All Cargo operations that consume the lockfile use `--locked`.

The bootstrap workflow does not use a dependency/build cache. AgentForge is currently small enough
that cache complexity is not justified.

## Stable code gate

Stable CI installs current stable Rust with `rustfmt` and `clippy`, then runs:

- `cargo fmt --all --check`;
- `cargo check --workspace --all-targets --locked`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `cargo test --workspace --locked`.

## MSRV

The MSRV job installs Rust 1.85.0 and runs locked workspace check and tests independently of stable.

## CLI smoke

The smoke job verifies:

- `forge version`;
- `forge doctor`;
- `forged --version`.

## Platform matrix

The `Platform matrix` job runs stable workspace checks, portable worktree and daemon dogfooding
tests, and CLI version smoke on Ubuntu 24.04, macOS 14, and Windows 2022. These fixtures invoke
Git and the agent executable directly, so the same managed-worktree workflow is exercised on every
supported host. Any remaining Unix-specific integration fixture stays in the Linux stable gate
with its platform dependency documented beside the test.

## Job timeouts

Every CI job declares `timeout-minutes` (15 for repository policy and CLI smoke, 30 for the stable
and MSRV gates, 40 for each platform-matrix host). A hung test fails its job within that bound
instead of holding a runner until the six-hour platform default. The daemon lifecycle tests
themselves use explicit readiness and exit deadlines, so a hang there fails the test with a named
diagnostic well before the job timeout.

## Release workflow

`.github/workflows/release.yml` runs for `vMAJOR.MINOR.PATCH` tags and manual dispatch. Tagged runs
verify that the tag matches the workspace version, build `forge` and `forged` for four targets,
package archives, emit SHA-256 checksums, and publish a GitHub release with least-privilege write
permission only on the publish job. Manual runs package artifacts without publishing.

## P0-M008 observation boundary

The `agentforge-ci` crate observes remote CI through an operator-configured absolute executable.
The executable receives literal arguments and explicit environment values; ambient credentials,
Git overrides, and shell interpolation are not inherited. Its stdout is a bounded UTF-8 protocol:
the first line is `agentforge-ci-v1`, followed by tab-delimited `run` and `job` records containing
provider IDs, exact head SHA, status, conclusion, names, and bounded failure excerpts. Fields may
not contain control characters.

The monitor accepts exactly one run whose reported head SHA matches the requested full SHA. Stale,
missing, ambiguous, malformed, non-UTF-8, timed-out, and output-limit observations remain explicit
errors. It never dispatches, retries, cancels, mutates CI, changes source, or transitions a task.

`FailureClassifier` maps failed-job evidence to the repository taxonomy—semantic/test,
compilation/type, formatting/lint, generated-content corruption, dependency/toolchain,
documentation/text policy, workflow/governance, infrastructure, or unknown—with documented
specificity precedence. Classification is evidence for later review, not root-cause proof or repair
authority. Provider authentication and network behavior remain outside the workspace command.

## Operator CI observation

P1-M005 makes the monitor and classifier reachable from the CLI. A project declares its provider
command once, in `.forge/ci/provider.conf`, using the bounded agent-profile format (`version=1`,
absolute `executable`, literal `argument` values, `env.NAME=value`, optional `timeout_ms` up to one
hour, and `max_output_bytes`). `AGENTFORGE_CI_*` names are reserved for the request variables.

```bash
forge ci observe <root> <repository> <workflow> <40-hex-sha> [--task <task-id>]
```

One call runs the provider once and selects exactly one run for the SHA. It then records:

- one `CiObserved` audit event with `repository`, `workflow`, `sha`, `run`, `status`, and
  `conclusion` (plus the task ID with `--task`); and
- one `FailureClassified` event per job whose conclusion is `failure`, with `stage=ci`, `job`,
  `category`, and the matched `marker`.

It prints the run and one line per job, then exits 0 for a successful run, 1 for any other
completed conclusion or an observation error, 3 while the run is queued or in progress, and 2 for
usage errors. Missing, stale, ambiguous, or malformed evidence records nothing. Observation never
changes task state or starts a repair. The project must be initialized; the audit log is created
on first observation, as the run paths do.

### Reference GitHub provider

`scripts/ci-provider-github` adapts an authenticated GitHub CLI to the protocol. When re-runs or
manual dispatches produce several runs for one workflow and SHA, it reports only the most recently
created run. Failed jobs carry a bounded excerpt of the failed-step log lines that look like errors,
panics, or lint output. Because providers run with a cleared environment, the profile passes what
`gh` needs:

```text
version=1
executable=/usr/bin/python3
argument=/path/to/agentforge/scripts/ci-provider-github
env.PATH=/usr/bin:/bin
env.HOME=/home/operator
timeout_ms=180000
```

Add `env.GH_TOKEN=...` only if `gh auth` is not configured for that `HOME`. The profile then holds
a credential: keep it out of version control and readable only by the operator.

## Exact-head evidence

A green run is evidence only for the commit SHA that produced it.

Implementation, closure, and post-merge validation records must identify the exact head they
validated. A successful run from an older commit is not transferable evidence.

## Bootstrap exception

The P0-M003 Approved plan checkpoint predates this workflow. It is validated by the existing local
full gate.

After this workflow is introduced, remote exact-head CI is required for the remaining P0-M003
checkpoints and future milestones.

## Main branch protection target

After P0-M003 is merged and the checks exist on `main`, protected-main policy should require:

- pull requests for changes;
- `Repository policy`;
- `Stable code gate`;
- `MSRV 1.85.0`;
- `CLI smoke`;
- branches to be up to date before merge;
- force pushes disabled;
- branch deletion disabled.

Repository administrators may add stricter controls later.
