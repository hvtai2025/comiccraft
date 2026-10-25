---
description: "Merge upstream VectorCraft into the ComicCraft fork, prove no comic semantics changed, re-run the gate, and report the divergence that remains."
agent: comiccraft-lead
argument-hint: "Upstream ref to merge, e.g. upstream/main"
tools: [read, search, edit, execute, agent, todo]
---

> ## ⚠️ SUPERSEDED — do not run this prompt
>
> This prompt merges upstream VectorCraft into the ComicCraft fork. **There is no longer a
> fork to merge**: the reference tree is read-only and is never imported, built or merged
> (plan §2.4). Its replacement is
> [`reference-sync.prompt.md`](reference-sync.prompt.md), which keeps the reference pinned
> and re-proves behaviour against it instead of merging it.
>
> Kept because the Rust fork is history — it existed, it was recorded, and its milestone
> reports are cited as evidence.

---
Bring upstream changes into the fork without losing comic behaviour. Default ref:
`upstream/main`.

## 1. Before

- Record where we are: `git rev-list --count upstream/main..HEAD`, and the list of files
  that differ for comic reasons.
- Fetch upstream and summarise what changed in the crates we extend: `engine` (the command
  registry), `ui-egui` (panels, `UI_COMMANDS`), `doc`, `format`, `text`, `render`, `pdf`,
  `svg`, `xtask/src/layers.rs`.

## 2. Merge

Merge or rebase, and resolve conflicts in favour of upstream **except** where a comic
change is deliberate. The deliberate ones are:

- the additive comic command modules and their single registration line,
- the comic panel family in `ui-egui`,
- the three comic crates and their `layers.rs` entries.

Anything else that conflicts is a comic change that should not exist — investigate it.

## 3. Prove comic semantics did not change

- `cargo xtask ci` on the merged tree.
- The comic acceptance suite, and the comic commands' behaviour through
  `vectorcraft-cli run` and `mcp --headless`.
- Bundle round trip: an existing comic project still opens, validates and exports.
- If upstream changed `format::VERSION`, a document inside a bundle migrates correctly and
  the comic schema version is untouched.

## 4. Review hand-resolved conflicts

Any conflict you resolved by hand is an implementation, so it gets the two rounds of review
on `deepseek-v4.1-flash` with unit tests, recorded in `docs/reviews/sync-<owner>-<slug>.md`. A
mechanical resolution that took no judgement does not, but say which is which in the report.

Confirm with `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh`.

## 5. Report

```text
Merged:      <ref> — <n> commits, <n> files
Conflicts:   <each one and how it was resolved>
Comic diff:  <the files that differ for comic reasons, unchanged in count or smaller>
Gate:        cargo xtask ci — pass | fail (<first failing step>)
Verified:    <the comic behaviour that was re-proved after the merge>
Upstream:    <whether the registration seam or the artboard-id counter can be contributed now>
```

If the divergence grew for a reason that is not on the deliberate list, say so plainly —
that is the signal the fork is drifting.
