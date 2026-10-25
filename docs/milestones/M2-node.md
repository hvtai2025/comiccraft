# M2 (Node.js stack) — First-class panel and lettering tools

**Status:** Complete.
**Gate:** `npm run verify` — type check, boundary, brands, **139 tests**, fixtures, command docs, smoke.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §8.

## Exit criteria

| # | Criterion | Evidence |
|---|---|---|
| 1 | Panels created from templates, reordered and resized with validation warnings | `test/layout.test.js` (templates, gutters, overlap/outside/degenerate warnings, reorder), `test/m2.test.js` (move/resize, lock) |
| 2 | Every lettering kind renders, wraps, detects overflow and applies its policy | six presentations (speech, thought, narration, shout, whisper, sound effect) in `test/m2.test.js`; policies in `test/lettering.test.js`; rendering in `public/editor/render.js`, `server/export/svg.js`, `server/export/png.js`, `server/export/pdf.js` |
| 3 | Moving a balloon preserves or updates its tail without detaching it | `test/lettering.test.js` — tail follows, keeps its length and side; attach/detach are explicit and reversible (`test/m2.test.js`) |
| 4 | Reading order explicit, editable and validated independently of coordinates | `test/layout.test.js` coordinate-leak test; `comic.reading.set` / `comic.reading.validate`; `validateReadingOrder` |
| 5 | Lettering tests pass with no fonts configured | `test/m2.test.js` runs the fontless contract; `shared/domain/fonts.js`; every export test runs with `families: []` |
| 6 | Golden-render tests for one representative page | `test/golden.test.js` — PNG and SVG SHA-256 fixtures, determinism, and a mutation that removes a border and must change the bytes |

## What landed

- `shared/domain/fonts.js`: the faces are referenced, never shipped; a missing face is a report, not a silent substitution.
- `shared/domain/layout.js`: right-to-left templates, template warnings, gutters and margins.
- Lettering tools: `setStyle`, `setFrame`, `setVariant`, `setOverflowPolicy`, `attachTail`, `detachTail`, `comic.fonts.list`.
- Panel tools: `move`, `resize`, `setShot`, `setArtworkRole`, `list`.
- Page tools: `comic.page.create`, `reorder`, `setSize`, `template.apply`, `setStatus`, `comic.reading.*`.
- Rendering distinguishes thought bubbles (bubble tail), shout (jagged), whisper (dashed) and display sound effects; locked panels are visible on the canvas.
- Golden fixture: `test/fixtures/golden-representative.json`.

## Deliberate limits

- Irregular panel shapes are built with `comic.panel.setFrame`, not a template. The plan makes custom shapes an extension, not a prerequisite.
- A tail is *intelligently updated* on a move (base reattached, length kept) rather than pinned to one point; the criterion allows either and the tests state which one this build implements.
- `CRAFT_FONTS`/font files are never required. The browser only counts faces the document actually loaded; an unloaded catalogue face is reported missing.
