# Dispatch table

Source of truth for who owns what. Mirrors
`.github/instructions/comiccraft-work.instructions.md`; if they disagree, the instruction
file wins and this table is wrong.

| Agent | Owns (module tree) | Must not touch | Design sections |
|---|---|---|---|
| `comiccraft-lead` | `package.json`, `server.js`, `eslint.config.js`, `shared/domain/index.js`, `shared/commands` wiring, `docs/**`, ADRs | any specialist's module tree | §4, §16, §17 |
| `code-reviewer` | `docs/reviews/**`, test code in the reviewed diff | **production code** — it reports, the owner fixes | §12.3, §17 |
| `schema-architect` | `shared/domain/**` (model, ids, migrations, document, format), `schema/**`, `docs/comiccraft-project-schema.md` | any comic field in the document or its format | §5, §6.1, §7, ADR-011/012/013 |
| `command-engineer` | `shared/commands/**`, `server/commands/**` | existing commands' semantics | §10, §11 |
| `layout-engineer` | `shared/domain/{layout,panel,template,reading-order,validate}/**` | mutating the document; geometry in a comic record | §5.1.1, §6.1, §8.1, §8.2 |
| `lettering-engineer` | `shared/domain/{lettering,fit}/**` | writing a text engine; adding font files; measuring text directly instead of through the injected function | §5.2, §6.1, §8.3, §12.1 |
| `publishing-engineer` | `shared/domain/publish/**`, `server/export/**` | writing a renderer, PDF writer or encoder from scratch | §7.5, §9.4, §12.1, §13 |
| `ai-pipeline-engineer` | `server/ai/**`, `server/jobs/**` | mutating project state; leaking a key or a provider path into the client | §9, §10.3, ADR-015/016 |
| `studio-ui-engineer` | `public/**` | comic logic in a panel; adding a framework, bundler or JSX transform | §3.1, §4.2, §14.1 |
| `verification-engineer` | `test/**`, golden fixtures, the acceptance suite | production code | §12.3, §14, §17 |

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
- **No comic data in the visual document.** No comic field in the document model, no comic
  section in its format, and never a round-trip pocket meant for foreign data.
- **No duplicated geometry, page size or dialogue.** The artboard owns the page size, the
  document object owns the frame geometry, the comic record owns the id.
- **The layout module computes and returns a plan.** It never mutates the document; the
  command handler applies the plan inside one transaction, which is what keeps the result
  one undo step.
- **Nothing in `shared/` reaches for a runtime.** No DOM global, no `node:*` import, no
  `fetch`. Anything the domain needs is injected. This is the boundary that makes the whole
  architecture work, and it is checked by a lint rule *and* a test.
- **Only `comic.job.apply` writes.** A job produces an asset and a status.
- **The verifier does not fix code.** It reports to the owner with a reproduction.
