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
| AgentForge Release | `.github/workflows/release.yml` | tags `v*.*.*`, manual dispatch | Verifies the tag matches the workspace version, builds `forge`/`forged` for four targets, attests each archive's build provenance keylessly (P5-M004), generates a CycloneDX SBOM of `forge` and `forged` with `cargo-cyclonedx` 0.5.9 and attests it for each archive (P5-M006), publishes checksummed archives with the SBOM, and verifies every uploaded asset and both of its attestations. A manual run is an upload rehearsal: it attests, uploads to a draft release (no tag), verifies it, and deletes it (P5-M003) |

CI jobs:

| Job | Checks |
| --- | --- |
| Repository policy | text policy, repository structure, plan-first policy, site feed sources (`scripts/site-updates`), agent bridge and milestone tagger self-tests |
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
| `scripts/tag-milestone [<ID> \| --all] [--dry-run]` | Creates annotated `milestone/<ID>` tags on milestone closure commits (see Tags) |
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
6. Push the closure, and once it is green, tag it (see Tags).

Every commit carries a detailed body: what changed, why, and the evidence. Closing a milestone also
publishes it on the site at the next Pages deployment.

## Tags

| Namespace | Marks | Created by | Triggers |
| --- | --- | --- | --- |
| `milestone/<ID>` | The closure commit of a completed milestone | `scripts/tag-milestone` | Nothing |
| `vX.Y.Z` | A release | An explicit release decision (`docs/RELEASE.md`) | The release workflow |

```bash
scripts/tag-milestone P2-M030                   # tag one milestone's closure commit
scripts/tag-milestone --all --dry-run           # show the whole mapping without tagging
git push origin refs/tags/milestone/P2-M030      # push one tag
git push origin 'refs/tags/milestone/*'          # push all milestone tags
git show milestone/P1-M007                       # title, acceptance signal, completion record
git log --oneline milestone/P2-M028..milestone/P2-M029   # everything a milestone added
```

Each annotated tag points at the earliest commit whose plan has a `Status: Complete` status line,
verified by reading the plan at that commit. Only plans that never had any status line use their
last commit. The tagger reads only committed state and refuses uncommitted changes to the milestone
table or the milestone's plans (P2-M034), so **commit the closure, check `git commit`'s own exit
code (never through a pipe), and only then tag.** Always check the `--dry-run` output: every
subject should read "close ...". It carries the milestone's title, acceptance
signal, and completion record. Tags are never moved or deleted. A conflicting existing tag is an
error, not something to overwrite.

### Documentation indexes

`xtask validate` (run by `./scripts/gate.sh`, the pre-commit hook, and CI) fails when an ADR file
has no row in `docs/adr/README.md`, a registry row has no file, or a `docs/*.md` file is not linked
from the README's documentation map (P2-M034). When you add an ADR or a doc, add its registry row
or README link in the same commit.

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
  task must have been created with `merge_protected_branch` authority. Record that approval only
  now, after accepting: `forge task approve . <task-id> merge_protected_branch --actor <you>`. It
  prints the commit it is bound to. Then run `forge task integrate . <task-id> --target main --actor
  <you>`. Launch never needs the merge approval (P1-M008). Retire the worktree only after
  integration succeeds (use `&&`, not `;`).

## Remote-worker leases

Workers are registered as `.forge/workers/<id>.conf` profiles, and leases are managed with `forge
lease grant|list|renew|release|expire` (P4-M004; details in `docs/REMOTE_WORKERS.md`). A leased
task, or one overlapping a lease, cannot run locally. `forged` expires due leases every 5 s.
`forge worker run <root> <worker-id> (--profile <p> | <exe>) [--once]` runs a worker on the same
host: it claims its leases, runs each task through the standard launch path, renews while the agent
runs, and releases the lease (P4-M005). An enabled `.forge/dispatch.conf` lets `forged` (or `forge
lease dispatch`) grant ready tasks in listed milestones to workers automatically, once per task
(P4-M006). Workers on other machines connect through GhostPort to `forged`'s loopback worker API
(`.forge/worker-api.conf`), authenticated with `forge worker enroll` secrets. `forge worker remote
claim|renew|release` is the client (P4-M007; recipe in `docs/REMOTE_WORKERS.md`). `forge worker remote run --repo <clone>`
runs claimed tasks on the worker host and returns a git bundle. `forge worker remote doctor` checks a
worker host (clone, identity, hooks, profile paths, secret, and an authenticated `PING`), and `run`
refuses to start while any check fails (P4-M009). The coordinator imports it only at
the verified exact SHA with in-bounds paths, and runs gates locally (P4-M008). A running worker
retries an unreachable coordinator with capped backoff and stops on a refusal; the
`contrib/systemd/agentforge-worker@.service` template supervises one worker per unit, and `forge
hud` shows workers (capacity, last seen) and active leases (P4-M010). `scripts/worker-bundle` (on the
coordinator) and `scripts/worker-host-setup` (on the worker host) set up a worker in two commands
(P4-M011). `scripts/rehearse-two-hosts` rehearses the two-machine run in two containers with
network chaos (P4-M012; see REMOTE_WORKERS.md).

