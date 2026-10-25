# ComicCraft
## Porting Plan: VectorCraft → ComicCraft (Node.js + HTML/JavaScript)

**Status:** Revised for a lightweight implementation path
**Base repository:** `https://github.com/storytold/vectorcraft`
**Baseline:** `3da4080b` (`main`, clean tree) — read-only behavioural reference
**Target architecture:** `Node.js` backend + `client-side HTML/JavaScript` frontend — no framework, no bundler, no build step
**Primary objective:** Build a comic editor and AI-assisted production workflow that is fast to develop and run on a modest laptop, without carrying a heavy Rust toolchain or native desktop application requirement.
**Companions:** [`comiccraft-design.md`](comiccraft-design.md) · [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)

> **Why this stack.** The product is a document editor that must work offline, keep an undo
> history, and be drivable by agents through one command surface. None of that needs a
> framework: it needs a DOM, a canvas, and a Node process. So the frontend is plain HTML,
> CSS and ES modules the browser runs as-is, and the backend is a Node.js process that
> serves those files and exposes the command API. There is no transpile step, no bundler
> and no framework runtime — `git clone && node server.js` is the whole setup.

### What changed, and why (three revisions)

| Revision | Stack | Why it changed |
|---|---|---|
| v1.0 | Rust workspace — 3 new crates on the VectorCraft fork, `cargo xtask ci` gate | The original plan. Sound architecture, but a ~1.95 MSRV toolchain, a rustup install, a `target/` tree and a full CI run per iteration |
| v1.1 | TypeScript + Next.js + React | React and a bundler were assumed necessary for an editor UI |
| **v1.2 (this)** | **Node.js backend + vanilla HTML/JS frontend** | A canvas editor is mostly imperative drawing and hit-testing, which is what a framework makes *harder*, not easier. Dropping React/Next removes the bundler, the JSX transform, the hydration model and the server/client boundary, and leaves one executable artifact |

The architecture — the domain model, the command surface, the bundle format, the validation
gate and the milestone sequence — is unchanged across all three. Only the runtime changed.

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

**The reference implementation (VectorCraft) is the authority on how vector editing and
document rendering behave. ComicCraft owns comic semantics, narrative structure, AI
orchestration, and publishing workflows.**

The reference tree is read, not imported: it answers *"how should this behave?"* and
supplies the bundle format. No ComicCraft code links against it, and no build step compiles
it.

---

## 2. Repository strategy

### 2.1 Replace the Rust-heavy baseline with a Node.js + vanilla-JS implementation

The original VectorCraft-based plan was useful as an architectural reference, but the
execution model is now intentionally lighter: a Node.js backend and a framework-free
browser client, sharing one plain-JavaScript domain model, instead of a multi-crate Rust
workspace.

The shape of the repository follows from that:

- `server/` — the Node.js process: HTTP API, static file serving, command execution,
  background jobs, bundle read/write, PDF and image export.
- `public/` — the client, exactly as it runs in the browser: `index.html`, CSS, and ES
  modules. No JSX, no transpile, no bundle.
- `shared/` — the domain model, command definitions and validation **imported unchanged by
  both sides**.
- `schema/` — the versioned JSON Schemas and a fixture per version.
- `test/` — `node --test` suites.
- `reference/vectorcraft/` — the audited Rust baseline, read-only.

**Why sharing matters here.** The same `shared/` module that validates a bundle on the
server also runs in the browser to validate before saving. That is one implementation of
the rules, not two, and it is only possible because neither side needs a build step or a
framework transform to consume it.

§4 gives the authoritative structure and the dependency direction.

This is a port rather than a fork of the Rust workspace. The goal is a faster, more
maintainable implementation that still preserves the product requirements and the
editorial workflow from the original design.

### 2.2 Maintain product intent while simplifying the stack

The original architecture goals remain valid:

- Keep the static marketing site on Cloudflare Pages.
- Run the editor and API on a separate subdomain (`app.comiclab.io.vn`).
- Keep local-first editing and document state management simple enough to run on a normal
  laptop.
- One language, one runtime, one dependency tree — JavaScript on Node.js.

The project no longer tries to inherit a Rust-native engine, a framework, or a bundler as
the initial implementation path. Concretely, the deliberate omissions are:

| Not used | Because |
|---|---|
| Rust / cargo | Toolchain weight; a native build for every change |
| React, Vue, Svelte | The editor is imperative canvas and DOM work; a virtual DOM adds a transform and a rendering model to fight |
| Next.js | Its value is SSR and routing; this app is a client-side editor and an API |
| Webpack, Vite, esbuild | Browsers run ES modules natively; a bundler buys nothing at this size |
| TypeScript build step | Types are *checked* via JSDoc (`tsc --checkJs`) and never emitted — see §17.1 |
| A database (for v1) | A comic project is a directory bundle; a filesystem is enough until a feature needs otherwise |

