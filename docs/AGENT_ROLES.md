# AgentForge Agent Roles

AgentForge roles describe responsibility, not model or provider identity.

A model may execute different roles on different tasks. One task execution has one primary role.

## Canonical role registry

| Role | Primary responsibility | Default write posture |
| --- | --- | --- |
| Planner | Decompose approved work into executable tasks | Planning artifacts only |
| Architect | Define boundaries, interfaces, and architecture decisions | Architecture artifacts when approved |
| Researcher | Gather and summarize external evidence | Research artifacts only |
| Implementer | Produce task-scoped implementation | Owned paths only |
| Tester | Reproduce behavior and create regression evidence | Approved test paths |
| Reviewer | Independently assess implementation and evidence | Read-only by default |
| SecurityReviewer | Assess security and privilege risk | Read-only by default |
| Integrator | Combine approved task outputs and resolve integration conflicts | Integration boundary only |
| ReleaseManager | Prepare release artifacts and authorized release operations | Release boundary only |

## Universal role rules

Every role:

- operates under an explicit task;
- receives capabilities separately from role identity;
- obeys allowed and forbidden path boundaries;
- reports evidence and unresolved risk;
- escalates when required authority is missing;
- may not silently expand its own scope.

A task result never grants the agent new authority.
