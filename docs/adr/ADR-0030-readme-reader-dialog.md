# ADR-0030: Use a bounded native dialog for the public README reader

- Status: Accepted
- Date: 2026-09-22
- Decision owners: AgentForge project

## Context

The public landing page should answer the first safety and usage questions without forcing every
visitor to leave the page. Fetching the repository README at runtime would add a network dependency,
content drift, injection risk, and failure mode to an otherwise static Pages artifact.

## Decision

The landing page’s “Read the README” control opens a native HTML `<dialog>` containing a curated,
versioned summary of the README’s purpose, alpha limitations, safe first-run checklist, and quick
start commands. A small local script calls `showModal()`, restores focus to the trigger after close,
supports Escape and backdrop dismissal through the native dialog behavior, and falls back to the
canonical GitHub README URL when native dialogs are unavailable.

The full README link remains visible inside the dialog. The page never fetches, embeds, evaluates,
or mutates README content at runtime; the curated reader is updated in the same reviewed change
as the README when its operator guidance changes.

## Consequences

Visitors get a focused in-page explanation with keyboard and narrow-screen support while the public
site remains dependency-free and locally auditable. The curated copy can become stale if it is not
updated alongside the README, so static checks and the plan file require the dialog and canonical
README link to remain present. The dialog is presentation only and grants no AgentForge authority.