### 2.3 Avoid a premature mass rename

Do not rename every symbol in the first milestone.

First establish a working ComicCraft product on the new stack. Rename user-facing branding,
application metadata, project templates and documentation incrementally.

Internal package renaming should occur only when it provides a concrete maintenance or
distribution benefit.

### 2.4 The Rust baseline is a reference, and the record of M0–M2 stands

The earlier Rust milestones are **not deleted and not invalidated**. They are the evidence
behind several claims this plan still relies on, and they are the behavioural reference for
the port:

| Rust milestone | What it produced | Status | What the new stack takes from it |
|---|---|---|---|
| M0 (`86bd47ef`) | The frozen bundle schema v1, the three comic crate skeletons, the baseline CI gate, the M1 hand-proof | **Complete** | The bundle shape, the id-link rules, the migration policy — i.e. `docs/comiccraft-project-schema.md` becomes the input to `schema/` |
| M1 (`b68150ba`) | Comic page vertical slice: bundle, artboards as pages, panels, artwork, lettering, undo, save/reopen, PNG and PDF out; 12 acceptance tests | **Complete** | The acceptance criteria of the vertical slice are re-run as the new slice's definition of done |
| M2 | Panel templates and validation warnings, all lettering kinds with overflow policy, tail preservation on a move, explicit reading order, the `CRAFT_FONTS`-empty contract, the golden render | **Features landed, milestone not closed** — five review records incomplete and the gate not re-run on a quiescent tree | The layout, lettering, reading-order and validation *rules* — these are pure logic and port directly |

**What this means for the plan.** The milestone documents under `docs/milestones/` stay as
the historical record of the Rust implementation and keep their commit hashes. Nothing in
the new plan depends on `cargo`, on the Rust toolchain, or on that tree compiling. The Rust
workspace is retained read-only so that questions of the form *"does the reference
implementation do X?"* remain answerable — that is its whole remaining job.

**The build cache was deleted, deliberately (2026-10-18).** `comiccraft/target/` held
**157 GB** — 43% of everything used on the development machine — and `craft-fonts/target/`
another 231 MB. Both were cargo build output, both were gitignored, and both were removed.

Two consequences, stated so nobody rediscovers them as surprises:

1. **`cargo xtask ci` on the reference tree is no longer cheap.** It will not fail, but it
   will rebuild the whole workspace, which took hours rather than minutes. Nothing in the
   plan asks for it — M0 step 3 now says the reference gate result is *already recorded* in
   `docs/ci-baseline-3da4080b.md` and must not be re-run. M2's one open item ("the gate has
   not been re-run on a quiescent tree") is therefore **closed by decision rather than by
   execution**: M2 stays unclosed as history, and its rules port to the new M2 regardless.
2. **The evidence survived.** `target/` also held 58 gate, mutation-test and MCP-transcript
   logs that the review records and `docs/ci-m2-w3.md` cite by path — 3.5 MB in total. They
   were kept **at their exact cited paths**, so every reference still resolves, and
   `target/mut-r2/mut.py` — the mutation tool a review record names as a reproduction
   step — was kept with them. Only regenerable output was removed.

   **Those files are now the fragile part**, because they sit in a directory everyone
   expects to be disposable: a `cargo clean` would erase them silently.
   `target/README.md` says so at the point of danger, and the runs it lists are the only
   raw record of their gate and mutation results. Two cited paths are missing and were
   missing before this cleanup — `target/review-bak/` and `target/r2-baseline.sha256` —
   so nothing above is incomplete because of it.

**Why the pivot happened.** The Rust workspace requires a toolchain the developer's laptop
cannot carry comfortably (the M0 record shows a ~1.95 MSRV, a rustup install, and a full
`cargo xtask ci` per iteration), and it puts the product behind a native build for every
change. The product goals — editable AI output, semantic comic state, agent-human parity —
were never Rust-specific. They are expressed as a typed command surface, a versioned JSON
bundle and a validation gate; all three are language-neutral, so they move.

---

## 3. Target architecture

Two runtimes, one language, one shared domain:

```text
┌─ Browser ────────────────────────────────┐   ┌─ Node.js ────────────────────────────┐
│ public/  index.html + ES modules         │   │ server/  HTTP API + static serving   │
│   editor/  canvas, hit-test, undo stack  │◄──┤   commands/  registry + dispatcher   │
│   ui/      panels, story navigator       │   │   jobs/      generation, render, exp │
│   storage/ IndexedDB + local bundle      │   │   storage/   bundle read/write, R2   │
└──────────────┬───────────────────────────┘   └──────────────┬───────────────────────┘
               │            both import ↓                       │
               └────────────► shared/ ◄──────────────────────────┘
                       domain model · commands · validation · schema
```

`shared/` is the load-bearing idea: the same modules validate a project in the browser
before saving and on the server before writing, so a rule is implemented once. This works
because nothing in `shared/` is transpiled, polyfilled or framework-bound — it is ES
modules with no imports outside `shared/`.

