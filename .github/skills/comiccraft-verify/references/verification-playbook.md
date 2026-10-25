# Verification playbook

How to check each kind of ComicCraft criterion, and which harness to use. The harness
already exists — do not build a second one.

## Harnesses

There is one harness, and it is the one that ships. Do not build a second one.

| Need | Use | Where |
|---|---|---|
| Unit and integration tests | `node --test` (built in — no framework) | `test/**` |
| Render a page | the client's renderer, driven headlessly through the command surface | `public/editor/render/**` |
| Compare renders | the perceptual comparator; keeps the diff artifact next to the fixture | `test/golden.js` |
| Run commands headlessly | `npm run cli -- run --project DIR --cmd ID --params JSON --export FILE` | `server/cli.js` |
| Drive it as an agent would | the MCP server over the same registry | `server/mcp.js`, `test/mcp.e2e.js` |
| Check the registry | `npm run cli -- commands`; MCP `tools/list`; `GET /api/comic` | `shared/commands/index.js` |
| **Prove the shared boundary** | the boundary test: load every `shared/` module in bare Node, then assert no `node:` import and no DOM global anywhere in the tree | `test/boundary.test.js` |
| Performance | page-load and render timings recorded as JSON for comparison across runs | `test/perf.js` |
| Malformed input | `test/fuzz.js` — malformed bundles and malformed parameters for every registered command | `test/fuzz.js` |
| The whole gate | `npm run verify` via `./scripts/gate.sh` | `package.json` scripts |

## Per-criterion checks

**A page renders at the expected size.** Render the artboard at a known `ppi` and assert the
pixel dimensions, then compare perceptually against the fixture. Do not assert PNG bytes.

**The domain runs in both runtimes.** If a criterion touches `shared/`, it is not proven by a
Node test alone. Bound it with the boundary test, plus one case that loads the module the way
the browser does (an ES-module import with no Node globals available). Launching a real
browser is *not* required, and requiring one is itself a defect (plan §17.1).

**Ids survive a round trip.** Save the bundle, load it, and assert every `PageId`,
`PanelId`, `NodeId` and `artboard_id` is unchanged — and that no id was reallocated. This is
the criterion that catches the artboard-id hazard.

**One user action is one undo step.** Perform the action, undo once, and assert the
document and the comic records both returned to the prior state. Two undos needed is a
failure.

**Replacing artwork preserves everything else.** Replace one panel image and assert the
panel's dialogue, lettering objects, page geometry and reading order are byte-identical, and
that the previous asset still exists.

**Text fits or reports.** For each lettering kind: assert no overflow, or assert the
declared policy fired — `Warn` reported, `Shrink` scaled via `fit_scale`, `ResizeFrame`
grown. Run it with **no fonts configured**: measurement is injected, so the test supplies a
deterministic stub. A test that needs a real font installed is a broken test.

**PDF geometry.** Assert the page count, and each page's trim box equals its artboard, the
bleed box is grown by the bleed, and the media box includes the marks. Read the boxes; do
not eyeball the file.

**A job cannot overwrite newer state.** Submit a job against a panel, change that panel,
then apply the job. It must refuse and report the conflict.

**The project works offline.** With no provider configured, open, edit, validate, save and
export. This must pass at every milestone from M1 on.

**Validation reports rather than repairs.** Introduce each inconsistency — missing artwork
node, dangling lettering link, retired artboard, invalid reading order — and assert
`comic.project.validate` reports it and changed nothing.

## Failure paths to include

Every milestone's verification includes these, not just the success paths:

- interrupted save (kill between staging and manifest replacement)
- provider failure and cancellation
- missing asset file; content-hash mismatch
- missing lettering font
- deleted artboard, and the next artboard created afterwards
- a document migrated from the previous `format::VERSION`

## Reporting

Per criterion: `pass`, `fail`, or `not run` — with the command and its output. "Looks good"
is not a result. If the gate failed, quote the first failing step verbatim.
