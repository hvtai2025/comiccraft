# ComicCraft

Turn prompts, scripts and existing stories into **editable** comic books.

ComicCraft is a story-to-comic production studio: write or import a story, have it broken
into chapters, scenes, pages and panels, generate consistent character and panel artwork,
compose the page with editable frames, balloons, captions and sound effects, then render,
validate and publish it.

**AI output stays editable.** Changing a line of dialogue, moving a balloon or replacing one
panel image must never require regenerating a page.

> **Status: M0–M8 are done.** `npm install && node server.js` gives you the studio: a story
> becomes a reviewable storyboard, pages are composed with editable panels, balloons, captions
> and sound effects, artwork is generated as reviewable jobs, and books export to screen, print
> and proofing. The same 95 commands drive the browser, the CLI, HTTP and MCP. The gate —
> 139 tests plus type, boundary, brand, fixture, documentation and smoke checks — passes in
> seconds. See [Where we are](#where-we-are).

---

## The stack

A **Node.js backend** plus a **client-side HTML/JavaScript frontend**, sharing one
plain-JavaScript domain that both runtimes import unchanged.

**No Rust. No framework. No bundler. No build step.**

```sh
npm install        # fastify — the only runtime dependency
node server.js     # serves public/ and the command API
npm run verify     # the gate
```

That is the whole setup. `npm run verify` runs the type check, the linter (including the
dependency boundaries), the layering test, the unit suites, the schema-fixture validation and
a server smoke test — in seconds, because nothing is compiled.

| | |
|---|---|
| **Runtime** | Node.js 22+ |
| **Client** | HTML, CSS and ES modules the browser runs as-is |
| **Editing** | Canvas 2D for rendering, SVG where vector output is the deliverable |
| **Domain** | Plain JavaScript with JSDoc types, checked but never emitted |
| **Storage** | A project is a directory bundle; a filesystem in development, object storage in deployment |
| **Tests** | `node --test`, built in — no test framework |

### Why this stack

The product is a document editor that must work offline, keep an undo history, and be
drivable by agents through one command surface. None of that needs a framework — it needs a
DOM, a canvas and a Node process. Dropping React/Next removed a bundler, a JSX transform, a
hydration model and a server/client boundary, and left one executable artifact that installs
in one command on a laptop.

Two consequences worth knowing:

- **The same module runs in both runtimes.** A rule is implemented once: the browser validates
  a project before saving and the server validates it before writing, using the same code.
- **Roles are separated by *injection*, not by a framework.** The domain never reaches for a
  clock, a filesystem, text metrics or the network; the caller supplies them. That is what
  makes it testable in plain Node and identical on both sides.

Full reasoning: [porting plan §2](docs/comiccraft-porting-plan.md).

---

## Architecture

```
┌─ Browser ────────────────────────────────┐   ┌─ Node.js ────────────────────────────┐
│ public/  index.html + ES modules         │   │ server/  HTTP API + static serving   │
│   editor/  canvas, hit-test, undo stack  │◄──┤   commands/  server-only handlers    │
│   ui/      panels, story navigator       │   │   jobs/      generation, render, exp │
│   storage/ IndexedDB + local bundle      │   │   storage/   bundle read/write, R2   │
└──────────────┬───────────────────────────┘   └──────────────┬───────────────────────┘
               │            both import ↓                       │
               └────────────► shared/ ◄──────────────────────────┘
                       domain · commands · validation · schema
```

**One link points one way.** The comic model owns meaning — who speaks, what they say, which
panel follows which — and the visual document owns geometry and artwork. They are linked by
id, never by a duplicate field, because a second copy of a rectangle is the failure this
architecture exists to prevent.

**Everything is a command.** A click, a `curl` and an agent over MCP all reach the same
dispatcher and run the same code, so there is never a second implementation of a behaviour
for the UI's convenience.

Read the architecture: [design document](docs/comiccraft-design.md) ·
[the layers](docs/comiccraft-porting-plan.md) · [the audit of the reference](docs/comiccraft-porting-audit.md).

---

## The product workflow

1. **Idea.** The user writes a story, premise or manuscript in **Design with AI**.
2. **Design.** `comic.story.design` turns it into structured scenes, pages, panels, camera
   notes, dialogue, captions and sound effects. The result is an inert **proposal**, never
   authoritative project data. The default director is deterministic and offline; an optional
   HTTP model can replace it through `server/ai/story.js` with credentials in the environment.
3. **Review.** The Review view shows the logline, style, cast and every scene. Accept all, or
   accept one scene at a time; an unselected scene is left byte-for-byte untouched.
4. **Edit.** Accepting creates ordinary pages in the studio, where the user edits frames,
   dialogue, typography, reading order and page composition with the same commands an agent uses.
5. **Artwork.** `comic.artwork.paint` draws a panel locally in one of five styles with no key
   or network; server-side generation jobs remain available for real models, with approval
   before a result reaches a panel.
6. **Publish.** Validate for screen, print or proofing, export pages/books/contact sheets, or
   package the project for a round trip.

The workflow bar in the studio derives the current step from project state — nothing is stored
twice, and every step is reachable without an agent.

---

## Where we are

The work is organized into nine milestones, M0–M8. Each has exit criteria that are the
definition of done, and each closes on the gate passing on an integrated tree.

| Milestone | Delivers | State |
|---|---|---|
| M0 | Repository layout, the frozen bundle schema as JSON Schema with a fixture per entity, the gate | **Done** — [report](docs/milestones/M0-M1-node.md) |
| M1 | Three-panel page vertical slice: bundle, artboards as pages, panels, artwork, lettering, undo, save/reopen, PNG and PDF out | **Done** — twelve acceptance tests, [report](docs/milestones/M0-M1-node.md) |
| M2 | Panel engine, templates, gutters, lettering policy, reading order, golden render | **Done** — [report](docs/milestones/M2-node.md) |
| M3 | Story import to a schema-validated, reviewable storyboard with stable ids | **Done** — [report](docs/milestones/M3-node.md) |
| M4 | Character bible, capability-based providers, pollable jobs, asset provenance | **Done** — [report](docs/milestones/M4-node.md) |
| M5 | Completed agent surface: parameter docs, batch operations, MCP, malformed-input sweep | **Done** — [report](docs/milestones/M5-node.md) |
| M6 | Studio UI: story navigator, storyboard, canvas tools, review queue, publication gate | **Done** — [report](docs/milestones/M6-node.md) |
| M7 | Rendering and publishing: presets, export gate, PDF geometry, deterministic package | **Done** — [report](docs/milestones/M7-node.md) |
| M8 | QA, representative book, failure paths, performance, release checklist | **Done** — [report](docs/milestones/M8-node.md) |

**What is built** is 95 commands over 20 groups, one dispatcher behind the browser, HTTP, CLI and
MCP, a domain that runs unchanged in both runtimes, and a gate that passes in seconds:

```sh
npm run verify            # typecheck, boundary, brands, 139 tests, fixtures, docs, smoke
node server.js            # then open http://127.0.0.1:3000
node server/cli.js demo   # the vertical slice, printed as it goes
npm run mcp:list          # the command catalogue an agent sees, with parameter schemas
npm run perf              # comic-shaped render/save/open budgets
```

The studio and the server are deliberately asymmetric: editing, lettering, validation and undo
run in both; providers, print PDF and the filesystem are server-only, and the UI says so rather
than embedding a credential path. Providers default to an **offline deterministic painter**, so
the whole job lifecycle works with no key and no network; the optional HTTP adapter reads its
credentials from the environment and never writes them into a bundle.

The **Rust implementation of the same design** is preserved as the reference: the frozen schema
([the document](docs/comiccraft-project-schema.md)), a vertical slice, and a panel/lettering
engine whose rules ported directly — which is why M0–M2 were port milestones with a known
target rather than design milestones. Its milestones are in
[`docs/milestones/M0.md`](docs/milestones/M0.md) onward and its reviews in
[`docs/reviews/`](docs/reviews); [reference-owned files](docs/reference/README.md) lists what
still belongs to the baseline.

---

## Working on it

**Read [`AGENTS.md`](AGENTS.md) first** — it is the contract: the three structural rules, the
clean-room and asset rules, the error-handling policy, and the review requirement.

```sh
npm run verify                 # the gate — must pass before every commit
node --test test/foo.test.js   # one suite
npm run cli -- commands        # the command surface, as an agent sees it
npm run docs:commands          # regenerate docs/comiccraft-command-api.md
npm run mcp:list               # MCP tools/list, without a client
npm run perf                   # performance budgets on a synthetic book
```

**Nothing is complete without two rounds of code review** on `deepseek-v4.1-flash` with unit
tests, recorded in `docs/reviews/`. The reviewer writes the tests; nobody reviews their own
work.

The team is defined in [`.github/agents/`](.github/agents), its ownership table in
[`.github/instructions/comiccraft-work.instructions.md`](.github/instructions/comiccraft-work.instructions.md),
and the milestone procedure in [`.github/prompts/`](.github/prompts).

---

## The reference implementation

`reference/vectorcraft/` holds the audited baseline (VectorCraft, `3da4080b`) — a clean-room,
open-source vector illustration application. It is **read, never imported**: it answers
*"how should this behave?"* and it supplies the bundle format. No ComicCraft code links
against it and no build step compiles it.

Its documentation is preserved under [`docs/reference/`](docs/reference/README.md) with a note
on every file, because reading *"Rust-native vector illustration app"* at the repository root
would be the single most misleading thing a newcomer could open.

---

## Licensing

Code and original assets are **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`,
`NOTICE`). Every asset has a row in [`ASSETS.md`](ASSETS.md) with its author, source and
licence.

Two absolute rules come from the project owner and are not negotiable:

- **Never use Adobe software** to research behaviour, make test files, or compare output.
  Behaviour comes from public documentation and from files users own.
- **No Adobe iconography, images or fonts, ever** — and no font file enters this repository
  at all. Fonts live in [`storytold/craft-fonts`](https://github.com/storytold/craft-fonts)
  and are referenced by URL.