### Layer A — The client (`public/`)

Plain HTML, CSS and ES modules. It owns:

- the story navigator and chapter management,
- the page canvas (Canvas 2D for rendering, SVG where vector output is the product),
- hit-testing, selection, transform handles and the undo stack,
- panel placement and balloon editing,
- the review queue and AI suggestion surfacing,
- publishing and export controls.

It holds **no authoritative business logic**: it calls commands, renders the result, and
keeps view state (zoom, scroll, selection) that is deliberately not persisted. View state
must never leak into the bundle — that rule is inherited from the reference design, where
selection was session state rather than document state.

### Layer B — The domain model (`shared/domain/`)

The authoritative project model, as plain JavaScript with JSDoc types:

- `ComicProject`, `Chapter`, `Scene`, `Page`, `Panel`, `Dialogue`, `Lettering`
- stable ids and schema versioning,
- narrative ordering and continuity rules,
- panel layout maths, templates and reading order,
- lettering fitting and overflow policy,
- asset metadata and provenance,
- serialization and migration.

It is pure: no DOM, no `fetch`, no `node:fs`. Everything it needs (a text-measurement
function, a clock, an id generator) is injected by the caller, which is what makes it
testable in `node --test` and identical on both sides.

### Layer C — AI orchestration and jobs (`server/jobs/`)

Server-side only, because provider keys must never reach the browser:

- story analysis and storyboard planning,
- prompt orchestration and provider adapters,
- the job state machine (submit, poll, succeed, fail, supersede),
- continuity checks and assisted validation,
- the review/approval flow.

Providers sit behind adapters so a provider change never touches the domain model.

### Layer D — Persistence, rendering and export (`server/storage/`)

- bundle read/write to the filesystem in development, and to object storage in deployment,
- asset storage and hashing,
- canvas rendering for thumbnails and page images (via a headless canvas),
- PDF assembly for print output,
- schema migration on open, and validation before save,
- export packaging.

The bundle format is the reference design's: a directory holding a manifest, the comic
project, story files, per-page comic files, one visual document, the asset registry,
provenance and job metadata.

### Layer E — The command surface, and the one route through it

Every comic operation is a **command** with a stable id, a declared parameter schema and a
JSON result. Commands are defined once in `shared/commands/` and executed through one
dispatcher, exposed on three transports:

- **in-process** in the browser, for the UI's own actions;
- **HTTP** (`POST /api/comic/<command>`) for anything the browser delegates to the server;
- **MCP** for agents and CLIs.

A command applies domain changes and produces **one** document mutation, applied inside a
single undoable transaction (design §11.2). No command calls back into the UI — the arrow
points one way.

**Why the same code path matters.** A click, an agent call over MCP and a `curl` all
produce byte-identical results, because all three reach the same dispatcher and the same
domain functions. The earlier Rust plan achieved this with one in-process registry and two
adapters; here it falls out of the fact that the browser and the server can run the same
module.

**Layering rule:** `shared/` must not import from `public/` or `server/`, and must not
reference `document`, `window`, `fetch`, `process` or `node:*`. The client must not import
from `server/`. These are enforced by an ESLint boundary rule plus a unit test that asserts
`shared/` has no forbidden global reference — a rule that only lives in a document is the
thing this section replaces.

---

## 4. Workspace structure

One repository, one language, no build step. Directories are used where a package manager
would normally be: a folder is a boundary only if its import rules differ.

```text
comiccraft/
├── server.js                     # the entry point — `node server.js`
├── server/
│   ├── http.js                   # routes: static, /api/comic/<command>, /api/jobs/<id>
│   ├── commands/                 # server-only command handlers (fs, export, jobs)
│   ├── jobs/                     # generation queue, render and export runners
│   ├── storage/                  # bundle read/write, assets, object storage adapter
│   ├── export/                   # PDF assembly, raster encode, package archive
│   └── ai/                       # provider adapters + prompt builders (keys live here)
├── public/
│   ├── index.html                # the only HTML page; the app is a client-side editor
│   ├── app.js                    # boot: load bundle, mount the editor, wire commands
│   ├── editor/                   # canvas render, hit-test, selection, undo stack
│   ├── ui/                       # story navigator, panels, dialogs, review queue
│   ├── storage/                  # IndexedDB + local file handles
│   └── styles/                   # CSS, no preprocessor
├── shared/
│   ├── domain/                   # AUTHORITATIVE MODEL — the rules, pure JavaScript
│   ├── commands/                 # command definitions + parameter schemas + the dispatcher
│   ├── validate/                 # validators, used before save on either side
│   └── ids.js                    # id allocation, shared by both runtimes
├── schema/                       # JSON Schema per bundle version + a fixture each
├── test/                         # `node --test` suites
├── assets/                       # ComicCraft's own brand assets
├── reference/vectorcraft/        # the audited Rust baseline — read-only (see §2.4)
├── docs/
│   ├── comiccraft-porting-audit.md
│   ├── comiccraft-project-schema.md
│   ├── comiccraft-command-api.md
│   ├── comiccraft-ai-workflow.md
│   └── comiccraft-publishing.md
└── package.json                  # dependencies: fastify only, for v1
```

