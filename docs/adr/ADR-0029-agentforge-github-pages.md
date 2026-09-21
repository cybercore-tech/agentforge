# ADR-0029: Static GitHub Pages product surface

- Status: Accepted
- Date: 2026-09-21
- Decision owners: AgentForge project

## Context

AgentForge has a mature local workflow and documentation corpus, but it lacks a concise public
entry point that explains the product’s safety model before asking a visitor to read the repository.
The landing page needs to communicate the operator workflow while preserving the project’s alpha and
pre-release status.

## Decision

Publish a dependency-free static site from site/ through a dedicated GitHub Actions Pages
workflow. The page uses semantic HTML and CSS plus the bounded local README reader described in
ADR-0030, with no analytics, authentication, remote API, third-party runtime, or external font
dependency. Deployment is triggered by main pushes or an explicit manual dispatch, and the deploy
job receives only Pages write and OIDC token permissions.

The visual language is the “forge rail”: a connected PLAN → ISOLATE → RUN → REVIEW → INTEGRATE
sequence using amber for intent, cyan for execution, and green for verified change. The site is a
presentation layer; the repository, task contracts, approvals, audit records, and exact-head CI
evidence remain authoritative.

## Consequences

The public project story becomes discoverable without changing the Rust product surface. A static
artifact with one small local interaction is easy to audit, cache, and host; it remains resilient
because the reader does not fetch content or depend on a remote service. The site must be updated
when the operator workflow or release status changes, and GitHub Pages configuration must be
enabled for the repository before the first deployment.
