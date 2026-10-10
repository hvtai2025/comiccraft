---
description: "Use when doing any ComicCraft work: adding comic crates or modules, implementing a milestone (M0-M8), writing comic commands, the comic project schema or bundle, panels, balloons, lettering, reading order, storyboard, provider adapters, generation jobs, export presets, or comic validation. Covers ownership, single-writer surfaces, the layer table, and the rules that keep the fork mergeable."
applyTo: ["crates/comic-*/**", "crates/engine/src/cmd/comic/**", "crates/ui-egui/src/panels/comic/**"]
---
# ComicCraft working rules

ComicCraft is a fork of VectorCraft that turns stories into editable comics. VectorCraft
owns vector editing and rendering; ComicCraft owns comic semantics, narrative structure,
AI orchestration and publishing.

## Read first, in this order

1. `AGENTS.md` (the fork inherits it) — clean-room, assets, no panics, everything is a
   command, layering, quality gates.
2. `docs/comiccraft-design.md` — the finalized architecture. Sections 4, 5, 6, 7 and 10
   are the ones that constrain code.
3. `docs/comiccraft-porting-plan.md` — the phase you are in, and its exit criteria.
4. `docs/comiccraft-porting-audit.md` — what already exists, with `path:line` citations.
5. `docs/comiccraft-project-schema.md` — the frozen bundle schema (M0 deliverable).

These four live in `docs/` — M0 moved them there from `porting_plan_and_design/`, keeping
the filenames the agents' read-first lists already use. If a later change renames one,
update this file and every agent's read-first list in the same commit.

## Entry points

Three skills drive the team; use them rather than improvising a start.

| Skill | Use it to |
|---|---|
| `/comiccraft-start` | begin or resume a milestone, create the fork, or dispatch one specialist. Runs a preflight and assembles the brief |
| `/comiccraft-status` | see where the team is: stage, milestone trail, artifacts, divergence |
| `/comiccraft-verify` | close a milestone: the gate plus a per-criterion verdict |

The three prompt files (`/m0-fork-and-baseline`, `/milestone-kickoff`, `/upstream-sync`)
are the lead's detailed procedures; the skills dispatch to them.

## The mandatory review rule

**No implementation is complete until it has had two rounds of code review on
`deepseek-v4.1-flash`, and its unit tests pass.** Round 1 covers correctness and whether the tests
test behaviour; round 2 covers the hard rules, failure paths and edge cases, on the
post-fix revision. `code-reviewer` writes the unit tests; the lead spawns it; nobody
reviews their own work. Every change leaves a record in
`docs/reviews/<milestone>-<owner>-<slug>.md`.

Full rule: `.github/instructions/implementation-review.instructions.md`.
Check compliance: `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh`.

## Two structural rules that are easy to get wrong

1. **Never put comic data in the visual document.** No comic field in `crates/doc`, no
   comic section in `crates/format`, and never `Document::unknown` / `Document::extra` —
   those exist to round-trip *foreign* data. Comic metadata lives beside the document and
   reaches visual objects by `NodeId` and `artboard_id` (design ADR-013, §5.2.1).
2. **Never keep a second copy of geometry, page size or dialogue.** The artboard owns the
   page size; the document object owns the frame geometry; the comic record holds the id.
   A duplicated rectangle is the failure this architecture exists to prevent (design §6.1).

## Ownership — one writer per surface

| Surface | Owner agent |
|---|---|
| `crates/comic-model/**`, `docs/comiccraft-project-schema.md` | `schema-architect` |
| `crates/engine/src/cmd/comic/**`, the `command_specs()` line | `command-engineer` |
| `crates/comic-layout/src/{layout,panel,template,reading_order,validate}/**` | `layout-engineer` |
| `crates/comic-layout/src/{lettering,fit}/**` | `lettering-engineer` |
| `crates/comic-layout/src/{publish,export}/**` | `publishing-engineer` |
| `crates/comic-ai/**` | `ai-pipeline-engineer` |
| `crates/ui-egui/src/panels/comic/**`, `UI_COMMANDS` comic entries | `studio-ui-engineer` |
| `crates/*/tests/**`, golden fixtures, acceptance suite | `verification-engineer` |
| `Cargo.toml`, `xtask/src/layers.rs`, `lib.rs` module wiring, `docs/**` | `comiccraft-lead` |

`crates/comic-layout` has three owners because it carries three concerns; each owns a
**module tree**, not the crate. Its `lib.rs`, `Cargo.toml` and any shared type belong to
the lead. Never edit another owner's module tree — ask for the change instead.

## Adding a crate or a module

- A new crate must be registered in `xtask/src/layers.rs` in the same commit, or
  `cargo xtask layers` fails.
- New crates set `[lints] workspace = true`.
- Keep every dependency pointing downward. The three comic crates are placed so that no
  same-layer edge exists (design §4.2); a same-layer edge needs a hand-maintained
  `INTRA_LAYER_ORDER` entry and the lead's approval.
- No comic crate may depend on `engine` (L5), `ui-egui` (L6) or `mcp` (L6).

## Parallel work

- One agent per module tree, and one `CARGO_TARGET_DIR` per agent (`AGENTS.md` › Running
  and looking at the app). Each target directory grows to ~30 GB; delete it when done.
- `crates/engine/src/cmd/comic/` is a **single-writer** surface: serialise every change
  to it through `command-engineer`.
- Write manifests atomically; do not reformat a file another agent owns.

## The gate

`cargo xtask ci` — `fmt`, `clippy`, `test`, `assets`, `brands`, `layers`, `wasm`. It runs
in that order and stops at the first failure. Nothing is complete until it passes, and no
milestone closes until it passes on the integrated tree.

Comic work adds three things to it:

- **`wasm`** — provider code must be feature-gated off, and everything else must build.
- **`brands`** — command labels, parameter docs, panel text and MCP tool definitions are
  scanned for the vendor deny-list.
- **`layers`** — every new crate registered.

## Artifacts and where they live

Fixed paths, so any agent and the status scripts can find them:

| Artifact | Path |
|---|---|
| Design (moved by M0) | `docs/comiccraft-design.md` |
| Porting plan (moved by M0) | `docs/comiccraft-porting-plan.md` |
| Baseline audit (moved by M0) | `docs/comiccraft-porting-audit.md` |
| Frozen project schema (M0) | `docs/comiccraft-project-schema.md` |
| Command reference (M5) | `docs/comiccraft-command-api.md` |
| Provider contract (M4) | `docs/comiccraft-ai-workflow.md` |
| Publishing rules (M7) | `docs/comiccraft-publishing.md` |
| Baseline gate evidence (M0) | `docs/ci-baseline-<short-sha>.md` |
| Gate evidence (each run) | `docs/ci-<short-sha>.md` |
| Review record (each implementation) | `docs/reviews/<milestone>-<owner>-<slug>.md` |
| Milestone report (each) | `docs/milestones/M<n>.md` |
| Golden-render fixtures | `crates/*/tests/fixtures/` |

Do not invent a second location for any of these.

## Reporting

Report **completed** behaviour separately from **planned** or partially implemented
behaviour, and cite the test that proves it. Do not describe an intention as a result.