**Dependency direction.** Strictly downward, and there are only three rules:

```text
public/  ──► shared/        (imported by the browser over HTTP)
server/  ──► shared/        (imported by Node by relative path)
public/  ──✗  server/       (never — the reverse edge does not exist)
shared/  ──✗  public/, server/, node:*, DOM globals
```

`shared/` is the root. `server/` and `public/` are siblings that never import each other;
anything they both need lives in `shared/`.

**Enforcement, not convention.** Three mechanisms, all in `npm run verify`:

1. An ESLint `no-restricted-imports` rule forbids `public/` importing `server/` and forbids
   `shared/` importing either.
2. A unit test walks `shared/` and fails on a forbidden global (`document`, `window`,
   `process`, `fetch`) or a `node:` import. Textual rules are checked by running them.
3. A test asserts every module in `shared/` is importable in a bare Node context with no
   DOM — which is the same thing the browser requires, expressed the other way around.

**The dependency list is deliberately one line.** `fastify` for HTTP, and nothing else at
runtime. Everything else — the test runner, the argument parser, the file watcher, the
crypto, the JSON handling — is in Node's standard library. New dependencies are a
deliberate decision recorded in the commit, not an accident.

**The Rust baseline is a reference, not a dependency.** `reference/vectorcraft/` holds the
audited baseline (`3da4080b`) for reading only: its behaviour, its file format and the
reuse report are cited throughout this plan, but no module imports it, no build step
compiles it, and it is excluded from every toolchain command. It exists so that "does the
baseline behave this way?" stays an answerable question.

---

## 5. Define the canonical comic project model

This is the most important architectural task, and the schemas are now fixed in
[`comiccraft-design.md`](comiccraft-design.md) §5. This section states the model and
the invariants; the design document states the fields.

> **Reading note for the new stack.** Sections 5–14 were written against the Rust
> reference implementation and still cite it by path (`crates/doc/src/node.rs`,
> `crates/format`) where a claim was verified against real code. Read those citations as
> *evidence for the rule*, not as a build dependency: the rules live in `shared/domain/`,
> and `docs/comiccraft-project-schema.md` — the M0 deliverable — is the frozen input they
> are ported from.

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

- A stable id that is never reused. Ids are strings with a type prefix (`page_`, `panel_`,
  `balloon_`), allocated from a counter persisted in the manifest. Document node ids stay
  **numeric** so a bundle round-trips through the reference format without renumbering.
  Ids are never random and never derived from an array index.
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
files, one visual document (artboards = pages), the asset registry,
provenance and durable job metadata (ADR-012). The visual document keeps the reference
format's shape — it is read and written by `shared/domain/format/` — while the comic schema
is versioned separately in the manifest and never overloads that format (ADR-013). Saving is
ordered so the manifest is written last and never references partially written data, and
every schema has an identified version with fixtures for each previous version.

The same bundle is the unit of storage on both sides: in the browser it is an in-memory
object persisted to IndexedDB or downloaded as a folder, and on the server it is a
directory. The module that serializes it is the same one in both cases, so a bundle saved
locally opens on the server unchanged — and there is no separate "export format" to keep in
sync.

The earlier draft's "prefer a versioned container" wording is superseded by that
decision; a single-file archive is an export (`File ▸ Package Project`), specified in
design §7.5 and delivered in Phase 7, not the working format.

---

## 6. Phase 0 — Baseline (M0)

**Goal:** Stand up the repository on the new stack and freeze the project schema.

**Status: the audit half is done.** [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)
*is* the reference architecture report, the dependency and API map and the comic
feature gap analysis, with `path:line` citations for every claim. The original tasks 1,
2, 3, 4 and 8 are complete against the reference implementation; what remains is porting
work with a known target, not investigation.

> **Status and gate conventions for §6–§14.** These sections are the behavioural
> specification of the product, written when the reference implementation *was* the
> implementation. Read them as follows:
>
> - **Behaviour, exit criteria and acceptance tests are normative** and carry over
>   unchanged.
> - **Where the work lands** is now `shared/`, `public/` or `server/` per §4, not a crate.
> - **Every gate that says `cargo xtask ci`, `cargo xtask layers`, `cargo xtask wasm` or
>   `cargo xtask brands` is `npm run verify`**, except the single M0 step below that runs
>   the reference gate to record a baseline.
> - **Every `Status:` line describing the Rust fork's progress is historical**, and is
>   summarised in §2.4 instead. What M0–M2 achieved in Rust is the target the port must
>   reach, not progress this plan has already banked.
> - **`crates/*` citations** are evidence for a rule, per §5's reading note.

