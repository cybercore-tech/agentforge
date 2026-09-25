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

9. **Audit appends are not coordinated across handles.** Found while planning P4-M004.
   `FileAuditStore` tracks the next sequence number in memory. A daemon execution keeps its handle
   open for minutes, so any other appender during that time (for example `forge task approve` in
   another shell) makes the execution's next append reuse a sequence number. The log then fails
   closed. P4-M004 keeps the daemon's own lease sweep out of the way by sharing the execution slot.
   The cross-process case needs an append lock or re-read-before-append in `agentforge-audit`.
   **Resolved in P0-M013:** reproduction showed the stale append *corrupted* the log (the next open
   failed the integrity check). Appends now take a short lock, verify other writers' new records,
   and renumber stale events; attempt logs are appended as one batch (ADR-0047).
10. **A fresh project cannot record its first approval.** Found by P4-M004's CLI test. `forge init`
    and `forge task create` do not create `.forge/audit.log`, and `forge task approve` requires it
    ("audit log is missing"). A task with a pre-execution approval therefore cannot be approved
    before anything else has written the log. **Resolved in P0-M013:** operator actions create the
    log on first use once the task snapshot exists.

11. **Blocking reads give up on `EINTR` instead of retrying.** Found by the P5-M002 release gate: a
    worker API test failed intermittently with `Interrupted system call (os error 4)`. The two
    socket readers (`worker_api::read_line` and the daemon's `read_frame`) were fixed in P5-M002.
    Eight process-pipe read loops with the same pattern remain: adapter capture (3), stdin
    forwarding, gate capture, the CI provider reader, and two CLI input readers. **Resolved in
    P0-M014:** Claude Code fixed all eight as a remote worker (`dec91f2`).
12. **Audit events carry no wall-clock time.** Found while reviewing the P0-M014 remote run: ten of
    the twelve production `AuditEvent::new` calls pass the placeholder timestamp `1`. The chain
    proves order and integrity, but nothing in the evidence says *when* a claim, renewal, agent run,
    gate, or import happened. The run's timing (7 min 14 s end to end) had to be measured outside
    AgentForge. Needed for remote work (lease timelines, slow gates, incident review). **Resolved in
    P0-M015:** events are stamped at creation (`AuditEvent::now`), the store stamps any stragglers,
    a guard test keeps placeholders out of production code, and the HUD shows `at=` and
    `duration=`.
13. **The remote worker does not report lease renewals.** `forge worker run` (same host) prints
    `lease renewed N time(s)`, but `forge worker remote run` has no renewal report. The one renewal
    (#27) was visible only in the coordinator's audit. The worker host cannot tell whether its lease
    is healthy. **Resolved in P4-M009:** the remote runner reports renewals after each task.
14. **Worker-host setup is manual and unchecked.** The worker host needed its own clone and Git
    identity, `./scripts/install-hooks`, a `claude-code` profile rewritten to point at the clone's
    own bridge (profiles hold absolute paths), and the secret file with mode 600. `forge worker
    remote run` checks none of this. With hooks missing, the agent's pre-commit gate would silently
    not run. The coordinator gate still protects `main`, but the agent loses its own feedback. A
    worker-host preflight (a "doctor") would catch it. **Resolved in P4-M009:** `forge worker remote
    doctor` checks each requirement with a fix, and `worker remote run` refuses to start on any
    failing check.
15. **The milestone tagger can tag an unclosed plan.** During the P0-M014 closure, the closure
    commit was rejected by the text policy (an extra trailing newline), and the rejection was hidden
    because the operator piped `git commit` through `tail`. `scripts/tag-milestone` still passed,
    for two reasons:
    - it reads `docs/MILESTONES.md` from the working tree, where the uncommitted closure already
      said `complete`;
    - finding no committed `Status: Complete`, it fell back to the legacy "last commit touching the
      plan" rule, meant for old plans without status lines.

    It tagged and pushed the **approve** commit `cff4846`. The dry run printed that subject ("approve
    ..."), and the operator did not stop, against AGENTS.md rule 14. The tag was deleted locally and
    remotely within about two minutes (the same precedent as P2-M030), and the milestone was tagged
    again after the real closure. Fix: the tagger reads the milestone table from `HEAD`, refuses
    uncommitted plan or table changes, and uses the legacy fallback only for plans that never had a
    status line. **Resolved in P2-M034** (with CI-run self-test cases and all 69 tags unchanged).
    The same milestone also fixed the documentation indexes: 26 ADRs were missing from the registry,
    and 4 docs were not linked from the README. `xtask validate` now enforces both.
16. **A spawned `forged` dies a thread at a time.** Found by the P4-M010 live run. `forge daemon
    start` gave `forged` a stderr pipe back to itself (for startup errors) and then exited, closing
    the pipe. The next `eprintln!` in `forged` hit `EPIPE` and panicked the thread that logged:
    - the lease sweep recorded its first expiry, logged it, and died; later leases never expired
      (reproduced: audit #9 expiry, then #10 granted with a 1 s TTL and never expired; two
      threads instead of three), and automatic dispatch, which runs in the same sweep, stopped too;
    - a worker API connection logged a refusal before answering, so a worker with a rotated secret
      saw `connection closed` instead of `unauthorized`, and the new supervised runner retried it.

    Tests missed it because they run `forged` in-process or keep its stderr open. **Resolved in
    P4-M010 (Amendment 1):** logging ignores write failures, `forge daemon start` sends `forged`'s
    stderr to `.forge/daemon/forged.log`, and a regression test requires two consecutive expiries
    and an `unauthorized` answer from a daemon whose starter is gone (it failed first with `[1, 1]`
    and `connection closed`).
17. **A remote worker cannot retry a task.** Found by the same run. After each claim the worker
    retired its worktree but kept the task branch, and a dirty (refused, out-of-bounds) worktree was
    not retired at all. The next lease of that task on the same host was abandoned with `managed
    task branch already exists` or `managed worktree path already exists`, on every attempt, so an
    unattended worker could not make progress on retried work. **Resolved in P4-M010 (Amendment
    2):** each attempt is archived under its lease (`agentforge/remote/<lease-id>`, and a dirty
    worktree in `.forge/remote-abandoned/<lease-id>`), and leftovers of an interrupted attempt are
    archived as `<lease-id>.stale` before the next claim. Tested through the real worker loop,
    including a simulated kill mid-task.


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

## P0-M014 remote run log (2026-09-24)

This was the first real-agent run of the remote-worker path. Both ends ran on one host, with
separate clones, GhostPort key sets, and loopback ports; everything except a network hop was real.

| Step | Evidence |
| --- | --- |
| Operator grant | `LeaseRecorded granted` #25 (15 min window) |
| Claim over the GhostPort tunnel | #26 `claimed` with `channel=remote` and base `cff4846` |
| Agent run in the worker's own clone | Claude Code through the bridge; worker-side pre-commit gate passed (isolated target dir; plan policy ok; 66 test groups ok, 0 failed) |
| Renewal during the run | #27 `renewed` over the tunnel |
| Result | `dec91f2` (4 files, +356/−9), bundle sent with `RESULT` |
| Coordinator import | #28–#32: exact-SHA verified, worktree at `dec91f2`, coordinator `workspace` gate (`./scripts/gate.sh full`) 1/1 |
| Release | #33 `released` |
| Review and land | diff read and accepted; merge approval bound to `dec91f2`; `forge task integrate` fast-forwarded `main`; worktree retired |

Wall clock (measured externally; see finding 12): 7 min 14 s from claim to import. It passed on the
first attempt. The commit on `main` is authored by the remote worker, exactly as imported.

## P4-M010 supervised-worker live run (2026-09-25)

One host, a real coordinator (`forge daemon start` with the worker API on loopback) and a worker
started from the `agentforge-worker@.service` template under the systemd user manager, with a fixture
agent. Findings 16 and 17 came from the first pass; the second pass ran the fixed binaries.

| Check | Evidence |
| --- | --- |
| Start under systemd | doctor 6 × ok, 1 × warn (no hooks in the scratch clone), then `no claimable leases` |
| Coordinator outage | `coordinator unreachable ... retrying in 2s`, `4s`, `8s`; same PID, `NRestarts=0` |
| Recovery | `coordinator reachable again`, then claim, agent exit 0, `imported state=running`; the coordinator holds the exact result SHA |
| Same-task retry | leases L5 and L6 for one task abandoned for the task's real reason (a missing capability), not a branch conflict; old leftovers archived as `L5.stale` |
| Lease sweep | two consecutive 1 s leases expired, both logged in `forged.log`; `forged` kept 3 threads |
| Refusal | after a secret rotation: `worker remote run failed: unauthorized`, exit 1; systemd restarted it (`NRestarts=1`) once the new secret was installed, and the doctor passed |
| HUD | `live-1 platform=linux-x86_64 leases=0/1 last-seen=...(released)` and `leases: active=0 expired=4 released=5` |
