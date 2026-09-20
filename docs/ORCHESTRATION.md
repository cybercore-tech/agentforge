# Orchestration loop

P1-M002 will compose the existing contracts into an operator-invocable single-task flow. Its stages
are durable task selection, policy, managed worktree, bounded adapter execution, gates, audit, and
review handoff. Each stage stops on failure; task acceptance and protected integration remain explicit
human-controlled decisions.

The first operator entry point is `forge run <root> <task-id> <absolute-executable>`. It reads
`.forge/state/tasks.snapshot`, writes `.forge/audit.log`, requires a pre-created managed worktree,
and leaves successful tasks in `Running` until independently accepted.

Project intake precedes execution. `forge init <root>` creates non-overwriting blueprint and
guideline templates; `forge blueprint validate <root>` validates them read-only; and
`forge task create ...` creates an explicit task contract in the durable snapshot. Guideline prose
never grants authority, and task creation does not execute work.

The first operator projection is `forge hud <root>`. It reads the validated intake, durable task
snapshot, verified audit log, and managed worktree state without creating files or changing Git.
Missing or corrupt sources fail closed; the HUD is a bounded plain-text snapshot rather than an
interactive editor or a second source of truth.

P2-M002 adds an optional line-oriented watch mode with bounded polling and `refresh`, `help`, and
`quit` commands. It remains a read-only projection: it cannot create tasks, launch agents, change
state, mutate Git, or approve work.
