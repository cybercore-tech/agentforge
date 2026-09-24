# Orchestration loop

P1-M002 will compose the existing contracts into an operator-invocable single-task flow. Its stages
are durable task selection, policy, managed worktree, bounded adapter execution, gates, audit, and
review handoff. Each stage stops on failure; task acceptance and protected integration remain explicit
human-controlled decisions.

The direct operator entry point is `forge run <root> <task-id> <absolute-executable>`. It reads
`.forge/state/tasks.snapshot`, writes `.forge/audit.log`, requires a pre-created managed worktree,
and leaves successful tasks in `Running` until independently accepted.

P2-M015 adds the real-project foreground pilot, which composes worktree preparation with that same
persisted execution path:

```bash
forge task launch /path/to/project P2-M015-T0001 /absolute/path/to/agent
forge task launch /path/to/project P2-M015-T0001 --profile local-agent --base HEAD
```

Launch validates readiness, capabilities, task-linked approvals, and adapter configuration before
creating anything. `--base` is resolved to an exact commit; it defaults to `HEAD`. An existing
task-owned clean worktree is verified and reused, while dirty, unresolved, ambiguous, or unrelated
worktrees fail closed. The launch records a `WorktreeObserved` audit event and then delegates to the
same bounded process adapter used by `forge run`.

The pilot leaves successful tasks in `Running` and leaves the managed worktree available for
inspection, acceptance, diff review, integration, and explicit retirement. If launch fails, use
the printed `task inspect` and `worktree inspect` recovery commands; no forced cleanup is attempted.
The existing `forge run` command remains available when worktree preparation should stay a separate
operator step.

For a disposable real-project pilot, point the same command at the explicitly installed local
executable you intend to review. For example, Codex- and Claude-style installations can be run as
separate tasks or reviewed profiles:

```bash
forge task launch /tmp/my-project P2-M015-T0001 /home/user/.local/bin/codex --interactive
forge task launch /tmp/my-project P2-M015-T0002 /home/user/.local/bin/claude --interactive
```

The executable path is passed directly; replace these illustrative paths with the absolute paths
reported by the local installation. Keep the project disposable until the task output, diff, audit
records, acceptance, integration, and retirement steps have all been reviewed.

P2-M012 adds an opt-in cooked foreground interaction mode, and P2-M013 adds an explicit PTY mode:

```text
forge run <root> <task-id> <absolute-executable> --interactive
forge run <root> <task-id> --profile <profile-id> --interactive
forge run <root> <task-id> <absolute-executable> --interactive --pty
forge run <root> <task-id> --profile <profile-id> --interactive --pty
```

Cooked interactive runs keep the current terminal attached through a line-oriented bridge. The
initial task prompt is delivered automatically, subsequent operator input is forwarded live, and
child output is displayed as it arrives while bounded copies remain execution evidence. Adding
`--pty` allocates a native pseudo-terminal, enables raw input, forwards terminal resize events,
and supports full-screen/raw-mode agents such as terminal UIs. PTY mode requires a real terminal
on stdin and stdout and fails closed when invoked through a pipe or detached service. Both modes
retain the same preflight, capability, approval, worktree, timeout, audit, and acceptance
boundaries. `forge daemon run` remains detached and captured.

P2-M005 adds the optional `forged` local daemon. Start it with `forged serve --root <root>` and
use `forge daemon status|run|stop` for the same bounded process path through a loopback-only,
versioned protocol. The daemon serializes one mutating execution at a time and records failures,
timeouts, and interruption evidence before returning an error. It never accepts tasks or performs
forced worktree cleanup.

P2-M016 adds daemon launch parity for detached execution:

```bash
forge daemon launch /path/to/project P2-M016-T0001 /absolute/path/to/agent --base HEAD
forge daemon launch /path/to/project P2-M016-T0002 --profile local-agent --base HEAD
```

`daemon launch` performs the same readiness, capability, approval, exact-base, and managed
worktree preparation as `task launch`, then submits the bounded captured process to the running
daemon. It reuses an owned clean worktree when possible and records the durable worktree
observation before execution. `daemon run` remains compatible and intentionally requires a
pre-created worktree; the new launch operation does not alter that existing request contract.
Both detached paths remain serialized and cooperative, and neither grants acceptance, review,
integration, or retirement authority.

P2-M006 adds explicit operator preparation for that path through `forge worktree create|inspect|list|retire`.
These commands delegate to the managed worktree authority, preserve task branches, and refuse dirty,
ambiguous, or unrelated worktrees. A configured executable is still passed directly to the daemon;
the operator remains responsible for approval, inspection, acceptance, and final retirement.

Project intake precedes execution. `forge init <root>` creates non-overwriting blueprint and
guideline templates; `forge blueprint validate <root>` validates them read-only; and
`forge task create ...` creates an explicit task contract in the durable snapshot. Guideline prose
never grants authority, and task creation does not execute work.

P2-M010 adds `forge intake <root> [--task]`, a bounded line-oriented guided authoring flow. It
collects structured blueprint fields and a terminated Markdown guideline body, optionally compiles
an explicit task draft, renders a deterministic preview, and waits for confirmation. It uses the
same intake validation and task snapshot boundaries as the direct commands. EOF, cancellation,
invalid input, duplicate task IDs, or a source edit detected after preview fail closed without
silently granting authority or executing work. The command is an operator mutation surface, not a
HUD mode; durable `.forge/` files remain the source of truth.

P2-M011 adds an optional `--input-file <path>` to that command for reviewed session replay. The
file is read directly as bounded UTF-8 data and is routed through the exact stdin prompt engine;
confirmation remains mandatory and the session file is never persisted as project state. This
supports disposable-project dogfooding and repeatable operator handoff without introducing a
second parser or authority source.

The first operator projection is `forge hud <root>`. It reads the validated intake, durable task
snapshot, verified audit log, and managed worktree state without creating files or changing Git.
Missing or corrupt sources fail closed; the HUD is a bounded plain-text snapshot rather than an
interactive editor or a second source of truth.

P2-M002 adds an optional line-oriented watch mode with bounded polling and `refresh`, `help`, and
`quit` commands. It remains a read-only projection: it cannot create tasks, launch agents, change
state, mutate Git, or approve work.

P2-M003 adds a separate controlled operator service for task inspection, explicit actor-bound
approvals, and audited lifecycle decisions. `forge run` consumes only verified task-linked approval
events; the HUD remains an observation surface.

P2-M009 ensures persisted daemon execution seeds each attempt audit from the verified sequence and
digest tail, so later runs append contiguous integrity-linked events without changing the audit
format or acceptance boundary.

P2-M014 adds the explicit review boundary. `forge task diff <root> <task-id>` is a bounded,
read-only projection of a managed task branch against the checked-out target. `forge task integrate`
requires a succeeded task, the `merge_protected_branch` capability, a merge approval bound to the
current task branch head (recorded after accept; P1-M008, ADR-0045), clean verified
source and target worktrees, and a matching target branch. It serializes one literal
`git merge --ff-only` through `.forge/integration.lock`, verifies the resulting commit, and appends
an integrity-linked integration event. Repeated integration is idempotent; branches remain
preserved and worktrees are never auto-retired.
