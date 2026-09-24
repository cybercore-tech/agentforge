# Operations reference

One place for how AgentForge is built, checked, published, and operated: GitHub workflows,
repository scripts and hooks, remotes, CI evidence practice, the local `.forge/` setup, agent
runs, and recovery procedures. Design rationale lives in the ADRs (`docs/adr/`); the governance
rules live in [`AGENTS.md`](../AGENTS.md) and [`GOVERNANCE.md`](GOVERNANCE.md).

## Repositories and remotes

| What | Where |
| --- | --- |
| Canonical repository | <https://github.com/cybercore-tech/agentforge> (`origin`, HTTPS) |
| Historical repository | `darkstardevx/agentforge`, history up to `976c4f9`, not updated (ADR-0041) |
| Project site | <https://cybercore-tech.github.io/agentforge/> |
| Default branch | `main`; integration is fast-forward only |

Pushes use HTTPS. On the operator machine, SSH keys belong to the personal account, so push with a
token for the owning account:

```bash
git push "https://x-access-token:$(gh auth token -u cybercore-tech)@github.com/cybercore-tech/agentforge.git" main
```

## GitHub workflows

| Workflow | File | Triggers | What it does |
| --- | --- | --- | --- |
| AgentForge CI | `.github/workflows/ci.yml` | push to `main`, pull requests, manual dispatch | Seven jobs (below); every job has `timeout-minutes` |
| AgentForge Pages | `.github/workflows/pages.yml` | push to `main`, manual dispatch | Generates `site/updates.json`, validates the static site, deploys `site/` to Pages |
| AgentForge Release | `.github/workflows/release.yml` | tags `v*.*.*`, manual dispatch | Verifies the tag matches the workspace version, builds `forge`/`forged` for four targets, and publishes checksummed archives |

CI jobs:

| Job | Checks |
| --- | --- |
| Repository policy | text policy, repository structure, plan-first policy, site feed sources (`scripts/site-updates`), agent bridge self-test |
| Stable code gate | `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings`, `cargo test` (all `--locked`) |
| MSRV 1.85.0 | `cargo check` and `cargo test` on the minimum supported Rust |
| CLI smoke | `forge version`, `forge doctor`, `forged --version` |
| Platform matrix (Ubuntu 24.04, macOS 14, Windows 2022) | workspace check and portable tests, `forge version` |

Manual dispatch (for example, repeat runs on the same commit):

```bash
gh workflow run 'AgentForge CI' -R cybercore-tech/agentforge --ref main
gh workflow run 'AgentForge Pages' -R cybercore-tech/agentforge --ref main
```

### CI evidence practice

- Green status counts only for the exact commit that produced it.
- A milestone closes after the push run **and** one or more dispatched repeat runs on the same
  commit are green. One green run has hidden intermittent failures before (P2-M023).
- Record run IDs in the plan's completion record, `PROJECT_STATE.md`, and `AGENT_HANDOFF.md`.
- Classify every failure (AGENTS.md taxonomy) before repairing it. `forge ci observe` records
  classified CI evidence in a project's audit log ([`CI.md`](CI.md#operator-ci-observation)).

## Scripts

