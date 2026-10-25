# Review record — M2–M8 implementation (Node.js stack)

**Scope:** the uncommitted M2–M8 implementation: domain (`story`, `jobs`, `publish`, `fonts`,
history/validation changes), commands (story, generation, asset, character, agent, panel/page
tools, lettering tools, publishing), server (MCP, providers, job manager, package/contact/PDF/
PNG/SVG exporters, session), the studio UI, schemas and tests.
**Rounds:** two automated review rounds with a code-review agent, followed by targeted
regression tests. Findings below are the ones that were actionable; style-only notes are not
recorded.

## Round 1

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | High | `createRenderer().bookToPdf` dropped the caller's `{ preset }`, so print/proofing exports used screen geometry | forward and merge the call options; `test/publish.test.js` now checks `TrimBox`/`BleedBox` through the command path |
| 2 | High | `carryIds` matched scenes by display name, so duplicate names cross-mapped and one scene wrote into another | seed identity from the previous proposal's stable `idMap`, name matching only when unambiguous; duplicate-name regeneration test |
| 3 | High | page-scoped undo rewound all id counters, so a later job/asset could reissue ids and overwrite records | counters merge upward and are never lowered on undo; monotonic id test in `test/jobs.test.js` |
| 4 | High | the browser kept the previous project's undo history after New/Open | `adoptBundle()` clears both stacks; asserted by `test/ui.test.js` |
| 5 | High | every new project's default local save key was `comic-project_1`, so a second Save overwrote the first | unique `saveKey()` from title + timestamp; asserted by `test/ui.test.js` |
| 6 | Medium | moving/resizing a panel left its artwork at the old frame | `retargetArtwork` transforms image frames and clips; test in `test/m2.test.js` |
| 7 | Medium | storyboard rebuild used the project default page size and LTR | rebuild uses the page's artboard size and the project's reading direction; test in `test/story.test.js` |
| 8 | Medium | undoing a template replacement lost asset→node links | `assets` added to the command's undo tables; test in `test/m2.test.js` |
| 9 | Medium | canvas drag ended on the first pointer move | drag previews locally and dispatches one absolute command on pointerup; one-step drag verified in the browser |
| 10 | Medium | atomic batch rollback padded/truncated the undo stack and lowered the revision | stacks are snapshotted and restored exactly; revision is preserved; test in `test/agent.test.js` |
| 11 | Medium | undoing a step before a job submission deleted the job and its result | project-wide undo excludes jobs, preserves generated derivatives, and merges counters; test in `test/jobs.test.js` |

## Round 2

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 12 | High | page-scoped undo replaced the whole `assets`/`jobs` table, deleting results created after the snapshot | page-scoped restore now merges provenance: snapshot wins for existing ids, generated derivatives and newer jobs survive; test in `test/jobs.test.js` |
| 13 | High | `History.rollback()` did not restore the snapshot, so a thrown handler left a half-applied edit | `rollback(bundle)` applies the pending snapshot; dispatcher passes the bundle. Regression: a page whose template cannot fit leaves no page/artboard (`test/m2.test.js`) |
| 14 | Medium | dragging a balloon tail set the balloon body to the tail's 12×12 rect, and dragging the body detached the tail | tail clicks resolve to the body; a lettering drag dispatches `comic.lettering.move`; `setFrame` also translates the tail; verified in the browser and by `test/m2.test.js` |
| 15 | Medium | provenance-only commands recorded undo steps that changed nothing | proposals/reviews are back in project-wide snapshots; source assets are undone, generated derivatives preserved; import/accept undo tested in `test/story.test.js` |
| 16 | Medium | artwork covered its panel, so a click selected the image and panel tools were unreachable | `selectableId()` resolves artwork/tail clicks to the panel/lettering body; verified in the browser |
| 17 | Medium | every preset PDF content stream had one more `Q` than `q` | removed the extra pop; `test/publish.test.js` counts the operators in every stream |

## Evidence

- `npm run verify`: type check, boundary, brands, **139 tests**, fixtures, command docs, smoke.
- `npm run perf`: build ~389 ms, save ~23 ms, open ~13 ms, validate ~2 ms, render ~39 ms, package ~9 ms on a 24-page/96-panel book.
- Browser walkthrough: page creation, panel selection through artwork, balloon drag with tail, one-step undo, story import/accept, storyboard review, publication validation, New/Open history reset.

## Remaining limits (accepted, stated)

- The offline provider is a deterministic painter, not a hosted model; the adapter boundary is the deliverable.
- The browser cannot call providers or write print PDFs; the UI reports that instead of embedding a key path.
- Real-time collaboration and a distributed job platform remain out of scope (§11.3, §14).

## Workflow increment (idea → design → review → edit → art → publish)

A focused review of the new design/drawing workflow found and these were fixed:

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 18 | High | an HTTP design command held the undo transaction open for up to 60 s; concurrent commands failed and a project could be adopted mid-transaction | `server/session.js` serialises commands through one queue, `adopt` refuses while a transaction is open, and `comic.story.design` no longer opens an undo step (the proposal is inert and rejectable) |
| 19 | High | accepting one scene marked the whole proposal approved, hiding the remaining scenes from review | `acceptedSceneKeys` tracks partial acceptance; status stays `proposed` until every scene is accepted; unknown/empty scene lists are refused; UI shows accepted scenes and keeps the rest actionable |
| 20 | High | decoded artwork was cached by per-project asset id and survived New/Open | cache keyed by `contentHash` and cleared by `adoptBundle` via `clearImageCache()` |
| 21 | Medium | accepting a storyboard left the canvas on "No page yet" or an unrelated page | the first returned page is selected before switching to Canvas |
| 22 | Medium | an untrusted model template/count mismatch stacked extra panels at the page origin | normalisation chooses a template that fits, `applyProposal` also refuses to under-allocate, and a too-small template is now a validation error |
| 23 | Medium | regenerating a designed proposal used the manuscript parser and lost style/audience/cast | regeneration uses the previous proposal's design options and planner |
| 24 | Medium | design-supplied silent characters never entered the bible, and the character id map was wrong | design cast and panel cast are created at accept time; `idMap.characters[name]` is recorded; `carryIds` maps names, not table keys |
| 25 | Medium | local design output could exceed the committed proposal schema while `comic.story.validate` said it was valid | local fields are clamped and `validateProposal` enforces the schema caps; a test validates the result against `schema/v1/proposal.schema.json` |
| 26 | Medium | `comic.story.providers` reported HTTP as default while design defaulted to local | `comic.story.design` now uses the agent's default when a provider is omitted |
| 27 | Medium | a question mark inside quoted dialogue split the sentence, turning `"Who is there?"` into captions | sentence splitting tracks quotes; a regression test asserts the balloon and speaker |
| 28 | Medium | applying the typography form issued four commands and needed four undo presses | `comic.lettering.edit` applies text, face, size, alignment, variant and policy as one command and one undo step |
| 29 | Low | a noir design still drew black-ink local art because the style control did not follow the design | successful design selects the matching drawing style |

Verified in the browser: Design with AI → Review → Accept all → Draw all moved the workflow from
Idea to Publish (3 pages, 9 panels, 9 drawings), with the drawings visible on the canvas. The
automated equivalent is `test/workflow.test.js`.
