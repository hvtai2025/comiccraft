# M1 W5 — the twelve acceptance tests, by `verification-engineer`

**Status:** complete; all eight round-1 findings actioned (§11), and the milestone gate is green on all seven steps (§8) as of round 2
**Round 2:** round 1's verdict was *changes-requested* (2 defects, 6 notes) — see `docs/reviews/M1-acceptance-review.md`. §11 records what each finding changed and the red output of every wrong implementation this suite now fails against. Round-1 text below is left as measured, with the values the tree has since moved past annotated in place.
**Milestone:** M1 (porting plan §7, Phase 1) · work item W5
**Owner:** `verification-engineer`
**Specification:** `docs/milestones/M1.md` §1 — the acceptance-test table, and the exit criteria it proves
**Design refs:** design §5.1.1 (page = artboard), §5.3.1 (the artboard-id hazard and its five mitigations),
§6.1 (ownership), §8.1/§8.2 (plan then write; the frame clips the artwork), §10.2 (the command contract),
§11.1 (the related-entity policy), §11.2 (one action = one undo step), §12.1 (the checks table);
schema §4, §5.7, §9.1, §10, §13 rules 1–18, §14
**Revision measured:** the working tree, not a commit. `sha256sum` of every file involved is in §9; the round-2 values are in §11.
Production code changed by me: **none** (§9, §11).

---

## 1. What was built

Three new test files, all additive:

| File | What it is |
|---|---|
| `crates/engine/tests/comic_acceptance.rs` | acceptance tests 1–11, driven through `Session::execute`, each asserting on the artefacts (the bundle on disk, the document, `comic.project.validate`, the rendered PNG, the exported PDF) rather than on the implementation |
| `crates/mcp/tests/comic_headless.rs` | acceptance test 12, driven through the MCP server `vectorcraft-cli mcp --headless` runs |
| `crates/ui-egui/tests/comic_quit_prompt.rs` | the ⌘S / quit-prompt evidence of §6, driving the real UI path (`unsaved::close_all` / `unsaved::confirm`) |

Every test drives the public command surface and asks the *artefacts* whether the criterion holds. Where
a criterion names a user action — "the user moves and resizes a panel on the canvas" — the test drives both
the comic path (`comic.panel.move` / `.resize`) and the plain document path (`object.transform`), because
those are two different users of the same record. Nothing asserts on a private field, a helper function or
an internal sequence: if a test could only pass by mirroring the implementation, it is not one of these.

Raw run:

```text
$ cargo test -p vectorcraft-engine --test comic_acceptance
running 15 tests
test acceptance_1_one_document_and_one_artboard_per_page ... ok
test acceptance_2_three_panels_with_stable_ids_and_real_frames ... ok
test acceptance_3_artwork_is_placed_clipped_and_cropped ... ok
test acceptance_4_move_and_resize_keep_the_panel_identity_art_letters_and_order ... ok
test acceptance_5_dialogue_update_touches_no_artwork_and_no_typography ... ok
test acceptance_6_replace_art_leaves_every_other_object_and_the_previous_asset_intact ... ok
test acceptance_7_two_balloons_and_a_caption_are_lettering_records_with_editable_text_and_frames ... ok
test acceptance_8_every_operation_is_exactly_one_undo_and_redo_step ... ok
test acceptance_9_save_close_reopen_keeps_every_id_and_the_saved_layout ... ok
test acceptance_10_render_dimensions_and_one_pdf_page_per_artboard_in_reading_order ... ok
test acceptance_10b_page_reorder_orders_the_artboards_and_moves_nothing_on_the_canvas ... ok
test acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it ... ok
test evidence_a_bundle_with_no_default_page_size_refuses_to_invent_a_size ... ok
test evidence_a_degenerate_artboard_is_reported_when_the_project_names_no_default_size ... ok
test evidence_cmd_s_leaves_the_manifests_hash_for_the_document_stale_and_a_reopen_says_so ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

$ cargo test -p vectorcraft-mcp --test comic_headless
running 1 test
test acceptance_12_the_essential_operations_succeed_through_mcp_headless ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

$ cargo test -p vectorcraft-ui-egui --test comic_quit_prompt
running 2 tests
test evidence_the_quit_prompts_save_discards_a_model_only_comic_edit ... ok
test evidence_the_same_quit_keeps_the_edit_after_comic_project_save ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

*Round 1's revision, as measured. **Round 2 (§11) moved two of these three files**: the quit-prompt file
now has **three** tests (the evidence test is inverted and renamed, plus a refusal arm), and test 7 and
test 11 below changed shape. The current run, with counts, is §11. Everything else is unchanged and still
passes.*

---

## 2. The twelve tests

| # | Test | What it drives | Result |
|---|---|---|---|
| 1 | `acceptance_1_one_document_and_one_artboard_per_page` | `comic.project.create` + three `comic.page.create`; counts `.vectorcraft` files under the bundle, loads the bundle, and checks every page has its own artboard of the page's size | pass |
| 2 | `acceptance_2_three_panels_with_stable_ids_and_real_frames` | three `comic.panel.add`; each record's `frame_node_id` resolves to a document `Path` inside the artboard with the rectangle asked for, then `comic.page.template.apply` rewrites every frame and the ids, the panel list and the reading order are unchanged | pass |
| 3 | `acceptance_3_artwork_is_placed_clipped_and_cropped` | a `Linked` asset, `comic.panel.replace_art` (clip group whose first child copies the panel frame), then `object.cropImage` in document space; the clip is still the frame and the crop is whole source pixels | pass |
| 4 | `acceptance_4_move_and_resize_keep_the_panel_identity_art_letters_and_order` | `comic.panel.move` translates the frame **and** its owned artwork and balloon; `.resize` changes the frame only; then a plain `object.transform` of the frame. The record — ids, artwork refs, dialogue, reading order — is byte-identical after all three | pass |
| 5 | `acceptance_5_dialogue_update_touches_no_artwork_and_no_typography` | `comic.dialogue.update`; the words change in the record and on the page, and the frame path, tail path, `TextLayoutSettings`, `overflow_policy`, the text object (bar its runs and its layout cache) and the artwork are unchanged | pass |
| 6 | `acceptance_6_replace_art_leaves_every_other_object_and_the_previous_asset_intact` | two panels with art and a caption; replacing one panel's art deletes exactly its old clip group, leaves the other panel's nodes, refs and caption, and leaves every `Asset` record in the registry | pass |
| 7 | `acceptance_7_two_balloons_and_a_caption_are_lettering_records_with_editable_text_and_frames` | two speech balloons + one narration **+ one sound effect** (R1-7); four `Lettering` records, rule 8's ownership, closed `Path` frames with ≥ 3 anchors, no tail for the sound effect; the text is editable through `dialogue.update` / `lettering.set_text` and the frame through an ordinary transform | pass |
| 8 | `acceptance_8_every_operation_is_exactly_one_undo_and_redo_step` | sixteen mutating commands, each run through `one_step`: exactly one journal entry, one undo restoring document + selection + model, one redo restoring them | pass |
| 9 | `acceptance_9_save_close_reopen_keeps_every_id_and_the_saved_layout` | full page, `comic.project.save`, `close_document`, `comic.project.open`; every id, every frame rectangle, the reading order and every document node survive, and `comic.project.validate` reports 0 errors | pass |
| 10 | `acceptance_10_render_dimensions_and_one_pdf_page_per_artboard_in_reading_order` | three pages of different sizes, `comic.page.reorder` to book order `[2, 1, 3]`; each artboard renders at 72 ppi to the page's dimensions, and `document.exportPdf` parses (`vectorcraft_pdf::info`) to three pages whose sizes are the three pages' sizes in that order | pass |
| 10b | `acceptance_10b_page_reorder_orders_the_artboards_and_moves_nothing_on_the_canvas` | the second half of test 10 (see §7): the document's artboards really are put in book order, a spare artboard stays after them, no artboard rectangle moves, and an out-of-range `to` is refused | pass |
| 11 | `acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path` | design §5.3.1 — **failing-then-passing**, and now against **three** wrong implementations (see §3 and §11) | pass |
| 12 | `acceptance_12_the_essential_operations_succeed_through_mcp_headless` | the same operations over MCP, see §4 | pass |

Two tests beyond the twelve are labelled `evidence_*` and are the lead's hand-carried questions (§6, §7).
They are not part of the exit criteria and are named so they cannot be mistaken for them.

---

## 3. Test 11 — the failing-then-passing demonstration

### The test

`acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it` drives the
hazard through **supported commands only**, with no hand-edited state:

```text
comic.project.create
comic.page.create                      → page 6, artboard 1
comic.page.create {size:[200,200]}     → page 7, artboard 2
comic.page.delete {page:7}             → artboard 2 removed, and its id RETIRED (mitigation 2)
artboard.new {…200×200}                → the ordinary, still-registered command hands id 2 straight back
comic.page.create {size:[250,250]}     → must NOT take id 2
```

and then asserts, of the new page: its `artboard_id` is not in `manifest.retired_artboard_ids`; it is `3`;
no page names 2; the project reports 0 errors; and the same holds after a save and a reopen.

### The plausible-but-wrong implementation

**M-W5-1** — move the retirement check out of `free_artboards` and leave it only in the allocation loop:

```diff
--- a/crates/engine/src/cmd/comic/page.rs
+++ b/crates/engine/src/cmd/comic/page.rs
@@ -152,7 +152,7 @@ fn free_artboards(s: &Session) -> Result<Vec<u32>> {
     let b = &open(s)?.bundle;
     let used: std::collections::BTreeSet<u32> = b.pages.values().map(|f| f.page.artboard_id).collect();
-    Ok(s.doc()?.doc.artboards.iter().map(|a| a.id).filter(|id| !used.contains(id) && !b.manifest.is_artboard_retired(*id)).collect())
+    Ok(s.doc()?.doc.artboards.iter().map(|a| a.id).filter(|id| !used.contains(id)).collect())
 }
