# AgentForge 🛠️🤖

AgentForge is a local-first, model-agnostic development environment for running AI-assisted
software work as explicit, reviewable engineering workflows.

It treats models as replaceable workers—not as the source of truth. The durable source of truth is
the repository: plans, task contracts, permissions, isolated worktrees, quality-gate evidence,
CI observations, audit records, and human decisions. 🧭

> **Status:** `0.0.1` · P2-M006 real-agent dogfooding and operator workflow complete · experimental and
> under active development

[![CI](https://github.com/darkstardevx/agentforge/actions/workflows/ci.yml/badge.svg)](https://github.com/darkstardevx/agentforge/actions/workflows/ci.yml)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org/)

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
- Deterministic local gate execution and structured reports.
- Exact-SHA CI observation and conservative failure classification.
- Append-only audit records with chained integrity checks.
- Read-only one-shot and watch-mode operator HUD. 👀
- Explicit task inspection, approval, accept, cancel, and retry commands. 🧑‍💻
- A persisted `forge run` path that consumes only verified, task-linked approval evidence.
- An optional loopback-only `forged` runtime with bounded `forge daemon` lifecycle commands. ⚙️
- Explicit `forge worktree` commands for safe task worktree preparation and retirement. 🌳

The repository also contains reusable Rust crates for scheduling, policy, orchestration, intake,
state, worktrees, gates, audit, HUD, and operator actions. The `forged` binary now provides an
optional bounded local runtime; the most complete interface remains the `forge` CLI plus the crate
APIs.

## Quick start 🚀

### Prerequisites

- Rust **1.85 or newer** with Cargo
- Git
- A local checkout of this repository

Build and verify the workspace:

```bash
git clone https://github.com/darkstardevx/agentforge.git
cd agentforge
cargo build --workspace --locked
cargo test --workspace --locked
```

Run the CLI from the checkout:

```bash
cargo run -p agentforge-cli -- version
cargo run -p agentforge-cli -- doctor
cargo run -p agentforge-cli -- status
```

For a local binary installation, build in release mode and place the binaries on your `PATH`:

```bash
cargo install --path crates/agentforge-cli --locked
cargo install --path crates/agentforge-daemon --locked
```

This installs `forge` and `forged` from the local checkout. 📦

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

The current CLI entry point is:

```bash
forge run <project-root> <task-id> <absolute-executable>
```

The run path performs policy, task-state, worktree, adapter, gate, audit, and handoff checks around
the configured local executable. The executable is passed directly as a process argument; shell
interpolation is not used. A successful process remains in the appropriate review/acceptance flow
until an independent operator decision is recorded.

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
| `crates/agentforge-cli` | `forge` command-line interface |
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
- [Policy and capabilities](docs/POLICY.md)
- [Worktree isolation](docs/WORKTREE_ISOLATION.md)
- [Orchestration loop](docs/ORCHESTRATION.md)
- [Local daemon](docs/DAEMON.md)
- [Operator HUD](docs/HUD.md)
- [Audit log](docs/AUDIT.md)
- [Gate engine](docs/GATES.md)
- [CI observation](docs/CI.md)
- [Scheduling](docs/SCHEDULING.md)
- [Governance](docs/GOVERNANCE.md)
- [Milestones](docs/MILESTONES.md)
- [Release process](docs/RELEASE.md)
- [Architecture notes](docs/architecture/README.md)
- [Architecture decision records](docs/adr/README.md)

## Roadmap 🗺️

Completed foundations include durable task state, worktree isolation, adapters, gates, CI
classification, audit history, scheduling primitives, orchestration, project intake, release
readiness, and the operator experience milestones through P2-M006. The optional loopback daemon
and its real temporary-project dogfooding flow are complete.

The next increment is intentionally not pre-approved. Future work should be driven by real operator
usage and may expand daemon-driven orchestration, richer integration surfaces, or additional provider
adapters without weakening the existing approval and evidence boundaries.

## Contributing 🤝

Read [`AGENTS.md`](AGENTS.md), [`PROJECT_SPEC.md`](PROJECT_SPEC.md), and the current project state
before changing the repository. Keep changes narrow, preserve durable evidence, classify failures
before repair, and include tests and documentation for new behavior.

AgentForge is early-stage software. Expect evolving APIs, local-only workflows, and deliberate
fail-closed behavior while the control surfaces mature. 🌱
