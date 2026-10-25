---
description: "Run M0: stand up the Node.js + vanilla-JS repository, port the frozen project schema, make `npm run verify` green, and prove the M1 path by hand — each implementation reviewed twice with unit tests."
agent: comiccraft-lead
argument-hint: "Optional: the path to the reference VectorCraft checkout, if not ../vectorcraft"
tools: [read, search, edit, execute, agent, todo]
---
Run milestone **M0 — Baseline** from `docs/comiccraft-porting-plan.md` §6, following
`§18 First task for the coding agent` exactly. Do not start comic features.

Target stack: **Node.js backend + client-side HTML/JavaScript frontend** — no Rust, no
framework, no bundler, no build step. Read plan §2 (the three revisions and why), §3 (the
layers) and §4 (the workspace) before writing anything.

Reference only: `storytold/vectorcraft` @ `3da4080b`, normally at `../vectorcraft`. It is
read to answer *"how should this behave?"* — it is **never** imported, compiled or built.

## 1. Create the repository layout

- Create the §4 tree: `server.js`, `server/`, `public/`, `shared/`, `schema/`, `test/`,
  `assets/`, `docs/`.
- `package.json` lists **`fastify` and nothing else** at runtime. Every other need — test
  runner, argument parsing, crypto, JSON — comes from Node's standard library. Adding a
  second runtime dependency is a decision you must state and justify in the commit.
- Move the reference tree to `reference/vectorcraft/` and record the exact commit it came
  from. Confirm nothing in `shared/`, `server/` or `public/` imports it.
- **Done:** `node server.js` serves `public/index.html` and answers `/api/health`; the
  planning documents are in `docs/` as `comiccraft-design.md`,
  `comiccraft-porting-plan.md` and `comiccraft-porting-audit.md`; every reference in
  `.github/` points at those names. Verify with
  `grep -rn "porting_plan_and_design" .github/ docs/` — it should only match historical
  notes.

## 2. Record the reference baseline — do not re-run it

The reference gate result is **already recorded** in `docs/ci-baseline-3da4080b.md`. It is
evidence for comparison, not a step to repeat. Do not install a Rust toolchain to
regenerate it; if a number is needed, cite the recorded file.

## 3. Freeze the schema

Delegate to `schema-architect`: turn `docs/comiccraft-project-schema.md` into
`schema/v1/*.schema.json` — the bundle from design §7.1 (manifest, `project.json`,
per-page files, one visual document), the `NodeId` / `artboard_id` link rules, id
allocation, migration rules, and **one fixture per entity**. It must not contradict
design §5.

Then implement `shared/domain/validate.js` against it and test it. This is promoted ahead
of the rest of M0 because everything else is written against the schema, and a schema
nothing validates is a document, not a contract.

## 4. Make `npm run verify` green

`verify` runs, in order, stopping at the first failure:

1. the JSDoc type check — `tsc --checkJs --noEmit`, strict; **no build output is produced**
2. ESLint, including the `shared/` / `public/` / `server/` boundary rules
3. the layering unit test — walks `shared/` and fails on a `node:` import or a DOM global
4. `node --test`
5. the fixture validation from step 3
6. a smoke test that boots the server and fetches `/`

Node 22 or newer. Confirm the whole thing runs on a machine with no Rust toolchain.

## 4a. Review M0's implementations — two rounds each

M0 produces real implementations, so the mandatory review rule applies:

- `server.js` and the HTTP layer;
- `shared/domain/validate.js` plus its tests and the `schema/v1/` fixtures;
- the `npm run verify` wiring and the boundary test.

For each, spawn `code-reviewer` on `deepseek-v4.1-flash` for round 1, have the owner fix the
findings and add the missing unit tests, then spawn it again for round 2 on the post-fix
revision. Record both rounds in `docs/reviews/M0-<owner>-<slug>.md`.

Round 1 checks that the boundary test can actually fail — **break a boundary on purpose,
watch the gate go red, then revert** — and that a fixture genuinely rejects a mutation.
A gate that has never been observed failing is not evidence. For the schema, round 1 checks
every claim against the design, and round 2 checks the fixtures and migration rules; where a
document has no testable behaviour, the record states what was verified instead.

Confirm with `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh M0`.

## 5. Prove the M1 path by hand

Against the reference, through its commands only: `artboard.new`; place an image and clip
it; crop with `object.cropImage`; add area text; `document.serialize`; `document.export`;
PDF export of the artboards. Confirm cropping, clipping, typography and PDF page geometry
behave as the audit §7–§9 says.

Then repeat each step in the new stack through `shared/commands`, and **record every
difference as a finding** — a difference is a finding, not a failure. The port is only
approved when the behaviour matches or the divergence is written down.

## Finish

Have `verification-engineer` confirm M0's exit criteria independently, then report with the
lead's milestone format: status, exit criteria with evidence, the gate result, divergence,
decisions, what is not done, and what M1 needs. Then stop.
