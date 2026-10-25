# Reference-owned files

ComicCraft is a **Node.js backend plus a client-side HTML/JavaScript frontend**. The audited
**VectorCraft** baseline is a read-only reference: it answers *"how should this behave?"* and
it supplies the bundle format. It is never imported, built or listed as a dependency.

This directory holds the reference's own documentation, preserved and annotated, so that a
reader cannot mistake it for ComicCraft's instructions.

## Why these files are here and not in `reference/vectorcraft/`

M0 moves the audited baseline to `reference/vectorcraft/`, and **every file listed below
moves with it** in that same commit. They are copied here first rather than left at the
repository root, because a root `README.md` that says *"VectorCraft is a Rust-native vector
illustration app"* is the single most misleading thing an agent or a newcomer can open.

Deleting them outright was the alternative and it is wrong: they are the only description of
how that tree is built and tested, for a restructure that has not happened yet.

## What is here

| File | What it is | Moves to |
|---|---|---|
| [`vectorcraft-agents.md`](vectorcraft-agents.md) | The reference's agent contract: clean-room rules, asset provenance, no-panic policy, the command model | `reference/vectorcraft/AGENTS.md` |
| [`vectorcraft-claude.md`](vectorcraft-claude.md) | The same contract for harnesses that look for `CLAUDE.md` | `reference/vectorcraft/CLAUDE.md` |
| [`vectorcraft-readme.md`](vectorcraft-readme.md) | The reference's README | `reference/vectorcraft/README.md` |
| [`vectorcraft-roadmap.md`](vectorcraft-roadmap.md) | The reference's roadmap and parity estimates | `reference/vectorcraft/ROADMAP.md` |
| [`vectorcraft-development.md`](vectorcraft-development.md) | Building and testing the Rust workspace | `reference/vectorcraft/docs/development.md` |
| [`vectorcraft-releasing.md`](vectorcraft-releasing.md) | The Rust release pipeline | `reference/vectorcraft/docs/releasing.md` |
| [`vectorcraft-contributors.md`](vectorcraft-contributors.md) | Contributor credits for the reference | `reference/vectorcraft/docs/contributors.md` |

Still at the repository root, and also reference-owned:

| Path | Why it is still there |
|---|---|
| `crates/`, `apps/`, `xtask/`, `Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `clippy.toml` | The Rust workspace itself — it *is* the baseline, and M0 moves it |
| `target/` | Its build output. Gitignored; delete it, it is tens of GB |
| `packaging/` | Installers for the desktop app: Linux, macOS, Windows, FreeBSD, web |
| `.github/workflows/release.yml`, `windows7.yml`, `windows-arm64.yml`, `freebsd.yml`, `affinity-corpus.yml`, `packaging-lint.yml` | The reference's release pipeline. See [`../../.github/workflows/README.md`](../../.github/workflows/README.md) |
| `docs/affinity-validation.md`, `control-protocol.md`, `mcp.md`, `plugins.md`, `shaper.md`, `windows7.md`, `upstream-quit-prompt-fix.md` | Reference documentation for features ComicCraft does not yet have |
| `contributors/` | The reference's contributor data |
| `.cargo/`, `vendor/`, `examples/` | Reference build configuration and examples |

## What is *not* reference-owned

These are ComicCraft's, and they stay:

| Path | What it is |
|---|---|
| `AGENTS.md`, `CLAUDE.md` | ComicCraft's agent contract |
| `docs/comiccraft-design.md` | The architecture |
| `docs/comiccraft-porting-plan.md` | The plan, including the milestone table and the exit criteria |
| `docs/comiccraft-porting-audit.md` | What the reference does, cited by `path:line` |
| `docs/comiccraft-project-schema.md`, `schema/` | The frozen bundle schema |
| `docs/milestones/`, `docs/reviews/` | The record of work done, Rust era included |
| `docs/ci-baseline-3da4080b.md`, `docs/ci-*.md` | Gate evidence. Historical records — **do not edit them** |
| `ASSETS.md`, `NOTICE`, `LICENSE-*` | Asset provenance and licensing |
| `.github/agents/`, `.github/instructions/`, `.github/prompts/`, `.github/skills/`, `.github/workflows/verify.yml` | The team and its gate |

## The one rule about all of this

**Reference documentation is evidence, not instruction.** If a file in this directory tells
you to run `cargo xtask ci`, that is true of the reference tree and false of ComicCraft. When
the two disagree about how ComicCraft behaves, the plan and the design win, and the reference
settles only questions of *behaviour*.
