---
description: "Use when doing any ComicCraft work: adding packages or modules, implementing a milestone (M0-M8), writing comic commands, the comic project schema or bundle, panels, balloons, lettering, reading order, storyboard, provider adapters, generation jobs, export presets, or comic validation. Covers ownership, single-writer surfaces, the dependency boundaries, and the rules that keep the domain shareable."
applyTo: ["shared/**", "public/**", "server/**", "schema/**", "test/**"]
---
# ComicCraft working rules

ComicCraft turns stories into editable comics. It is a **Node.js backend plus a
client-side HTML/JavaScript frontend**, with one shared plain-JavaScript domain that both
runtimes import unchanged. There is no Rust, no framework, no bundler and no build step.

The reference implementation (`reference/vectorcraft/`, the audited VectorCraft baseline)
is **read, never imported**: it answers *"how should this behave?"* and supplies the
bundle format.

## Read first, in this order

1. `docs/comiccraft-porting-plan.md` — **§2** (why this stack), **§3** (the five layers),
   **§4** (the workspace and the dependency rules), **§17.1** (the enforced constraints),
   then the phase you are in and its exit criteria.
2. `docs/comiccraft-design.md` — the finalized architecture. Sections 4, 5, 6, 7 and 10
   are the ones that constrain code.
3. `docs/comiccraft-porting-audit.md` — what the reference implementation does, with
   `path:line` citations into `reference/vectorcraft/`.
4. `docs/comiccraft-project-schema.md` — the frozen bundle schema (M0 deliverable), and
   its machine-checkable form under `schema/v1/`.

These live in `docs/`. If a later change renames one, update this file and every agent's
read-first list in the same commit.

## Entry points

Three skills drive the team; use them rather than improvising a start.

| Skill | Use it to |
|---|---|
| `/comiccraft-start` | begin or resume a milestone, stand up the repository layout, or dispatch one specialist. Runs a preflight and assembles the brief |
| `/comiccraft-status` | see where the team is: stage, milestone trail, artifacts, what the gate says |
| `/comiccraft-verify` | close a milestone: `npm run verify` plus a per-criterion verdict |

The three prompt files (`/m0-node-baseline`, `/milestone-kickoff`, `/reference-sync`) are
the lead's detailed procedures; the skills dispatch to them.

## The mandatory review rule

**No implementation is complete until it has had two rounds of code review on
`deepseek-v4.1-flash`, and its unit tests pass.** Round 1 covers correctness and whether the
tests test behaviour; round 2 covers the hard rules, failure paths and edge cases, on the
post-fix revision. `code-reviewer` writes the unit tests; the lead spawns it; nobody reviews
their own work. Every change leaves a record in `docs/reviews/<milestone>-<owner>-<slug>.md`.

Full rule: `.github/instructions/implementation-review.instructions.md`.
Check compliance: `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh`.

## Three structural rules that are easy to get wrong

1. **Never put comic data in the visual document.** No comic field in the document model,
   no comic section in its format, and never a round-trip pocket meant for foreign data.
   Comic metadata lives beside the document and reaches visual objects by `NodeId` and
   `artboard_id` (design ADR-013, §5.2.1).
2. **Never keep a second copy of geometry, page size or dialogue.** The artboard owns the
   page size; the document object owns the frame geometry; the comic record holds the id.
   A duplicated rectangle is the failure this architecture exists to prevent (design §6.1).
3. **`shared/` must run in both runtimes.** No DOM global (`document`, `window`), no
   `node:*` import, no `fetch`, no provider SDK. Anything the domain legitimately needs — a
   clock, an id counter, text measurement, an HTTP transport — is **injected** by the
   caller. A `shared/` module that imports a runtime is the one mistake that breaks both
   sides at once, so it is a lint rule *and* a test (plan §4).

## Ownership — one writer per surface

