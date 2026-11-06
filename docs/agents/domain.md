# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase. Layout: **single-context**.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root: the domain glossary. Created lazily by `/domain-modeling` when a term gets resolved; if it doesn't exist yet, proceed silently.
- **`docs/decisions.md`**: this repo's ADR log. Each `## Dn — ...` entry is one ADR (D1, D2, ...). Read the entries that touch the area you're about to work in. Superseded entries are marked, not deleted. There is no `docs/adr/` directory; don't create one.
- **`docs/spec.md`**: the product/technical spec (milestones M0–M6 are built). **`docs/brief.md`** is the original product brief; `docs/decisions.md` wins on conflict.
- **`docs/acceptance.md`**: the acceptance checklist for the first slice.

## Recording a new decision

Append a new entry to `docs/decisions.md` with the next number (D14, D15, ...), following the existing shape (Date, Decision, Why, Implications). Never renumber existing entries.

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing decision, surface it explicitly rather than silently overriding:

> _Contradicts D7 (layout customization sequencing), but worth reopening because…_
