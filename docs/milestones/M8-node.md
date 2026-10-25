# M8 (Node.js stack) — Quality assurance, performance and release

**Status:** Complete for the automated release criteria; the manual human/agent walkthrough on the representative book is recorded below.
**Gate:** `npm run verify` green, **139 tests**; `npm run perf` green.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §14.

## Release criteria

| Criterion | Evidence |
|---|---|
| No approved content is silently lost during edits | asset/character versioning; `comic.job.apply` retains superseded assets; story regeneration preserves approved lettering; tests in `jobs`, `story`, `release` |
| Saved projects reopen without structural loss and every id survives | `test/release.test.js` compares pages, panels, characters, assets, lettering and node ids; existing acceptance test 10 |
| Panel and lettering operations are reversible | `test/release.test.js` undoes and redoes a move; `jobs` test undoes an applied generation |
| Exported pages match the intended layout | golden-render fixture (M2), PDF boxes and contact sheet (M7) |
| AI jobs can fail or be retried without corrupting the project | provider failure leaves assets/page untouched; cancellation discards the result; revision conflict refuses apply; `test/jobs.test.js` |
| Licensing and asset provenance are documented | `LICENSE-*`, `NOTICE`, `ASSETS.md`, `README` licensing section; asset records carry provider/model/request |
| The artboard-id hazard has an automated test | `test/release.test.js`: delete a page, add another, assert the retired id is not reused; deleted artboard is reported |
| Opens, validates and exports with no provider and no fonts | `test/release.test.js` last case; every publishing test runs fontless |

## Workflow evidence

`test/workflow.test.js` runs the product workflow end to end: an idea is designed into a
structured proposal, one scene is accepted and then the rest, every panel is drawn locally,
a line is edited and undone, the book validates, exports proofing images with a contact sheet,
packages, unpacks, reopens and preserves its ids. It also proves that each local drawing style is
distinct and deterministic, and that an untrusted HTTP story-model response is normalised and
validated or refused.

## Failure paths covered

- Interrupted save: a storage that fails after two writes still leaves no manifest; a retry saves and reopens cleanly (`test/release.test.js`).
- Provider failure and cancellation (`test/jobs.test.js`).
- Missing asset and missing font at the export gate (`test/publish.test.js`).
- Deleted artboard and retired-id reuse (`test/release.test.js`).
- Malformed parameters for every command (`test/agent.test.js`); malformed bundles and fixtures (`test/domain.test.js`, `check:fixtures`).

## Representative book

`test/book.js` builds two chapters, three scenes, recurring characters (Maya, Jonas), five pages
of different sizes and templates, irregular panels, long dialogue, captions, thought/whisper/shout
and a sound effect. `test/release.test.js` exports it for screen, print and proofing.

**Walkthrough:** the browser studio was exercised on a live server — page creation, panel
selection, balloon creation, story import, proposal acceptance and storyboard review. The
automated equivalent is the smoke test (HTTP) and the MCP tool call test.

## Performance

Recorded by `test/release.test.js` on a 23-page book and by `npm run perf` on a
24-page / 96-panel book:

| Operation | Measured | Budget |
|---|---|---|
| Build 24 pages through commands | ~404 ms | 30 000 ms |
| Save bundle | ~22 ms | 4 000 ms |
| Open bundle | ~16 ms | 5 000 ms |
| Validate | ~4 ms | 4 000 ms |
| Render one page PNG | ~34 ms | 4 000 ms |
| Package | ~9 ms | 4 000 ms |

`PERF_STRICT=1 npm run perf` tightens every budget for a release run.

## Release checklist

[`docs/releasing.md`](../releasing.md) is the checklist: install, verify, strict perf, MCP
catalogue, package round trip, walkthrough, licence/provenance audit, then tag.

## Remaining limits (stated, not hidden)

- The offline provider is a deterministic painter, not a hosted model. The adapter boundary is complete; a real model is configuration, not new architecture.
- The browser cannot call a provider or write print PDFs; those are server services, and the UI says so.
- Real-time collaboration and distributed job platforms are explicitly out of scope (§11.3, §14).
