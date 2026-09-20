# Orchestration loop

P1-M002 will compose the existing contracts into an operator-invocable single-task flow. Its stages
are durable task selection, policy, managed worktree, bounded adapter execution, gates, audit, and
review handoff. Each stage stops on failure; task acceptance and protected integration remain explicit
human-controlled decisions.

The first operator entry point is `forge run <root> <task-id> <absolute-executable>`. It reads
`.forge/state/tasks.snapshot`, writes `.forge/audit.log`, requires a pre-created managed worktree,
and leaves successful tasks in `Running` until independently accepted.
