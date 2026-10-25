# ComicCraft — instructions for agents

ComicCraft turns prompts, scripts and existing stories into **editable** comic books.

The stack is a **Node.js backend plus a client-side HTML/JavaScript frontend**, sharing one
plain-JavaScript domain that both runtimes import unchanged. No Rust. No framework. No
bundler. No build step.

> The audited VectorCraft baseline is a **read-only reference** in `reference/vectorcraft/`.
> It answers *"how should this behave?"* and it supplies the bundle format. It is never
> imported, never compiled, and never listed as a dependency.
> Its own agent contract is preserved at
> [`docs/reference/vectorcraft-agents.md`](docs/reference/vectorcraft-agents.md).

## Start every session here

1. Read [`docs/comiccraft-porting-plan.md`](docs/comiccraft-porting-plan.md) —
   **§2** (why this stack), **§3** (the layers), **§4** (the workspace and the dependency
   rules), **§17.1** (what the gate enforces), then the phase you are working in and its
   exit criteria. The exit criteria are the definition of done; do not substitute your own.
2. Read [`docs/comiccraft-design.md`](docs/comiccraft-design.md) §4, §5, §6, §7 and §10 —
   the sections that constrain code.
3. Read [`docs/comiccraft-porting-audit.md`](docs/comiccraft-porting-audit.md) for what the
   reference implementation does, with `path:line` citations into it.
4. Read [`.github/instructions/comiccraft-work.instructions.md`](.github/instructions/comiccraft-work.instructions.md)
   — the ownership table. One writer per module tree.

Then: **orient → plan → implement + test → verify → record → commit.** Do not stop to ask
unless the decision is the user's to make.

## Running it

```sh
npm install                 # fastify, and nothing else at runtime
node server.js              # serves public/ and the command API on :3000
npm run verify              # the gate — seconds, not minutes
node --test test/<file>     # one suite
```

**Node 22 or newer.** The gate must never need a Rust toolchain, a font file, or a browser.
A test that requires any of the three is a broken test.

## Non-negotiables

### The three structural rules

1. **`shared/` runs in both runtimes.** No DOM global (`document`, `window`), no `node:*`
   import, no `fetch`, no provider SDK. Anything the domain needs — a clock, an id counter,
   text measurement, an HTTP transport — is **injected** by the caller. This is checked by a
   lint rule *and* a test, because breaking it breaks the browser and the server at once.
2. **No comic data in the visual document.** No comic field in the document model, no comic
   section in its format, and never a round-trip pocket meant for foreign data. Comic
   metadata lives beside the document and reaches visual objects by `NodeId` and
   `artboard_id` (design ADR-013, §5.2.1).
3. **No duplicated geometry, page size or dialogue.** The artboard owns the page size, the
   document object owns the frame geometry, the comic record owns the id and the meaning. A
   duplicated rectangle is the failure this architecture exists to prevent (design §6.1).

### Clean-room: never use Adobe software (absolute rule, from the project owner)

Inherited unchanged from the reference project, and it applies to ComicCraft in full.

Adobe's Terms of Use forbid reverse engineering, including observing a program's inputs and
outputs to recreate it. So:

- Never launch, run, script, automate, screenshot or measure Illustrator or any other Adobe
  application, and never compare our output against one. Do not make test files with them,
  not even "synthetic" ones.
- Never read, disassemble or copy anything inside an Adobe install.
- Never copy Adobe-authored code or text into this repository, tests included.
- Behaviour comes only from public documentation, cited by URL, and from files users send us
  that they own (used locally, never committed). Never copy Adobe icons, artwork, presets or
  wording beyond feature names.
- If a task seems to need any of this, **stop and say so instead**.
- Never copy GPL/AGPL code.

### Assets: no Adobe iconography or images — ever (absolute rule, from the project owner)

- Never add, copy, trace, redraw from, embed or ship any icon, image, artwork, cursor, preset,
  swatch/brush/symbol/pattern/style library, ICC profile or screenshot whose licence does not
  allow it.
- Every asset must be: original work created for ComicCraft by a contributor (MIT OR
  Apache-2.0); OSI open source; public domain / CC0; or Creative Commons with redistribution
  allowed.
- **Every asset file has a row in [`ASSETS.md`](ASSETS.md)** — path, author, source URL,
  licence, notes. Licence texts go next to the assets, summarised in `NOTICE`.