Remaining tasks:

1. Create the repository layout of §4 and confirm `node server.js` serves `public/`.
2. **Port the schema, then prove it.** Move `docs/comiccraft-project-schema.md` into
   `schema/v1/` as JSON Schema plus one fixture per entity, and implement
   `shared/domain/validate.js` against it — this is task 5 below, promoted, because
   everything else is written against it.
3. Record the *reference* baseline once, for comparison: the Rust tree's gate result is
   already saved in `docs/ci-baseline-3da4080b.md`. Do not re-run it — it is evidence,
   not a step.
4. Prove M1's acceptance path by hand against the reference, then re-prove it in the new
   stack: `artboard.new`, place an image (crop with `object.cropImage`, clip with the
   clip-rectangle command), add an area text object, `document.serialize`,
   `document.export`, and the PDF export of all artboards. Confirm that cropping,
   clipping, typography and PDF page geometry behave as the audit §7–§9 says they do.
   Then repeat each step through `shared/commands` and record any difference as a finding.
5. Make `npm run verify` green: type check (JSDoc, no emit), lint including the
   `shared/` boundary rule, the layering unit test, `node --test`, and the fixture
   validation.

Deliverables:

- `docs/comiccraft-porting-audit.md` — **done** (the reference audit).
- `schema/v1/*.schema.json` and a fixture per entity — the frozen initial project schema.
- The reference baseline gate report, already recorded (`docs/ci-baseline-3da4080b.md`).
- `shared/domain/validate.js` plus its tests, and a green `npm run verify`.

**Exit criteria:** the repository installs with `npm install` and runs with `node server.js`
on a machine with no Rust toolchain; the schema is approved and frozen for M1; and a
three-panel page can be created and exported by hand through commands, with the output
checked against the audit.

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

**Exit criteria:** every test above passes; `npm run verify` is green including the
boundary, brand, asset and fixture gates; and the reuse report of §18 has been produced.

**Gate:** `npm run verify`. **Do not start AI image-generation integration until this
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
`shared/domain/layout`'s templates and gutter maths, the lettering policy layer over the
injected text-measurement function (`Lettering.overflow_policy`, design §8.3), the
`comic.lettering.*` commands, and the reading-order model. Lettering must work with no
fonts configured and report the missing font rather than substituting silently.

**Exit criteria:** panels can be created from templates, reordered and resized with
validation warnings; every lettering kind renders, wraps, detects overflow and applies
its policy; moving a balloon preserves or updates its tail without detaching it; and
reading order is explicit, editable and validated independently of panel coordinates.
Lettering tests pass with no fonts configured. **Gate:** `npm run verify`, plus
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
and regenerating descriptions preserves approved content. **Gate:** `npm run verify`,
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

The stack deliberately has **no AI provider dependency** at M0, so this phase begins by
choosing one (ADR-015): a permissive-licence client library, called **only** from
`server/ai/`, never reachable from `shared/` or `public/`, and with credentials read from
the environment and never written into the bundle. Decide this before writing provider
code, not after. Provider code must not run in the browser — keys must never reach it —
which the boundary test enforces by forbidding `fetch` and provider SDKs in `shared/`.

Jobs are **pollable**, not push-based: MCP accepts and ignores progress and
cancellation notifications, so `comic.job.status` is the interface, `comic.job.apply`
reconciles against the document revision, and a result that no longer matches is
reported rather than applied (design §10.3).

**Exit criteria:** `comic.job.submit` returns a job id and never blocks; status,
cancellation and application are separate commands; `comic.job.apply` refuses to
overwrite a target changed since `target_revision` and reports the conflict; a failed
or cancelled job leaves the project and approved artwork untouched; every asset records
provider, model and request; and the whole project still opens, edits and exports with
no provider configured. **Gate:** `npm run verify`, with providers
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

These commands are **new**. They are defined once in `shared/commands/` and registered in
one place, so the browser UI, the HTTP API and MCP all reach them through one dispatcher
(§3 Layer E). This is the same property the reference achieved with a single in-process
registry — the difference is that here it needs no divergence from anything, because a
registry in a shared module is the natural shape rather than an addition to a fixed list.

### Command requirements

Each command must:

- Accept validated structured parameters, checked against the command's declared schema
  before the handler runs, returning a typed `BadParams` failure rather than throwing.
  The parameter schema is the same object the UI and MCP read.
- Reference entities by stable id: comic ids, `NodeId`, `artboard_id`.
- Return a structured result or a typed, actionable error.
- Respect document history and undo/redo, as one step per user action.
- Avoid destructive side effects by default.
- Be safe to retry where practical, and state clearly where it is not.
- Expose its schema to automation clients — it does so for free, because MCP's
  `tools/list` and the HTTP route are both generated from the same registry.
- Have unit and integration tests, plus a malformed-input case in the command sweep.

