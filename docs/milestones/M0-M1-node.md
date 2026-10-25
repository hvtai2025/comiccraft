# M0 + M1 (Node.js stack) — Baseline and the vertical slice

**Status:** Complete — the gate passes, the browser client renders and edits, and the twelve
criteria of Phase 1 are proven by tests and by hand.
**Stack:** Node.js backend + client-side HTML/JavaScript frontend. No Rust, no framework, no
bundler, no build step.
**Gate:** `npm run verify` — **pass, 3 s** (evidence: [`ci-33fcc947.md`](../ci-33fcc947.md))
**Runtime:** Node 24.21.0, `fastify` the only runtime dependency.

> This is the new stack's M0 and M1. The earlier milestone reports (`M0.md`, `M1.md`, `M2.md`)
> are the Rust implementation's record and stay as history — see the porting plan §2.4.

---

## 1. What M0 produced

**Exit criteria:** the repository installs with `npm install` and runs with `node server.js` on
a machine with no Rust toolchain; the schema is approved and frozen for M1; a three-panel page
can be created and exported by hand; `npm run verify` is green.

| # | Criterion | State | Evidence |
|---|---|---|---|
| 1 | Repository layout per plan §4 | ✅ | `server.js`, `server/`, `public/`, `shared/`, `schema/`, `test/`, `tools/` |
| 2 | Runs with no Rust toolchain | ✅ | `preflight.sh` reports "no Rust toolchain on PATH — correct for this stack"; nothing imports or builds `reference/` |
| 3 | Schema frozen and machine-checkable | ✅ | `schema/v1/` — 8 schemas, 8 fixtures, checked by `check:fixtures` |
| 4 | `shared/domain/validate.js` implemented and tested | ✅ | `shared/domain/validate.js`, proven by `test/domain.test.js` |
| 5 | Gate green | ✅ | `npm run verify` exit 0 in **3 seconds** |
| 6 | The reference gate is not re-run | ✅ | `docs/ci-baseline-3da4080b.md` cited, not regenerated (plan §2.4) |

**The reference tree moved** to `reference/vectorcraft/` (1 272 tracked files), so the root is
ComicCraft's. `target/` stayed at the root because the milestone and review records cite its
evidence by path.

## 2. What M1 produced

**Exit criteria (Phase 1, ported):** a three-panel page — bundle, artboards as pages, panels,
artwork, lettering, undo, save/reopen, and PNG and PDF out — with the twelve acceptance tests
passing.

All twelve are proven in [`test/acceptance.test.js`](../../test/acceptance.test.js), each test
naming the criterion it covers. Proved **by hand** as well:

```
$ node server/cli.js demo
✓ comic.project.create
✓ comic.page.add
✓ panel 1: artwork placed, cropped, clipped, balloon added
✓ panel 2: artwork placed, cropped, clipped, caption added
✓ panel 3: artwork placed, cropped, clipped, sound-effect added
✓ validate: 0 error(s), 0 warning(s)
✓ moved a balloon; tail attached: true
✓ undo: Move Lettering (8 step(s) left)
✓ svg → out/demo-page-1.svg (2652 bytes)
✓ png → out/demo-page-1.png (3653 bytes)
✓ pdf → out/demo.pdf (1 page(s), 1813 bytes)
✓ saved 4 file(s) to demo/
✓ reopened: 1 page(s), 3 panel(s), 0 error(s)
```

And **in the browser**: the client boots from `node server.js`, imports the same `shared/`
modules over HTTP, builds the same 36-command registry, creates a three-panel page from the
`three-up` template, and draws it — canvas `638×975` with 8 037 ink pixels after a balloon was
added. The reading-order panel shows `panel_1, panel_2, panel_3` at `409×1950` each, which is
the arithmetic `(1275 − 2×24) / 3`.

### The three outputs are real files, not stubs

| Output | Structural check |
|---|---|
| PNG | `319×488`, bit depth 8, colour type 6 (RGBA), **all chunk CRCs valid**, `IHDR`/`IDAT`/`IEND` |
| PDF | starts `%PDF-1.4`, ends `%%EOF`, **MediaBox `0 0 306 468`**, `startxref` points at `xref` |
| SVG | `<svg>` with the artboard's `viewBox`, clip paths for clipped artwork |

The PDF geometry is the criterion: a 1275×1950 artboard at 300 ppi is `1275 × 72/300 = 306` and
`1950 × 72/300 = 468` points. A test reads the box back and asserts it.

## 3. Third-party dependencies

