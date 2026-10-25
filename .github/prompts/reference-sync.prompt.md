---
description: "Advance or re-pin the read-only reference tree, re-prove that ComicCraft's behaviour still matches it, and record any divergence the port has accumulated."
agent: comiccraft-lead
argument-hint: "Optional: the reference commit or tag to move to, e.g. 3da4080b"
tools: [read, search, edit, execute, agent, todo]
---
The reference implementation is **read, never imported** (plan §2.4). It answers *"how should
this behave?"* and it supplies the bundle format. This procedure keeps it useful and keeps
the port honest about where it differs.

## 1. Before

- Record the pinned reference commit: `git -C reference/vectorcraft rev-parse HEAD`.
- List the differences the port has knowingly taken, from `docs/comiccraft-design.md` §16
  (the ADR table) and the latest milestone report.
- **Do not merge anything.** There is no fork and no upstream remote to merge from; the
  reference tree is a read-only checkout, and `reference/` must never appear in a
  `package.json`, an import, or a build step.

## 2. Move the pin (only when there is a reason)

Move it when a specific question needs a newer answer, not to stay current. State the
question first. Then:

- Check out the new commit into `reference/vectorcraft/`.
- Summarise what changed in the areas ComicCraft depends on: the document model, the bundle
  format and its version, text layout, rendering, PDF export, and the command registry.
- If the document format version changed, that is the item that matters: the port's
  round-trip test must still open a document written by the reference and vice versa.

## 3. Re-prove behaviour against the reference

- `npm run verify` on the current tree.
- The acceptance suite, and the comic commands through the HTTP endpoint and MCP.
- Bundle round trip: an existing comic project still opens, validates and exports.
- **Re-run the hand-proof from M0 step 4** against the reference, and compare: `artboard.new`,
  place and clip an image, `object.cropImage`, area text, serialize, export, PDF of the
  artboards. Cropping, clipping, typography and PDF page geometry are the four that have
  historically differed.
- Any difference is a **finding**, not a failure: record it, decide whether it is a port
  defect or a deliberate reduction, and if it is deliberate, add it to the §16 table so the
  next reader finds it where they will look.

## 4. Review

A change to ComicCraft made *because of* something the reference does is an implementation:
it gets the two rounds of review on `deepseek-v4.1-flash` with unit tests, recorded in
`docs/reviews/reference-<owner>-<slug>.md`.

Confirm with `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh`.

## 5. Report

```text
Reference:   <old commit> → <new commit>   (or: unchanged)
Reason:      <the question that required the move>
Format:      <document format version, and whether it changed>
Gate:        npm run verify — pass | fail (<first failing step>)
Re-proved:   <the behaviour that was re-proved against the reference>
Divergence:  <each difference found, and whether it is deliberate — with the §16 entry>
Findings:    <anything the reference does that the port does not, stated plainly>
```

If a deliberate difference has no entry in the §16 table, add it in this same change. An
unrecorded divergence is how a port stops matching its specification without anyone noticing.