Separate deterministic editing commands from expensive AI operations: a command that
changes dialogue is deterministic, and an image-generation request creates a tracked
job and returns a job id. Every command is `enabled`-gated on an open comic project.

**Exit criteria:** `mcp --headless` lists every comic command with its parameter
documentation; a deterministic command produces identical results when replayed; an
agent can complete M1's acceptance list end-to-end with no human interaction; and a
malformed parameter set must not crash the dispatcher. **Gate:** `npm run verify`,
including the property tests that feed malformed parameters to every registered command.

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

Studio panels live in `public/ui/` and are mounted by `public/app.js`. They read project
state from the store and act **only** by dispatching commands through `shared/commands`;
no comic logic lives in a panel. This is the same rule the reference enforced by having
its panels call `app.run(id, params)` — the enforcement mechanism differs (here: a lint
rule and a review check that no panel imports `shared/domain` internals directly), the
rule does not. The storyboard and page
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
the same document state. **Gate:** `npm run verify` plus a UI check
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
**Gate:** `npm run verify` plus golden-render and PDF-geometry tests.

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
- The project opens, validates and exports with no provider configured and with no
  fonts installed.

**Build the representative test book from M1 onward, not here.** The three-panel page
is a unit test; a chapter with recurring characters, irregular panels, long dialogue
and a print export is what actually exercises the model. Growing that book one page per
milestone means a design flaw surfaces at the milestone that caused it, rather than at
the end of the project when it is expensive.

**Exit criteria:** the automated suite covers every item in the list above, including
failure paths (interrupted save, provider failure, cancelled job, missing asset,
missing font, deleted artboard); `npm run verify` is green, including the boundary,
brand, asset and fixture gates; the page-load and render timings show no regression
against the numbers recorded at M1; and the human and agent walkthroughs both complete
on the test book. **Gate:** the full release checklist in `docs/releasing.md`.

---

## 15. Implementation order

One milestone per phase, M0–M8, expressed for the Node.js + vanilla-JS stack. The phase
*content* is unchanged from §6–§14 — those sections remain the behavioural specification —
but every deliverable below names where it lands and a gate that runs on a laptop in
seconds.

| Milestone | Phase | Deliverable | Lands in | Priority | State | Exit gate |
|---|---|---|---|---|---|---|
| M0 | 0 | Repo skeleton, `shared/domain` conversion of the frozen schema, `schema/` with JSON Schema v1 + a fixture per entity, `npm run verify` | `shared/`, `schema/` | P0 | **Done** | `npm run verify` green — **3 s** |
| M1 | 1 | Three-panel page vertical slice: bundle open/save, artboards as pages, panels, artwork, lettering, undo, PNG and PDF out | `shared/`, `public/`, `server/` | P0 | **Done** | the twelve acceptance tests of Phase 1, passing in `node --test` |
| M2 | 2 | Panel engine, templates, gutters, lettering policy, reading order | `shared/domain/` | P0 | **Rules done and tested**; the extended panel tools are not | lettering policy tests pass with no fonts configured; the golden render matches its fixture |
| M3 | 3 | Story import to schema-validated storyboard, with review | `shared/`, `server/ai`, `public/ui` | P1 | Not started | id stability across regenerate-and-accept |
| M4 | 4 | Character bible, provider adapters, pollable jobs | `server/ai`, `server/jobs` | P1 | Not started | no job can overwrite newer approved state |
| M5 | 5 | Completed agent surface: parameter docs, batch operations, MCP parity | `shared/commands`, `server/` | P0 | Not started — the registry, HTTP and CLI are in | the MCP server completes M1's operation list unaided |
| M6 | 6 | Studio UI complete: story navigator, storyboard, canvas tools, review queue | `public/` | P1 | Editor, navigator, inspector and reading order are in | M1's operations are reachable without an agent |
| M7 | 7 | Rendering and publishing: presets, validation gate, package export | `server/export` | P1 | SVG, PNG and PDF out are in | PDF page geometry and package round trip |
| M8 | 8 | Quality assurance, performance and release hardening | all | P1 | Not started | the release criteria of Phase 8 |

**M0 and M1 are complete; the report is
[`docs/milestones/M0-M1-node.md`](milestones/M0-M1-node.md).** It records the gate output
verbatim, the nine defects the work found and what each one says, and what is deliberately not
done — including that `public/` is outside the strict type check and why.

Three orderings are deliberate:

- **The schema and the first page are developed together, M0 → M1.** A data model is only
  tested by real editing behaviour, so the schema is frozen in M0 and put under load
  immediately in M1. There is no separate "versioned project model" milestone because the
  bundle is built in M1.
- **Commands ship with each feature; M5 completes the surface.** Every phase adds its own
  commands, because that is the only way to test them. M5 is therefore not "add commands"
  but "finish the agent surface": parameter documentation, batch operations, retry
  semantics, MCP end-to-end coverage, and `docs/comiccraft-command-api.md`.
