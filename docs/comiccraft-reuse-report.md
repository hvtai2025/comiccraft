# ComicCraft M1 — reuse report

Required by `comiccraft-porting-plan.md` §7's M1 exit criteria: *"the reuse report of §18 has
been produced."* Design §18 asks for a report listing **which functionality reused VectorCraft
unchanged, which required an adapter, and which required new code**, extending the capability
reuse matrix in [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md) §11 *"with what was
actually built."*

This is that report. It is written after the work, against the tree, not against the plan.

## 1. Method, and what makes this more than an opinion

The audit's §11 classified each requirement **before** anything was built (`R` reuse unchanged,
`A` needs a thin adapter, `N` new work in ComicCraft). This report compares that prediction with
what the milestone actually produced.

**The measurement that matters is the size of the change to existing code**, because that is what
"reuse" means operationally: a milestone that adds four crates and nineteen commands while leaving
the existing workspace nearly untouched has reused it; one that rewrites it has not.

`git diff --numstat` from the M0 commit (`73dbec11`), excluding the new comic crates:

| Existing file | ± |
|---|---|
| `crates/ui-egui/src/i18n/{es,fr,it,ja,ru,uk}.tsv` | +48 each |
| `crates/ui-egui/src/i18n/{zh-hans,zh-hant}.tsv` | +20 each |
| `crates/engine/src/lib.rs` | +50 −4 |
| `crates/ui-egui/src/io.rs` | +34 −1 |
| `crates/ui-egui/src/unsaved.rs` | +16 −6 |
| `crates/engine/src/cmd/edit.rs` | +10 −0 |
| `crates/engine/src/cmd/mod.rs` | +4 −0 |
| `Cargo.lock`, `Cargo.toml` files | manifest only |

**Total: roughly 442 added lines, of which 328 are translation catalog rows.** Strip the catalogs
and the entire footprint of a nineteen-command comic subsystem on the pre-existing engine and UI
is **about 114 lines across five files**.

The catalogs are not optional and not a choice: `menus.rs:3493-3494` inserts **every registered
command's `label`** into `menu_strings()` **regardless of its menu path**, and `MESSAGE_CRATES`
includes `engine`, so registering a command forces a row in every language. A reviewer verified
this against a proposed alternative (`menu: []`) and showed it would not have saved a line. It is
pinned by a test.

## 2. Reused unchanged (`R`)

The audit was right about these. Each was used through its existing public interface with no
modification to its crate, and in most cases with no modification to any *caller's* file either.

| Comic requirement | Reused mechanism | How M1 used it |
|---|---|---|
| Page canvas with a fixed page size | `Artboard.rect` in points | A page **is** an artboard (ADR-011, design §5.1.1); `comic.page.create` makes one |
| One document per project, page per artboard | multi-artboard documents | The visual half of a project is a single `.vectorcraft` document |
| Panel frames, any shape | `Path` / `Compound` / `Group` | A panel's frame is an ordinary path; `frame_node_id` points at it |
| Artwork placement, crop, clip | `ImageObject`, `object.cropImage`, clip groups, `Path.clipping` | `comic.panel.replace_art` builds a clip group whose first child copies the frame |
| Balloon and caption geometry | vector outlines + `TextKind::Area` | `comic-layout` computes the outline; the engine applies it |
| Text wrapping, padding, alignment, overflow, shrink-to-fit | `text::layout_with`, `LayoutOptions`, `TextLayout` | Lettering fitting calls `layout_with` **only** — it reimplements no text measurement |
| Multi-page print with trim, bleed and marks | `pdf::export`, one page per artboard | Acceptance test 10 parses the PDF via `vectorcraft_pdf::info` |
| Page rendering at print resolution | `Renderer::render_region`, `RasterExportOptions.ppi` | Acceptance test 10 reads the PNG's real IHDR |
| Storyboard thumbnails | `Renderer::render_thumbnail` | Available and unused by M1 — M2's storyboard |
| Stable semantic ↔ visual references | `NodeId(u64)`, never reused, serialized | Every comic → document reference is a `NodeId` |
| Undo/redo of composite edits | snapshot `History` + `UndoGroup` | One comic action is one `HistoryEntry` |
| Command registry, discovery, dispatch | static in-crate registry, `command_list` | Nineteen commands registered by one `v.extend` line |
| Headless batch and scripting | `vectorcraft-cli mcp --headless` | Acceptance test 12, through `Server::handle_line` **and** the built binary over a pipe |
| Golden-render regression tests | `testkit::raster` | Used by the existing suites M1's gate runs |