| Script | Purpose |
| --- | --- |
| `scripts/gate.sh {precommit,fast,full}` | The repository gate. It runs text policy, repository and plan-policy validation, fmt, check, clippy, and tests. It is hermetic under hooks and in linked worktrees (below). |
| `scripts/check-text-files` | Tracked text files: UTF-8, LF, exactly one trailing newline |
| `scripts/install-hooks` | Sets `core.hooksPath=.githooks` |
| `scripts/project-status` | Active plan, branch, head, working-tree status |
| `scripts/site-updates [--output]` | Builds the site's *What's new* feed from milestones, plans, and the changelog ([`SITE.md`](SITE.md)) |
| `scripts/package-preflight` | Offline `cargo package` inspection of `agentforge-platform` ([`REGISTRY.md`](REGISTRY.md)) |
| `scripts/ci-provider-github` | Reference CI provider for `forge ci observe` (GitHub CLI) |
| `scripts/agents/claude-code-bridge` | Runs Claude Code as an AgentForge agent ([`AGENT_PROFILES.md`](AGENT_PROFILES.md#real-agents-the-claude-code-bridge)) |
| `cargo run -p xtask -- validate` / `validate-plan-policy` | Repository structure and plan-first checks (used by the gate and CI) |

### Hooks

`.githooks/pre-commit` runs `./scripts/gate.sh precommit` on every commit, including commits made
by the agent bridge inside task worktrees. Never bypass it with `--no-verify`. Since P2-M028 the
gate:

- clears every `GIT_*` variable before the cargo steps, so test fixtures cannot act on the
  enclosing repository through a hook's absolute `GIT_DIR`/`GIT_INDEX_FILE`;
- builds a linked worktree into `<target dir>/agentforge-worktrees/<worktree name>`, so checkouts
  sharing one Cargo target directory never run each other's binaries.

## Milestone workflow

Every change follows [`AGENTS.md`](../AGENTS.md):

1. `docs(plan): draft <ID>`: the plan file only, with `Status: Draft`.
2. `docs(plan): approve <ID>`: `Status: Approved` and `.plans/ACTIVE` pointing at it.
3. Implementation commits, inside the plan's declared file boundary. Boundary or scope changes get
   their own `docs(plan): amend <ID>` commit first.
4. Push, then collect CI evidence (above).
5. `docs(...): close <ID>`: plan `Status: Complete` with a completion record, `docs/MILESTONES.md`
   row `complete`, evidence in `PROJECT_STATE.md` and `AGENT_HANDOFF.md`, and `.plans/ACTIVE`
   removed.

Every commit carries a detailed body: what changed, why, and the evidence. Closing a milestone also
publishes it on the site at the next Pages deployment.

## Local `.forge/` setup (operator machine)

`.forge/` is gitignored. In this checkout it holds:

| Path | Contents |
| --- | --- |
| `.forge/agents/claude-code.conf` | Agent profile running `scripts/agents/claude-code-bridge` (with `env.PATH`, `env.HOME`) |
| `.forge/gates/workspace.conf` | Project gate: `/usr/bin/bash ./scripts/gate.sh full` (with `env.PATH`, `env.HOME`) |
| `.forge/state/`, `.forge/audit.log` | Task graph and audit chain for AgentForge-on-AgentForge runs |
| `.forge/evidence/<task>/` | Each agent run's full stdout/stderr, referenced by its `AgentFinished` audit event |
| `.forge/worktrees/<task>/` | Task worktrees. Cancelled attempts are kept as evidence |

## Running agents

```bash
forge task create . <task-id> <milestone> implementer "<goal>" \
  --allowed <path> ... --capability run_local_commands --capability write_owned_paths \
  --gate workspace [--capability merge_protected_branch --approval merge_protected_branch]
forge task launch . <task-id> --profile claude-code --base HEAD
```

- The launch prints the agent's exit code, the evidence log paths, the gate results, and, when the
  agent failed, the last 20 lines of its output. It exits 1 unless the agent exited 0 and every gate
  passed (P2-M029).
- In a linked worktree the bridge gives the agent the same isolated `CARGO_TARGET_DIR` as the gate,
  so builds the agent runs directly cannot affect other checkouts.
- Review with `forge task diff`, then `forge task accept`. To integrate through AgentForge, the
  task must have been created with `merge_protected_branch` authority and the approval recorded;
  then run `forge task integrate . <task-id> --target main --actor <you>`. Retire the worktree only
  after integration succeeds (use `&&`, not `;`).

## Recovery procedures

| Symptom | Cause and fix |
| --- | --- |
| Git in the main checkout says "must be run in a work tree", or commits come out as `AgentForge Test` | A hook leaked fixture Git commands into the repository (fixed by P2-M028). Back up `.git/config`, then `git --git-dir=.git config core.bare false` and `git --git-dir=.git config --remove-section user`. |
| Main checkout's tests show behavior from another checkout's code | Shared target dir pollution. Rebuild only AgentForge: `cargo clean $(cargo metadata --no-deps --format-version 1 \| python3 -c 'import json,sys; print(" ".join("-p "+p["name"] for p in json.load(sys.stdin)["packages"]))')` |
| `forge task launch` exits 1 with `agent-exit=...` | Read the printed tail, or the full logs under `.forge/evidence/<task>/`. The task stays `running`; use `forge task cancel` and create a new attempt, or `forge task retry` after fixing the cause. |
| Bridge exit 4 (path violation) or 5 (commit/gate failure) | Nothing was committed. Inspect the worktree and the evidence logs. |
| `daemon is busy` | One daemon execution at a time; wait or check `forge daemon status` ([`DAEMON.md`](DAEMON.md)). |
| A CI job fails intermittently | Classify it, then reproduce with repeat dispatches before repairing; see `docs/DOGFOODING.md` and the P2-M023 plan for examples. |