```

It is plausible because it reads correctly: *"the artboard was just removed, so a free one cannot be
retired"*. The allocation loop below still checks `retired`, so the guard looks present. It is wrong because
`artboard.new` is registered, reachable while a project is open, and gives the retired number back to the
document — after which the new page takes an id the manifest says **never again**, and shares page identity
with the page that was deleted.

### The red output

```text
$ cargo test -p vectorcraft-engine --test comic_acceptance acceptance_11
running 1 test
test acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it ... FAILED

failures:

---- acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it stdout ----

thread 'acceptance_11_…' panicked at crates/engine/tests/comic_acceptance.rs:780:5:
FAIL (§5.3.1 mitigation 4): comic.page.create gave page 8 the retired artboard 2, the id the deleted page 7 used. A retired number means "never again": rules 4 and 17 make a page that takes one permanently reported and permanently unfixable

failures:
    acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.01s
error: test failed, to rerun pass `-p vectorcraft-engine --test comic_acceptance`
```

*R1-8: that `:780` was taken before the file was edited again, and the delivered revision red at 832; round
2 restructured the test, so the line reads differently again (892 for this message). **The line number is
not reproducible and should not be quoted — the message is.** Round 2's re-taken reds for this message and
for the two wrong implementations round 1 found unheld are in §11.*

### It fails for the right reason, and nothing else does

The mutation was applied **alone**, run, and reverted immediately; `sha256sum -c` on
`crates/engine/src/cmd/comic/page.rs` confirms it is back to `b1a1c888…`, the hash it had before (§9).

Under that wrong implementation the rest of the engine suite is **green**:

```text
$ cargo test -p vectorcraft-engine --lib cmd::comic
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 1579 filtered out; finished in 0.22s

$ cargo test -p vectorcraft-engine --test comic_commands_r1
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

$ cargo test -p vectorcraft-engine -- --skip acceptance_11
test result: ok. 1603 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s
   … 113 test targets, all ok …
