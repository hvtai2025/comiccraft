# Dispatch table

Source of truth for who owns what. Mirrors
`.github/instructions/comiccraft-work.instructions.md`; if they disagree, the instruction
file wins and this table is wrong.

| Agent | Owns (module tree) | Must not touch | Design sections |
|---|---|---|---|
| `comiccraft-lead` | `Cargo.toml`, `xtask/src/layers.rs`, crate `lib.rs` wiring, `docs/**`, ADRs | any specialist's module tree | §4, §16, §17 |
| `code-reviewer` | `docs/reviews/**`, test code in the reviewed diff | **production code** — it reports, the owner fixes | §12.3, §17 |
| `schema-architect` | `crates/comic-model/**`, `docs/comiccraft-project-schema.md` | `doc`, `format`, any visual-document field | §5, §6.1, §7, ADR-011/012/013 |
| `command-engineer` | `crates/engine/src/cmd/comic/**` + the `command_specs()` line | existing commands' semantics | §10, §11 |
| `layout-engineer` | `crates/comic-layout/src/{layout,panel,template,reading_order,validate}/**` | `&mut Document`; geometry in a comic record | §5.1.1, §6.1, §8.1, §8.2 |
| `lettering-engineer` | `crates/comic-layout/src/{lettering,fit}/**` | writing a text engine; adding font files | §5.2, §6.1, §8.3, §12.1 |
| `publishing-engineer` | `crates/comic-layout/src/{publish,export}/**` | writing a renderer, PDF writer or encoder | §7.5, §9.4, §12.1, §13 |
| `ai-pipeline-engineer` | `crates/comic-ai/**` | mutating project state; `doc`/`render`/`engine` | §9, §10.3, ADR-015/016 |
| `studio-ui-engineer` | `crates/ui-egui/src/panels/comic/**`, comic `UI_COMMANDS` | comic logic in a panel; a new UI crate | §3.1, §4.2, §14.1 |
| `verification-engineer` | `crates/*/tests/**`, golden fixtures, `engine/tests/` comic cases | production code | §12.3, §14, §17 |

## Which agents a milestone needs

| Milestone | Lead | Review | Schema | Command | Layout | Lettering | Publishing | AI | UI | Verify |
|---|---|---|---|---|---|---|---|---|---|---|
| M0 | ● | ● | ● | | | | | | | ● |
| M1 | ● | ● | ● | ● | ● | ● | | | | ● |
| M2 | ● | ● | | ● | ● | ● | | | | ● |
| M3 | ● | ● | ● | ● | | | | ● | | ● |
| M4 | ● | ● | ● | ● | | | | ● | | ● |
| M5 | ● | ● | | ● | | | | ● | | ● |
| M6 | ● | ● | | ● | ● | ● | | | ● | ● |
| M7 | ● | ● | | ● | | | ● | | | ● |
| M8 | ● | ● | ● | ● | ● | ● | ● | ● | ● | ● |

**Every milestone needs the reviewer**, because every milestone contains implementations.
Two rounds each, on `deepseek-v4.1-flash`, with unit tests.

## Hard boundaries

These are the mistakes that cost the most to unwind:

- **Nothing is done without two review rounds.** Round 1 on correctness, round 2 on the
  post-fix revision, both on `deepseek-v4.1-flash`, both with unit tests, both recorded in
  `docs/reviews/`. The lead spawns the reviewer; nobody reviews their own work.
- **The reviewer never fixes production code.** It reports with a reproduction and writes
  tests; the owner changes the implementation.
- **No comic data in the visual document.** No comic field in `crates/doc`, no comic
  section in `crates/format`, and never `Document::unknown` / `Document::extra`.
- **No duplicated geometry, page size or dialogue.** The artboard owns the page size, the
  document object owns the frame geometry, the comic record owns the id.
- **The layout crate computes and returns a plan.** It never takes `&mut Document`; the
  command module applies the plan, which is what keeps the result one undo step.
- **Only `comic.job.apply` writes.** A job produces an asset and a status.
- **The verifier does not fix code.** It reports to the owner with a reproduction.
