# ComicCraft
## Porting Plan: VectorCraft → ComicCraft

**Status:** Finalized (v1.0) — ready to execute
**Base repository:** `https://github.com/storytold/vectorcraft`
**Baseline:** `3da4080b` (`main`, clean tree) — workspace `0.8.0`, edition 2024, MSRV 1.95, MIT OR Apache-2.0
**Target repository:** a new `comiccraft` fork of that baseline
**Primary objective:** Transform VectorCraft into an agent-native, story-to-comic production studio while preserving its vector editing engine, document reliability, rendering, file interoperability, and automation capabilities.
**Companions:** [`comiccraft-design.md`](comiccraft-design.md) · [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)

> The design in [`comiccraft-design.md`](comiccraft-design.md) is finalized and the
> baseline audit is complete, so this plan no longer asks the reader to verify
> assumptions: it names the commits, crates, commands and gates it depends on.

---

## 1. Product vision

ComicCraft turns prompts, scripts, and existing stories into editable comic books.

The workflow is:

1. Import or write a story.
2. Use an AI story editor to break it into chapters, scenes, pages, and panels.
3. Generate consistent character references and panel artwork.
4. Compose pages with editable panel frames, speech balloons, captions, and sound effects.
5. Let users edit artwork, geometry, dialogue, and page composition.
6. Render, validate, and publish the finished comic.

AI-generated output must remain editable. The system must never require regenerating an entire page just to change dialogue, move a balloon, or replace one panel image.

### Core design principle

**VectorCraft owns vector editing and document rendering. ComicCraft owns comic semantics, narrative structure, AI orchestration, and publishing workflows.**

Do not rewrite the existing editor engine unless a specific comic requirement cannot be supported through an extension.

---

## 2. Repository strategy

### 2.1 Fork rather than rewrite

Fork VectorCraft and create a dedicated ComicCraft repository. **That fork does not
exist yet**; creating it is the first task of M0. The planning documents currently
live under `comiccraft/porting_plan_and_design/` in the ComicLab site repository and
belong in the fork once it exists.

