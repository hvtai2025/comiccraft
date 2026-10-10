# Verification playbook

How to check each kind of ComicCraft criterion, and which harness to use. The harness
already exists — do not build a second one.

## Harnesses

| Need | Use | Where |
|---|---|---|
| Render a page region | `testkit::raster::render_region`, `render_artboard`, `render_view` | `crates/testkit/src/raster.rs` |
| Compare renders | `assert_similar` with a tolerance; keeps the diff artifact | `crates/testkit/src/raster.rs` |
| Run commands headlessly | `vectorcraft-cli run --in FILE --cmd ID --params JSON --export FILE` | `apps/vectorcraft-cli` |
| Drive it as an agent would | `vectorcraft-cli mcp --headless --automation-read-root … --automation-write-root …` | `apps/vectorcraft-cli/tests/mcp_e2e.rs` |
| Check the registry | `vectorcraft-cli commands`; MCP `command_list` / `tools/list` | `crates/mcp/src/tools.rs` |
| Performance | `vectorcraft-cli perf`; `vectorcraft-cli bench FILE` | `apps/vectorcraft-cli/src/perf.rs` |
| Malformed input | `engine/tests/command_sweep.rs`, `import_fuzz.rs`, `format/tests/prop_format.rs`, `mcp/tests/protocol_props.rs` | `crates/engine/tests/` |
| The whole gate | `cargo xtask ci` via `./scripts/gate.sh` | `xtask/src/main.rs` |

## Per-criterion checks

**A page renders at the expected size.** Render the artboard at a known `ppi` and assert the
pixel dimensions, then compare perceptually against the fixture. Do not assert PNG bytes.

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
grown. Run it with `CRAFT_FONTS` empty as well as set.

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