**The single strongest evidence of reuse is a test that did not have to be written.** The M0
hand-proof already drove `artboard.new → file.place → object.cropImage → object.maskImage →
text.create → document.serialize → document.export → document.exportPdf`. M1 added **no engine
rendering capability at all** — the comic layer decides *which* panels, frames and balloons exist,
and every pixel is produced by code that already existed before this milestone began.

## 3. Adapters (`A`)

Thin layers over existing mechanisms, where the prediction of `A` held.

| Adapter | Over what | Why it was needed |
|---|---|---|
| `comic-model`'s bundle reader/writer | `format` v4 documents | One project is a **directory bundle** (ADR-012), not one file. The document format is reused unchanged *inside* the bundle |
| Additive comic command modules | the registry | Design §10.1: no existing command changes semantics. Registration is one `mod comic;` and one `v.extend(comic::specs());` |
| Comic validation | `format`'s load path | `format` is L3, so document verification is **delegated**, not reimplemented |
| `HistoryEntry::comic` | per-`DocState` history | A comic step that touched only the model (e.g. `set_shot`) must undo like a geometric one. `+10 −0` in `edit.rs`, a no-op when `comic` is `None` |
| `is_dirty()` covering the model | the existing dirty flag | A model-only edit is unsaved work; without this, `⌘S` looked satisfied while the comic half was not |
| `⌘S` routing | `io::save` | With a project open, `⌘S` saves the **project**, not just the document — an **app-level binding**, so `document.save` is untouched and no registry shortcut shadows Save |
| i18n catalog rows | the existing message system | Forced by `menus.rs:3493-3494` for every registered command, in every language |

**The most instructive adapter is the one the audit warned about.** Audit §19 notes that
`LinkInfo.relative` is relative to the **document's folder** and may contain `..`, while a comic
`StorageRef` path is **bundle-relative** — *"two different bases; confusing them is a path-traversal
bug."* M1's bundle layer keeps them distinct, and `comic-model` exposes `path_within` and
`join_relative` so a caller cannot silently conflate them.

## 4. New work (`N`)

| New | Where | Note |
|---|---|---|
| The narrative and production model | `comic-model` | Entity types, ids, 18 referential rules, canonical JSON, bundle I/O, v1 frozen fixture |
| Deterministic page templates and reading order | `comic-layout` | Five templates, bounded grid sides, per-row `rtl`, five geometry checks, relative tolerance |
| The lettering **policy** layer | `comic-layout` | Six kinds, tails, fitting and overflow policy — a decision layer over the existing text engine, not a text engine |
| Comic validation | `comic-model` + `comic-layout` | Design §12.1's checks table, split by ownership |
| The `comic.*` command surface | `crates/engine/src/cmd/comic/**` | 19 commands, 3457 lines |
| Comic lettering faces | `storytold/craft-fonts` | **ADR-017**: Comic Neue + Bangers, OFL-1.1, pinned and checksummed. Audit gap 12.5 |
| Comic-specific jobs and providers | `comic-ai` (registered, L3) | Not implemented in M1 — ADR-015/016 scope, M2+ |

**Audit gap 12.5 is now closed, and closing it exposed a real bug.** The audit recorded *"bundled
faces are UI/body text only; comic faces must come from `craft-fonts`."* Adding the faces was not
sufficient: they were **embedded but unreachable**, because every `CRAFT_FONTS` consumer filtered
to Japanese and Arabic. The fix added `craft_fonts::comic_fonts()` and chained it into
`FontDb::new()`. *Faces present but never consulted* is the kind of gap a matrix predicts and only
a test finds.

## 5. Predictions that were wrong

Recording these matters more than recording the ones that held.