- **M6 is not the only place a human can work.** In the Rust plan the UI was a late
  milestone layered on a headless engine, so an agent was the only way to exercise M1. Here
  M1 already ships a working page in the browser and M6 completes the surrounding studio.
  This remains the biggest scheduling difference from v1.0.

**What the Rust milestones contribute to sequencing.** M0–M2 in Rust are done (M2 has
unfinished review records, listed in `docs/milestones/M2.md` §8). Their rules port
directly, so the new M0–M2 are *port* milestones with a known target rather than design
milestones — the risk that remains is porting fidelity, not design uncertainty.

**One milestone report per port, not per stack.** The reports are dated and name the stack they
describe, because the same milestone number means two different implementations: `M0.md` and
`M1.md` are the Rust record, `M0-M1-node.md` is this one.

**Parallel work.** After M1 the surfaces are independent: story/AI (`server/ai`, M3–M4),
the agent surface (`shared/commands`, M5), the studio UI (`public/`, M6) and publishing
(`server/export`, M7). `shared/domain` is the shared surface and is single-writer.
Because there is no build step there is no compiled artifact to race on: two agents running
tests in the same checkout cannot invalidate each other's output, so no per-agent cache
directory is needed.

---

## 16. Scope boundaries

### Behaviour the reference implementation already exhibits (audit §11)

These capabilities were verified to exist in the audited baseline. The new stack
does not inherit the code, but it inherits the *behaviour* — and where a behaviour is
subtle (text wrapping, blend modes, PDF geometry), the reference tree is the tie-breaker.

- Vector editing: paths, path boolean operations, objects, groups, compound paths.
- Text and typography: layout, wrapping, overflow, shrink-to-fit, font handling.
- Live, non-destructive effects.
- Rendering and compositing, including clip groups, masks, blends.
- Document history and undo.
- File-format support: SVG, PDF, raster encoding.
- A command registry, a control channel, MCP and a CLI.
- Golden-render testing.

### Re-implemented in the new stack

Each row names what replaces the reference crate. This is the port backlog, and the
`new code?` column is the honest answer to "how much of this is genuinely new".

| Behaviour | Reference | New home | New code? |
|---|---|---|---|
| Path geometry, transforms | `geom`, `pathops` | `shared/domain/geom` — explicit matrix maths; SVG path data for interchange | Maths is new but conventional; `Path2D` supplies curves |
| Document model, groups | `doc` | `shared/domain/document` | New; the node/link rules come from `comiccraft-project-schema.md` |
| Text layout, wrapping, overflow | `text` | `shared/domain/lettering` over an **injected** measure function | New logic, no font files; the browser supplies metrics, tests supply a stub |
| Effects, blends | `effects` | canvas 2D composites and SVG filters; the effect set is deliberately smaller | Reduced scope, recorded as a known gap |
| Rendering, compositing | `render` | canvas 2D in `public/editor`; headless canvas in `server/export` | New |
| Undo/redo, history | `engine` | `public/editor/history.js` — command-pattern transactions | New, small |
| SVG, PDF, raster out | `svg`, `pdf`, `render::encode` | browser SVG serialization + `server/export` for print PDF via a headless canvas | New |
| Command registry | `engine` | `shared/commands` | New, small — see §3 Layer E |
| MCP, CLI | `mcp`, `vectorcraft-cli` | an MCP server over the same command definitions | New, thin |

### Build

- Comic project, story and asset schemas, with ids and migrations.
- Panel semantics, templates, gutters and reading order.
- Lettering semantics and the fitting/overflow policy.
- Storyboard and page navigation.
- Character continuity and asset provenance.
- AI generation orchestration, provider adapters and the job lifecycle.
- Comic-specific validation.
- Publishing presets and book assembly.

### Do not carry over

- The reference tree's `Document.unknown` / `Document.extra` round-trip pockets are for
  **foreign** data. They are not a place to store comic metadata.
- The reference format must not be extended with a comic section (ADR-013). The comic
  schema is versioned in the manifest.
- A node's `name` is not an identifier: it is user-editable and not unique.
- The ArtCraft brand assets in `docs/brand/` are trademarks and must not be reused;
  ComicCraft ships original branding. The `craft-fonts` repository stays the font home,
  and no font file enters this repository — see §17.1.
- The Rust workspace is not a build dependency and is never added to a `package.json`.

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