| Surface | Owner agent |
|---|---|
| `shared/domain/**` (model, ids, migrations), `schema/**`, `docs/comiccraft-project-schema.md` | `schema-architect` |
| `shared/domain/document/**`, `shared/domain/format/**` | `schema-architect` |
| `shared/commands/**`, `server/commands/**` | `command-engineer` |
| `shared/domain/layout/**`, `shared/domain/panel/**`, `shared/domain/template/**`, `shared/domain/reading-order/**` | `layout-engineer` |
| `shared/domain/lettering/**`, `shared/domain/fit/**` | `lettering-engineer` |
| `shared/domain/publish/**`, `server/export/**` | `publishing-engineer` |
| `server/ai/**`, `server/jobs/**` | `ai-pipeline-engineer` |
| `public/**` | `studio-ui-engineer` |
| `test/**`, golden fixtures, acceptance suite | `verification-engineer` |
| `package.json`, `server.js`, `eslint.config.js`, `docs/**` | `comiccraft-lead` |

`shared/domain` has three owners because it carries three concerns; each owns a **module
tree**, not the directory. Its `index.js`, the `package.json` and any shared type belong to
the lead. Never edit another owner's module tree — ask for the change instead.

## Adding a module or a directory

- **Prefer an existing directory.** A new top-level directory needs a real boundary — its
  own runtime or its own dependency set — not a diagram.
- Keep every dependency pointing downward: `public/` → `shared/`, `server/` → `shared/`.
  `public/` must never import `server/`, and `shared/` must never import either.
- Nothing in `shared/` may reach for a runtime. See structural rule 3.
- A new production dependency is a decision, not an accident: it goes in the commit
  message with what it replaces from the standard library. `fastify` is the only one at M0.

## Parallel work

- One agent per module tree. There is no compiled output and therefore no build cache to
  collide on, so no per-agent target directory is needed — but two agents must still not
  edit the same file.
- `shared/commands/**` and `shared/domain/**` are **single-writer** surfaces: serialise
  every change through their owner.
- Write JSON files atomically; do not reformat a file another agent owns.

## The gate

`npm run verify` — type check (JSDoc, no emit), ESLint including the boundary rules, the
layering unit test, `node --test`, the schema-fixture validation, and the server smoke
test. It runs in that order and stops at the first failure. Nothing is complete until it
passes, and no milestone closes until it passes on the integrated tree.

Comic work adds three things to it:

- **the boundary test** — `shared/` contains no `node:` import and no DOM global.
- **`check:brands`** — command labels, parameter docs, UI copy and MCP tool definitions are
  scanned for the vendor deny-list.
- **the fixture gate** — every bundle schema version has a fixture, and every fixture
  validates.

Node 22 or newer. **The gate must never require a Rust toolchain, a font file, or a
browser.** A test that needs any of the three is a broken test.

## Artifacts and where they live

Fixed paths, so any agent and the status scripts can find them:

| Artifact | Path |
|---|---|
| Design | `docs/comiccraft-design.md` |
| Porting plan | `docs/comiccraft-porting-plan.md` |
| Reference audit | `docs/comiccraft-porting-audit.md` |
| Frozen project schema (human) | `docs/comiccraft-project-schema.md` |
| Frozen project schema (machine) | `schema/v1/*.schema.json` + one fixture per entity |
| Command reference (M5) | `docs/comiccraft-command-api.md` |
| Provider contract (M4) | `docs/comiccraft-ai-workflow.md` |
| Publishing rules (M7) | `docs/comiccraft-publishing.md` |
| Reference gate evidence | `docs/ci-baseline-3da4080b.md` (recorded; do not re-run) |
| Gate evidence (each run) | `docs/ci-<short-sha>.md` |
| Review record (each implementation) | `docs/reviews/<milestone>-<owner>-<slug>.md` |
| Milestone report (each) | `docs/milestones/M<n>.md` |
| Golden fixtures | `test/fixtures/golden/` |

Do not invent a second location for any of these.

## Reporting

Report **completed** behaviour separately from **planned** or partially implemented
behaviour, and cite the test that proves it. Do not describe an intention as a result.