```

That is the point of the test, and it is the same standard W2's reviewer set with **M-CLAMP** and W4's
reviewer repeated with **M-A**/**M-B**: 24 implementer tests and 14 reviewer tests were plentiful and green,
and the *guarantee* — mitigation 4 — was still unheld. Only the acceptance test reds. Asked the standard's
question — *"could a wrong implementation satisfy this test?"* — the answer here is **no**, and the proof is
the mutation above.

**R1-3 (round 2): that answer was only true of one half, and the test now covers both.** Round 1's reviewer
found that dropping the retirement check from `claim_artboard`'s **loop** (`page.rs`, the path taken when no
artboard is free) left all fifteen acceptance tests green, and that a third wrong implementation — the loop
no longer skipping a number a **page still names** after `artboard.delete` removed the artboard behind it —
left them green too. `acceptance_11` now drives all three, and fails against all three; the two new red
outputs are in §11.

---

## 4. Test 12 — `mcp --headless`, done properly

### How it is driven

`vectorcraft-cli mcp --headless` is `Server::new(Box::new(Headless::with_document()))` reading JSON-RPC 2.0
off stdio (`apps/vectorcraft-cli/src/main.rs::mcp`). `crates/mcp/tests/comic_headless.rs` drives that exact
server through `Server::handle_line` — the same entry point `Server::serve` uses per line — with
`tools/call` → `run_command`, which is the path an agent takes. There is no engine back door in the test:
every operation is a tool call, and every assertion reads an artefact.

The test, in order:

1. `initialize` + `notifications/initialized`; `tools/list` must offer `run_command`.
2. `list_commands {filter:"comic."}` → **19** commands, and with no project open **exactly two** are
   enabled (`comic.project.create`, `comic.project.open`) — the rest report disabled, and
   `comic.page.create` is refused with *"no comic project open"*.
3. `comic.project.create`, three `comic.page.create` (one with a template), `comic.page.template.apply`
   → three panels; `comic.panel.set_shot` / `.move` / `.resize`; two `comic.lettering.add` (speech +
   narration); `comic.dialogue.update`.
4. Artwork over MCP: `file.place` with an inline PNG, `object.cropImage`, `object.maskImage`.
5. `undo` / `redo` through the tool, with `inspect_document`'s `history` before, during and after.
6. `comic.project.validate` → `errors: 0`, and `geometry` names one artboard per page.
7. `comic.project.save` → the file list names `document/comic.vectorcraft` and `pages/page-*`; `dirty` is
   false; and the **page files on disk** are parsed to check that the `PanelId`s, `LetteringId`s,
   `DialogueId`s and the revised dialogue text the commands returned are the ids in the wire format.
8. `open_file` + `comic.project.open`, then `comic.project.validate` again: the page ids, artboard ids and
   page sizes are identical to before, and the reopened project still answers to the same `PanelId` and
   `LetteringId` (`set_shot`, `set_text`).
9. `comic.page.reorder`, then **render**: `export {format:"png", range:"n"}` per page, and the PNG's IHDR is
   read (no decoder) to check the page's dimensions; then **PDF**: `export {format:"pdf"}` with
   `preserveEditing:false`, and the PDF is re-opened through `document.open` — a real page read, asserted by
   `restored != true` — and `inspect_document`'s artboards are the three page sizes **in reading order**.

### The real binary, not only the in-process server

The same operations were driven through the built `target/debug/vectorcraft-cli mcp --headless` over a pipe
(a JSON-RPC client reading the server's stdout line by line). Raw transcript, abridged to the replies:

```text
-> initialize {"protocolVersion": "2025-06-18", …}
<- {"id":1,"jsonrpc":"2.0","result":{"capabilities":{…},"instructions":"VectorCraft is a professional vector illustration app. …"}}
-> tools/call {"name": "list_commands", "arguments": {"filter": "comic."}}
<- {"id":2,…,"result":{"content":[{"text":"{\n  \"commands\": [\n    {\n      \"enabled\": true,\n      \"id\": \"comic.project.create\", …
comic commands listed: 19
-> tools/call {"name": "run_command", "arguments": {"command": "comic.project.create", "params": {"path": "…/book.comiccraft", "title": "CLI book", "pageSize": [612, 792]}}}
<- {"id":3,…"text":"{\n  \"artboards\": 1,\n  \"chapter\": 4,\n  \"comic_id\": 2,\n  \"document\": \"document/comic.vectorcraft\",\n  \"files\": [… 10 files …]}"}}
create artboards: 1
-> … {"command": "comic.page.create", "params": {}}
<- {"id":4,…"text":"{\n  \"artboard_id\": 1,\n  \"artboard_rect\": [ 0.0, 0.0, 612.0, 792.0 ],\n  \"page\": 6, …}"}
-> … {"command": "comic.page.template.apply", "params": {"page": 6, "template": "three-tier"}}
<- {"id":5,…"text":"{\n  \"findings\": [],\n  \"page\": 6,\n  \"panels\": [ { \"frame\": 2, \"index\": 0, \"panel\": 7 }, … ]}"}
-> … {"command": "comic.lettering.add", "params": {"page": 6, "panel": 7, "kind": "speech", "text": "Through the real binary."}}
<- {"id":6,…"text":"{\n  \"bounds\": [ … ],\n  \"dialogue\": 11,\n  \"frame\": 2, …}"}
-> … {"command": "comic.dialogue.update", "params": {"dialogue": 11, "text": "Through the real binary, revised."}}
<- {"id":7,…"text":"{\n  \"approval\": \"pending\",\n  \"dialogue\": 11,\n  \"geometryTouched\": false, …}"}
-> … {"command": "comic.project.validate", "params": {}}
<- {"id":8,…"text":"{\n  \"errors\": 0,\n  \"findings\": [],\n  \"geometry\": [ { \"artboard_id\": 1, \"page\": 6, \"rect\": [0.0, 0.0, 612.0, 792.0] } ], …}"}
validate: {"errors": 0, "warnings": 0, "geometry": [{"artboard_id": 1, "page": 6, "rect": [0.0, 0.0, 612.0, 792.0]}]}
-> … {"command": "comic.project.save", "params": {}}
<- {"id":9,…"text":"{\n  \"document\": \"document/comic.vectorcraft\",\n  \"files\": [ \"assets/index.json\", \"document/comic.vectorcraft\", \"pages/page-6.json\", \"project.json\", \"provenance/generations.json\", \"story/chapters.json\", \"story/characters.json\", \"story/locations.json\", \"story/scenes.json\", \"story/styles.json\" ], …}"}
save files: ["assets/index.json", "document/comic.vectorcraft", "pages/page-6.json", "project.json", "provenance/generations.json", "story/chapters.json", "story/characters.json", "story/locations.json", "story/scenes.json", "story/styles.json"]
-> tools/call {"name": "export", "arguments": {"format": "pdf", "path": "…/book.pdf", "options": {"preserveEditing": false}}}
<- {"id":10,…"text":"{\n  \"bytes\": 9352,\n  \"format\": \"pdf\",\n  \"path\": \"…/book.pdf\", …}"}
pdf bytes: 9352
-> … {"command": "document.open", "params": {"path": "…/book.pdf"}}
<- {"id":11,…"text":"{\n  \"format\": \"pdf\",\n  \"index\": 2,\n  \"missingLinks\": [], …}"}
-> tools/call {"name": "inspect_document", "arguments": {}}
<- {"id":12,…"text":"{\n  \"artboards\": [ { \"height\": 792.0, \"name\": \"Artboard 1\", \"width\": 612.0, \"x\": 0.0, \"y\": 0.0 } ], …}"}
pdf artboards: [{"height": 792.0, "name": "Artboard 1", "width": 612.0, "x": 0.0, "y": 0.0}] restored: False
```

### What it proves

The comic surface is reachable and complete over MCP: all nineteen commands are listed, gated honestly, and
driven end to end; the artefacts they produce are the same artefacts the engine-side acceptance tests check
(one document, one artboard per page, real frames, editable lettering, one undo step per action, every id
preserved across a save and a reopen, a PDF page per artboard in reading order). **Test 12 is no longer
unverified.** Two things it also shows, which are honest properties rather than gaps: exactly two of the
nineteen commands work without a project, and — **as written, this was wrong, and R1-2 corrected it** —
`comic.panel.replace_art` cannot be driven through MCP at all because M1 has no `comic.asset.*` command.
The second half is true; the first is not. What is unreachable over MCP is *creating the `Linked` `Asset`
record*, not the command: against a bundle that already carries one, `comic.panel.replace_art` runs over the
pipe and over `Server::handle_line`. Round 2 added that step, so the artwork criterion ("place and clip
raster artwork **inside panels**") is now covered through MCP as well as through the document path
(`file.place` + `object.maskImage` + `object.cropImage`) — see §11.

---

## 5. Defects found

**D1 — answering "Save" at the quit prompt discards a model-only comic edit.** *(bug: data loss)*

*Round 2: **fixed** by the `command-engineer` (`unsaved.rs` re-checks `is_dirty()` on both save paths;
`io::save` routes a plain Save to `comic.project.save` when a project is open). The evidence test below was
the reproduction and asserted the **loss**, so it was green while the defect existed and red once it was
fixed; R1-1 inverted and renamed it. See §6 and §11.*

* Where: `crates/ui-egui/src/unsaved.rs:78-95` (`confirm`) with `crates/ui-egui/src/unsaved.rs:86-92`; the
  ⌘S binding is `crates/ui-egui/src/menus.rs:69` (`"Cmd+S"` = `document.save`).
* Expected: the user is asked because `is_dirty()` now counts the comic model (W4's F5), so answering Save
  must either save that model or ask again. Actual: `confirm` saves the **document**
  (`io::save(SaveMode::Save)`), then `close_now` closes the document; the `is_dirty()` re-check that would
  stop it only runs in the `r["background"] == true` branch, so a synchronous save closes a still-dirty
  model. The edit is gone.
* Smallest reproducer: `crates/ui-egui/tests/comic_quit_prompt.rs::evidence_the_quit_prompts_save_discards_a_model_only_comic_edit`
  — and its control, `evidence_the_same_quit_keeps_the_edit_after_comic_project_save`, which is the same
  quit with the one save that does write the model.
* Not fixed here: W5 reports, it does not repair. Recommendation in §6.

**D2 — the ⌘S-then-reopen manifest warning.** *(note, not a defect)*

`document.save` writes `document/comic.vectorcraft` and nothing else, so `manifest.json` goes on describing
the previous generation of that file. §14 step 1 reports it, as a warning on a usable project. That is the
consistency record doing its job, not a bug — but it is user-visible, and it is the second half of §6's
question. Evidence: `evidence_cmd_s_leaves_the_manifests_hash_for_the_document_stale_and_a_reopen_says_so`,
which prints both hashes and the finding:

```text
manifest sha256 c0089ed8b854638cd58e1310a2176cc620ae6f4f077b4a5e20c5e139f2b4919e
file     sha256 26f85ccc540cc917a4688105dc91cdc8b315bfc7d3d684a0f3556e628a63a75c
reopen findings: [{"message":"`manifest.files` records sha256 c0089ed8… (23337 bytes) but the file is 26f85ccc… (23622 bytes): the manifest describes the previous generation (§14)","rule":null,"severity":"warning","subject":"document/comic.vectorcraft"}]
```

*R1-8: the `file` hash above is **not reproducible and must not be quoted as a value** — `document.save`
stamps the document with the current time, so the same run produces different bytes every time. Round 2
re-ran it: `c0089ed8… (23337 bytes)` and `(23622 bytes)` are stable, and the file hash was
`5e6ed4f7…` in that run. The **manifest** hash, both byte counts and the §14 warning are the reproducible
part; the finding's own claim is about the mismatch, not about the value. Round 2's run is in §11.*

**D3 — not mine, but it is in the tree and it is red.** *(observation)*

`crates/engine/tests/comic_commands_r2.rs::r2_a_page_file_written_since_the_load_is_destroyed_by_the_next_save`
(W4's round-2 reviewer) was red when I began — a real finding: the round-1 F7 fix protected a stray `pages/`
file for exactly one save, because the save then wrote a manifest describing it. The W4 round-2 fix landed
while I worked (a `files_of_record` field on `ComicBundle`), and the test is green at the revision in §9. I
did not touch it and I did not fix it.

**D4 — an engine message literal was extended without its catalog rows, and the round-2 record says
otherwise.** *(bug: the milestone gate is red)*

* Where: the literal at `crates/engine/src/cmd/comic/lettering.rs:609`, and the six rows in
  `crates/ui-egui/src/i18n/{es,fr,it,ja,ru,uk}.tsv`.
* Expected: a message that reaches a user through `EngineError::Other` has an `@msg` row in every
  message-complete catalog, with the same English key (the rule W4 itself recorded in its report §11 and
  §R-7, and had already had to satisfy once for the R-7 message).
* Actual: the literal was **extended** with a second sentence; all six catalogs still carry the short key
  `"lettering {_1} names dialogue {_2}, which is not on page {_3}"`, so
  `i18n::tests::complete_languages_translate_every_message` fails for each of them (`es` is reported first):

```text
$ cargo test -p vectorcraft-ui-egui --lib i18n::tests::complete_languages_translate_every_message
thread 'i18n::tests::complete_languages_translate_every_message' panicked at crates/ui-egui/src/i18n/tests.rs:870:9:
es: 1 untranslated messages: [
    "lettering {_1} names dialogue {_2}, which is not on page {_3}: comic.lettering.set_text edits the record in the balloon's own page, and a dialogue owned by another page's panel belongs to that page — use comic.dialogue.update {{dialogue}} to change its words",
]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 873 filtered out; finished in 0.13s
```

* The stale rows, one per catalog:

```text
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	la rotulación {_1} nombra el diálogo {_2}, …
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	le lettrage {_1} nomme le dialogue {_2}, …
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	il lettering {_1} nomina il dialogo {_2}, …
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	レタリング {_1} が指定するダイアログ {_2} は…
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	леттеринг {_1} указывает диалог {_2}, …
@msg	lettering {_1} names dialogue {_2}, which is not on page {_3}	летеринг {_1} вказує діалог {_2}, …
```

* The round-2 record claims the opposite: §R2-11, "**The catalogs** were not touched (no `@msg` row, no label
  changed), so the eight-catalog and native-speaker caveats carry over unchanged from round 1." That is true
  of *labels* and false of *messages*: the message changed, and the catalogs therefore needed the change.
* Smallest reproducer: the single test command above; and
  `grep -P "^@msg\tlettering \{_1\} names dialogue" crates/ui-egui/src/i18n/*.tsv`.
* Not fixed here: W5 reports, it does not repair — and these are production files.
* *Round 2: **fixed**, on the catalog side only (`lettering.rs` is still `71bfa9f8…` — the literal is the
  English key, so the rows were the stale half):
  `cargo test -p vectorcraft-ui-egui --lib i18n::tests::complete_languages_translate_every_message` →
  `test result: ok. 1 passed; 0 failed; 873 filtered out`. **The six catalog hashes in §9 are therefore
  stale**, and are annotated there with their old values (R1's stale-pin sweep).*

**D5 — a mutation command left debris in the repository root.** *(note: tree hygiene; reported, not removed)*

A 30-byte untracked file named `ritten.contains(key);|XX|` sits in the repository root, created at 15:03 and
containing a fragment of `comic-model`'s `bundle.rs` guard:

```text
$ ls -la --time-style=full-iso 'ritten.contains(key);|XX|'
-rw-rw-r-- 1 hvt hvt 30 2026-10-11 15:03:00.810241252 +0700 ritten.contains(key);|XX|
$ cat 'ritten.contains(key);|XX|'
|| written.contains(key);
```

It is the shell accident of a `sed`/`grep` mutation against `is_written_by_this_version`'s
`written.contains(key)` — the same class of hazard M1 §5.5 records with `.review3-tmp/backup/`, and the same
lesson: a mutation that escapes into the tree leaves something another reader cannot tell from intent. It is
**not mine** (I never touched that guard, and at 15:03 I was running `cargo xtask ci`). I have left it where
it is rather than delete another owner's artefact — a verifier who tidies destroys the evidence — but it
should not be committed, and `git status` should be clean before the milestone closes.

**Nothing else.** No acceptance test needed a production change; nothing contradicted the exit criteria; and
no test of mine reds on the revision in §9.

---

## 6. The ⌘S / `is_dirty` question — evidence and recommendation

**What a user experiences**, measured rather than reasoned (§5 D1, D2, and the two UI tests):

1. In an open project, a model-only edit (a shot brief) sets `is_dirty()`. The asterisk is on, Save is
   enabled, and the quit prompt appears — W4's F5, working as intended.
2. ⌘S writes `document/comic.vectorcraft` (32 330 bytes in the test), `is_dirty()` **stays true**, and the
   asterisk stays on. The user sees a save that did not finish.
3. The bundle's `manifest.json` now records a stale hash and size for that document, so the **next** open
   reports a §14 warning naming both hashes.
4. If the user then quits and answers **Save**, the edit is discarded (D1) — the prompt is honest about the
   dirty state and the save it performs cannot honour the answer.
5. `comic.project.save` at any point fixes all of it: the model reaches `pages/*.json`, both halves go clean
   and the manifest is rebuilt from the tree.

**Recommendation: route ⌘S to `comic.project.save` when a project is open — as an app-level binding, not by
changing `document.save`.**

* *Why not leave it (the warning is the honest answer):* the warning is honest, but the **quit prompt's Save
  loses the work**. Leaving it means accepting D1, which is silent data loss on an explicit user choice.
  That is the one outcome §11.1/§14 exist to prevent, and it is worse than the warning it would avoid.
* *Why not make `document.save` refuse when a project is open:* design §10.1's "No existing command changes
  semantics" is the single line in M1 that must not bend — it is what keeps upstream merges cheap — and the
  undo-`comic` change was defensible precisely because it is a **no-op** for every non-comic document. A
  refusal is not. It would also break a legitimate use (writing just the document), and it would make ⌘S
  fail outright on a bundle that needs hand repair, re-introducing the lockout §14 refuses.
* *Why routing is enough, and where it goes:* the decision is already made in one place. `unsaved::confirm`
  and the ⌘S menu entry both go through `io::save`; a comic-aware branch there (`if the active document has
  an open project, `comic.project.save`, else `document.save``) fixes the prompt **and** ⌘S together, leaves
  `document.save`'s contract untouched, and is the same shape as the app-level routing the menus already do
  (`menus::run_ui_command`). `comic.project.save` is already idempotent, already one write of the whole
  bundle, and already refuses only the ambiguous duplicate-id case — so the failure modes a user could meet
  are named ones, not new ones.
* *What I would not do:* bind `Cmd+S` to `comic.project.save` in the **command registry**. The shortcut
  lookup takes the first matching spec without consulting enablement, so a registry shortcut would shadow
  Save in every session with no project — the reason W4 deliberately gave `comic.project.save` no shortcut.

**I changed no behaviour.** This is evidence and a recommendation, for the lead to decide.

*Round 2: **the recommendation was taken, exactly as scoped** — `io::save` now routes a plain Save with a
project open to `comic.project.save`, `document.save` is untouched, and `comic.project.save` still has no
registry shortcut (`crates/ui-egui/src/io.rs`, `menus.rs:69`). Steps 2 and 4 above no longer happen. The two
tests that carried this evidence are in `crates/ui-egui/tests/comic_quit_prompt.rs`; the inverted one is
`the_quit_prompts_save_writes_a_model_only_comic_edit_and_closes_the_document` and the new refusal arm is
`a_save_that_cannot_complete_refuses_the_close_and_discards_nothing` (§11).*

---

## 7. The two smaller hand-carried questions

### `comic.project.create`'s `default_page_size` seeding

`create` always seeds `default_page_size: Some(size)` — `pageSize`, or 612×792 — so no command can leave a
project without one. The uncovered case W4's round-2 reviewer named is therefore the **`None`** one, and it
is reachable only from a bundle the reader accepts (§5.7: a reader accepts any valid JSON): a newer
ComicCraft, or a hand edit. Two evidence tests:

* `evidence_a_bundle_with_no_default_page_size_refuses_to_invent_a_size` — a project whose
  `default_page_size` is `None` opens with `None`, `comic.page.create {}` is **refused by name** ("give
  `size`, or create the project with a `pageSize`") and creates nothing, `comic.page.create {size}` works and
  the size is the artboard's, `comic.project.validate` reports 0 errors and names the silence in
  `notChecked` ("page size against a default"). It also records the reachability limit: `artboard.setProps`
  clamps a side at **1 pt** (`rect_from`, `crates/engine/src/cmd/layer.rs:729`), so no command can make an
  artboard unusable.
* `evidence_a_degenerate_artboard_is_reported_when_the_project_names_no_default_size` — a zero-area artboard
  (written into the document directly, because no command can produce one) with `default_page_size: None`
  produces exactly one **warning** whose message says rule 15 *cannot* report it either, plus the same page
  in `notChecked`. That is the hole the W4 report §7 describes, closed with a test rather than a claim.

### `comic.page.reorder` and artboard ordering

`acceptance_10b_…` pins what test 10's "correct reading order" rests on, beyond the implementer's own test:
the document's `artboards` really are put in book order (not merely reported as such), a spare artboard no
page names stays **after** the pages, **no artboard rectangle moves** (design §5.1.1: artboard order is
presentation, not narrative — a reorder is not a canvas move), every page still resolves to its own artboard
of its own size, the reordered project validates with 0 errors, repeating the move is idempotent, and a `to`
outside the scene is refused. `comic.page.reorder` is also one of the sixteen commands test 8 runs through
`one_step`.

---

## 8. Gate

**`cargo xtask ci` is red, on two other owners' in-flight artefacts, not on mine.** Three runs, each
failing on a different one — the tree was never quiescent while W4's round 2 and round 3 were landing:

| Run | Result |
|---|---|
| 14:43 | `fmt` ok, `clippy` ok, `test` **FAIL** — the i18n catalog row (D4 below) |
| 15:19 | `fmt` **FAIL** — `crates/engine/tests/comic_commands_r3.rs`, the round-3 reviewer's file, unformatted while it was being written |
| 15:25/15:26 | `fmt --all -- --check` still fails on that same file; `clippy --workspace --all-targets -- -D warnings` **EXIT 0** |

Run 1, the one that reached `test`:

```text
=== ci: fmt ===
=== ci: clippy ===
=== ci: test ===
…
failures:

---- i18n::tests::complete_languages_translate_every_message stdout ----

thread 'i18n::tests::complete_languages_translate_every_message' panicked at crates/ui-egui/src/i18n/tests.rs:870:9:
es: 1 untranslated messages: [
    "lettering {_1} names dialogue {_2}, which is not on page {_3}: comic.lettering.set_text edits the record in the balloon's own page, and a dialogue owned by another page's panel belongs to that page — use comic.dialogue.update {{dialogue}} to change its words",
]

failures:
    i18n::tests::complete_languages_translate_every_message

test result: FAILED. 873 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.10s

error: test failed, to rerun pass `-p vectorcraft-ui-egui --lib`

CI summary:
  ok    fmt
  ok    clippy
  FAIL  test: cargo test --workspace: exited with exit status: 101
  skip  assets
  skip  brands
  skip  layers
  skip  wasm
error: ci failed at `test`
```

Run 2 and the re-check:

```text
$ cargo xtask ci
Diff in /home/hvt/web/comiclab.io.vn/comiccraft/crates/engine/tests/comic_commands_r3.rs:15:
…
CI summary:
  FAIL  fmt: cargo fmt --all -- --check: exited with exit status: 1
  skip  clippy
  skip  test
  skip  assets
  skip  brands
  skip  layers
  skip  wasm
error: ci failed at `fmt`

$ cargo fmt --all -- --check          # 15:25, the same file (mtime 15:20), still unformatted
Diff in …/crates/engine/tests/comic_commands_r3.rs:15:
Diff in …/crates/engine/tests/comic_commands_r3.rs:99:
Diff in …/crates/engine/tests/comic_commands_r3.rs:114:
Diff in …/crates/engine/tests/comic_commands_r3.rs:147:
EXIT 1
```

The other five steps, run directly on the same revision:

```text
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 50.34s
EXIT 0

$ cargo test --workspace                  # 14:57 — 113 test targets report ok, one failure (the i18n row)
EXIT 101

$ cargo xtask layers
… vectorcraft-ui-egui  L6  brush, cad, color, doc, effects, engine, eps, format, geom, pdf, plugins, render, svg, text, tools, testkit (dev)
… vectorcraft-web      exempt  engine, format, text, ui-egui
… xtask                exempt
OK: 29 crates, no layering violations.
EXIT 0

$ cargo xtask brands
brands: no vendor names in user-visible text
EXIT 0

$ cargo xtask assets
assets: all 320 asset files attributed in ASSETS.md
EXIT 0

$ cargo xtask wasm
… ok  vectorcraft-geom
… ok  vectorcraft-ui-egui
EXIT 0
```

**Neither failure is mine and neither is in code I wrote.**

* `fmt` is `crates/engine/tests/comic_commands_r3.rs` — a **reviewer's** test file, which W5 is forbidden to
  touch and which the reviewer was still writing (`mtime 15:20`, unchanged at 15:25).
* `test` is **D4** (§5): W4's round-2 fix **extended** an engine message literal
  (`crates/engine/src/cmd/comic/lettering.rs:609`) from `"lettering {_1} names dialogue {_2}, which is not on
  page {_3}"` to the longer text, and the six message-complete catalogs still carry the **short** English
  key. `es` is reported first; `fr`, `it`, `ja`, `ru` and `uk` are each one row behind in exactly the same
  way. The round-2 record says the opposite — §R2-11, "**The catalogs** were not touched (no `@msg` row, no
  label changed)" — which is false as written: the message *literal* changed, and that is what makes the
  rows stale.

One precision about the `test` step: the workspace run stops inside `vectorcraft-ui-egui`, at that crate's
**lib** target, so its own integration targets are not reached by the gate.
`cargo test -p vectorcraft-ui-egui --test comic_quit_prompt` was run directly and both of its tests pass.

My three files pass on the revision in §9, in the workspace context:

```text
$ cargo test -p vectorcraft-engine --test comic_acceptance --test comic_commands_r1 --test comic_commands_r2
     Running tests/comic_acceptance.rs   -> test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
     Running tests/comic_commands_r1.rs  -> test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/comic_commands_r2.rs  -> test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
EXIT 0

$ cargo test -p vectorcraft-mcp --test comic_headless
     Running tests/comic_headless.rs     -> test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s

$ cargo test -p vectorcraft-ui-egui --test comic_quit_prompt
     Running tests/comic_quit_prompt.rs  -> test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.83s
```

### Round 2 — the gate, green on all seven steps

The one red round 1 left (`comic_quit_prompt::evidence_the_quit_prompts_save_discards_a_model_only_comic_edit`)
was this record's own evidence test asserting the D1 loss; R1-1 inverted it. With that moved, and D1 and D4
landed by their owners, the gate is green on a tree that was quiet for the whole run:

```text
$ cargo xtask ci
=== ci: fmt ===
$ cargo fmt --all -- --check
=== ci: clippy ===
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.53s
=== ci: test ===
$ cargo test --workspace
   … 146 test targets report `ok` … (5452 passed; 0 failed; 0 ignored; 0 measured)
=== ci: assets ===
assets: all 320 asset files attributed in ASSETS.md
=== ci: brands ===
brands: no vendor names in user-visible text
=== ci: layers ===
… OK: 29 crates, no layering violations.
=== ci: wasm ===
  ok     vectorcraft-comic-ai
  ok     vectorcraft-comic-layout
  ok     vectorcraft-comic-model
  ok     vectorcraft-engine
  …
  ok     vectorcraft-ui-egui

CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)
EXIT 0
```

The four targets this record owns, run directly on the same revision (§11):

```text
$ cargo test -p vectorcraft-engine --test comic_acceptance
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

$ cargo test -p vectorcraft-mcp --test comic_headless
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

$ cargo test -p vectorcraft-ui-egui --test comic_quit_prompt
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

$ cargo test -p vectorcraft-comic-model        # the W1 baseline round 1 was measured against
test result: ok. 71 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok.  8 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok.  8 passed; 0 failed; 0 ignored; 0 measured; …
test result: ok.  1 passed; 0 failed; 0 ignored; 0 measured; …   # 135 in all, unchanged

$ cargo test -p vectorcraft-engine             # counted over the whole run: 22 targets, 0 failed
1773 passed
```

---

## 9. Revision, and the production files

**Nothing in production was changed by me.** Every file I created is a test file, and the one production file
I *mutated* for §3 was restored byte-for-byte and verified:

```text
$ sha256sum -c evidence-sha256-before.txt
crates/engine/tests/comic_acceptance.rs: FAILED      # mine, edited after the record was taken
crates/engine/src/cmd/comic/page.rs: OK              # the M-W5-1 mutation, restored
crates/engine/src/cmd/comic/mod.rs: OK
crates/engine/src/cmd/comic/project.rs: OK
crates/engine/src/cmd/comic/panel.rs: OK
crates/engine/src/cmd/comic/lettering.rs: FAILED     # another owner's in-flight round-2 fix
crates/engine/src/lib.rs: OK
crates/engine/src/cmd/edit.rs: OK
crates/comic-model/src/bundle.rs: FAILED             # another owner's in-flight round-2 fix
crates/comic-layout/src/lib.rs: OK
```

The two `FAILED` production lines are **not mine**: `lettering.rs` went `09e558b7…` → `8d2ea1bb…` → `71bfa9f8…`
(the W4 round-2 fix) and `bundle.rs` went to `f928a4d1…` (the same session), both while I worked. `page.rs` —
the only production file I ever wrote to — is `b1a1c888…`, the hash it had **before** the mutation, recorded
before I applied it.

The revision these results were measured on:

```text
71bfa9f83db05d77509391916ed4dbe9d31409748aa009a7a044a4c94ea500f6  crates/engine/src/cmd/comic/lettering.rs
836c4a9242027b5561c6a1165f219d09ca16029051af263ff30c227bbd284ccb  crates/engine/src/cmd/comic/mod.rs
b1a1c888b5257da41cc131064cdeaa8ebb972f4dbca549964674fd576cb152db  crates/engine/src/cmd/comic/page.rs
9e048349406b75d3bdde8008af667bcd861556e3841b6d8861f8905911a170a8  crates/engine/src/cmd/comic/panel.rs
bd5577cacd44a38b6fc8fc037f8ea30fe350e2b4eaad2864dc97b139d121f4ee  crates/engine/src/cmd/comic/project.rs
6c2075f07f38f4cbf38cabe3b82fcf1b32e23ee5a53e3544b26d1b8c3119317d  crates/engine/src/cmd/comic/tests.rs
f928a4d1849b42dae4c7f43cb0d164f19ec8ee2b90fc1f436fc4ad16af90c8a4  crates/comic-model/src/bundle.rs
58f23b98e8f548a03c41601d97db8ae2d18927a0c75c3faee1470f232de4697c  crates/comic-model/src/validate.rs
720bf15d596763942a42a10aa55af385db520b58fb2a3a2ea60c70f8081a8615  crates/engine/src/lib.rs
77c8f534e7300f5b54381a39014907d54391dcb64815a5a8cd3dc99f5ca3f5b9  crates/engine/src/cmd/edit.rs
fd5491e03d03ed5568f3c9f3ce90ff02b8de89af645262fa92a84c5c01ecd8af  crates/comic-layout/src/lib.rs
bf7434dc79fabf79d71bca7a00003a07d17a69713f3b21e08ffcd7395c7ffd25  crates/ui-egui/src/i18n/es.tsv
77b438c7c0918dfd89cd38742fe1af55aab66721e7c33958ca53d0c0cc1745a7  crates/ui-egui/src/i18n/fr.tsv
b7c652870acc52b8e05c035e53c288a65d6bf7a1b3c13ad993cf6dc1e43abf93  crates/ui-egui/src/i18n/it.tsv
bde08891737008931c6aa8433863a9311564a239d289a3fb2977006df2cc6006  crates/ui-egui/src/i18n/ja.tsv
8535abdffa99a10b2af50ea72942d1ecfd33a9a05609e0e88a08544c674b38c7  crates/ui-egui/src/i18n/ru.tsv
7beaae865206636782999f2c347bef3709b3bbe536c79e13dda0e93e169a9504  crates/ui-egui/src/i18n/uk.tsv
```

*R1's stale-pin sweep: the six `i18n/*.tsv` values above were re-taken in round 2, because the D4 fix
changed exactly those files. The superseded round-1 values, in the same order, were `es` `6b259808…`,
`fr` `86cfecfc…`, `it` `7552512c…`, `ja` `ebe2a507…`, `ru` `1359e2a1…`, `uk` `d5a031c6…` — each catalog
gained the `@msg` row for the extended `lettering {_1} names dialogue {_2}…` literal. Every other hash in
this list was re-checked in round 2 and is unchanged; `lettering.rs` is still `71bfa9f8…`, which is what
makes the fix **catalog-side only**. The two W1 pins another owner's round-3 notes flagged
(`validate.rs cc0701bc… → 7f4983ca…`, `bundle.rs f16fda39… → 926adc8a…`) are in **W1's** record, not this
one: this record has always carried `sha256` of the tree, and §11 re-verifies them.*

Files I added:

```text
c5564cf144eb881cc353391bf9eb4f263997a78ec2a96a1ca6cbd426a3d512ab  crates/engine/tests/comic_acceptance.rs
d95157d8ce0bee2779e856441b87512dd913463452e3bd6f5ee4fdc5c5447348  crates/mcp/tests/comic_headless.rs
b728f21337fec845bc5006d58eb532db92401ad56eaad2da9d51126452927340  crates/ui-egui/tests/comic_quit_prompt.rs
```

*Round 2 added no file and changed these three; their current hashes are §11.* All three are
`rustfmt --check` clean and clippy-clean (`-D warnings`). No file ending `_reviewer.rs` was
touched, and neither `comic_commands_r1.rs` nor `comic_labels_are_menu_strings.rs` was touched.

---

## 10. Unverified

* **The gate has not been seen green on a quiescent tree.** Three runs failed on two other owners'
  artefacts — W4's stale catalog rows (D4) and the round-3 reviewer's unformatted
  `comic_commands_r3.rs` — and the tree moved under me throughout (`lettering.rs` at 14:11, 14:48;
  `bundle.rs` at 14:30, 15:04; `project.rs` at 15:05; the round-3 reviewer's file at 15:20). Both are
  the owner's to land; the moment they do, the five steps I ran directly say the rest is green.
  *Round 2: the gate **was** seen green on all seven steps, on a tree that was quiet for the whole run —
  §8.*
* **Windows.** Everything ran on Linux only, as in W1's and W4's own notes.
* **Rendering and the comic faces.** `CRAFT_FONTS` is empty in this environment, so every lettering plan
  reports the missing comic faces — the contract `comic-layout`'s own tests rely on. Test 10 checks render
  *dimensions* and PDF *page geometry*, not pixels of lettering.
* **The UI beyond the quit path.** Test 12 covers the MCP path and the engine tests cover the command path;
  the ⌘S evidence drives `unsaved::close_all`/`confirm` and `document.save` directly, not an egui frame.
* **The catalog rows** (`es`, `fr`, `it`, `ja`, `ru`, `uk`, `zh-hans`, `zh-hant`) as natural language, as W4
  already recorded. §8 is a gate consequence of one of those rows, not a judgement of its Spanish.
* **`comic.panel.replace_art` over MCP** — *round 1: "impossible in M1, because there is no
  `comic.asset.*` command to register a `Linked` asset".* **R1-2 corrected the first half**: creating the
  record is impossible over MCP, the command is not — round 2 drives it over MCP against a prepared bundle,
  so test 12 no longer uses the document path alone (§4, §11).
* **Round 2 — what is still not covered.** The sound effect (R1-7) is asserted through
  `comic.lettering.add`/`set_text` on the engine side; it is **not** driven through MCP, and none of the
  twelve rows renders a sound effect's pixels (`CRAFT_FONTS` is empty here, as above). The refusal arm of
  the quit prompt (R1-1) reaches `unsaved::confirm`'s documented "not saved, so not closed" branch only by
  the **error** path — a save that cannot complete; that branch's `is_dirty()` re-check is otherwise
  unreachable from the command surface in M1, so it is pinned by inspection rather than by a driven case.

Revision: `crates/engine/tests/comic_acceptance.rs` = `c5564cf144eb881cc353391bf9eb4f263997a78ec2a96a1ca6cbd426a3d512ab`,
`crates/mcp/tests/comic_headless.rs` = `d95157d8ce0bee2779e856441b87512dd913463452e3bd6f5ee4fdc5c5447348`,
`crates/ui-egui/tests/comic_quit_prompt.rs` = `b728f21337fec845bc5006d58eb532db92401ad56eaad2da9d51126452927340`.
The production tree measured is in §9: `crates/engine/src/cmd/comic/{lettering,mod,page,panel,project,tests}.rs` =
`71bfa9f8…`, `836c4a92…`, `b1a1c888…`, `9e048349…`, `bd5577ca…`, `6c2075f0…`, and
`crates/comic-model/src/bundle.rs` = `f928a4d1…`, `crates/comic-model/src/validate.rs` = `58f23b98…`.

---

## 11. Round 2 — the eight round-1 findings, actioned

`docs/reviews/M1-acceptance-review.md` returned **changes-requested — 2 defects and 6 notes**. All eight are
actioned here. **This round changed no production file** (§11.5): every change is in the three test files
this record owns, plus this record. The production files the round *mutated*, to show that a wrong
implementation really does red, are restored byte-for-byte and `sha256`-verified.

The revision round 2 measured:

*Line numbers quoted anywhere in §11 are the ones **this** revision produced; R1-8's lesson is that the
line moves whenever the file is edited, while the **message** reproduces — so the messages are the part to
compare.*

```text
c865499eee5d3a1e31a0ffb2f903e4e81ac0c80d84fed92ab87c6ff366c1e976  crates/engine/tests/comic_acceptance.rs
a29cc9552717c0c6d638212f1a7e388f66c07265f92d9d3f7c595fcca0782275  crates/mcp/tests/comic_headless.rs
a8c9c00d828190e4ca5c9a8dad8c5c9d7881c6823ac8639b1017cc8f9595eb26  crates/ui-egui/tests/comic_quit_prompt.rs
0392553f49d239bde66b2467fe2edff15bc1c474473ac052dc73532360814ad4  crates/engine/tests/comic_acceptance_reviewer.rs   # untouched
70932c3398bf82267dc0fd42d3c4be429b31e86718ef4e5dcadba4937ae5bf42  crates/engine/tests/comic_commands_r1.rs           # untouched
8b4893542437c63fc17952241f0efe67dc94e77adff322fb53053a2e58a6d12e  crates/engine/tests/comic_commands_r2.rs           # untouched
963632ebaf911b7dcb3951b0e30c3b02ae171e50d57863e3d7ede5e79d767a17  crates/engine/tests/comic_commands_r3.rs           # untouched
```

### 11.1 R1-1 — the evidence test, inverted (the red the gate was on)

`crates/ui-egui/tests/comic_quit_prompt.rs`

* `evidence_the_quit_prompts_save_discards_a_model_only_comic_edit` is renamed
  **`the_quit_prompts_save_writes_a_model_only_comic_edit_and_closes_the_document`** and asserts **the
  invariant that replaced it**, on both save paths a user reaches:
  1. **⌘S**, driven through the **binding** (`file.save`) rather than the command the old test called: with a
     project open it writes the model — the bundle on disk holds `"Second brief."`, `is_dirty()` is
     **false**, and the reply names a `root` (`document.save` reports `path`, so a regression to the
     document-only write is visible on the first line);
  2. a **second** model-only edit, then File → Quit and **Save** at the prompt: `pending: "saveChanges"`,
     the on-disk brief becomes `"Third brief."`, the document closes and `status == "quit"` — and it closes
     **only because** `unsaved::confirm` re-checks `is_dirty()`, so "closed" *is* "not dirty".
* New test **`a_save_that_cannot_complete_refuses_the_close_and_discards_nothing`** — the **other** arm of
  the invariant. The prompt's Save is made to fail through the bundle's own §13 rule-13 refusal
  (`pages/page-07.json` beside `pages/page-7.json`: schema §3 makes the padding cosmetic and rule 13 parses
  the number), so nothing is written. Then: the close is **refused** with the bundle's message, the document
  is still open, still `is_dirty()`, the edit is still in the model, the pre-edit brief is still on disk,
  and `status` is not `quit`. Never both.
* The control (`evidence_the_same_quit_keeps_the_edit_after_comic_project_save`) is unchanged.
* **Teeth, without touching production**: with the test's own ⌘S call reverted to `document.save` — the
  pre-D1 binding — the renamed test reds:

```text
⌘S → {"bytes":32330,"format":"vectorcraft","path":"…/book.comiccraft/document/comic.vectorcraft","warnings":[]}
thread 'the_quit_prompts_save_writes_a_model_only_comic_edit_and_closes_the_document' (3609289) panicked at crates/ui-egui/tests/comic_quit_prompt.rs:132:5:
⌘S with a project open is comic.project.save, not document.save: {"bytes":32330,"format":"vectorcraft","path":"…/comic.vectorcraft","warnings":[]}
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.32s
```

The **prompt** arm's failing-then-passing property is the one the D1 fixer measured by reverting production
itself (`docs/milestones/M1.md` §5.11: *"reverted to the reported revision it reds exactly on the defect
(`left: "First brief." / right: "Second brief."`, ⌘S reporting no `root`, `is_dirty` still true)"*). Round 2
does not re-revert production and does not claim to have.

### 11.2 R1-2 — artwork placed and clipped **inside a panel**, over MCP

`crates/mcp/tests/comic_headless.rs`. The reviewer established the true premise: what MCP cannot do is
*create* the `Linked` `Asset` record (`comic.asset.*` does not exist in M1); the command itself runs — it was
driven over the pipe against a prepared bundle. Round 2 adds that step, so test 12 covers M1 §1's
"place and clip raster artwork **inside panels**" rather than the document path alone:

* `prepare_linked_asset(root, id)` writes the bundle's own wire format while the project is closed — the
  same 75-byte red plate the document-path step places, under `assets/plate.png`, with its §5.5
  `assets/index.json` record (`linked`/`sha256`/`image/png`). **A fixture, not an engine back door**: the
  file is written exactly as a hand-built or third-party bundle is, and every step of the operation is still
  `tools/call` → `run_command`. The bundle is then reopened over MCP (`open_file` + `comic.project.open`).
* The asset's id is `1000`, not the reviewer's `10`, because **rule 2 makes ids unique across kinds** and
  this test's pages, panels, dialogues and lettering already own the low numbers — with `10` the reopened
  project reported `rule 2` as an **error** (`the same id is used by Asset, Page`).
* Then, over MCP: `comic.panel.replace_art {panel, asset}` → the reply names the panel, the asset, the clip
  group and the image inside it; the **image is selected**, so `object.cropImage` acts on it; the step is
  repeated, and the second reply reports `previous: <the first clip group>` and a **new** group (the
  replacement is a new object, not the old one re-used); a `comic.project.save` then lets the two artefacts
  be read — the page file, where `panels[0].artwork_refs` names the asset and the clip group exactly once
  and the page's lettering is untouched, and `assets/index.json` **as the save wrote it**, which still holds
  exactly that one `Asset` record, so the replacement added and removed none (§11.1's lineage claim, over
  MCP). The reply's `asset_kept` is deliberately **not** counted as coverage (R1-4): in test 12 the
  registry on disk is the evidence, and it is read after the model wrote it; the bundle with artwork in the
  panel then validates with 0 errors.

### 11.3 R1-3 — test 11 against **all three** wrong implementations

`acceptance_11_a_page_never_takes_a_retired_artboard_id_even_after_artboard_new_resurrects_it` is renamed
**`acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path`** and now has
four parts, one per way mitigation 4 can be broken:

1. **the fast path** — `artboard.new` hands the retired number back to the document; the page must allocate
   past it (the round-1 case, unchanged but renamed and re-worded);
2. **the loop** — no artboard is free, so the retired number is the only candidate `next_artboard_id`
   offers; the loop's own check must step over it (`skipped_retired_artboards: 1`);
3. **a number a page still names** — `artboard.delete` (`Window → Artboards`) removes an artboard *without*
   retiring it and stays reachable while a project is open, so a page still resolves to the number and the
   guard cannot be `retired_artboard_ids` alone: the new page must not take it, and the pre-existing rule-4
   report must not grow;
4. **the consequence** — a page that *does* take a retired id is reported by rule 4, naming
   `retired_artboard_ids`, and stays reported across a save and a reopen. Part 1's failure message asserts
   this in prose; part 4 makes it true, and a mutation of rule 4 reds it (the reviewer's **M-RULE4**).

All three wrong implementations are measured, each applied **alone**, with the red output taken raw:

| # | Wrong implementation (`crates/engine/src/cmd/comic/page.rs`) | Red |
|---|---|---|
| 1 | `free_artboards` drops `!b.manifest.is_artboard_retired(*id)` (the reviewer's **M-W5-1**) | test 11 only |
| 2 | `claim_artboard`'s loop drops `retired.contains(&candidate)` (the reviewer's **M-W5-2**) | **test 11 only** (round 1: green) |
| 3 | `claim_artboard`'s loop drops `named.contains(&candidate)` (the reviewer's **M-NAMED**) | **test 11 only** (round 1: green) |

```text
$ python3 target/mut-r2/mut.py w5-2 && cargo test -p vectorcraft-engine --test comic_acceptance
test acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path ... FAILED

---- acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path stdout ----

thread 'acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path' (3615696) panicked at crates/engine/tests/comic_acceptance.rs:933:5:
FAIL (§5.3.1 mitigation 4, the loop): the allocation loop handed page 8 the retired artboard 2, which no page may ever resolve to (rule 4)
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

$ python3 target/mut-r2/mut.py named && cargo test -p vectorcraft-engine --test comic_acceptance
test acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path ... FAILED

---- acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path stdout ----

thread 'acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path' (3616404) panicked at crates/engine/tests/comic_acceptance.rs:962:5:
assertion `left != right` failed: FAIL (§5.3.1 mitigation 4, a named id): a page took 2, which the second page still resolves to — one artboard, two pages, and the rule-4 report of the first page's breakage is deleted
  left: 2
 right: 2
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.65s

$ python3 target/mut-r2/mut.py w5-1 && cargo test -p vectorcraft-engine --test comic_acceptance
test acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path ... FAILED

---- acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path stdout ----

thread '…' (3618703) panicked at crates/engine/tests/comic_acceptance.rs:892:5:
FAIL (§5.3.1 mitigation 4, the fast path): comic.page.create gave page 8 the retired artboard 2, the id the deleted page 7 used. A retired number means "never again": rules 4 and 17 make a page that takes one permanently reported and permanently unfixable
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

Each mutation was applied alone, run, and reverted immediately; `page.rs` is back to
`b1a1c888b5257da41cc131064cdeaa8ebb972f4dbca549964674fd576cb152db` after each (§11.5).

### 11.4 R1-4 … R1-8

* **R1-4 — the three assertions that restated a literal now compare against the produced value.**
  * test 6, `asset_kept`: `assert_eq!(second["asset_kept"], json!(assets_after.contains(&asset_a)))` — the
    command's report is checked against the **Asset registry**, so a `replace_art` that drops the previous
    record reds it. Measured: a mutation that makes `replace_art` also delete the previous `Asset` record
    reds test 6 at that line —
    `assertion left == right failed: the report is what the registry shows: the replaced asset's record survives / left: Bool(true) / right: Bool(false)`
    (`crates/engine/tests/comic_acceptance.rs:522`). Note this assertion now **bites**, where the literal
    could not.
  * test 5, `typographyTouched`/`geometryTouched`: both are now derived from the artefacts
    (`text_layout_settings`/`overflow_policy` for typography; the frame path, the tail path for geometry)
    and compared to the command's report, so a `dialogue.update` that moved the balloon while reporting
    `geometryTouched: false` reds them, and a reply that lost the field fails too.
* **R1-5 — test 10's reading order is now read from the document.** The old line compared the command's
  reply with a value built from the same model (`page_file(...).page.artboard_id`). It is replaced by
  `assert_eq!(artboard_ids(&s), artboards, …)` — the **document's** artboards — with the command's report
  compared against that. Measured: a `comic.page.reorder` that reorders the scene but not the document's
  artboards reds it —
  `assertion left == right failed: comic.page.reorder put the document's artboards in book order / left: [1, 2, 3] / right: [2, 1, 3]`
  (`crates/engine/tests/comic_acceptance.rs:760`), and 10b as before.
* **R1-6 — the unfailable disjunction is gone.** `comic_headless.rs`'s
  `assert!(opened_doc.get("index").is_some() || opened_doc.get("title").is_some())` accepts any object with
  either key. It is now `assert_eq!(opened_doc["title"], json!("comic.vectorcraft"), …)` plus a check that
  the reply carries an `index`: the file that was opened is named, and a reply that is not a document fails.
* **R1-7 — the sound-effect object has a test.** Test 7 adds
  `comic.lettering.add {kind: "sound_effect", text: "KRAK!"}` — M1 §1's "add a basic sound-effect object",
  which no row exercised. It is asserted as the kind whose **tail rules are the tightest** (no tail: the
  record's `tail_node_id` is `None` and the reply reports no `tail`), with rule 8's split (its words live on
  the record, like a narration: `dialogue_id` is `None`, `text` is its own; the page shows them), with the
  page's dialogue count staying at two (only the spoken kinds own one), and as **editable** where the words
  live (`comic.lettering.set_text`, then the text object shows the new words).
* **R1-8 — the two quoted values.**
  * §3's `comic_acceptance.rs:780` red is annotated in place: the line number moved when the file was
    edited (the delivered revision red at 832) and moved again in round 2 (892 for that message), so **the
    message, not the line, is the reproducible part**, and §11.3 carries the re-taken output.
  * §5 D2's `file sha256 26f85ccc…` is annotated: `document.save` stamps the document with the current
    time, so the file hash is **not reproducible**. Round 2 re-ran the evidence test:
    `manifest sha256 c0089ed8… (23337 bytes)`, file `5e6ed4f7… (23622 bytes)`, and the same §14 warning —
    the manifest hash, both byte counts and the warning reproduce; the file hash does not.

```text
$ cargo test -p vectorcraft-engine --test comic_acceptance evidence_cmd_s -- --nocapture
document.save → "…/cmds-hash-3711244/book.comiccraft/document/comic.vectorcraft"
manifest sha256 c0089ed8b854638cd58e1310a2176cc620ae6f4f077b4a5e20c5e139f2b4919e
file     sha256 5e6ed4f70fbc4a7cb5665a0c27e1f065eafa3c22f55ea1f7f8a888803e29f4ef
reopen findings: [{"message":"`manifest.files` records sha256 c0089ed8… (23337 bytes) but the file is 5e6ed4f7… (23622 bytes): the manifest describes the previous generation (§14)","rule":null,"severity":"warning","subject":"document/comic.vectorcraft"}]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.07s
```

### 11.5 Mutation hygiene, and what this round changed

The five mutations live in a review tool under `target/mut-r2/` (never in the tree: `target/` is not
tracked). Each takes a byte-exact snapshot, asserts its anchor appears **once**, applies, and restores the
file with a `touch` so cargo cannot reuse a mutated rlib — the trap the reviewer recorded in
`M1-acceptance-review.md` §Mutation hygiene. After every restore:

```text
$ python3 target/mut-r2/mut.py restore
b1a1c888b5257da41cc131064cdeaa8ebb972f4dbca549964674fd576cb152db  crates/engine/src/cmd/comic/page.rs
9e048349406b75d3bdde8008af667bcd861556e3841b6d8861f8905911a170a8  crates/engine/src/cmd/comic/panel.rs
```

Both are the hashes §9 recorded before round 2 — the production tree is exactly as round 1 measured it, and
**no production file was edited by this round at any point**. The changed files are the three test files
(hash list at the top of §11) and this record.

Round 1's own record hashes were re-verified rather than assumed: every `sha256` in §9 still describes the
tree except the six `i18n/*.tsv` catalogs, which D4's fix moved (annotated in §9 with their superseded
values). The two stale W1 `md5` pins another owner's round-3 notes swept up (`validate.rs cc0701bc… →
7f4983ca…` and `bundle.rs f16fda39… → 926adc8a…`) are in W1's record, not this one — this record has only
ever quoted `sha256` of the tree, and §9's are the values that match it.
