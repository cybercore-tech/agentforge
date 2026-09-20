# Orchestration loop

P1-M002 will compose the existing contracts into an operator-invocable single-task flow. Its stages
are durable task selection, policy, managed worktree, bounded adapter execution, gates, audit, and
review handoff. Each stage stops on failure; task acceptance and protected integration remain explicit
human-controlled decisions.

The direct operator entry point is `forge run <root> <task-id> <absolute-executable>`. It reads
`.forge/state/tasks.snapshot`, writes `.forge/audit.log`, requires a pre-created managed worktree,
and leaves successful tasks in `Running` until independently accepted.

P2-M005 adds the optional `forged` local daemon. Start it with `forged serve --root <root>` and
use `forge daemon status|run|stop` for the same bounded process path through a loopback-only,
versioned protocol. The daemon serializes one mutating execution at a time and records failures,
timeouts, and interruption evidence before returning an error. It never accepts tasks or performs
forced worktree cleanup.

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