`forge mcp serve --contract <file> --worktree <dir> [--root <project>]` is the MCP task-tool
gateway an agent's MCP client starts (P3-M005). The Claude Code bridge wires it in for tasks that
hold `use_mcp_tools`. Its tools (`task_contract`, `check_changes`, `run_gate`) are checked against
the task's capabilities and recorded as `ToolInvoked` audit events (`docs/MCP_GATEWAY.md`).

## Recovery procedures

| Symptom | Cause and fix |
| --- | --- |
| Git in the main checkout says "must be run in a work tree", or commits come out as `AgentForge Test` | A hook leaked fixture Git commands into the repository (fixed by P2-M028). Back up `.git/config`, then `git --git-dir=.git config core.bare false` and `git --git-dir=.git config --remove-section user`. |
| Main checkout's tests show behavior from another checkout's code | Shared target dir pollution. Rebuild only AgentForge: `cargo clean $(cargo metadata --no-deps --format-version 1 \| python3 -c 'import json,sys; print(" ".join("-p "+p["name"] for p in json.load(sys.stdin)["packages"]))')` |
| `forge task launch` exits 1 with `agent-exit=...` | Read the printed tail, or the full logs under `.forge/evidence/<task>/`. The task stays `running`; use `forge task cancel` and create a new attempt, or `forge task retry` after fixing the cause. |
| Bridge exit 4 (path violation) or 5 (commit/gate failure) | Nothing was committed. Inspect the worktree and the evidence logs. |
| `daemon is busy` | One daemon execution at a time; wait or check `forge daemon status` ([`DAEMON.md`](DAEMON.md)). |
| A release run built every target but **Publish GitHub release** failed | Do not move the tag. Publish from that run's artifacts with the procedure in [`RELEASE.md`](RELEASE.md#if-the-publish-job-fails), then fix the workflow. |
| A release run fails at **Attest build provenance**, or with "has no valid provenance attestation" | Attestation failed before upload (re-run the failed job) or an uploaded archive does not verify against `release.yml` (inspect it as in RELEASE.md's recovery). Never move the tag. |
| A release run fails at **Generate the release SBOM** or **Attest the SBOM**, or with "has no valid SBOM attestation" | Before upload, re-run the failed job. `sbom-merge` errors name the conflicting or dangling component. After upload, an archive's SBOM attestation does not verify: inspect it as in RELEASE.md's recovery. Never move the tag. |
| A release rehearsal was cancelled and left a `rehearsal-*` draft | Delete the draft by hand (`gh release delete rehearsal-<run>-<attempt> --yes`); it has no tag ([`RELEASE.md`](RELEASE.md#upload-rehearsal-manual-dispatch)). |
| `task integrate` says the merge was approved for another commit, or is not bound to a reviewed commit | The task branch moved after approval, or the approval predates P1-M008. Review `forge task diff` again, then `forge task approve ... merge_protected_branch` binds the current head. |
| A launch says a task "is leased to worker ..." or "overlaps task ... leased to worker ..." | Release the lease (`forge lease release`) or wait for it to expire, then launch. `forge lease list` shows who holds what. |
| A task is never dispatched automatically | Check `forge lease dispatch . --actor <you>`: it lists each skipped task with its reason (milestone not listed, approval missing, overlap, or "already had a lease"). Tasks that had a lease are dispatched only once; grant them manually. |
| `worker remote` says `unauthorized` | The secret file does not match `.forge/workers/<id>.secret` on the coordinator, or the worker is not registered. Re-enroll: delete the coordinator's `.secret`, run `forge worker enroll`, and copy the new secret (mode 600). |
| `worker remote` cannot reach the endpoint, or the connection resets | Check that the GhostPort client is running and connected (`ghostport status`), the link ID matches a link that peer is allowed, and `forged` shows `worker API listening`. Repeated bad handshakes from one address are rate-limited by GhostPort for a while. |
| `worker remote run refused: N worker-host check(s) failed` | Run `forge worker remote doctor` with the same options. Each `fail` line has a `fix:`: install hooks, set the clone's Git identity, rewrite profile paths for this host, fix the secret file, or bring up the GhostPort client. |
| `worker remote run` reports `abandoned: remote result rejected: ...` | The coordinator refused the import (SHA mismatch, ancestry, out-of-bounds path, bad bundle, or a stale claim). Nothing was written; the lease is released and the task stays `pending`. Fix the cause, then grant again. |
| The bridge exits 2 with "the task holds use_mcp_tools but no forge executable was found" | Add `argument=--forge` and `argument=/absolute/path/to/forge` to the bridge's agent profile (profiles run with a cleared environment). |
| An agent's MCP call fails with `-32602 tool ... is not available to this task` | The task lacks the capability that tool needs (`use_mcp_tools` plus `read_repository` or `run_local_commands`), or `run_gate` has no project root or required gates. The refusal is recorded as `ToolInvoked decision=denied`. See `docs/MCP_GATEWAY.md`. |
| An MCP call fails with `-32603 cannot record the call` | The gateway's `--root` has no task state, or the audit log cannot be opened. Nothing ran. Point `--root` at the initialized project, or omit it (calls are then logged to stderr only). |
| `worker-host-setup` fails with "missing prerequisites" | Install what it lists (`forge` from the project with `cargo install --path crates/agentforge-cli`, GhostPort, Claude Code) or pass `--forge`/`--claude`/`--agent-executable`. |
| `worker-host-setup` ends with `doctor fail endpoint` | Expected on the first run: add the printed `[[peers]]` entry to the coordinator's GhostPort server, start both GhostPort ends, and run the script again. |
| `worker-bundle` says a profile or `worker-api.conf` "already" differs | It never changes them. Pass the existing settings (`--api-port`, the profile options), or edit the file yourself. |
| `worker remote run` prints `result upload failed: ...; retrying in <n>s` | The finished result is being re-sent after a transport failure; nothing to do (P4-M012). If it gives up after 10 attempts, the claim is abandoned as before. |
| `worker remote run` says the first upload "may already have been imported" | An answer was lost and the retry was refused. Check `forge task inspect . <task>` on the coordinator: the task is usually imported and awaiting review. |
| Remote requests fail with `Connection reset by peer`, and the coordinator's GhostPort logs `too many recent handshake attempts` | GhostPort older than v0.1.2 throttles busy workers. Upgrade GhostPort on the coordinator (see REMOTE_WORKERS.md). |
| `worker remote run` prints `coordinator unreachable: ...; retrying in <n>s` | The coordinator is down or the tunnel dropped. Nothing to do on the worker; it resumes by itself (`coordinator reachable again`). Check `forge daemon status` and the GhostPort client on each end. |
| `worker remote run failed: unauthorized` and the unit restarts every 30 s | The worker's secret no longer matches (for example after a rotation). Copy the coordinator's new secret to the worker host (mode 600); the next restart passes the doctor. |
| `forge hud` shows a worker holding an active lease with an old `last-seen` | The worker stopped renewing: check its unit (`systemctl --user status agentforge-worker@<id>`) and journal. The lease expires by itself; release it early with `forge lease release`. |
| You need to know what a background `forged` did | Read `.forge/daemon/forged.log`: the worker API address, lease expiries, dispatch grants, refused worker requests, and sweep errors (P4-M010). |
| `worker remote run` says the base commit is not in the clone | Fetch the coordinator's commits into the worker's clone (shared origin), then grant again. |
| A worker was stopped mid-run | Its lease expires on its own (or run `forge lease expire`). The task stays `running` with its evidence: review it, or `forge task cancel` and create a new attempt. |
| `lease state is locked by another operation` | Another lease command or the daemon sweep is running; retry. If the lock outlived a crash and no `forge` or `forged` process runs for the project, remove `.forge/state/remote-leases.lock`. |
| `audit append lock ... is held by another writer` | Another append is in progress (milliseconds); retry. If it persists and no `forge` or `forged` process is running for the project, a crash left `.forge/audit.log.lock` behind. Remove it. |
| A CI job fails intermittently | Classify it, then reproduce with repeat dispatches before repairing; see `docs/DOGFOODING.md` and the P2-M023 plan for examples. |