| Audit §11 said | Reality | Evidence |
|---|---|---|
| `N` — *"no existing command registration seam"* (§12.2) | **Correct, and it is a wall, not a hill.** There is no external hook, so comic commands are additive modules **inside `crates/engine`**. This is why the registry line is `+4 −0` | The design accommodates it; the audit's reading held |
| `A` — *"cross-page single undo step: per-document history means one document per project is required"* | **Correct, and it became ADR-011.** M1 took the constraint seriously rather than working around it, and the constraint propagated into the wire format | One `.vectorcraft` per project; the page is the artboard |
| `A` — *"asset registry: `LinkInfo` already holds … provenance and a cryptographic hash are new"* | **Held**, and the bundle reader/writer needed SHA-256 over project files too | `sha256_hex` in `comic-model::json` |
| `R` — *"undo/redo of comic operations"* | **True but incomplete.** It works, and it is `R`, but it needed an **adapter** for model-only steps and a **second** fix once undo rewound the "what we wrote" record that save pruning depends on | `HistoryEntry::comic`; `DocState::saved_comic` |
| `R` — *"panel artwork placement"* | The **operation** is reachable and meets the criteria; the **input** is reduced. From a **fresh project** over MCP an agent **cannot reach `comic.panel.replace_art` at all** — there is no `comic.asset.*` in M1, `file.place` links nothing, and `Embedded`/`Package` assets are refused by name. It *is* reachable against a bundle that already carries a `Linked` asset record, which a verifier demonstrated by driving it over the pipe | Acceptance test 12 |
| *(not in the matrix)* | **A latent bug in the existing quit path, affecting any document, not just comics.** The `is_dirty()` re-check after a save ran **only for a background save**, so answering "Save" at the quit prompt could discard a dirty document | `unsaved.rs` — see §6 |

## 6. Divergences from upstream, and what should go back

M1 touched existing code in five places. Four are comic-gated; **one is not, and it is a bug fix
upstream should want.**

1. **`crates/ui-egui/src/unsaved.rs` — not comic-specific.** The post-save `is_dirty()` re-check
   was reachable only when `r["background"] == true`. A foreground save therefore left
   `is_dirty()` unexamined, and the quit prompt could discard work it had just claimed to save.
   **This is an upstream defect.** M1's fix makes the check run on both paths; the invariant is now
   *"after the prompt's Save, `is_dirty()` is false and the document closes, **or** the close is
   refused — never both."* It should be offered upstream as an independent fix, because it is worth
   more to VectorCraft than to ComicCraft.
2. `crates/engine/src/lib.rs` (+50 −4) and `crates/engine/src/cmd/edit.rs` (+10 −0) — comic-gated,
   no-op when `comic` is `None`. Verified as a no-op by a reviewer's mutation (**M-A**): replacing
   the guard with a plausible `std::mem::replace` silently closes the project on undo, and the
   whole engine suite still passes — so the guarantee is now pinned by tests, not by inspection.
3. `crates/engine/src/cmd/mod.rs` (+4 −0) — one `mod` line, one `v.extend` line.
4. `crates/ui-egui/src/io.rs` (+34 −1) — the `⌘S` binding, entered only when a comic project is
   open.
5. i18n catalogs — forced by registration, in every language.

## 7. What M1 did *not* reuse, and what remains genuinely new

The audit's three real gaps, restated with M1's outcome:

| Gap | Outcome |
|---|---|
| No external command registration seam | **Unchanged — still a wall.** Comic commands live inside `crates/engine`. Accepted, not solved |
| No networking dependency anywhere in the workspace | **Unchanged.** `comic-ai` is registered and empty of providers. ADR-015 keeps it feature-gated |
| No storyboard or page navigator | **Unchanged.** M1 has no UI panels for comics; the commands are reachable through the registry and MCP |

**A fourth gap the audit did not have, found by this milestone:** MCP accepts
`notifications/progress` and `notifications/cancelled` and **ignores both**, so generation must be
**pollable** (ADR-016). M1 did not need it; M2 does.

## 8. Conclusion

**The audit's central conclusion — *"most of the comic pipeline already exists"* — survived contact
with the implementation, and the measurement is the strongest thing in this report.** Nineteen
commands, four crates, a frozen wire format and twelve acceptance tests were added while changing
**about 114 lines of pre-existing production code**, two of those files being a guard that is a
no-op by construction.

**But "reuse" was not free, and the failures were not where the matrix predicted them.** Every
defect that mattered in this milestone was in a **boundary**: the manifest as *description* versus
as *record of the save*; a fuzzy-`f64` guarantee the code asserted exactly; a geometry tolerance
whose evaluation order silently disabled the check it existed to perform; a dialogue whose
`panel_id` is required, so "detached" could not mean what the policy said; an undo that rewound a
record the save path depended on. The matrix predicted **capability** gaps accurately and said
nothing about **semantic** gaps, because those only appear where two reused mechanisms meet — and
that is where all the work actually was.
