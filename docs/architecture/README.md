# AgentForge Architecture

The intended long-term flow is:

```text
operator
   |
   v
forge CLI / TUI
   |
   v
orchestrator
   |
   +--> durable task graph
   +--> policy / capabilities
   +--> agent adapters
   +--> worktree manager
   +--> gate engine
   +--> CI monitor
   +--> audit log
   |
   v
forged daemon
```

Phase 0 implements these capabilities incrementally rather than building the entire architecture in
one milestone.