1. Read `AGENTS.md` (ComicCraft's contract) and the relevant directory's documentation
   before making changes. The reference tree's own contract is
   `docs/reference/vectorcraft-agents.md` and does **not** apply to ComicCraft work.
2. Inspect the reference implementation in `reference/vectorcraft/` before assuming a
   behaviour exists, and inspect the JavaScript before assuming it is implemented. The
   baseline audit describes `3da4080b` and its claims are about Rust code.
3. Write a short implementation plan before each milestone.
4. Keep changes small and scoped to one directory.
5. Avoid broad refactors while implementing comic features.
6. Run targeted tests after each change (`node --test test/<file>`).
7. Run `npm run verify` before completing a milestone.
8. Add tests for new commands and persistence behaviour.
9. Update documentation and the milestone's status alongside the implementation.
10. Report completed behaviour separately from planned or partially implemented behaviour.
11. Do not silently overwrite user content or approved assets.

### 17.1 Repository constraints that apply to every change

These are enforced by tooling, not advisory. `npm run verify` runs, in order: the type
check, ESLint, the layering test, the unit tests, the schema-fixture validation, and a
smoke test that boots the server and serves `index.html`. It stops at the first failure.

- **JSDoc types are checked, never emitted.** `tsc --checkJs --noEmit` runs over `shared/`,
  `server/` and `public/` with `strict` on, plus `noUncheckedIndexedAccess` and
  `exactOptionalPropertyTypes`. There is no build output: the browser and Node both run the
  source. A type error is a failed gate, not a warning.
- **No `any` and no silent coercion.** `@typescript-eslint/no-explicit-any` fails lint,
  `@ts-ignore` is banned (use `@ts-expect-error` with a reason), and a domain function that
  can fail returns a typed `Result` — it does not throw and it does not return `undefined`
  to mean "invalid".
- **Layer boundaries are lint rules and a test.** `shared/` may not import from `public/`,
  `server/`, a `node:` builtin, or reference `document`, `window`, `process` or `fetch`; the
  client may not import from `server/`. A unit test asserts this by walking the tree, so a
  new file cannot quietly break the rule that makes the shared domain shareable.
- **Every bundle schema version has a fixture, and the fixtures are validated.** A schema
  change without a migration and a fixture fails `verify`. This is the load-bearing gate of
  the repository, inherited from the reference's per-version fixture rule.
- **No vendor names in user-visible text.** Command labels, parameter documentation, UI
  copy and MCP tool definitions are scanned by `npm run check:brands`.
- **Every asset has a row in `ASSETS.md`** and no third-party iconography or GPL/AGPL code
  enters the repository.
- **No font files in this repository.** Fonts live in the `craft-fonts` repository and are
  referenced by URL. Every lettering test must pass with **no fonts configured**: the
  measurement function is injected, so tests supply a deterministic stub and the browser
  supplies real metrics. A test that needs a font installed is a broken test.
- **One runtime dependency.** `fastify`, and nothing else, without a recorded decision.
  A new production dependency is a commit that states what it replaces from the standard
  library.
- **One task id per commit**, and the commit passes `npm run verify`.
- **Parallel agents:** one writer per directory tree. `shared/domain` and `shared/commands`
  are the shared surfaces and are single-writer; `public/` is partitioned by feature
  directory, and two agents must not edit `public/app.js` concurrently.

`shared/domain` carries three concerns that are genuinely separable — narrative structure,
panel layout, and lettering/fitting — so it has three module-tree owners rather than one
owner. Its `index.js` and any shared type belong to the coordinating agent. The concrete
team, its module trees and the surfaces each owner must not touch are defined in
`.github/agents/` and `.github/instructions/comiccraft-work.instructions.md`; that ownership
table is the source of truth for who may edit what.

---

## 18. First task for the coding agent

**Task: stand up the repository and land M0. Do not start comic features yet.**

The audit, the design and the frozen schema are complete, so this is a porting task, not a
research task. Do exactly this:

1. Create the repository layout of §4 — `server/`, `public/`, `shared/`, `schema/`,
   `test/`, `docs/` — with `package.json` listing `fastify` as the only runtime dependency,
   and `server.js` serving `public/` and answering `/api/health`.
2. Move the reference tree to `reference/vectorcraft/` and confirm nothing imports it.
   Record the exact commit it was taken from.
3. Port the frozen schema: read `docs/comiccraft-project-schema.md` and produce
   `schema/v1/*.schema.json` plus one fixture per entity, and
   `shared/domain/validate.js` that validates a bundle against it. The document is the
   authority; where it is ambiguous, the reference implementation settles it and the
   resolution is recorded as a finding.
4. Add `npm run verify` and make it green: type check (JSDoc, no emit), lint including the
   layering rule, the layering unit test, `node --test`, and the schema-fixture
   validation.
5. Prove the M1 path by hand through commands — create a project, add a page with three
   panels, place and clip an image, add lettering, save, reopen, export PNG and PDF — and
   record what behaved differently from the audit. A difference is a finding, not a
   failure.

Then stop and report, without starting M1's implementation.

**Success condition:** a reviewer can approve M1 using the schema files, a green
`npm run verify`, and a demonstrated three-panel page exported to PDF — with a working tree
that installs in one `npm install` and runs with one `node server.js`, on a laptop that has
never had a Rust toolchain on it.
