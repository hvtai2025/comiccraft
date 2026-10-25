---
name: comiccraft-status
description: 'Report where the ComicCraft team stands: stage (planning or fork), the milestone trail, which documents and artifacts exist, whether the comic crates are registered in the layer table, how many comic commands are registered, and how far the fork has diverged from upstream. Use when asked for team status, what is done, what is next, whether the fork is drifting, or how to resume.'
argument-hint: 'optional repo root'
---

# ComicCraft status

Answer "where are we?" from evidence on disk, not from memory.

## When to use

- Before starting work, to see what already exists.
- When asked what is done, what is next, or whether the fork is drifting.
- After an interruption, to find the resume point.
- Before a milestone, to confirm the predecessor actually closed.

## Procedure

### 1. Run the status script

```sh
bash ./scripts/status.sh
```

Read-only and fast — it runs no build. If it is run before M0, `stage: planning` is the
expected and correct answer.

### 2. Read each section

| Section | What it tells you |
|---|---|
| `stage` | `planning` = M0 has not created the fork; `fork` = the workspace exists |
| `Milestones` | the report trail; the newest entry is the current position. `<no status line>` means the report exists but does not follow the lead's format — fix the report |
| `Artifacts` | which deliverables exist. A `--` on a document the current milestone owes is the gap to close |
| `Module trees` | a `shared/` module that imports a runtime is a hard failure: the boundary test rejects it |
| `Comic commands` | the size of the agent surface; zero before M1 is expected |
| `Upstream` | divergence. A rising count, or files diverging that are not comic files, is drift — investigate before merging upstream |

### 3. For a fuller picture

- Gate evidence: `docs/ci-baseline-<short-sha>.md` (M0) and `docs/ci-<sha>.md` (each run).
  Markdown on purpose — kept that way so the existing evidence files stay readable and the
  status scripts can find the latest run by extension,
  and a build log contains vendor names in upstream test names.
- The last milestone report's `Not done` and `Next` lines are the resume point.
- To re-check readiness rather than position, use the `comiccraft-start` skill — its
  preflight is the stricter check.

### 4. Report

```text
Stage:      planning | fork
Position:   M<n> — <status>
Done:       <artifacts and milestones that exist>
Missing:    <what the current milestone still owes>
Divergence: <commits ahead of upstream, and whether the diverging files are all comic files>
Next:       <the skill or prompt to run, and with which argument>
```

## Rules

- Do not infer status from a chat summary; the scripts read the tree.
- An artifact existing is not the same as a milestone being accepted — check the report's
  status line and the gate result.
- If `Milestones` is empty but work has happened, the report was never written. That is a
  finding, not a detail: write it.

## Resources

- [Status script](./scripts/status.sh) — stage, milestones, artifacts, crates, divergence
