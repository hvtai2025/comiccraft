# ComicCraft — agent instructions

The full contract is [`AGENTS.md`](AGENTS.md). Read it; this file exists only because some
agent harnesses look for `CLAUDE.md` by name.

## The short version

**Stack.** Node.js backend + client-side HTML/JavaScript frontend, sharing one plain-JavaScript
domain. No Rust, no framework, no bundler, no build step. Node 22+.

```sh
npm install && node server.js     # runs it
npm run verify                    # the gate
```

**Read first:** [`docs/comiccraft-porting-plan.md`](docs/comiccraft-porting-plan.md) §2, §3,
§4, §17.1, then the phase you are in. The exit criteria are the definition of done.

**Three rules that are easy to break:**

1. `shared/` runs in **both** runtimes — no DOM global, no `node:*`, no `fetch`; inject
   anything the domain needs.
2. No comic data in the visual document; links reach it by id (design ADR-013).
3. No duplicated geometry, page size or dialogue.

**Absolute rules inherited from the project owner:** never use Adobe software; no Adobe
iconography or images, ever; no font file in this repository; every asset has a row in
`ASSETS.md`.

**Before you are done:** `npm run verify` passes, and the change has had two rounds of code
review on `deepseek-v4.1-flash` with unit tests, recorded in `docs/reviews/`.

**The reference tree** (`reference/vectorcraft/`) is read-only. It answers *"how should this
behave?"*. It is never imported and never built. Its own contract, which describes the Rust
workspace and `cargo xtask ci`, is at
[`docs/reference/vectorcraft-claude.md`](docs/reference/vectorcraft-claude.md) — **do not
follow those instructions for ComicCraft work.**
