# Issue tracker: GitHub

Issues, specs and tickets for this repo live as GitHub issues in `fernandolisboa/taful`.

## Tooling

- **Local Claude Code / terminal:** use the `gh` CLI for every operation below.
- **claude.ai cloud sessions:** `gh` there is limited to search. Use the GitHub MCP tools instead: `issue_write` (create/update, labels, close with `state_reason`), `issue_read` (body, comments, labels), `add_issue_comment`, `list_issues`, and `sub_issue_write` (link a ticket under its spec). Issue dependencies (blocked-by) go through the REST endpoint below when available; otherwise use the `Blocked by:` fallback line.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`
- **Sub-issue of a spec**: `gh issue create --parent <spec>` or `gh issue edit <spec> --add-sub-issue <n>`.
- **Blocked by**: `gh issue create --blocked-by <n>,<n>`, or `gh api --method POST repos/fernandolisboa/taful/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` (database id from `gh api repos/fernandolisboa/taful/issues/<n> --jq .id`). Fallback: a `Blocked by: #<n>, #<n>` line at the top of the body.

## Pull requests as a triage surface

**PRs as a request surface: no.** _(Set to `yes` if this repo treats external PRs as feature requests; `/triage` reads this flag.)_

GitHub shares one number space across issues and PRs, so a bare `#42` may be either: resolve with `gh pr view 42` and fall back to `gh issue view 42`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments` (or `issue_read` in a cloud session).
