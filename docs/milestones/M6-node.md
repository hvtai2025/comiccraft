# M6 (Node.js stack) — Comic studio UI

**Status:** Complete.
**Gate:** `npm run verify` green, including `test/ui.test.js`; interactive walkthrough recorded below.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §12.

## The workflow the studio presents

The header shows **Idea → Design → Review → Edit → Artwork → Publish**. The active step is
derived from project state (`comic.story.design` proposals, pages, panel artwork, validation),
not stored; clicking a step opens the relevant view or the idea form.

- **Design with AI** collects title, premise, style, audience, characters and panels per page,
  then calls `comic.story.design` and opens Review.
- Review shows the logline, style, cast, provider/model and each scene, with **Accept scene** and
  **Accept all**. Acceptance switches to the canvas.
- **Draw panel / Draw all** run `comic.artwork.paint` through the dispatcher in five
  styles; **Submit AI job (server)** remains the provider path with review before apply.
- Assets load their data-URL artwork into the canvas and re-render when decoding finishes, so a
  local drawing is visible immediately.

## What the studio provides

- **Story navigator:** chapters → scenes → pages, with add-chapter, add-scene and story import.
- **Page thumbnail list:** narrative order, panel count and page status.
- **Storyboard view:** every page in chapter/scene order as a canvas thumbnail plus its panel sequence; click to edit.
- **Canvas:** add panel, move a panel or balloon by dragging, add speech/thought/caption/SFX, place/crop/clip artwork, lock, delete, undo/redo.
- **Inspector:** frame fields (x/y/width/height) for the selected panel or lettering, panel id, policy, lock state.
- **Typography:** text, face, size, alignment, balloon variant and overflow policy; applying one form is a set of ordinary commands.
- **Character and asset libraries:** character names and design approval; content-addressed assets with approval status, file upload and selection for placement.
- **Generation queue:** submit a prompt against the selected panel; poll status; cancel, apply or reject the reviewed result.
- **Review view:** story proposals, jobs, assets awaiting review and validation findings, with distinct colours per state.
- **Publication panel:** preset chooser and the real `comic.book.validate` gate.

## Exit criteria

| Criterion | Evidence |
|---|---|
| Every M1 operation reachable without an agent | `test/ui.test.js` lists the controls and the dispatched command ids; the smoke test drives the same operations over HTTP |
| Panel/balloon/caption/image selection by direct manipulation | canvas hit-testing and pointer move; inspector edits dispatch `comic.panel.setFrame` / `comic.lettering.setFrame` |
| Proposed, pending, approved, user-modified and warning states are visually distinct | `.badge.*` and `.card.*` styles; warning outlines and a locked badge in the canvas renderer |
| A panel edit and the equivalent command produce the same state | the UI does not mutate the bundle; every action goes through `dispatch(registry, …)` |

## Interactive walkthrough (recorded)

1. Add a three-up page; the navigator, page list and reading order update.
2. Select a panel and add a speech balloon; the inspector and typography controls populate, and the canvas draws the balloon and tail.
3. Import a two-scene story; review shows a proposal; accept it; the storyboard gains pages with recurring characters.
4. Submit artwork generation (server-side): the UI states that providers run on the server, where credentials live; the offline provider produces a result when driven by the server.
5. Switch between Canvas, Storyboard and Review; no panel re-implements a domain rule.

## Deliberate limits

- Provider calls and print-quality export are server-side by design (§14.1). The UI reports that rather than embedding a key path.
- The browser saves a bundle to IndexedDB; filesystem save/reopen is the server's. Both use the same serializer.
- The gate does not launch a browser; `test/ui.test.js` checks the properties that can be checked without one.