| Kind | Count | What |
|---|---|---|
| Runtime | 1 | `fastify` |
| Dev | 2 | `typescript` (checks, never emits), `@types/node` |

No framework, no bundler, no rasteriser, no canvas package, no PDF library, no test framework
(`node --test`), no linter package. The PNG encoder is `node:zlib` plus a CRC table; the PDF
writer emits the xref by hand.

## 4. Defects the work found, and what each says

Each of these was caught by a check, not by reading:

| # | Defect | Found by | Why it mattered |
|---|---|---|---|
| 1 | A node added to a group was **also** on the artboard, so it was reachable twice and drawn twice | `artboardNodes` test | Silent double-draw and double-count on every tree walk |
| 2 | `addNode` accepted a `children` array, bypassing the ownership rule | the same test, after fix 1 | The rule needs one writer, not every caller |
| 3 | Undo scope declared `nodes: []`, so a command that **creates** a node snapshotted nothing and undo left the object behind | acceptance test 8 | A command cannot know an id it has not allocated yet — the scope model was wrong, not the command |
| 4 | `comic.panel.add` declared `exclusiveMinimum`, which the validator refuses, so **every well-formed call returned an internal error** while every malformed one looked correctly rejected | commands test | Also fixed the validator; and a new test now walks every command's schema to prove the validator can enforce it |
| 5 | `imageSvg` had an unused `byId` parameter and a `void` statement | reading while fixing the type check | Noise that hides the next real problem |
| 6 | The boundary gate's own import scan was dead: `stripNonCode` removed the string literals it searched for, so only the structural half kept it honest | **proving the gate bites** | A gate that reports nothing and passes looks identical to a gate that works |
| 7 | The boundary gate fired on `document` in seven correct modules, because the domain names its visual document `document` | first run | A check that fires wrongly teaches people to ignore the gate |
| 8 | The boundary test poisoned globals at module scope and cleaned up before `node:test` ran the callbacks, so it passed vacuously with nothing poisoned | its own final assertion | The test now has a test |
| 9 | `history.canUndo(bundle)` documented a parameter it does not take; several JSDoc gaps | `tsc --checkJs` | — |

**Defect 3 is the one worth remembering.** The scope type looked reasonable and was wrong: a
command that creates an entity cannot name it in advance. The fix was to stop pretending the
granularity could be per-node, and snapshot the document for a page-scoped step.

## 5. Gate result, verbatim

```text
npm run verify
  typecheck   tsc --noEmit                     pass
  boundary    boundary ok — 22 module(s) in shared/ import cleanly with no runtime
  brands      brands ok — no vendor name in shipped text
  test        70 tests, 70 pass, 0 fail
  fixtures    fixtures ok — every schema has a fixture, every fixture validates,
              and the check rejects a mutation
  smoke       smoke ok — 17 checks, the server boots, serves the client and the
              domain, and runs the slice
elapsed: 3s   exit: 0
GATE PASS
```

**3 seconds.** The Rust baseline's `cargo xtask ci` produced a 423 KB log.

### The gate was proved to bite

Not asserted — observed, three times, then reverted:

| Planted in `shared/domain/geom.js` | Result |
|---|---|
| `import { readFileSync } from 'node:fs'` | `shared/domain/geom.js: imports node:fs` + 15 import failures → exit 1 |
| `import fastify from 'fastify'` | `imports the package 'fastify', which a browser must fetch` → exit 1 |
| `export const w = window.innerWidth` | `references the browser global 'window'` → exit 1 |
| reverted | `boundary ok — 22 module(s)`, **file byte-identical to the backup** |

## 6. What is not done

- **M2–M8.** Panel *templates* beyond the six named grids, story import, character continuity
  and generation, the agent surface's batch operations, UI polish, publishing presets, and
  release hardening are all unstarted. The plan's §15 table is the list.
- **The reference tree is not deleted** — it is read-only evidence. The six Rust workflows and
  `packaging/` are deliberately still present and travel with it (`.github/workflows/README.md`).
- **`public/` is not in the strict type check.** It needs the DOM lib and runs in a browser; it
  is covered by the boundary test, which loads modules the way the browser does. This is a
  documented narrowing, not an omission — see `tsconfig.json`.
- **No real rasteriser.** `server/export/png.js` draws rectangles, triangles and a block-glyph
  approximation, and says so. It produces a deterministic raster for tests, thumbnails and CI.
  The browser draws the real thing with canvas, and both share the same wrapping and overflow
  rules, so a line breaks in the same place in both.
- **No AI provider.** `server/ai/` is empty; `comic.job.*` does not exist yet. That is M4.
