# M3 (Node.js stack) — Story and storyboard workflow

**Status:** Complete.
**Gate:** `npm run verify` green, including `test/story.test.js`.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §9.

## Exit criteria

| Criterion | Evidence |
|---|---|
| A story input produces schema-validated structured data with stable ids, never prose to parse | `analyzeStory` returns scenes → pages → panels → lettering; `validateProposal`; `test/story.test.js` |
| Every proposal is reviewable and reversible before it is authoritative | `comic.story.import` stores a proposal and creates **no** pages; `comic.storyboard.accept` / `reject`; tests assert the page count stays zero before acceptance |
| A change to one scene leaves unrelated pages untouched | scene-scoped acceptance; the test snapshots the rooftop page, accepts only the alley scene and deep-compares |
| Regenerating descriptions preserves approved content | `carryIds` maps stable keys to existing ids; `applyProposal` with `preserveExisting` refreshes descriptions only; test edits a lettering line and asserts it survives |
| Id stability across regenerate-and-accept | test captures every page and panel id, regenerates, accepts, and compares both sets |

## The design step

`comic.story.design` is the idea-to-storyboard command behind the studio's **Design with AI**
button. It takes a premise, manuscript, style, audience, characters and panels-per-page and
returns a validated proposal with a logline and design provenance.

The default `local-director` is deterministic and offline: it reads sentences as beats, groups
them into pages, chooses a camera progression, recognises quoted dialogue and sound effects, and
names scenes from visible locations. `server/ai/story.js` optionally calls a configured HTTP model
instead; its output is normalised and passed through `validateProposal` like any untrusted input.
The proposal shape is the same either way, so review, acceptance, ids and undo do not change.
Acceptance is tracked per scene: the proposal stays `proposed` until every scene has been
accepted, so remaining scenes stay reviewable and the workflow stepper does not jump ahead.

## Commands

`comic.story.design`, `comic.story.providers`, `comic.story.import`, `comic.story.get`, `comic.story.validate`, `comic.storyboard.generate`,
`comic.storyboard.list`, `comic.storyboard.accept`, `comic.storyboard.reject`.

## Notes

- Recognition is deterministic and stated: `NAME:` is dialogue, `CAPTION:`/`NARRATION:` is narration, `SFX:` is a sound effect, a plain line is the panel description.
- A proposal lives in the bundle beside the book (`story/proposals.json`), so an interrupted review session can resume; it is not authoritative project data.
- Publishing a proposal is one undoable step across chapters, scenes, pages, panels, lettering, dialogue, characters and reviews.