The baseline is already recorded
([`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)); M0 confirms and
extends it rather than re-deriving it:

- **Pin the baseline commit:** `3da4080b` on `main`, a clean working tree,
  `origin = https://github.com/storytold/vectorcraft`.
- **Record the workspace layout, dependencies, public APIs, native format, command
  registry and MCP protocol:** done — audit §3–§10. Re-verify only what drifted.
- **Run the existing CI suite and save its result:** `cargo xtask ci` runs `fmt`,
  `clippy`, `test`, `assets`, `brands`, `layers` and `wasm`
  (`xtask/src/main.rs:175-205`). Save the passing output as the baseline evidence, and
  record how long it takes — it is the gate every later commit must pass.
- **Create a baseline branch and preserve the upstream remote**, so divergence is
  measurable (`git rev-list --count upstream/main..HEAD`) rather than estimated.
- **Record licensing and attribution obligations:** code and original assets are
  MIT OR Apache-2.0; fonts live in `storytold/craft-fonts` and never in this
  repository; every asset file needs a row in `ASSETS.md`; the ArtCraft marks are
  trademarks and ComicCraft must ship its own original branding
  (`AGENTS.md:14-48`).

The workspace is `crates/*` + `apps/{vectorcraft, vectorcraft-cli, vectorcraft-web}` +
`xtask`, with the dependency layering enforced by `cargo xtask layers`
(`xtask/src/layers.rs:9-57`). Preserve these boundaries; the new comic crates slot
into them rather than replacing them (design §4).

### 2.2 Avoid a premature mass rename

Do not rename every crate, module, and internal symbol in the first milestone.

First establish a working ComicCraft product using the existing engine. Rename user-facing branding, application metadata, project templates, and documentation incrementally.

Internal package renaming should occur only when it provides a concrete maintenance or distribution benefit.

### 2.3 Preserve upstream compatibility

Keep upstream changes isolated and easy to merge.

- Maintain an upstream tracking branch.
- Prefer new comic-specific crates over invasive changes to core crates.
- Avoid changing existing command semantics.
- Add regression tests for every modification to shared engine behavior.
- Document unavoidable upstream divergence.

**Known unavoidable divergence, decided up front (ADR-014).** The command registry is
a fixed concatenation inside the engine crate and no public hook exists for another
crate to register commands (`crates/engine/src/cmd/mod.rs:85-132`). Comic commands are
therefore additive modules inside `crates/engine` — `crates/engine/src/cmd/comic/*.rs`,
each contributing `specs()` — which is the only way the desktop UI, the control
channel, the CLI and MCP can all reach them through one registry
(`crates/engine/src/lib.rs:1071-1095`).

The divergence is small and mechanical, and it is bounded by three rules:

1. Additive only. No existing command's id, parameters, enablement or behaviour
   changes.
2. One module per command group, so a merge conflict is confined to the single
   `command_specs()` line that lists them.
3. An optional upstream contribution — a generic command-group registration seam —
   is tracked as a follow-up. If it lands, comic modules stop being a divergence at
   all.

The same reasoning applies to the UI: comic panels are added inside
`crates/ui-egui` (`panels/comic/`) rather than a new UI crate, because egui/eframe are
permitted only at L6 (`xtask/src/layers.rs:88-90`) and a separate UI crate would need a
same-layer edge to `ui-egui`.

---

## 3. Target architecture

Four logical layers, mapped onto the enforced layer numbers
(`xtask/src/layers.rs:9-57`). The design document's §4 gives the crate-by-crate view;
this is the dependency view.

### Layer A — Existing VectorCraft foundation

Preserve, and do not duplicate:

- Geometry, paths, vector shapes, transformations — `geom`, `pathops` (L0, L2).
- Document model, layers, groups, artboards — `doc` (L1).
- Text layout and typography — `text` (L2).
- Live, non-destructive effects — `effects` (L2).
- Raster image placement and image operations — `doc` + `object.cropImage`.
- Rendering and compositing — `render` (L3).
- Undo/redo and document history — `engine` (L5).
- Native file persistence — `format` (L3).
- SVG, PDF and raster import/export — `svg`, `pdf`, `render::encode` (L3).
- Command registry and automation interfaces — `engine` (L5).
- Desktop, CLI, web and MCP infrastructure — `apps/*`, `mcp` (L6).

### Layer B — Comic domain model

**New crate `crates/comic-model` (L2).** The project bundle schema, stable comic ids,
narrative entities, page/panel records, lettering records, asset provenance records,
schema migrations, and the bundle reader/writer — no network, no AI, no UI. It depends on
`doc` (for `NodeId`) and `geom`, and on nothing else in the workspace.

This model must be independent of the GUI and must not rely on layer names, object
names or arbitrary vector shapes to recover narrative meaning. A page's identity is a
comic `PageId`, its artboard is reached by `Page.artboard_id`, and every visual object
is reached by `NodeId` (design §5.2.1).

### Layer C — Comic operations and AI services

**New crates `crates/comic-ai` (L3) and `crates/comic-layout` (L4).** Comic-specific
operations:

- Story decomposition and storyboard planning.
- Character bible and continuity constraints.
- Page and panel layout, templates, gutters, reading order.
- Lettering placement, fitting and overflow policy.
- Image generation, regeneration and variation, through provider adapters.
- Deterministic and assisted validation.
- Batch rendering and publishing.

`comic-layout` is deterministic and testable headlessly; it owns the layout maths and
the export assembly and returns a plan, and the engine command modules apply that plan
to the document so the result is one undoable operation (design §11.2). It never calls
back into the engine — the arrow points one way. `comic-ai` owns provider-neutral
request building, capability descriptors and the job state machine, and knows nothing
about `doc`, `render` or the UI.

**Commands are additive engine modules (L5)** — `crates/engine/src/cmd/comic/*.rs`.
This is the only route to GUI, control channel, CLI and MCP, and it is the documented
divergence described in §2.3.

### Layer D — Comic studio interface

Comic panels and views are added **inside `crates/ui-egui`** (L6) as a panel family
(`crates/ui-egui/src/panels/comic/`), registered through `UI_COMMANDS`
(`crates/ui-egui/src/menus.rs:1-20`), reading engine state and acting only through
`app.run(id, params)`. They surface:

- Story and chapter navigator.
- Character and asset library.
- Storyboard view, built on `Renderer::render_thumbnail(doc, NodeId, size)`.
- Page thumbnails and the artboard/page navigator.
- Comic page canvas.
- Panel properties.
- Balloon and lettering tools.
- AI generation queue and review controls.
- Validation and publication panel.

The existing vector tools stay available as an advanced editing mode.

**Layering rule:** nothing in Layer B or C may depend on `egui`, `eframe`, `winit`,
`egui_kittest` or `rfd` — the layer check enforces it (`xtask/src/layers.rs:88-90`), and
it is what keeps the comic core usable from the CLI and MCP.

---

## 4. Workspace structure

The baseline workspace is the source of truth. Three crates are added, two surfaces
are extended, and existing crates keep their layer.

```text
comiccraft/                              # fork of vectorcraft @ 3da4080b
├── apps/
│   ├── vectorcraft/                     # desktop app (rebranded to ComicCraft)
│   ├── vectorcraft-cli/                 # batch + agent workflows, `mcp`, `run`
│   └── vectorcraft-web/                 # wasm build, deterministic surface only
├── crates/
│   ├── geom/ color/                     # L0  unchanged
│   ├── doc/                             # L1  unchanged — VISUAL DOCUMENT
│   ├── text/ effects/ pathops/ brush/ trace/ plugins/    # L2  unchanged
│   ├── comic-model/                     # L2  NEW — bundle schema, ids, narrative, provenance
│   ├── render/ svg/ pdf/ format/ cad/ eps/ metafile/     # L3  unchanged
│   ├── comic-ai/                        # L3  NEW — provider adapters, job state machine
│   ├── comic-layout/                    # L4  NEW — layout, templates, lettering policy,
│   │                                    #         validation, publishing assembly
│   ├── tools/                           # L4  extended with comic tool state machines
│   ├── engine/                          # L5  extended: cmd/comic/*.rs (additive)
│   ├── ui-egui/                         # L6  extended: panels/comic/* (additive)
│   ├── mcp/                             # L6  unchanged — comic commands appear automatically
│   └── testkit/ affinity/               # unchanged (testkit: dev-dependency only)
├── xtask/                               # layers.rs gains the three new crates
└── docs/
    ├── comiccraft-porting-audit.md      # = the baseline audit
    ├── comiccraft-project-schema.md     # M0 deliverable: the bundle schema
    ├── comiccraft-command-api.md        # the comic.<group>.<verb> command reference
    ├── comiccraft-ai-workflow.md        # provider interfaces, jobs, provenance
    └── comiccraft-publishing.md         # export presets, print rules
```

Three new crates, not twelve. The earlier draft proposed `comic-lettering`,
`comic-assets`, `comic-workflow`, `comic-validation` and `comic-publish`; each of those
is a module inside one of the three, because every extra crate is another layer
declaration, another manifest and another CI surface.

The layers are chosen so that **every dependency points downward**: `comic-model` (L2)
sits above `doc` (L1), `comic-ai` (L3) above `comic-model`, and `comic-layout` (L4)
above the renderers and exporters it assembles. That avoids every same-layer edge, and
therefore every hand-maintained `INTRA_LAYER_ORDER` entry
(`xtask/src/layers.rs:65-84`) — a same-layer dependency is only legal there if the pair
is listed.

**Adding the crates is a concrete task, not a formality:** `cargo xtask layers` fails
on any crate it does not know about (`xtask/src/main.rs:109-139`), so
`xtask/src/layers.rs` gains three `Class::Layer(n)` entries. That edit belongs in the
same commit that adds each crate.

Before creating a crate, check whether the responsibility fits inside one of the three.
Do not introduce a crate for every concept merely to match a diagram.

---

## 5. Define the canonical comic project model

This is the most important architectural task, and the schemas are now fixed in
[`comiccraft-design.md`](comiccraft-design.md) §5. This section states the model and
the invariants; the design document states the fields.

### 5.1 Entity hierarchy

```text
ComicProject
 ├── Characters / Locations / Styles
 ├── AssetLibrary                        (registry: provenance, hashes, approval)
 ├── visual_document  ─────────────────► the ONE .vectorcraft document
 └── Chapters
      └── Scenes
           └── Pages                      (Page.artboard_id ► an artboard)
                └── Panels                (Panel.frame_node_id ► a document object)
                     ├── ArtworkRefs      (ArtworkRef.node_id ► document objects)
                     ├── ShotDescription
                     ├── Dialogue
                     ├── Lettering        (balloon | caption | sound effect)
                     └── ContinuityConstraints
```

Three corrections to the earlier draft, all argued in the design document:

- **A page references an artboard, not its own visual document.** One project is one
  document and each page is one artboard (ADR-011, design §5.1.1), so a page can be
  undone, exported and thumbnailed with the machinery that already exists.
- **A panel does not carry a layout record.** The frame object in the visual document
  owns the geometry; the panel holds `frame_node_id` (design §6.1). A second copy of a
  rectangle is the failure mode this architecture exists to prevent.
- **Balloons, captions and sound effects are one entity**, `Lettering` with a `kind` —
  their difference is presentation, not structure (design §5.2).

### 5.2 Required properties

Every persistent entity has:

- A stable id that is never reused: `NodeId(u64)` for visual objects
  (`crates/doc/src/node.rs:19-20`), `Artboard.id: u32` for pages' artboards, and comic
  newtypes over `u64` allocated from a persisted counter for everything else.
- A schema version, at the bundle level and per migrated schema.
- Creation and modification metadata.
- Explicit references to related entities — never a duplicated field, and never an
  array index (design §5.2.1).
- Validation rules, run by `comic.project.validate`.
- Serialization and migration support, with a fixture per previous version.

Panel and asset identifiers must survive save/reopen, layout changes, and image
regeneration. The artboard-id reuse hazard and its five mandatory mitigations are in
design §5.3.1 — read that before writing any code that touches artboards.

### 5.3 Separate semantic state from visual state

The comic model is authoritative for narrative meaning:

- Which character speaks.
- What the dialogue says.
- Which panel belongs to which scene.
- Which panel comes before another.
- Which reference images constrain generation.

The visual document is authoritative for actual editable geometry and artwork placement.

The two representations must be synchronized through explicit commands and validated references.

Do not create two independent, conflicting copies of the same geometry.

### 5.4 Native project persistence

**Finalized in [`comiccraft-design.md`](comiccraft-design.md) §7.** In summary: a
directory bundle holds a manifest, the comic project, the story files, per-page comic
files, one visual document (`.vectorcraft`, artboards = pages), the asset registry,
provenance and durable job metadata (ADR-012). The visual document is written through
the native format unchanged; the comic schema is versioned separately in the manifest
and never overloads `crates/format` (ADR-013). Saving is ordered so the manifest is
written last and never references partially written data, and every schema has an
identified version with fixtures for each previous version.

The earlier draft's "prefer a versioned container" wording is superseded by that
decision; a single-file archive is an export (`File ▸ Package Project`), specified in
design §7.5 and delivered in Phase 7, not the working format.

---

## 6. Phase 0 — Baseline (M0)

**Goal:** Stand up the fork on a pinned baseline and freeze the project schema.

**Status: the audit half is done.** [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)
*is* the repository architecture report, the dependency and API map and the comic
feature gap analysis, with `path:line` citations for every claim. The original tasks 1,
2, 3, 4 and 8 are complete; what remains is mechanical and can proceed without further
investigation.

Remaining tasks:

1. Create the `comiccraft` fork at `3da4080b`, add the upstream remote, create the
   baseline branch, and move these planning documents into the fork.
2. Run `cargo xtask ci` on the untouched baseline and save the output. This is the
   pre-change evidence and the timing reference for design §14.
3. Build a minimal document through the command API and export it, exercising M1's
   acceptance path by hand before automating it: `artboard.new`, place an image
   (`object.cropImage` and the clip-rectangle command for cropping and clipping), add
   an area text object, then `document.serialize`, `document.export` and the PDF
   export of all artboards. Confirm that cropping, clipping, typography and PDF page
   geometry behave as the audit §7–§9 says they do.
4. Add the three crate skeletons and their `xtask/src/layers.rs` entries, then confirm
   `cargo xtask layers`, `cargo xtask wasm` and `cargo xtask brands` still pass.
5. Write `docs/comiccraft-project-schema.md`: the bundle schema — manifest,
   `project.json`, per-page files, the artboard/`NodeId` link rules, id allocation and
   migration rules, with a fixture per version. It must not contradict design §5.

Deliverables:

- `docs/comiccraft-porting-audit.md` — **done** (this audit).
- `docs/comiccraft-project-schema.md` — the approved, frozen initial project schema.
- The baseline `cargo xtask ci` report.
- Three empty comic crates, registered in the layer table.

**Exit criteria:** the fork builds and passes the full gate unmodified; the schema is
approved and frozen for M1; and a three-panel page can be created and exported by hand
through commands, with the output checked against the audit.

---

## 7. Phase 1 — Comic page vertical slice

**Goal:** Prove that VectorCraft can serve as the editing foundation.

Implement a single page containing three panels.

Required capabilities:

- Create a comic project and page.
- Create panels with stable IDs.
- Place and clip raster artwork inside panels.
- Adjust panel geometry and gutters.
- Add editable speech balloons and tails.
- Add editable caption boxes.
- Add a basic sound-effect object.
- Save, close, reopen, and render the page.
- Modify the page through both the GUI and agent commands.

Use existing artboards and document objects where they fit. Add specialized comic operations only where necessary.

### Acceptance tests

1. `comic.project.create` and `comic.page.create` produce a bundle with one
   `.vectorcraft` document and one artboard per page.
2. `comic.panel.add` creates three panels with stable `PanelId`s and
   `frame_node_id`s pointing at real document objects.
3. An image is placed and clipped in a panel, and cropped with `object.cropImage`.
4. The user moves and resizes a panel on the canvas; the panel id and its artwork,
   dialogue and reading order are unchanged.
5. `comic.dialogue.update` changes dialogue without touching artwork or typography.
6. `comic.panel.replace_art` replaces one panel image and leaves every other object,
   and the previous asset, intact.
7. Two speech balloons and one caption exist as `Lettering` records whose text is
   editable and whose frames are editable paths.
8. Undo and redo each of the above as a single step.
9. The bundle is saved, closed, reopened and validated, and the page matches the saved
   layout, with every id preserved.
10. Page render produces the expected page dimensions, and PDF export produces one
    page per artboard with correct reading order.
11. The artboard-id hazard of design §5.3.1 has a failing-then-passing test: deleting
    a page and adding another must not silently re-point a stale `Page.artboard_id`.
12. The same essential operations succeed through `mcp --headless`.

**Exit criteria:** every test above passes; `cargo xtask ci` is green including the
layer, brand, asset and wasm gates; and the reuse report of §18 has been produced.

**Gate:** `cargo xtask ci`. **Do not start AI image-generation integration until this
milestone passes.**

---

## 8. Phase 2 — First-class panel and lettering tools

**Goal:** Make comic composition efficient and predictable.

### Panel engine

Implement:

- Rectangular and supported custom-shaped panels.
- Panel borders and gutters.
- Panel image placement, scaling, cropping, and clipping.
- Reordering and resizing.
- Reusable page templates.
- Panel locking and selection.
- Layout validation and overlap warnings.

Support irregular panels as an extension, not as a prerequisite for the first release.

### Lettering engine

Implement:

- Speech balloons.
- Thought balloons.
- Narration boxes.
- Shout and whisper variants.
- Adjustable tails and control points.
- Editable text and typography.
- Text padding, alignment, and automatic fitting.
- Overflow detection.
- Safe margins.
- Sound-effect typography.

Balloon tails must remain editable and connected to their balloon. Moving a balloon should preserve or intelligently update its tail.

Do not flatten dialogue into panel artwork.

### Reading order

Maintain explicit narrative order independently of canvas coordinates.

Support user-defined reading order and validation for supported writing directions. Do not assume that geometric position alone determines the intended sequence.

### What already exists, and what is new

Panel shapes, borders, image transforms, cropping, clipping and locking are engine
features already (design §8.2). What Phase 2 builds is the *comic* layer over them:
`comic-layout`'s templates and gutter maths, the lettering policy layer over
`text::layout_with` (`Lettering.overflow_policy`, design §8.3), the
`comic.lettering.*` commands, and the reading-order model. Lettering must work with
`CRAFT_FONTS` empty and report a missing font rather than substituting silently.

**Exit criteria:** panels can be created from templates, reordered and resized with
validation warnings; every lettering kind renders, wraps, detects overflow and applies
its policy; moving a balloon preserves or updates its tail without detaching it; and
reading order is explicit, editable and validated independently of panel coordinates.
Lettering tests pass with `CRAFT_FONTS` empty. **Gate:** `cargo xtask ci`, plus
golden-render tests for one representative page.

---

## 9. Phase 3 — Story and storyboard workflow

**Goal:** Turn text into a structured production plan.

Create an AI story service that converts a prompt or manuscript into:

1. Story outline.
2. Chapter and scene breakdown.
3. Page allocation.
4. Panel descriptions.
5. Character appearances.
6. Dialogue and captions.
7. Camera framing and composition guidance.
8. Artwork generation instructions.

Every proposed change should be reviewable before it becomes authoritative project data.

The service should return structured, schema-validated data rather than free-form prose that the editor must parse.

Provide a storyboard interface where the user can:

- Review panels in sequence.
- Change the story or dialogue.
- Reorder panels.
- Adjust pacing.
- Approve or reject AI suggestions.
- Regenerate descriptions without losing approved content.

A revision to one scene should not silently overwrite unrelated pages.

**Exit criteria:** a story input produces schema-validated structured data with stable
ids or explicit id-mapping instructions — never prose the editor has to parse; every
proposal is reviewable and reversible before it becomes authoritative; a change to one
scene provably leaves unrelated pages untouched (asserted by comparing the bundles);
and regenerating descriptions preserves approved content. **Gate:** `cargo xtask ci`,
plus an id-stability test across regenerate-and-accept cycles.

---

## 10. Phase 4 — Character consistency and image generation

**Goal:** Generate panel artwork with traceable references and controlled revisions.

### Character bible

Store per-character:

- Canonical appearance description.
- Approved reference images.
- Face and body characteristics.
- Clothing and accessories.
- Palette and style constraints.
- Known variations across scenes.
- Explicit continuity exceptions.

Use immutable or versioned references so that a new character design does not unexpectedly alter previously approved pages.

### Image provider abstraction

Define a provider-neutral interface for:

- Text-to-image generation.
- Reference-conditioned generation.
- Image-to-image editing.
- Inpainting, when supported.
- Variation generation.
- Job status and error reporting.

Capabilities must be discovered or declared per provider. Do not pretend every model supports identical operations.

### Asset provenance

For every generated asset, retain:

- Provider and model identifier.
- Prompt and relevant generation parameters.
- Input reference asset IDs.
- Creation timestamp.
- Output asset ID.
- Parent or replaced asset ID.
- Approval status.

### Regeneration rules

Regenerating artwork must not delete or rewrite the panel's dialogue, balloon objects, page geometry, or reading-order information.

Keep the previous image available until the replacement is accepted.

Use explicit generation states such as queued, running, completed, failed, and awaiting review.

### The dependency that has to be decided here

The workspace has **no networking dependency** (audit §12.4), so this phase begins by
choosing and gating an HTTP client (ADR-015): MIT/Apache-class licence, a feature that
is off for wasm, unreachable from `crates/doc` and `crates/format`, and credentials
outside the bundle. Decide this before writing provider code, not after.

Jobs are **pollable**, not push-based, because MCP accepts and ignores progress and
cancellation notifications (`crates/mcp/src/server.rs:167-168`) — `comic.job.status`
is the interface, `comic.job.apply` reconciles against the document revision, and a
result that no longer matches is reported rather than applied (design §10.3).

**Exit criteria:** `comic.job.submit` returns a job id and never blocks; status,
cancellation and application are separate commands; `comic.job.apply` refuses to
overwrite a target changed since `target_revision` and reports the conflict; a failed
or cancelled job leaves the project and approved artwork untouched; every asset records
provider, model and request; and the whole project still opens, edits and exports with
no provider configured. **Gate:** `cargo xtask ci`, `cargo xtask wasm` with providers
disabled, and a test that a job result cannot overwrite newer approved state.

---

## 11. Phase 5 — Agent-native comic operations

**Goal:** Allow coding agents and creative agents to operate on semantic comic objects.

Extend the existing command registry and MCP integration rather than implementing an unrelated automation protocol.

The command set is finalized in [`comiccraft-design.md`](comiccraft-design.md) §10.1
under the namespace `comic.<group>.<verb>`, organized as `project`, `story`,
`character`, `page/panel`, `lettering`, `assets`, `generation` and `output`. The
earlier draft's `comic.balloon.*` and `comic.caption.*` names are replaced:
balloons, captions and sound effects are one `Lettering` entity, so they are one
command group. No `comic.*` id exists at the baseline, so there is no collision.

These commands are **new**, and the registry has no external registration hook, so they
are added as additive modules in `crates/engine/src/cmd/comic/*.rs` (ADR-014, §2.3).
That is the only route by which the GUI, the control channel, the CLI and MCP all reach
them through one path (`crates/engine/src/lib.rs:1071-1095`).

### Command requirements

Each command must:

- Accept validated structured parameters — the same schema as `CommandSpec.params`,
  validated by the engine's own helpers, returning `EngineError::BadParams` on bad
  input (`crates/engine/src/cmd/mod.rs:287-320`).
- Reference entities by stable id: comic ids, `NodeId`, `artboard_id`.
- Return structured results and actionable `EngineError`s.
- Respect document history and undo/redo, as one step per user action.
- Avoid destructive side effects by default.
- Be safe to retry where practical, and state clearly where it is not.
- Expose its schema to automation clients — it does so for free, since MCP's
  `command_list` and `tools/list` read the registry
  (`crates/mcp/src/tools.rs:650-741`).
- Have unit and integration tests, plus a malformed-input case in the command sweep.

Separate deterministic editing commands from expensive AI operations: a command that
changes dialogue is deterministic, and an image-generation request creates a tracked
job and returns a job id. Every command is `enabled`-gated on an open comic project.

**Exit criteria:** `mcp --headless` lists every comic command with its parameter
documentation; a deterministic command produces identical results when replayed; an
agent can complete M1's acceptance list end-to-end with no human interaction; and no
malformed parameter set can panic the command sweep. **Gate:** `cargo xtask ci`
including the fuzz targets.

---

## 12. Phase 6 — Comic studio UI

**Goal:** Make the system usable by creators who do not want to interact with an AI agent directly.

Adapt the existing UI incrementally.

### Workspace layout

Provide:

- Story/chapter navigation.
- Character and asset library.
- Page thumbnail navigator.
- Storyboard view.
- Main comic canvas.
- Context-sensitive panel properties.
- Lettering and typography controls.
- AI assistant and generation queue.
- Validation and publication panel.

### Editing behavior

The user must be able to select and edit a panel, balloon, caption, or image directly.

Agent actions and manual edits must operate on the same underlying document and semantic project state.

Show a clear distinction between:

- Proposed AI changes.
- Pending image generations.
- Approved artwork.
- User-modified objects.
- Validation warnings.

Do not require users to approve every small deterministic edit. Use approval gates for consequential changes, especially replacing approved artwork or changing story content.

### How this is implemented

Panels live inside `crates/ui-egui` (`panels/comic/`) and are registered through
`UI_COMMANDS` (`crates/ui-egui/src/menus.rs:1-20`), because egui/eframe are permitted
only at L6 (`xtask/src/layers.rs:88-90`). They read engine state and act only through
`app.run(id, params)`; no comic logic lives in a panel. The storyboard and page
navigator are new work — the baseline has document tabs and an artboard navigator but
no sequential view (audit §10) — and the storyboard's thumbnails come from
`Renderer::render_thumbnail(doc, NodeId, size)`.

The web build implements this phase partially: the deterministic surface only,
with panels that would need the file system or a provider hidden rather than broken
(design §14.1).

**Exit criteria:** every M1 operation is reachable without an agent; the panel,
balloon, caption and image selection behaviour works by direct manipulation; proposed
changes, pending generations, approved artwork, user modifications and validation
warnings are visually distinct; and a panel's edit and the equivalent command produce
the same document state. **Gate:** `cargo xtask ci` plus a UI check
(`--control` with `ui.screenshot`) for the new panels.

---

## 13. Phase 7 — Rendering and publishing

**Goal:** Produce reliable digital and print-ready output.

### Outputs

Prioritize:

- PNG page images.
- JPEG or WebP previews.
- Multi-page PDF.
- Web-comic page sequences.
- Contact sheets and review proofs.

Add EPUB or other publishing formats only after defining their requirements.

### Rendering pipeline

The publishing service should:

1. Validate the comic project.
2. Resolve required assets.
3. Check missing fonts and linked images.
4. Validate balloon and caption overflow.
5. Check page dimensions and bleed settings.
6. Render pages through the established rendering engine.
7. Assemble the requested output.
8. Verify output files and report errors.

Preserve editable project data independently of rendered output.

Support separate presets for screen, print, and proofing. Print-specific color management, bleed, trim, and resolution must be validated rather than assumed from screen rendering.

### What already exists

The pipeline's steps 2, 6, 7 and 8 are engine features: `pdf::export` writes one page
per artboard with trim = artboard, a bleed box grown by the bleed and a media box with
printer's marks, and `render::encode` writes PNG, JPEG, WebP, GIF, PNG8, TIFF, BMP,
TGA and PSD from a `ppi` or a scale (audit §9). Phase 7 builds the presets, the
validation gate in front of export, and the book-level assembly. This is also where
`File ▸ Package Project` (design §7.5) lands, with a determinism test.

**Exit criteria:** screen, print and proofing presets produce correct page geometry,
bleed and marks; a missing font or asset fails the export with an actionable error
instead of exporting silently wrong; the packaged archive round-trips (package, unpack,
reopen, compare); and editable project data is independent of every rendered output.
**Gate:** `cargo xtask ci` plus golden-render and PDF-geometry tests.

---

## 14. Phase 8 — Quality assurance and release

**Goal:** Make the application reliable for long-form comics.

### Automated testing

Cover:

- Project serialization and migration.
- Stable entity references.
- Panel layout and clipping.
- Balloon geometry and tail attachment.
- Text overflow.
- Reading order.
- Undo/redo.
- Agent command behavior.
- Save/reopen round trips.
- Image regeneration and asset replacement.
- PDF and image export.
- Missing assets and missing fonts.
- Large multi-page projects.

Use golden-render tests for representative pages.

### Manual validation

Create a representative test book containing:

- Several chapters.
- Multiple recurring characters.
- Different panel layouts.
- Long and short dialogue.
- Captions and sound effects.
- Irregular panels.
- Different page dimensions.
- At least one print export.

Verify the complete workflow with both a human and an agent.

### Release criteria

Do not declare production readiness until:

- No approved content is silently lost during edits.
- Saved projects reopen without structural loss, and every id survives the round trip.
- Panel and lettering operations are reversible.
- Exported pages match the intended layout.
- AI jobs can fail or be retried without corrupting the project.
- Licensing and asset provenance requirements are documented.
- The artboard-id hazard of design §5.3.1 has an automated test.
- The project opens, validates and exports with no provider configured and with
  `CRAFT_FONTS` empty.

**Build the representative test book from M1 onward, not here.** The three-panel page
is a unit test; a chapter with recurring characters, irregular panels, long dialogue
and a print export is what actually exercises the model. Growing that book one page per
milestone means a design flaw surfaces at the milestone that caused it, rather than at
the end of the project when it is expensive.

**Exit criteria:** the automated suite covers every item in the list above, including
failure paths (interrupted save, provider failure, cancelled job, missing asset,
missing font, deleted artboard); `cargo xtask ci` is green, including the fuzz,
layer, brand, asset and wasm gates; `vectorcraft-cli perf` shows no regression against
the baseline timings saved in M0; and the human and agent walkthroughs both complete
on the test book. **Gate:** the full release checklist in `docs/releasing.md`.

---

## 15. Implementation order

One milestone per phase, M0–M8. The previous table listed ten milestones against nine
phases, put the command API before the story workflow while the phases put it after,
and carried an "M3 Versioned comic project model" that no phase owned. All three are
fixed here.

| Milestone | Phase | Deliverable | Priority | Exit gate |
|---|---|---|---|---|
| M0 | 0 | Fork on the pinned baseline, frozen project schema, three crate skeletons registered in the layer table | P0 | `cargo xtask ci` green on the untouched baseline |
| M1 | 1 | Three-panel page: bundle, artboards as pages, panels, artwork, lettering, undo, save/reopen, PNG and PDF out | P0 | the twelve acceptance tests of Phase 1 |
| M2 | 2 | Panel engine, templates, gutters, lettering policy, reading order | P0 | lettering tests pass with `CRAFT_FONTS` empty; golden render |
| M3 | 3 | Story import to schema-validated storyboard, with review | P1 | id stability across regenerate-and-accept |
| M4 | 4 | Character bible, provider adapters, pollable jobs | P1 | no job can overwrite newer approved state |
| M5 | 5 | Completed agent surface: parameter docs, batch operations, MCP and CLI parity | P0 | `mcp --headless` completes M1's list unaided |
| M6 | 6 | Comic studio UI panels | P1 | M1's operations reachable without an agent |
| M7 | 7 | Rendering and publishing: presets, validation gate, package export | P1 | PDF page geometry and package round trip |
| M8 | 8 | Quality assurance, performance and release hardening | P1 | the release criteria of Phase 8 |

Two orderings are deliberate:

- **The schema and the first page are developed together, M0 → M1.** A data model is
  only tested by real editing behaviour, so the schema is frozen in M0 and put under
  load immediately in M1. The old "M3 Versioned comic project model" is gone because
  the bundle is built in M1 — there is no later step for it.
- **Commands ship with each feature; M5 completes the surface.** Every phase adds its
  own commands, because that is the only way to test them. M5 is therefore not "add
  commands" but "finish the agent surface": parameter documentation, batch operations,
  retry semantics, MCP and CLI end-to-end coverage, and `docs/comiccraft-command-api.md`.

**Parallel work.** `comic-model`, `comic-ai` and the UI panels are independent after
M1 and can proceed in parallel. `comic-layout` depends on `comic-model`. Every change
that touches `crates/engine/src/cmd/comic/` must be serialized through one coordinating
agent, because the registry is a shared surface. Give each parallel agent its own
`CARGO_TARGET_DIR` (`AGENTS.md:50`).

---

## 16. Scope boundaries

### Reuse — verified to exist (audit §11)

- Vector editing: `geom`, `pathops`, `doc` objects, groups, compound paths.
- Text and typography: `text` layout, wrapping, overflow, shrink-to-fit, fonts.
- Live, non-destructive effects: `effects`.
- Rendering and compositing: `render`, including clip groups, masks, blends.
- Document history and undo: `engine`.
- Existing file-format support: `format`, `svg`, `pdf`, and raster encoding.
- Command registry, control channel, MCP and CLI: `engine`, `ui-egui::control`, `mcp`,
  `vectorcraft-cli`.
- Established testing utilities: `testkit` golden renders, the CLI integration tests,
  the fuzz targets, and `vectorcraft-cli perf`.

### Build

- Comic project, story and asset schemas, with ids and migrations.
- Panel semantics, templates, gutters and reading order.
- Lettering semantics and the fitting/overflow policy over `text`.
- Storyboard and page navigation (none exists; the baseline has tabs and an artboard
  navigator only).
- Character continuity and asset provenance.
- AI generation orchestration, provider adapters and the job lifecycle.
- Comic-specific validation.
- Publishing presets and book assembly.

### Do not reuse

- `Document::unknown` and `Document::extra` are for round-tripping **foreign** data
  (`crates/doc/src/lib.rs:662-676`). They are not a place to store comic metadata.
- `crates/format` is not extended with a comic section (ADR-013).
- `Node.name` is not an identifier: it is user-editable and not unique.
- `crates/affinity`, `cad`, `eps` and `metafile` are unrelated importers/exporters;
  leave them alone.
- The ArtCraft brand assets in `docs/brand/` are trademarks and must not be reused;
  ComicCraft ships original branding.

### Defer

- Full screenplay editor.
- Animation and motion comics.
- Collaborative multi-user editing.
- Cloud asset synchronization.
- Advanced print-production automation.
- Complex automated page composition.
- Training or hosting custom image-generation models.

Do not expand scope until the initial page workflow is reliable.

---

## 17. Coding-agent execution rules

The coding agent must follow these rules:

1. Read `AGENTS.md` and the relevant crate documentation before making changes.
2. Inspect actual code and tests before assuming a feature exists. The baseline audit
   is the starting point, but it describes `3da4080b` and the fork will drift.
3. Write a short implementation plan before each milestone.
4. Keep changes small and scoped to the relevant crate.
5. Avoid broad refactors while implementing comic features.
6. Run targeted tests after each change.
7. Run the repository's required CI checks before completing a milestone.
8. Add tests for new commands and persistence behaviour.
9. Update documentation and the milestone's status alongside the implementation.
10. Report completed behaviour separately from planned or partially implemented behaviour.
11. Do not silently overwrite user content or approved assets.
12. Preserve the ability to merge upstream VectorCraft improvements.

### 17.1 Repository constraints that apply to every change

These are enforced, not advisory. `cargo xtask ci` runs `fmt`, `clippy`, `test`,
`assets`, `brands`, `layers` and `wasm`, stopping at the first failure
(`xtask/src/main.rs:175-205`).

- **No panics.** `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` and
  `unimplemented!` are denied outside tests, and `unsafe_code` is denied workspace-wide
  (`Cargo.toml:84-96`). New crates inherit the workspace lints — set
  `[lints] workspace = true` in each new `Cargo.toml`.
- **Register every new crate** in `xtask/src/layers.rs`, including any same-layer
  ordering it needs, or `cargo xtask layers` fails.
- **Never break wasm.** `cargo xtask wasm` is part of the gate, and provider code is
  feature-gated off for it.
- **No vendor names in user-visible text.** Command labels, parameter docs, panel text
  and MCP tool definitions are scanned by `cargo xtask brands`
  (`xtask/src/brands.rs:14-29`).
- **Every asset needs a row in `ASSETS.md`,** and no Adobe iconography or GPL/AGPL code
  may enter the repository (`AGENTS.md:14-34`).
- **Fonts go to `storytold/craft-fonts`, never here,** and everything must build and
  test with `CRAFT_FONTS` empty (`AGENTS.md:31`).
- **One task id per commit**, and the commit passes `cargo xtask ci`.
- **Parallel agents:** separate `CARGO_TARGET_DIR` each (`AGENTS.md:50`), and edit only
  the crates you own.

For parallel work, assign independent modules to separate agents. Keep shared schema
changes, command registration, and UI integration under one coordinating agent to avoid
conflicting edits — in this fork that means `crates/engine/src/cmd/comic/` is
single-writer.

`crates/comic-layout` carries three concerns, so it has three module-tree owners rather
than one crate owner: layout and templates, lettering and fitting, and publishing and
export. Its `lib.rs`, `Cargo.toml` and any shared type belong to the coordinating agent.
The concrete team — nine agents, their module trees and the surfaces they must not touch —
is defined in `.github/agents/` and `.github/instructions/comiccraft-work.instructions.md`;
that ownership table is the source of truth for who may edit what.

---

## 18. First task for the coding agent

**Task: stand up the fork and land M0. Do not start comic features yet.**

The audit and design are complete, so this is no longer a research task. Do exactly
this:

1. Create the `comiccraft` fork at `3da4080b`, add the upstream remote, create the
   baseline branch, and move `comiccraft/porting_plan_and_design/` into it.
2. Run `cargo xtask ci` on the untouched baseline and save the output with its timings.
3. Write `docs/comiccraft-project-schema.md`: the bundle schema from design §7.1 — the
   manifest, `project.json`, per-page files, the `NodeId`/`artboard_id` link rules and
   id allocation, migration rules, and a fixture per version. It must not contradict
   design §5.
4. Add `crates/comic-model`, `crates/comic-ai` and `crates/comic-layout` as empty,
   documented crates with `[lints] workspace = true`, registered in
   `xtask/src/layers.rs`, and confirm `cargo xtask ci` still passes.
5. Prove the M1 acceptance path by hand through commands (Phase 0, task 3) and record
   the result, including anything that behaved differently from the audit.

Then stop and report, without starting M1's implementation.

**Success condition:** a reviewer can approve M1 using the schema document, the
baseline CI report, and a demonstrated three-panel page exported to PDF through
commands — without needing to guess how the existing codebase works.
