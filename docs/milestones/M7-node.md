# M7 (Node.js stack) — Rendering and publishing

**Status:** Complete.
**Gate:** `npm run verify` green, including `test/publish.test.js`.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §13.

## Exit criteria

| Criterion | Evidence |
|---|---|
| Screen, print and proofing presets produce correct page geometry, bleed and marks | `shared/domain/publish.js` presets and `presetGeometry`; `test/publish.test.js` asserts media/trim/bleed boxes and crop marks in the PDF |
| A missing font or asset fails the export with an actionable error | `comic.book.validate` / `.export`; the test blocks a print export on missing asset bytes and on a project face absent from a configured host, and allows the deliberate placeholder only with `allowMissingAssets` |
| The packaged archive round-trips (package → unpack → reopen → compare) | `server/export/package.js` deterministic zip; test compares every page/panel/lettering/node id |
| Editable data is independent of every rendered output | deleting an export directory leaves `comic.project.validate` clean and the project packageable |

## What landed

- `comic.book.validate`, `comic.book.export` (PDF, image sequence, both), `comic.page.render`, `comic.page.export`, `comic.project.package`, `comic.project.unpack`.
- PDF geometry: `MediaBox` includes bleed + mark margin; `TrimBox` is the artboard; `BleedBox` is trim + bleed; content is translated into the trim; crop marks sit outside the bleed.
- Image export: per-page PNG previews plus a contact sheet with page labels and a machine-readable `export.json` report.
- Deterministic package: sorted entries, fixed DOS timestamps, CRC-32 verified on read, path-confined entries, decompression and size caps.
- The fontless contract still holds: with no font service the export uses the standard face and *reports* the missing project face as a warning; a host that has fonts but not the project's face fails a strict print export.

## Notes

- The archive is an export, not the working format. The working format remains the directory bundle.
- A screen preview may draw a labelled placeholder for missing artwork; print refuses it unless the caller explicitly opts in.