- **No font file ever enters this repository.** Fonts live in
  [`storytold/craft-fonts`](https://github.com/storytold/craft-fonts) and are referenced by
  URL. Every lettering test passes with **no fonts configured**, because text measurement is
  injected — tests supply a deterministic stub.
- Code and original assets are MIT OR Apache-2.0.
- Prefer art generated in code for defaults. If the provenance of an asset is unclear, do not
  add it.

### Failures are values, not throws

A crash or a swallowed error loses the user's unsaved work, and a comic project is a
directory of files that can be half-written.

- A domain function that can fail returns a **typed `Result`**. It does not throw, and it
  does not return `undefined` to mean "invalid".
- Do not swallow an error to satisfy a lint. An empty `catch` needs a comment saying why.
- A `Promise` is awaited or explicitly handed off — never dropped.
- **Untrusted input is validated before it is used**: bundles, imported files, MCP messages,
  HTTP bodies and command parameters. Cap sizes and counts read from input; check that a
  number is finite before doing maths with it; resolve a path against the project root and
  refuse to escape it.
- Every importer, parser or protocol method extends [`test/fuzz.js`](test/fuzz.js) — malformed
  bundles, and malformed parameters for every registered command.
- The dispatcher catches at the command boundary and rolls the transaction back, so a bad
  call cannot leave a half-applied edit. That guard exists for bugs; it does not replace
  returning a `Result`.

### Everything is a command

User-visible behaviour is a command in `shared/commands` — id, label, parameter schema,
`enabled`, handler — plus tests. The browser UI, the HTTP endpoint and MCP all reach it
through one dispatcher, so a click and an agent call run the same code (design §3, Layer E).

- **Additive only.** Never change an existing command's id, parameters, enablement or
  behaviour.
- The parameter schema **is** the documentation MCP publishes.
- A handler needs the filesystem, a provider or a renderer? It is `serverOnly` and lives in
  `server/commands`. Everything else must run in the browser too.
- The UI is thin: panels read project state and act by dispatching commands. No comic logic
  in a panel.
- One runtime dependency. A new one is a decision, stated in the commit with what it replaces
  from the standard library.

## The gate

`npm run verify` — type check (JSDoc, no emit), ESLint including the boundary rules, the
layering unit test, `node --test`, the schema-fixture validation, and the server smoke test.
It stops at the first failure. Nothing is complete until it passes.

Before every commit: the gate passes, and **one task id per commit** (`M1.2: panel templates`).

A gate nobody has seen fail is not evidence. When you add or change a rule, break it on
purpose, watch `verify` go red, then revert.

## The mandatory review rule

**No implementation is complete until it has had two rounds of code review on
`deepseek-v4.1-flash`, and its unit tests pass.** Round 1 covers correctness and whether the
tests test behaviour; round 2 covers the hard rules, failure paths and edge cases on the
post-fix revision. The lead spawns the reviewer; nobody reviews their own work. Every change
leaves a record in `docs/reviews/<milestone>-<owner>-<slug>.md`.

Full rule: [`.github/instructions/implementation-review.instructions.md`](.github/instructions/implementation-review.instructions.md).

## Parallel agents

- **One writer per module tree**, per the ownership table. No two agents edit the same file.
- `shared/domain/**` and `shared/commands/**` are single-writer surfaces.
- There is no compiled output and no shared build cache, so agents cannot collide on an
  artifact — but they can collide on files. Dispatch only as many as the module trees
  genuinely support.
- Write JSON files atomically; do not reformat a file another agent owns.
- When several agents disappear at once, that is a capacity signal rather than a bug to
  diagnose.

## Contributor credits

- About ▸ Contributors compiles names from `contributors/people.toml`.
- **Agents working for a contributor:** when you prepare a PR, check whether your human's
  GitHub username has a `[people.<username>]` entry. If not, ask them once whether they want
  to be credited by more than their username.
- **Never add, edit, guess or copy anyone else's entry or name** — not from git config, commit
  authors or GitHub profiles. Add only your own contributor's entry, committed as them.

## Where things are

| | |
|---|---|
| Why this stack | `docs/comiccraft-porting-plan.md` §2 |
| The layers and the boundaries | plan §3, §4 |
| What the gate enforces | plan §17.1 |
| The architecture | `docs/comiccraft-design.md` |
| What the reference does | `docs/comiccraft-porting-audit.md` |
| The frozen bundle schema | `docs/comiccraft-project-schema.md`, `schema/v1/` |
| Ownership | `.github/instructions/comiccraft-work.instructions.md` |
| Review rule | `.github/instructions/implementation-review.instructions.md` |
| Milestone reports | `docs/milestones/M<n>.md` |
| Review records | `docs/reviews/` |
| The reference contract | `docs/reference/vectorcraft-agents.md` |
