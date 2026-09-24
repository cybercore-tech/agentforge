# Dogfooding: AgentForge building AgentForge

P2-M027 runs a real coding agent (Claude Code, through `scripts/agents/claude-code-bridge`) on this
repository through the normal AgentForge flow: task contract, managed worktree, agent, project
gates, review, accept, integrate. This file is the run log and the list of friction points found.

## Bridge verification (2026-09-24)

| Check | Result |
| --- | --- |
| `--self-test` | Decoder round-trips multiline/Unicode fields; rejects truncated, bad-header, bad-length, renamed-field, and trailing-byte prompts; path rules correct |
| `--dry-run` on a `forge`-rendered prompt (captured with a `tee` profile) | Complete instructions for every contract field |
| Real Claude Code on a scratch repository (`forge task launch --profile claude-code`) | Edited only the allowed file, wrote a detailed conventional commit message, bridge committed it with task trailers; 14 s end to end; worktree clean |
| Path guard with a fake agent that also writes outside the boundary | Exit 4, nothing committed, task branch still at its base, worktree left dirty for review |

## Findings

1. **A non-zero agent exit is nearly invisible.** `forge task launch` prints only
   `termination=Exited` and exits 0 when the bridge exits 4 (path violation). The evidence is
   recorded, but the operator is not told. The CLI should print the agent's exit code and exit
   non-zero. **Resolved in P2-M029:** the CLI prints `agent-exit=<n>` and a failure tail, and exits
   1.
2. **The repository gate was not hermetic inside worktrees.** This was the first real run: task
   `P1-M007-T0001`, Claude Code, 230 s, 2026-09-24. The agent implemented the milestone. When the
   bridge committed, the pre-commit hook ran the gate, and test fixtures inherited the hook's
   absolute `GIT_DIR`/`GIT_INDEX_FILE`. They then acted on the real repository:
   - `core.bare = true` in the shared `.git/config`, which broke the main checkout;
   - an injected `[user] AgentForge Test`;
   - an `initial` fixture commit, with the README replaced by `fixture`, on the task branch,
     swallowing the agent's staged work.

   The operator repaired the config (backup kept), and nothing was pushed. It never appeared
   before because every earlier commit came from the main checkout, where hooks get no `GIT_DIR`.
   Fixed in P2-M028: the gate clears `GIT_*` before the cargo steps. The fix was proven by
   reproducing the incident in a disposable clone before it (same three effects) and after it
   (config untouched, exactly one intended commit).
3. **Checkouts sharing one Cargo target directory run each other's binaries.** Found while
   committing the P2-M028 plan. The agent's worktree build left its `forge` and test binaries in
   the shared `~/.cargo-target`, which Cargo treated as fresh for the main checkout, so main's gate
   ran P1-M007 code and failed. Recovery: `cargo clean -p <each AgentForge package>`. Fixed in
   P2-M028: a linked worktree's gate uses `<target>/agentforge-worktrees/<name>`.
4. **Agent and bridge output is not kept anywhere.** `forge task launch` neither prints nor
   persists the adapter's stdout/stderr, so diagnosing the first run needed forensics on the branch
   and config. **Resolved in P2-M029:** output is saved under `.forge/evidence/<task>/` and
   referenced by `AgentFinished`.
5. **The blueprint's default gate made the plan's compatibility claim false.** Found by the agent
   in attempt `P1-M007-T0002`: `forge init` defaults every task to gate `full`, so strict
   required-gate preflight would have broken nearly every CLI-created task. The agent stayed in
   bounds, didn't weaken the rule, and asked for a decision. Resolved by P1-M007 Amendment 1.
6. **The agent can bypass gate isolation by running `cargo` directly.** In `T0002` Claude Code
   ran `cargo test` in its worktree, which wrote to the shared target directory and broke the main
   checkout's next gate again. `T0003` used an operator-side profile override
   (`env.CARGO_TARGET_DIR`). **Resolved in P2-M029:** the bridge sets the isolated target dir for
   the agent.
7. **Integration authority must be declared when the task is created.** `forge task integrate`
   correctly refused `T0003` because its contract lacked the `merge_protected_branch` capability
   and approval. The operator also retired the worktree by chaining commands with `;`, so the
   reviewed commit was landed with an operator fast-forward. Tasks meant to integrate through
   AgentForge should be created with `--capability merge_protected_branch --approval
   merge_protected_branch`, and the approval recorded after review. **Resolved in P2-M033:** the
   task was created with merge authority and `forge task integrate --target main` landed the
   agent's commit; the worktree was retired only afterwards.
8. **Launch requires every approval up front, including the merge approval.** P2-M033 planned to
   record `merge_protected_branch` after review, but `forge task launch` refuses a task whose
   required approvals are not all recorded, so it had to be approved before the agent ran. Review
   still gates integration in practice (the operator runs `accept` and `integrate`), but the
   approval record no longer proves a review happened. **Resolved in P1-M008:** merge, release,
   and deployment approvals are post-execution. Launch never requires them, they can be recorded
   only after `accept`, and each is bound to the reviewed commit, which `integrate` enforces
   (ADR-0045).

## P1-M007 run log (2026-09-24)

| Attempt | Duration | Outcome |
| --- | --- | --- |
| `P1-M007-T0001` | 230 s | Work done; the hook leak damaged `.git/config` and the task branch → P2-M028 |
| `P1-M007-T0002` | 34 s via `forge` (output lost), then 203 s rerun by hand | Work done; agent stopped on 8 CLI failures caused by the default `full` gate → Amendment 1 |
| `P1-M007-T0003` | 321 s | Committed by the bridge, pre-commit gate and project gate passed, reviewed, accepted, landed as `6131508` |

Attempts `T0001` and `T0002` are cancelled. Their branches and worktrees are kept as evidence.

## P2-M033 run log (2026-09-24)

| Attempt | Duration | Outcome |
| --- | --- | --- |
| `P2-M033-T0001` | 252 s | `agent-exit=0`, gates 1/1, reviewed with `forge task diff`, accepted, integrated by `forge task integrate --target main` as `29484bc`, then retired |

The first attempt of an agent-built milestone to succeed without operator repair.
