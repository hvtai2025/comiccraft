---
description: "Run M0: create the ComicCraft fork from the local VectorCraft checkout at 3da4080b, record the baseline gate, freeze the project schema, and register the three comic crates — each reviewed twice with unit tests."
agent: comiccraft-lead
argument-hint: "Optional: the path to the baseline checkout, if not ../vectorcraft"
tools: [read, search, edit, execute, agent, todo]
---
Run milestone **M0 — Baseline** from `docs/comiccraft-porting-plan.md` §6, following
`§18 First task for the coding agent` exactly. Do not start comic features.

Baseline: `storytold/vectorcraft` @ `3da4080b` (`main`, clean tree), workspace `0.8.0`.
The local checkout is normally at `../vectorcraft`; confirm before copying.

## 1. Create the fork

- Make this repository the fork root, with the baseline tree at its top level.
- Copy the baseline working tree at `3da4080b`, **not** its `.git` history, unless the
  team has decided to preserve it — if in doubt, keep the history and add the upstream
  remote, because `git rev-list --count upstream/main..HEAD` is how divergence is measured.
- `origin` → the ComicCraft fork; `upstream` → `https://github.com/storytold/vectorcraft`.
- **Done:** the planning documents are in `docs/` as `comiccraft-design.md`,
  `comiccraft-porting-plan.md` and `comiccraft-porting-audit.md`, and every reference in
  `.github/` points at those names. Verify with
  `grep -rn "porting_plan_and_design" .github/ docs/` — it should only match historical
  notes.

## 2. Record the baseline gate

Run `cargo xtask ci` on the **untouched** tree and save the output to
`docs/ci-baseline-<short-sha>.md` with its timings. This is the pre-change evidence and
the performance reference for M8. If it does not pass, stop and report — do not proceed
on a broken baseline.

**The extension must be `.md`.** `cargo xtask brands` scans every non-Markdown text file
under `docs/`, and a build log quotes upstream test names such as
`an_illustrator_ai_file_opens_from_its_editing_data` — a `.txt` log in `docs/` fails the
brands gate, and the gate then reports the failure as `xtask::brands::tests::the_tree_is_clean`
rather than naming your log. Use the `gate.sh` script, which already writes `.md`.

## 3. Freeze the schema

Delegate to `schema-architect`: write `docs/comiccraft-project-schema.md` — the bundle
from design §7.1 (manifest, `project.json`, per-page files, one visual document), the
`NodeId` / `artboard_id` link rules, id allocation, migration rules, and a fixture per
version. It must not contradict design §5.

## 4. Register the three crates

Add `crates/comic-model`, `crates/comic-ai` and `crates/comic-layout` as documented empty
crates with `[lints] workspace = true`, registered in `xtask/src/layers.rs` at L2, L3 and
L4. Confirm `cargo xtask ci` still passes, including `layers`, `wasm` and `brands`.

## 4a. Review M0's implementations — two rounds each

M0 produces real implementations, so the mandatory review rule applies:

- the three crate skeletons and their `xtask/src/layers.rs` registration;
- `docs/comiccraft-project-schema.md`.

For each, spawn `code-reviewer` on `deepseek-v4.1-flash` for round 1, have the owner fix the
findings and add the missing unit tests, then spawn it again for round 2 on the post-fix
revision. Record both rounds in `docs/reviews/M0-<owner>-<slug>.md`.

The crate skeletons are code even though they are small: round 1 checks that the manifest,
lints inheritance and layer placement are right, and that a unit test actually exercises the
crate. For the schema document, round 1 checks every claim against the design, and round 2
checks the fixtures and migration rules; where the document has no testable behaviour, the
record states what was verified instead.

Confirm with `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh M0`.

## 5. Prove the M1 path by hand

Through commands only: `artboard.new`; place an image and clip it; crop with
`object.cropImage`; add area text; `document.serialize`; `document.export`; PDF export of
the artboards. Confirm cropping, clipping, typography and PDF page geometry behave as the
audit §7–§9 says. Record anything that differs — a difference is a finding, not a failure.

## Finish

Have `verification-engineer` confirm M0's exit criteria independently, then report with the
lead's milestone format: status, exit criteria with evidence, the gate result, divergence,
decisions, what is not done, and what M1 needs. Then stop.
