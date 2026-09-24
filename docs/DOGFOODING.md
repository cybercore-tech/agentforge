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
   non-zero (follow-up).
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
   and config (follow-up, together with finding 1).
