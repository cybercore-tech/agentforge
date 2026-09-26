# AgentForge 🛠️🤖

AgentForge is a local-first, model-agnostic development environment for running AI-assisted
software work as explicit, reviewable engineering workflows.

It treats models as replaceable workers—not as the source of truth. The durable source of truth is
the repository: plans, task contracts, permissions, isolated worktrees, quality-gate evidence,
CI observations, audit records, and human decisions. 🧭

> **Status:** `0.3.1-alpha` · latest release
> ([v0.3.1](https://github.com/cybercore-tech/agentforge/releases/tag/v0.3.1), attested builds and SBOM) · alpha: evolving
> APIs, local-first, not for unattended production use
>
> **Canonical repository:** [`cybercore-tech/agentforge`](https://github.com/cybercore-tech/agentforge).
> The earlier [`darkstardevx/agentforge`](https://github.com/darkstardevx/agentforge) holds history
> up to `976c4f9` and is no longer updated.

[![CI](https://github.com/cybercore-tech/agentforge/actions/workflows/ci.yml/badge.svg)](https://github.com/cybercore-tech/agentforge/actions/workflows/ci.yml)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![Project site](https://img.shields.io/badge/project%20site-AgentForge-08111f?logo=github)](https://cybercore-tech.github.io/agentforge/)

**[Visit the AgentForge project site →](https://cybercore-tech.github.io/agentforge/)**. Its
[What's new](https://cybercore-tech.github.io/agentforge/#updates) section shows recently shipped
milestones, work in progress, phase progress, and upcoming release notes, generated from this
repository's own records.

## ⚠️ Pre-release warning

AgentForge is usable for local experimentation and operator-led dogfooding, but it is **not a
complete product or a release-ready tool**. There is no stable API, compatibility, migration, or
production-support guarantee yet. Treat project state and agent output as reviewable work in
progress, use disposable test projects when possible, and keep an independent human in the loop.

The direct interactive path now has two explicit modes: cooked line-oriented `--interactive` and
native-terminal `--interactive --pty` for full-screen/raw-mode agents. PTY mode requires a real
terminal and is not available through pipes or the detached daemon. Interfaces, configuration
formats, and operator workflows may change before the first stable release. 🌱

### 📦 Distribution and crates.io warning

AgentForge is **not published to crates.io**. Do not run `cargo add agentforge` expecting this
project: that name is already occupied by an unrelated Rust crate. The supported public install
path is the official GitHub release archive. Since P5-M004 each archive carries a keyless build
provenance attestation, so verify a download with `gh attestation verify <archive> -R
cybercore-tech/agentforge` (or `sha256sum -c SHA256SUMS` for integrity only). Local development
can install `forge` and `forged` from this checkout with `cargo install --path`. See
[`docs/REGISTRY.md`](docs/REGISTRY.md) and [`docs/RELEASE.md`](docs/RELEASE.md) for the identity
policy and the gate required before any future package publication.

## 🔴 Important limitations and operator responsibility

Read this section before connecting AgentForge to a real project. The controls described below are
workflow controls, not a security product or a promise that an agent is safe by itself.

- **Alpha means change is expected.** There is no stable API, CLI compatibility guarantee, task or
  audit-schema migration guarantee, upgrade path, SLA, or production support commitment. Pin a
  checkout when reproducing a run and keep backups of project state.
- **Agents are untrusted workers.** A configured executable receives the task’s explicit working
  directory and process inputs, but AgentForge does not sandbox the executable, inspect its model
  reasoning, or certify the code it produces. A local agent may be able to read or modify anything
  its operating-system user can access.
- **Capabilities are policy checks, not operating-system isolation.** Owned paths, approvals, and
  gates constrain the AgentForge workflow; they do not replace filesystem permissions, containers,
  network controls, secret management, endpoint protection, or a security review.
- **Success is evidence, not acceptance.** A zero exit status, passing local gate, or green CI run
  does not automatically accept a task. An independent operator must inspect the exact diff, audit
  records, task state, and CI evidence before accepting or integrating it.
- **Integration remains explicit.** AgentForge does not silently merge branches, delete preserved
  task branches, retire dirty worktrees, run routine `git clean`, or run routine `git reset --hard`.
  Resolve ambiguous or dirty state deliberately and preserve evidence when something fails.
- **Secrets and production systems stay out of the first run.** Use a disposable project with no
  production credentials, tokens, customer data, or irreversible deployment hooks. Review profile
  environment values and executable paths before launching an agent.
- **The public site is informational.** GitHub Pages explains the product; the repository’s plans,
  task snapshots, approvals, audit log, worktrees, gates, and exact-head CI evidence remain the
  authoritative operational surfaces.

### Safe first-run checklist

Before `forge task launch`, `forge daemon launch`, or `forge run`:

1. Confirm the project root is the repository you intend to change and that `git status` is clean.
2. Read the project’s `.forge/blueprint.conf` and `.forge/guidelines.md`; prose guidance never
   grants authority by itself.
3. Inspect the task contract, allowed and forbidden paths, capabilities, dependencies, gates, and
   required approvals.
4. Verify the absolute executable or named profile, its literal arguments, timeout, output limit,
   and explicit environment values.
5. Confirm the exact base ref and the deterministic managed worktree before the agent starts.
6. Keep the terminal attached for interactive work, or verify the bounded daemon status and audit
   evidence when using detached execution.
7. After completion, review the diff and evidence, record an independent acceptance decision, and
   retire only a clean, unambiguous worktree. Preserve the task branch until integration and
   recovery decisions are complete.

If any check is unclear, stop and inspect. A fail-closed error is a signal to resolve the state, not
an invitation to bypass the policy.

## Why AgentForge? 🎯

AgentForge makes consequential agent actions explicit:

- **Human approval at consequential boundaries** 🙋
- **Plans before implementation** 📋
- **Least-privilege capabilities and owned paths** 🔐
- **One task, one isolated Git worktree** 🌳
- **Deterministic local gates and exact-head CI evidence** ✅
- **Append-only, tamper-evident audit history** 🧾
- **Repair forward instead of erasing failure history** 🔧
- **Useful local operation without a cloud dependency** 🏠

The result is an inspectable workflow where an agent can do useful work without silently gaining
authority to broaden its scope, mutate project policy, accept its own task, or merge its own result.

## What works today ✅

The current repository provides:

- Versioned project blueprint and guideline intake through `.forge/` files.
- Explicit task contracts with roles, dependencies, owned paths, capabilities, gates, outputs, and
  evidence requirements.
- Deterministic task state snapshots and validated lifecycle transitions.
- Managed Git worktree isolation with task-owned branches and conservative retirement.
- Provider-neutral local process-adapter execution with bounded output and deadlines.
- Project quality gates (`.forge/gates/`) that run after every successful agent and fail the task
  when they do not pass, with durable `GateFinished` evidence. 🚦
- Exact-SHA CI observation with conservative failure classification recorded in the audit log
  through `forge ci observe` and a reviewed provider profile. 🔎
- Concurrent launch of disjoint ready tasks through `forge task launch-batch`, with overlapping
  work deferred deterministically. ⚡
- Append-only audit records with chained integrity checks.
- Read-only one-shot and watch-mode operator HUD. 👀
- Explicit task inspection, approval, accept, cancel, and retry commands. 🧑‍💻
- A persisted `forge run` path that consumes only verified, task-linked approval evidence.
- Versioned local agent profiles with bounded direct arguments and explicit environment values. 🤖
- An optional loopback-only `forged` runtime with bounded `forge daemon` lifecycle commands that
  handles long agent runs, stays responsive to `status`, and refuses unsafe stops mid-run. ⚙️
- Transport-neutral remote-worker foundations: lease contracts, durable lease state, and
  deterministic dispatch planning, with no remote execution authority yet. 🛰️
- Explicit `forge worktree` commands for safe task worktree preparation and retirement. 🌳

The repository also contains reusable Rust crates for scheduling, policy, orchestration, intake,
state, worktrees, gates, audit, HUD, and operator actions. The `forged` binary now provides an
optional bounded local runtime; the most complete interface remains the `forge` CLI plus the crate
APIs. These capabilities are implemented and tested, but they should not be interpreted as a
stable release contract. 🧪

## Quick start 🚀

### Prerequisites

- Rust **1.85 or newer** with Cargo
- Git
- A local checkout of this repository

Build and verify the workspace:

```bash
git clone https://github.com/cybercore-tech/agentforge.git
cd agentforge
cargo build --workspace --locked
cargo test --workspace --locked
```

Run the CLI from the checkout:

```bash
cargo run -p agentforge-platform --bin forge -- version
cargo run -p agentforge-platform --bin forge -- doctor
cargo run -p agentforge-platform --bin forge -- status
```

For a local binary installation, build in release mode and place the binaries on your `PATH`:

```bash
cargo install --path crates/agentforge-cli --locked
cargo install --path crates/agentforge-daemon --locked
```

This installs `forge` and `forged` from the local checkout. The CLI package identity is
`agentforge-platform`; its source directory remains `crates/agentforge-cli` for now. 📦

These are local source installs, not crates.io packages. Verify the repository URL before running
any install command; similarly named crates and projects are not affiliated with this repository.

## A first project workflow 🧪

AgentForge keeps project guidance and task authority in durable files. Start by initializing a
project directory:

```bash
forge init /path/to/project
forge blueprint validate /path/to/project
```

Initialization creates, without overwriting existing files:

- `.forge/blueprint.conf` — bounded, versioned project metadata and structured defaults
- `.forge/guidelines.md` — human-readable project guidance; prose never grants authority

For a guided terminal workflow, use the bounded intake prompts instead of editing both files by
hand:

```bash
forge intake /path/to/project
```

The command previews the proposed blueprint and guidelines, then writes only after explicit
confirmation. Add `--task` to collect and persist an explicit task draft in the same flow. EOF,
declining confirmation, invalid input, or a concurrent source edit leaves durable state untouched;
use `-` to clear a repeated list and finish the guideline body with a line containing only `.`.

For a reviewed, repeatable session, provide the answers as a bounded UTF-8 file:

```bash
forge intake /path/to/project --input-file /path/to/session.txt
forge intake /path/to/project --task --input-file /path/to/task-session.txt
```

The file is used only as input and is never copied into `.forge`; preview and confirmation still
apply.

Create an explicit task contract:

```bash
forge task create /path/to/project P1-M003-T0001 implementer \
  "Add the intake command" \
  --allowed crates/agentforge-cli \
  --capability write_owned_paths \
  --gate full \
  --output "CLI command" \
  --evidence "tests and CI"
```

Inspect the resulting durable task state:

```bash
forge task inspect /path/to/project
forge task inspect /path/to/project P1-M003-T0001
```

When the task requires a human decision, record it explicitly with an actor identity:

```bash
forge task approve /path/to/project P1-M003-T0001 <approval-boundary> --actor alice
forge task accept  /path/to/project P1-M003-T0001 --actor alice
forge task cancel  /path/to/project P1-M003-T0001 --actor alice
forge task retry   /path/to/project P1-M003-T0001 --actor alice
```

Use only the stable approval names defined in [`docs/APPROVAL_BOUNDARIES.md`](docs/APPROVAL_BOUNDARIES.md).
Approval and lifecycle actions validate the task, actor, boundary, state, and audit chain before
recording durable evidence. Repeating an approval is idempotent. These mutation commands fail closed
if `.forge/audit.log` is missing or corrupt. 🔏

## Running a task ⚙️

The current CLI entry points are:

```bash
forge task launch <project-root> <task-id> <absolute-executable> [--base <ref>]
forge task launch <project-root> <task-id> --profile <profile-id> [--base <ref>]
forge daemon launch <project-root> <task-id> <absolute-executable> [--base <ref>]
forge daemon launch <project-root> <task-id> --profile <profile-id> [--base <ref>]
forge run <project-root> <task-id> <absolute-executable>
forge run <project-root> <task-id> --profile <profile-id>
forge run <project-root> <task-id> <absolute-executable> --interactive
forge run <project-root> <task-id> --profile <profile-id> --interactive
forge run <project-root> <task-id> <absolute-executable> --interactive --pty
forge run <project-root> <task-id> --profile <profile-id> --interactive --pty
```

For the simplest real-project foreground pilot, use `forge task launch`. It verifies the ready
task and approvals, resolves the exact base commit (default `HEAD`), creates or reuses the
deterministic managed worktree, and runs the existing bounded adapter path. Add `--interactive` or
`--interactive --pty` for the existing foreground terminal modes. A successful launch remains
`Running` until you explicitly review and accept it; inspect, integrate, and retire the worktree
as separate operator actions.

The run path performs policy, task-state, worktree, adapter, gate, audit, and handoff checks around
the configured local executable. The executable is passed directly as a process argument; shell
interpolation is not used. A successful process remains in the appropriate review/acceptance flow
until an independent operator decision is recorded.

Use `--interactive` for a foreground, cooked line-oriented session when the agent may ask questions
during the build. Output is shown live and bounded evidence is retained. This does not change the
approval or acceptance boundaries, and it does not apply to detached `forge daemon run` sessions.

Use `--interactive --pty` when the agent requires a full-screen or raw-mode terminal UI. AgentForge
allocates a native PTY, forwards keyboard input and resize events, restores terminal mode on exit,
and retains bounded output evidence. PTY mode fails closed unless stdin and stdout are real
terminals; it is not supported by pipes, CI capture, or `forge daemon run`.

Define and inspect a project-local profile in `.forge/agents/<profile-id>.conf`, then
validate it before use:

```bash
forge agent list /path/to/project
forge agent validate /path/to/project local-agent
forge agent inspect /path/to/project local-agent
```

Profiles use a versioned `key=value` format for an absolute executable, repeated literal
arguments, `env.NAME=value` entries, timeout, and output limits. The child receives a
cleared environment plus only explicit values. Profiles never grant task authority; normal
capability, approval, worktree, and audit checks still apply. See
[`docs/AGENT_PROFILES.md`](docs/AGENT_PROFILES.md).

Declare the project's quality gates the same way, one reviewed profile per gate in
`.forge/gates/<gate-id>.conf`. After an agent exits with status zero, `forge run`,
`forge task launch`, and the daemon equivalents run every gate in lexical order inside the task's
worktree and record `GateFinished` audit evidence. Any failing gate marks the task `failed` and the
command exits non-zero. A malformed gate profile stops the run before the agent starts.

```bash
forge gate list /path/to/project
```

See [`docs/GATES.md`](docs/GATES.md).

Record exact-SHA CI evidence, with every failed job classified before any repair, through a reviewed
provider command in `.forge/ci/provider.conf` (a GitHub CLI reference provider is included):

```bash
forge ci observe /path/to/project owner/repo "AgentForge CI" <40-hex-sha> [--task <task-id>]
```

See [`docs/CI.md`](docs/CI.md#operator-ci-observation).

Launch every ready task with disjoint owned paths at once. The agents run concurrently in their own
worktrees; overlapping tasks are deferred to a later batch:

```bash
forge task launch-batch /path/to/project --profile local-agent [--max 4]
```

See [`docs/SCHEDULING.md`](docs/SCHEDULING.md#concurrent-batch-launch).

Run a real coding agent: `scripts/agents/claude-code-bridge` lets Claude Code work as an AgentForge
agent. It translates the task contract into instructions, refuses to commit changes outside the
task's paths, and commits the agent's work so gates, review, and integration apply. See
[`docs/AGENT_PROFILES.md`](docs/AGENT_PROFILES.md#real-agents-the-claude-code-bridge) and the
[dogfooding log](docs/DOGFOODING.md).

For a supervised local daemon:

```bash
forge daemon start /path/to/project
forge daemon status /path/to/project
forge daemon restart /path/to/project
forge daemon stop /path/to/project
```

Use `forge daemon launch` when detached execution should also prepare the task-owned worktree:

```bash
forge daemon launch /path/to/project P2-M016-T0001 /absolute/path/to/agent --base HEAD
forge daemon launch /path/to/project P2-M016-T0002 --profile local-agent --base HEAD
```

This performs the same bounded readiness, approval, exact-base, and worktree checks as the
foreground `forge task launch` pilot. `forge daemon run` remains available for the lower-level
prepared-worktree contract. Neither command accepts work, integrates branches, or retires
worktrees implicitly.

Daemon executions can run as long as the agent's profile allows. The daemon runs one execution at
a time, keeps answering `forge daemon status` during it, and refuses a second execution or a
`forge daemon stop` until the run finishes. See
[`docs/DAEMON.md`](docs/DAEMON.md#long-running-executions).

Startup waits for a bounded, verified loopback endpoint. Stale or malformed metadata remains
fail-closed and requires explicit operator inspection; AgentForge never silently adopts or kills
an ambiguous daemon.

Worktree creation and retirement are implemented as provider-neutral crate APIs and are deliberately
conservative: dirty or ambiguous worktrees are not force-removed, reset, or cleaned. See
[`docs/WORKTREE_ISOLATION.md`](docs/WORKTREE_ISOLATION.md) for the ownership rules.

The CLI exposes the same safe lifecycle for operators:

```bash
forge worktree create /path/to/project P2-M006-T0001 HEAD
forge worktree inspect /path/to/project P2-M006-T0001
forge worktree list /path/to/project
forge worktree retire /path/to/project P2-M006-T0001
```

Review and protected integration are separate explicit actions:

```bash
forge task diff /path/to/project P2-M014-T0001
forge task integrate /path/to/project P2-M014-T0001 \
  --target main --actor alice
```

Diff is read-only. Integration requires a succeeded task with the
`merge_protected_branch` capability and a merge approval recorded after acceptance, which is bound
to the reviewed commit (an agent never needs it to launch), a clean verified source and target,
and a fast-forward-only history. It is serialized through `.forge/integration.lock`, records
integrity-linked audit evidence, is safe to repeat, and never retires the source worktree or
deletes its branch.

## Operator HUD 👀

Render a bounded, read-only project snapshot:

```bash
forge hud /path/to/project
```

Run the optional watch loop:

```bash
forge hud /path/to/project --watch --interval-ms 1000
```

Watch commands are line-oriented:

- `r` / `refresh` — collect a fresh frame
- `h` / `help` — print command help
- `q` / `quit` — exit

The `agent_runs:` section lists the five most recent agent runs with their exit code, gate results,
and evidence log paths, so a failed run and where to look are visible at a glance.

The HUD reads blueprint, guideline, task, audit, and managed-worktree state. It does not create
files, launch agents, transition tasks, approve work, or mutate Git. Operator mutations belong to
the explicit `forge task` commands above—not to the HUD. 🛡️

## Architecture at a glance 🧱

```text
project files + .forge/ state
             │
             ▼
       forge CLI / HUD ──────── read-only observation
             │
             ▼
  operator actions + orchestrator
             │
     ┌───────┼────────┬────────┐
     ▼       ▼        ▼        ▼
  policy  worktree  adapter  gates
     │       │        │        │
     └───────┴────────┴────────┘
             │
             ▼
     durable state + audit evidence
```

Important boundaries:

- Roles describe responsibility; roles do not grant authority.
- Capabilities, paths, approvals, and gates come from explicit task data.
- Audit records preserve facts; they do not silently grant approval.
- The HUD is a projection, not a second source of truth.
- CI status is meaningful only when it matches the exact commit under review.

### What to inspect when a run stops

AgentForge deliberately separates observation from repair. Start with the command that matches the
failure, then preserve the evidence while deciding what to do:

| Symptom | First inspection | Operator boundary |
| --- | --- | --- |
| Task is not ready | `forge task inspect <root> <task-id>` | Satisfy dependencies, capabilities, and approvals; do not force a transition. |
| Agent did not start | `forge doctor <root>` and the task/profile inspection commands | Fix the executable, profile, policy, or worktree preflight; no partial run is considered success. |
| Worktree is dirty or ambiguous | `forge worktree inspect <root> <task-id>` and `git status` in that worktree | Review and clean it intentionally, or preserve it for recovery; retirement is non-forced. |
| Process failed or timed out | `forge task inspect`, HUD, audit log, and the bounded output evidence | Classify the failure before retrying; a retry is a new explicit lifecycle decision. |
| CI is red | Verify the run’s exact commit SHA and classify the failure | Repair forward on a new commit; never treat a different green SHA as proof for the reviewed change. |
| Integration is requested | `forge task diff <root> <task-id>` | Require independent approval, clean source/target state, and the serialized fast-forward boundary. |

The operator should be able to explain why a task ran, what it changed, which checks passed, who
accepted it, and which exact commit was integrated. If that explanation cannot be reconstructed
from the repository and its audit evidence, the task is not ready to ship.

## Repository layout 📁

| Path | Purpose |
| --- | --- |
| `crates/agentforge-core` | Domain types, tasks, roles, capabilities, and lifecycle invariants |
| `crates/agentforge-intake` | Blueprint, guidelines, and task creation |
| `crates/agentforge-state` | Versioned durable task snapshots |
| `crates/agentforge-worktree` | Managed Git worktree lifecycle |
| `crates/agentforge-adapter` | Provider-neutral agent/process adapter boundary |
| `crates/agentforge-gate` | Bounded local gate execution |
| `crates/agentforge-ci` | Exact-SHA CI observation and failure classification |
| `crates/agentforge-audit` | Append-only chained audit store |
| `crates/agentforge-policy` | Capability, path, and approval decisions |
| `crates/agentforge-orchestrator` | Single-task execution composition |
| `crates/agentforge-scheduler` | Deterministic batches and serialized integration boundaries |
| `crates/agentforge-hud` | Bounded read-only operator projections |
| `crates/agentforge-operator` | Explicit task inspection, approvals, and lifecycle actions |
| `crates/agentforge-cli` | `agentforge-platform` package and `forge` command-line interface |
| `crates/agentforge-daemon` | `forged` loopback-only local daemon and protocol |
| `.plans/` | Plan and milestone control records |
| `docs/` | Contracts, architecture notes, ADRs, and operator guidance |
| `scripts/gate.sh` | Required local validation gate |

## Development workflow 🧰

AgentForge itself follows a plan-first workflow:

1. Draft a bounded plan.
2. Approve and commit the plan separately.
3. Implement only within the approved boundary.
4. Run the full gate.
5. Record exact implementation and CI evidence.
6. Close the milestone with a separate documentation checkpoint.

Run the repository gate before submitting a change:

```bash
./scripts/gate.sh full
```

The gate validates text policy, repository structure, plan policy, formatting, workspace checks,
Clippy, tests, and documentation tests. Do not bypass hooks or validation with `--no-verify`.

## Documentation map 📚

- [Blueprint and intake](docs/BLUEPRINT.md)
- [Task contract](docs/TASK_CONTRACT.md) and [task state](docs/TASK_STATE.md)
- [Agent roles](docs/AGENT_ROLES.md) and [agent adapters](docs/AGENT_ADAPTERS.md)
- [Policy and capabilities](docs/POLICY.md) and the [MCP task-tool gateway](docs/MCP_GATEWAY.md)
- [Worktree isolation](docs/WORKTREE_ISOLATION.md)
- [Orchestration loop](docs/ORCHESTRATION.md) and the
  [single-agent vertical slice](docs/VERTICAL_SLICE.md)
- [Local daemon](docs/DAEMON.md)
- [Operator HUD](docs/HUD.md) and [doctor and status diagnostics](docs/DOCTOR_STATUS.md)
- [Audit log](docs/AUDIT.md)
- [Gate engine](docs/GATES.md)
- [CI observation](docs/CI.md)
- [Scheduling and batch launch](docs/SCHEDULING.md)
- [Remote workers](docs/REMOTE_WORKERS.md)
- [Governance](docs/GOVERNANCE.md)
- [Milestones](docs/MILESTONES.md)
- [Release process](docs/RELEASE.md)
- [Project site and updates feed](docs/SITE.md)
- [Operations reference](docs/OPERATIONS.md): workflows, scripts, hooks, remotes, CI evidence,
  tags, agent runs, and recovery
- [Dogfooding log](docs/DOGFOODING.md)
- [Package identity and distribution](docs/REGISTRY.md)
- [Architecture decision records](docs/adr/README.md): the registry of every ADR (checked by
  `xtask validate`)
- [Architecture notes](docs/architecture/README.md)
- [Architecture decision records](docs/adr/README.md)

## Roadmap 🗺️

Completed foundations include durable task state, worktree isolation, adapters, gates, CI
classification, audit history, scheduling primitives, orchestration, project intake, release
readiness, the operator experience, real-project pilots, daemon task launch, and the public site.

The 2026-09-23 integration pass connected the remaining library-only subsystems to real operator
paths and restored reliable CI:

- **P2-M023** bounded daemon lifecycle tests and CI job timeouts (the Windows hang is fixed);
- **P1-M004** project gates run as task evidence;
- **P1-M005** `forge ci observe` records classified CI evidence;
- **P1-M006** `forge task launch-batch` runs disjoint tasks concurrently;
- **P2-M024** daemon executions of any length, with keepalives and a single execution slot;
- **P2-M025** the canonical repository moved to `cybercore-tech/agentforge`;
- **P2-M026** the project site gained a generated *What's new* section;
- **P2-M027** a real agent (Claude Code) builds AgentForge through AgentForge. Its first
  milestone was **P1-M007** (task-declared required gates);
- **P2-M028** the repository gate is hermetic under Git hooks and in worktrees;
- **P2-M029** every agent run shows its exit code and keeps its full output as evidence;
- **P2-M030** every completed milestone is tagged (`milestone/<ID>`) on its closure commit;
- **P2-M033** `forge hud` shows recent agent runs. Built by Claude Code and the first milestone
  landed through `forge task integrate`;
- **P1-M008** merge approvals come after review and are bound to the reviewed commit;
- **P0-M013** the audit log is safe with concurrent writers and is created on first use;
- **P4-M004** remote-worker leases are operable and audited from `forge` and `forged`, and leased
  tasks cannot run locally;
- **P4-M005** a same-host worker process (`forge worker run`) runs the tasks leased to it;
- **P4-M006** opt-in automatic dispatch: `forged` hands ready tasks to idle workers under a
  reviewed policy;
- **P4-M007** workers on other machines claim their leases over an authenticated GhostPort
  tunnel;
- **P4-M008** remote execution: remote results are imported only at their verified exact SHA,
  gated locally, and reviewed as usual;
- **P0-M014** the first real-agent remote run: Claude Code, as a remote worker over GhostPort, fixed
  the EINTR pipe readers, and it was imported at the exact SHA and integrated;
- **P0-M015** every audit event records when it happened, and the HUD shows event times and
  agent-run durations;
- **P5-M003** the release upload is rehearsed on every manual release run (upload to a draft
  release, verify every asset, delete it), so publish defects surface before a tag;
- **P5-M007** release
  [`v0.3.1`](https://github.com/cybercore-tech/agentforge/releases/tag/v0.3.1): the first with an
  attested SBOM, and the remote-worker fixes from the two-host rehearsal;
- **P2-M035** agent task hygiene: tasks that could never be integrated are refused at creation, and
  agents can run the scripts in their own allowed paths;
- **P5-M006** releases ship an attested CycloneDX SBOM, implemented by Claude Code through
  AgentForge using the MCP gateway;
- **P4-M012** a two-host rehearsal in containers with network chaos, which found and fixed four
  real defects (including one in GhostPort);
- **P4-M011** worker-host setup in two commands (`scripts/worker-bundle`, then
  `scripts/worker-host-setup`), rehearsed through real GhostPort;
- **P5-M005** release
  [`v0.3.0`](https://github.com/cybercore-tech/agentforge/releases/tag/v0.3.0), the first with
  attested archives: remote-worker supervision, the MCP gateway, and the daemon sweep fix;
- **P5-M004** keyless release provenance: every archive is attested by the release workflow and
  verifiable with `gh attestation verify`;
- **P3-M005** an MCP task-tool gateway: agents call AgentForge's task tools under their capability
  policy, with every call audited;
- **P4-M010** remote-worker visibility and supervision: `forge hud` shows workers and leases,
  workers ride out coordinator outages under a systemd user unit, and two daemon and worker defects
  found live are fixed;
- **P4-M009** a worker-host doctor: `forge worker remote run` refuses to start on a misconfigured
  host;
- **P5-M002** release
  [`v0.2.0`](https://github.com/cybercore-tech/agentforge/releases/tag/v0.2.0): remote workers and
  the audit-corruption fix;
- **P5-M001** the first tagged release,
  [`v0.1.0`](https://github.com/cybercore-tech/agentforge/releases/tag/v0.1.0), with checksummed
  archives for Linux, macOS, and Windows.

Candidate next milestones, each still requiring its own approved plan:

- a two-machine remote-worker run over a real network (P4);
- external MCP servers behind the gateway's capability map (P3);
- external MCP servers behind the gateway's capability map (P3), and the real two-machine run (P4).

## Contributing 🤝

Read [`AGENTS.md`](AGENTS.md), [`PROJECT_SPEC.md`](PROJECT_SPEC.md), and the current project state
before changing the repository. Keep changes narrow, preserve durable evidence, classify failures
before repair, and include tests and documentation for new behavior.

AgentForge is early-stage software. Expect evolving APIs, local-only workflows, and deliberate
fail-closed behavior while the control surfaces mature. 🌱
