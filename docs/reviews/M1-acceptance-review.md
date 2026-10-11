# M1 W5 — the twelve acceptance tests, reviewed in two rounds, by `code-reviewer`

**Status:** round 2 complete — **approved**, 0 defects and 3 notes. Round 1 was
**changes-requested** (2 defects, 6 notes); all eight are actioned and re-measured, not re-read.
**Reviews:** `docs/reviews/M1-acceptance.md` (W5, `verification-engineer`)
**Specification:** `docs/milestones/M1.md` §1 (the twelve-row acceptance table and the exit criteria
it proves), §5.4e (the model rule)
**Design refs:** design §5.1.1 (page = artboard), §5.3.1 (the artboard-id hazard and its five
mitigations), §6.1 (ownership), §8.1/§8.2, §10.2, §11.1, §11.2 (one action = one undo step),
§12.1; schema §4, §9.1, §13 rules 4 and 17, §14
**Method:** mutation testing of the delivered suite against production code. Fourteen mutations, each
applied **alone**, run, and restored immediately; every restore `sha256sum -c`-verified. The
working tree is byte-identical to the revision reviewed (§Mutation hygiene).

Reviewer:      code-reviewer on an unstated model (M1 §5.4e; see MODEL below)
Waiver:        the reviewer cannot determine which model served it, so no model claim is made; §5.4e records that this harness cannot observe it, and the lead must pair this record with the harness's recorded dispatch id. Reviewed by the code-reviewer role, which is the rule's requirement.
Revision:      the working tree, no git repository to name a commit. Files: crates/engine/tests/comic_acceptance.rs, crates/mcp/tests/comic_headless.rs, crates/ui-egui/tests/comic_quit_prompt.rs
Design refs:   design §5.1.1, §5.3.1, §6.1, §8.1/§8.2, §10.2, §11.1, §11.2, §12.1; schema §4, §9.1, §13 rules 4 and 17, §14

## MODEL

**I cannot determine the model I am running on, and per M1 §5.4e I will not guess.** My role file
declares `model: ['opencodego/deepseek-v4.1-flash', 'opencodego/OpenCode Go/deepseek-v4.1-flash']`
and `.github/instructions/implementation-review.instructions.md` requires `deepseek-v4.1-flash`; I
have no way to observe which weights served this session. `COPILOT_MODEL` would be the *requested*
id (routing intent), not proof of weights, and the harness's own recorded id is the only evidence —
I have none for myself. So this record carries no model claim, and the lead should read it against
the harness's recorded dispatch, exactly as §5.4e instructs. Two earlier agents read
`claude-sonnet-4.6` out of the `task` tool's enum and reported it as fact; that is the failure mode
this line exists to avoid.

Reviewer: code-reviewer on an unstated model — see MODEL above, and the `Waiver:` line at the top.

## Revision reviewed

```text
crates/engine/tests/comic_acceptance.rs         c5564cf144eb881cc353391bf9eb4f263997a78ec2a96a1ca6cbd426a3d512ab
crates/mcp/tests/comic_headless.rs              d95157d8ce0bee2779e856441b87512dd913463452e3bd6f5ee4fdc5c5447348
crates/ui-egui/tests/comic_quit_prompt.rs       b728f21337fec845bc5006d58eb532db92401ad56eaad2da9d51126452927340
```

All three match W5's §9/§10 record exactly. The production tree matches it too — every hash in §9
verifies, including `crates/engine/src/cmd/comic/page.rs` = `b1a1c888b5257da41cc131064cdeaa8ebb972f4dbca549964674fd576cb152db`:

```text
$ sha256sum crates/engine/src/cmd/comic/*.rs crates/comic-model/src/bundle.rs
71bfa9f8…  crates/engine/src/cmd/comic/lettering.rs
836c4a92…  crates/engine/src/cmd/comic/mod.rs
b1a1c888…  crates/engine/src/cmd/comic/page.rs
9e048349…  crates/engine/src/cmd/comic/panel.rs
bd5577ca…  crates/engine/src/cmd/comic/project.rs
6c2075f0…  crates/engine/src/cmd/comic/tests.rs
f928a4d1…  crates/comic-model/src/bundle.rs
```

So W5's `b1a1c888…` claim is confirmed: `page.rs` is at the hash it had **before** the M-W5-1
mutation, and no other production file moved for the mutation.

Files I added: `crates/engine/tests/comic_acceptance_reviewer.rs` =
`0392553f49d239bde66b2467fe2edff15bc1c474473ac052dc73532360814ad4` (test code only).
**I changed no production file**; the tree is byte-identical to the revision above (§Mutation
hygiene).

## Round 1 — correctness

Verdict:       changes-requested
Unit tests:    crates/engine/tests/comic_acceptance_reviewer.rs (3 added) — cargo test -p vectorcraft-engine --test comic_acceptance_reviewer — 3 passed; 0 failed. Also run: comic_acceptance 15 passed; comic_headless 1 passed; comic_quit_prompt 1 passed and 1 failed; the new file is cargo fmt --all -- --check clean and clippy -D warnings exit 0.
Findings:      2 defects (R1-1, R1-2) and 6 notes (R1-3 … R1-8) — each below as file:line — trigger — expected vs actual — bug|preference

### The question, and the answer

W5 is test code and it changed no production file, so *"is this code correct"* is the wrong
question. The question is **"would these tests fail if the behaviour they claim to verify were
broken?"** A test that cannot fail is worse than no test, because it certifies. So the review is a
mutation table, not a reading.

**Every one of the twelve reds under at least one wrong implementation, and ten of the twelve are
pinned by a mutation aimed at their own claim.** That is the headline, and it is a good suite.

### Mutation table

Each row: one change to production code, applied alone, run, reverted. `M-W5-1`/`M-W5-2` are W5's
own and mine respectively; the rest are mine.

| # | Mutation (file) | The wrong implementation | Acceptance tests that red |
|---|---|---|---|
| 1 | **M-W5-1** `page.rs` — drop the retirement check from `free_artboards` | the fast path offers a retired id | **11** only (`14 passed; 1 failed`) |
| 2 | **M-W5-2** `page.rs` — drop the retirement check from `claim_artboard`'s loop | the loop hands out a retired id | **none — 15 passed; 0 failed** |
| 3 | M-REUSE `page.rs` — drop `!used.contains(id)` from `free_artboards` | two pages share one artboard | 1, 9, 10, 10b, 11 |
| 4 | M-FRAME `panel.rs` — `panel.add` does not insert the frame node | `frame_node_id` is not a document object | 2, 3, 4, 5, 6, 7, 8, 9, 10 |
| 5 | M-CLIP `panel.rs` — the clip group is `clip: true` → `false` | the artwork is not clipped | **3** only |
| 6 | M-MOVE `panel.rs` — `move_panel` translates the frame only | the artwork and balloon stay behind | **4** only |
| 7 | M-ASSET `panel.rs` — `replace_art` also drops the previous `Asset` record | §11.1 lineage lost | 3, 4, 6, 9 |
| 8 | M-SNAP `mod.rs` — the undo entry snapshots the model **after** the step | undo restores the wrong state | **8** only |
| 9 | M-OPEN `project.rs` — `project.open` opens a **blank** document | the page does not match the saved layout | **9** only |
| 10 | M-ORDER `page.rs` — `page.reorder` reorders the scene, not the document's artboards | the PDF/PNG walk the wrong order | **10, 10b** |
| 11 | M-ENABLE `mod.rs` — `has_comic` returns `Ok(())` always | every `comic.*` is enabled with no project | **12** only (engine suite green) |
| 12 | M-NAMED `page.rs` — the loop stops skipping ids a page names | a page takes a number another page names | none of W5's 15; **my** reviewer test reds |
| 13 | M-OPENOUTLINE `comic-layout/lettering/outline.rs` — outlines are not closed | the balloon frame is not an editable closed path | **7** only |
| 14 | M-RULE4 `comic-model/validate.rs` — rule 4 stops reporting a retired artboard | the consequence test 11's message asserts is gone | none of W5's 15; **my** reviewer test reds |

Raw results, as printed:

```text
$ python3 target/review-bak/mut.py reuse && cargo test -p vectorcraft-engine --test comic_acceptance
test acceptance_1_one_document_and_one_artboard_per_page ... FAILED
test acceptance_9_… FAILED   test acceptance_10_… FAILED   test acceptance_10b_… FAILED   test acceptance_11_… FAILED
test result: FAILED. 10 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

$ python3 … mut.py clip && cargo test …
test acceptance_3_artwork_is_placed_clipped_and_cropped ... FAILED
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

$ python3 … mut.py snap && cargo test …
---- acceptance_8_every_operation_is_exactly_one_undo_and_redo_step stdout ----
assertion `left == right` failed: comic.page.create: one undo must restore the document, the selection and the model
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

$ python3 … mut.py enable && cargo test -p vectorcraft-mcp --test comic_headless
test acceptance_12_the_essential_operations_succeed_through_mcp_headless ... FAILED
panicked at crates/mcp/tests/comic_headless.rs:139:5:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

### Findings

**R1-1 — `crates/ui-egui/tests/comic_quit_prompt.rs:91-105` — the evidence test asserts the loss
the lead has since specified away, and it is the one test keeping the gate red.** *(bug)*

* Trigger: `cargo test -p vectorcraft-ui-egui --test comic_quit_prompt`, on the current tree (the
  D1 fix has landed: `unsaved.rs` = `7fe775f9…`, `menus.rs` = `a2113acc…`).
* Expected, once D1 is fixed and ⌘S routes to `comic.project.save` (§6's recommendation, now the
  specified behaviour): answering **Save** at the quit prompt writes the model, so the on-disk brief
  is `"Second brief."`.
* Actual: `assert_eq!(saved_shot(&root, page), "First brief.", …)` — the test demands that the edit
  is *gone*, and it now fails:

```text
---- evidence_the_quit_prompts_save_discards_a_model_only_comic_edit stdout ----
document.save wrote "…/quit-loses-3443283/book.comiccraft/document/comic.vectorcraft" (32330 bytes)
is_dirty() after ⌘S: true
the bundle on disk still holds the shot "First brief."
quit asked first: {"pending":"saveChanges"}
documents left open: 0 · status: "quit"

thread '…' panicked at crates/ui-egui/tests/comic_quit_prompt.rs:102:5:
assertion `left == right` failed: the user answered Save and the comic edit is gone anyway: …
  left: "Second brief."
 right: "First brief."
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

* This is not a misjudgement by W5 — on the revision it measured the test was *true*, and writing
  the defect down as a reproduction is the right thing for a verifier to do. But it is now the same
  shape M1 §5.4e records for W2 round 5: **the test that encodes the current behaviour becomes the
  obstacle the moment the behaviour improves**, which is exactly why its owner, not the fixer, must
  move it. It has to be **inverted** so it asserts the loss is gone; inverted, it is the
  failing-then-passing evidence for the D1 fix (it fails now, it will pass then, and it would fail
  again if the fix regressed). Note also that its `cmd_s()` helper calls `document.save` *as a
  command*, not through the ⌘S menu entry, so it never exercised the binding the lead's design call
  actually changes — the inverted test should drive ⌘S (or `unsaved::confirm` alone, which it
  already does) and assert the model reached the disk.

**R1-2 — `crates/mcp/tests/comic_headless.rs:181-189` — test 12 never places artwork *inside a
panel*, and that ground is reachable over MCP, so it is left uncertified for no reason.** *(bug)*

* W5's premise is that `comic.panel.replace_art` "cannot be driven through MCP at all because M1 has
  no `comic.asset.*` command". The second half is true (19 `comic.*` commands, none of them
  `comic.asset.*`). The first half is **false**: what is unreachable is *creating the `Asset` record*
  over MCP, not the command.
* Trigger: `vectorcraft-cli mcp --headless` over a pipe, against a bundle that already carries a
  `Linked` asset (the bundles W5's own `comic_acceptance.rs` writes are exactly that — `assets/art.png`
  + `assets/index.json` from `acceptance_9`):

```text
-> run_command comic.project.open {"path": "…/t9-3418082/book.comiccraft"}   <- 2 pages
-> run_command comic.panel.replace_art {"panel": 8, "asset": 10}
   isError = False -> { "asset": 10, "asset_kept": true, "image": 16, "node": 18,
                        "panel": 8, "previous": 6, "role": "primary" }
```

* Expected: test 12 covers the exit criterion "place and clip raster artwork **inside panels**"
  (M1 §1 required capabilities). Actual: it drives `file.place` + `object.cropImage` +
  `object.maskImage` — the *document* path — and never establishes a `Panel.artwork_refs` link.
  `comic.panel.replace_art`, `comic.panel.delete`, `comic.lettering.set_style` and
  `comic.reading_order.set` are the commands test 12 never touches; `replace_art` is the one that
  carries a criterion.
* **Answer to the lead's question:** test 12 should **not** count as covering the same ground as test
  6 until it does this. It is not an M1 command-surface gap (the command works over MCP); it is a
  fixture choice, and the fixture is producible by writing `assets/index.json` in the wire format —
  which is *not* an engine back door, any more than opening a `.vectorcraft` file is.

**R1-3 — `crates/engine/tests/comic_acceptance.rs:820-844` — test 11 pins one of mitigation 4's two
halves, and the half it misses leaves the whole acceptance suite green.** *(preference; closed by my
reviewer test)*

This is the harder question the lead asked, and the answer is measured, not reasoned.

W5's mutation (`M-W5-1`) removes the retirement check from `free_artboards`, which is the **fast
path** — the branch that runs when the retired artboard is back in the document and free (the
`artboard.new` step is what makes it free). It reds test 11 and nothing else. Confirmed.

The **loop** half is a different wrong implementation: keep `free_artboards` honest and drop
`retired.contains(&candidate)` from `claim_artboard`'s allocation loop. That leaves the loop's guard
as the only thing that steps over a retired id when nothing is free, and:

```text
$ python3 target/review-bak/mut.py w2 && cargo test -p vectorcraft-engine --test comic_acceptance
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.32s

$ … cargo test -p vectorcraft-engine --lib cmd::comic::tests::page_create
---- cmd::comic::tests::page_create_refuses_without_a_size_and_never_reuses_a_retired_artboard ----
assertion `left == right` failed: the retired number is stepped over, never handed out again
  left: Some(2)
 right: Some(3)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1604 filtered out; finished in 0.01s
```

So: **asked *"could a wrong implementation satisfy this test?"* the answer for the loop is yes**, and
the guard is held instead by W4's own unit test. Test 11 also does not exercise `claim_artboard`'s
other documented half — the ids a page **names** when `artboard.delete` removed their artboard
without retiring it (`M-NAMED`: none of the fifteen reds). Both are cheap to close, and my
`comic_acceptance_reviewer.rs` closes them; the recommendation is to fold the same two cases into
`acceptance_11` so the milestone's own table holds them, rather than a reviewer's file.

Test 11's failure message also makes a claim nothing checked — that a page taking a retired id is
"permanently reported and permanently unfixable" (rules 4 and 17). It is: rule 4 reports it as a
`Severity::Error` naming `retired_artboard_ids` (`comic-model/src/validate.rs:460-474`), it survives a
save and a reopen, and no command in the 19 changes a page's `artboard_id`. My third reviewer test
pins that, and M-RULE4 shows it can fail.

**R1-4 — `comic_acceptance.rs:655` and `:444-445` — three assertions restate hardcoded literals in
the implementation.** *(preference)*

`assert_eq!(second["asset_kept"], json!(true))` matches `"asset_kept": true`, a constant in
`panel.rs::replace_art`; `typographyTouched`/`geometryTouched` = `false` are likewise hardcoded in
`lettering.rs::dialogue_update`. None of the three can fail unless the field changes shape. In test
6 the real check is `assets_after == assets_before`, which bites (M-ASSET reds it); in test 5 the
real check is the field-by-field `Node` comparison, which does not. They are honest reports of the
command's contract and worth keeping, but they must not be counted as coverage.

**R1-5 — `comic_acceptance.rs:707` — test 10's reading-order assertion compares the command's reply
with the bundle, never with the document.** *(preference)*

`assert_eq!(reordered["artboards"], json!(artboards))` where `artboards` is derived from
`page_file(...).page.artboard_id` — both sides come from the model, so a `reorder` that reports the
right order and does not write it passes. M-ORDER proves it: the document's artboards were left in
creation order and **that line stayed green** while `acceptance_10` reddened on
`crates/engine/tests/comic_acceptance.rs:717` (`page 7's render is the page's dimensions`, 612×792
where 300×400 was expected) and `acceptance_10b` reddened on
`the document's artboards really are in book order, with the spare left after them` (`left: [1, 2, 3, 4, 5]`).
So the criterion **is** held — by the artefact assertions in test 10 and by 10b, exactly as W5 says —
but this one line is a self-report and should be replaced by a read of `doc().artboards`.

**R1-6 — `comic_headless.rs:246` — a disjunction that almost nothing can fail.** *(preference)*

`assert!(opened_doc.get("index").is_some() || opened_doc.get("title").is_some(), "{opened_doc}")` —
any JSON object with either key passes. The substantive assertion is the next line
(`restored != true`), which does bite.

**R1-7 — the sound-effect object of M1 §1 has no test.** *(preference; a gap in the table W5 was
given, but in W5's deliverable)*

M1 §1's required capabilities include "Add a basic sound-effect object", and
`LetteringKind::SoundEffect` exists with `comic.lettering.add {kind: "sound_effect", text}` driving it
(`lettering.rs:90`). No row of the twelve-row table exercises it — tests 7, 8 and 12 use speech,
narration and (12) the document path only. **Exit criteria: every test above passes**; the
sound-effect object is currently unproven. One more `comic.lettering.add {kind: "sound_effect"}`
inside test 7 would cover it and assert rule 8's split (its words live on the record, like a
narration).

**R1-8 — two quoted values in the report cannot be reproduced as written.** *(preference, documentation)*

* §3 quotes the M-W5-1 red at `comic_acceptance.rs:780`. The delivered file reds at **832** — the test
  file was edited after the record was taken, which W5's own §9 `sha256sum -c` output says
  (`comic_acceptance.rs: FAILED # mine`). The mutation and the message text reproduce exactly; the
  line number is stale relative to the delivered revision. Quote the message, or re-take the line.
* §5 D2 quotes `file sha256 26f85ccc…`. I get `42b9c88f…`, same 23622 bytes, because
  `document.save` stamps the document with the current time — the bytes are not reproducible, though
  the **manifest** hash (`c0089ed8…`), the size and the §14 warning all are:

```text
manifest sha256 c0089ed8b854638cd58e1310a2176cc620ae6f4f077b4a5e20c5e139f2b4919e
file     sha256 42b9c88f497adf2d1322ddc99d181f7ff78e8fb66ac4cbbfe8a519c831d0bf1e
reopen findings: [{"message":"`manifest.files` records sha256 c0089ed8… (23337 bytes) but the file is
  … (23622 bytes): the manifest describes the previous generation (§14)","rule":null,"severity":"warning",
  "subject":"document/comic.vectorcraft"}]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### Tests that cannot fail

For each of the twelve: **would it red if the behaviour broke, and how did I show it?**

| # | Reds? | The mutation that showed it |
|---|---|---|
| 1 | **yes** | M-REUSE — two pages share one artboard (`no two pages share an artboard`, `one artboard per page and no spare`). |
| 2 | **yes** | M-FRAME — the frame node is never inserted, so `frame_node_id` names nothing (`NodeId(n) is not in the document`). A mutation aimed at the fixture, not at test 2's own record assertions — see *Unverified*. |
| 3 | **yes** | M-CLIP — `clip: false` on the clip group; reds test 3 and nothing else. |
| 4 | **yes** | M-MOVE — `move_panel` translates the frame only; reds test 4 and nothing else (`and the artwork went with it`). |
| 5 | **partly** | Red under M-FRAME only — i.e. only when its *fixture* is broken, not when `comic.dialogue.update` itself touches typography or geometry. That command lives in `lettering.rs`, which a concurrent fixer owns; see *Unverified*. |
| 6 | **yes** | M-ASSET — `replace_art` drops the previous `Asset` record (`no Asset record was added or removed by replacing art`). |
| 7 | **yes** | M-OPENOUTLINE — the balloon outline is not a closed path; reds test 7 and nothing else (`and it is closed`). Same caveat as 5 for the rule-8/ownership half, which is produced by `lettering.rs`. |
| 8 | **yes** | M-SNAP — the undo entry snapshots the model **after** the step, so the count stays right and the state is wrong; reds test 8 and nothing else. This is the case the lead asked about, and the test is not fooled by it. |
| 9 | **yes** | M-OPEN — `comic.project.open` opens a blank document while the bundle loads; reds test 9 and nothing else. |
| 10 | **yes** | M-ORDER — reorder the scene but not the document's artboards; reds 10 (PNG/PDF dimensions) and 10b. |
| 11 | **yes** | M-W5-1 (W5's own) — reds test 11 and nothing else in 1769 engine tests. Also red under M-REUSE. |
| 12 | **yes** | M-ENABLE — every `comic.*` enabled with no project; reds test 12 and nothing else, engine suite green. |

**No test in the suite is decorative.** Three assertions inside tests are (§R1-4, §R1-5, §R1-6), but
each of those tests bites elsewhere.

### Tests 4, 5, 6, 8, 10 — what is actually compared

The failure mode the lead named — asserting on a **re-serialised** copy that normalises a real
difference away — is **not** present; every "unchanged" assertion compares typed values.

* **Test 4.** `assert_eq!(panel_record(&s, a), before)` compares the whole `Panel` struct
  (`frame_node_id`, `artwork_refs`, `dialogue_ids`, `lettering_ids`, `reading_order_index`, the
  continuity constraints), not JSON. The geometry is checked as a **delta** on real bounds
  (`image_moved[0] - image_before[0] == 60.0`), and the reading order as a `Vec<u64>`. Red under
  M-MOVE and nothing else.
* **Test 5.** `assert_eq!(text_after, before_copy)` compares the whole `Node`, with exactly the two
  fields the command is *supposed* to change blanked on both sides: `runs` (the words) and
  `cached_bounds`/`cached_baselines` (the layout engine's cache of the words). Everything else —
  font, size, fill, frame — is compared. The frame and tail are compared as `PathData` plus bounds,
  and the artwork as whole `Node`s. The exclusion is the minimum the contract allows, though it does
  mean a run-level typography change (a font reset inside `runs`) would not be caught; note that
  `TextLayoutSettings` and `overflow_policy` are compared separately on the `Lettering` record.
* **Test 6.** `assert_eq!(assets_after, assets_before)` is a set equality on `AssetId`s over the whole
  registry — the §11.1 lineage claim, and it bites (M-ASSET). The node-level claim is a
  `difference()` of `node_ids()` before and after, which is stronger than a count. Only
  `asset_kept` is a restatement (§R1-4).
* **Test 8.** As asked: **not** a count. `one_step` holds a full `State { doc: Arc<Document>,
  selection: Selection, bundle: ComicBundle }` and asserts `after != before`, then
  `undo → state == before`, then `redo → state == after` — document, selection **and** model. M-SNAP
  (right count, wrong state) reds it and only it, which is the proof that the count is not what
  carries the test. The count assertion (`undo_after == undo_before + 1`) is still there, and it is
  the one M-SNAP leaves green.
* **Test 10.** The PNG half reads the real IHDR (`image::open`) per artboard *index*, and the PDF
  half parses the file with `vectorcraft_pdf::info` and asserts `info.pages.len() == 3` **and** the
  width/height **per index** against three deliberately different page sizes. So it asserts order,
  not merely page count: swapping two PDF pages or two artboards reds it. Shown by M-ORDER.

### Test 12 — the back door

**Verified: there is no engine back door, and the claim is true.**

* The file imports `std::path::PathBuf`, `serde_json` and `vectorcraft_mcp::{Headless,
  PROTOCOL_VERSION, Server}` — **no `vectorcraft_engine` import at all**, so `Session` is not even in
  scope. `grep -n "session\|engine"` finds only the doc comment.
* Structurally the door exists — `Headless` has a `pub session: Session` field and `Server` exposes
  `pub fn backend(&mut self) -> &mut dyn Backend` — and the test opens neither. Every operation is
  `tools/call` → `run_command`, and every assertion reads an artefact (the reply, the bundle on disk,
  `inspect_document`) or a page file parsed from the wire format.
* The construction is the production one. `apps/vectorcraft-cli/src/main.rs`'s `mcp` builds
  `Server::new(Box::new(Headless::with_document()))` for `--headless` — the same expression, so
  `handle_line` here is the entry point `serve` calls per line.
* **I also drove the real built binary over a pipe**, which the test file does not do (it is a
  transcript in §4, not a test). It corroborates §4 exactly:

```text
$ { initialize; notifications/initialized; tools/call list_commands {"filter":"comic.","enabledOnly":true};
    run_command comic.project.create            run_command comic.page.create;
    run_command comic.project.validate } | target/debug/vectorcraft-cli mcp --headless
enabled comic.* without a project: ['comic.project.create', 'comic.project.open']
create ok: False ['assets/index.json', 'document/comic.vectorcraft', 'project.json']
page.create -> {'page': 6, 'artboard_id': 1}
validate errors: 0
```

* One precision: the phrase "a command the MCP layer would refuse" has no referent — the MCP layer
  refuses nothing. `tools.rs:748` passes `run_command` straight to `engine.execute`, and
  `filter_commands` consults enablement only for `list_commands`. The refusal test 12 asserts
  (`no comic project open`) is the **engine's** `Disabled`, seen through MCP. That is the right thing
  to assert; it just is not an MCP-layer policy.

### W5's three defect diagnoses

* **D1 — answering Save at the quit prompt discards a model-only comic edit.** **Diagnosis correct,
  evidence sufficient, and now corroborated three ways.** W5's own test passed on the pre-fix tree
  asserting the loss; the fix that has landed since changes *exactly* the code W5 named
  (`unsaved.rs`'s `is_dirty()` re-check now runs on both the foreground and the background path, and
  its doc comment says so in W5's terms); and the same test now fails in the direction the fix
  predicts. I re-ran it and reproduced W5's transcript line for line (§R1-1). The design call does
  **not** rest on a false premise — except in one detail worth knowing: W5's test drives
  `document.save` as a command, not the ⌘S binding, so it never isolated the binding; the fixer has
  since changed both (`menus.rs:69` now documents "With a ComicCraft project open … Save is
  comic.project.save").
* **D2 — ⌘S leaves the manifest's hash for the document stale.** **Correct, and it is genuinely a
  warning, not a refusal.** Reproduced above (§R1-8): the file hash differs from W5's quote because
  the document is timestamped, but the manifest hash, the byte count and the §14 "previous
  generation" warning all reproduce, and `comic.project.open` still reports the bundle usable.
* **D4 — the extended message literal has no `@msg` rows.** **Correct and sufficient.** The literal
  at `lettering.rs:609-613` now carries a second sentence; the six message-complete catalogs carried
  only the short key; `i18n::tests::complete_languages_translate_every_message` is an exact
  `HashSet` containment of the literal (`crates/ui-egui/src/i18n/tests.rs:868-870`), so each of the
  six failed in turn with `es` reported first. It has since been fixed **on the catalog side only**
  (`lettering.rs` is unchanged at `71bfa9f8…`), which is the right side: the literal is the English
  key. The test now passes:
  `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 873 filtered out`.

### W5's ⌘S evidence, against the design call

**The evidence supports the design call, but the tests do not pin the fix — they pin the defect.**
W5's §6 is a sound argument on measured behaviour: ⌘S leaves `is_dirty()` true, leaves the manifest
stale, and a Save at the quit prompt then discards the work; `comic.project.save` fixes all of it and
is already idempotent and already one write. The recommendation to route at the app level rather
than change `document.save`'s contract or claim the registry shortcut is well-argued and consistent
with §10.1.

What W5's **tests** do is record the pre-fix shape:
`evidence_the_quit_prompts_save_discards_a_model_only_comic_edit` asserts the loss *happens*;
`evidence_the_same_quit_keeps_the_edit_after_comic_project_save` proves the *alternative* path works
and that a project save clears both halves of the dirty flag. Neither asserts the specified
behaviour (a plain ⌘S or a Save at the prompt no longer losing the model), and the first now
contradicts it. That is R1-1.

### `replace_art` over MCP

**Confirmed unreachable *from a fresh project*, and demonstrated reachable *otherwise* (R1-2).**
There is no `comic.asset.*` in M1 (19 registered `comic.*` commands, verified against
`crates/engine/src/cmd/comic/{project,page,panel,lettering}.rs`), so nothing an agent can do over MCP
creates the `Linked` `Asset` record `replace_art` needs. But the command itself works over MCP
against a bundle that has one, and such a bundle is producible without any engine back door. So test
12's artwork step is weaker than test 6's for a reason that does not have to hold, and the exit
criterion "place and clip raster artwork inside panels" is currently certified only by the
engine-side test 3/6 — through `Session::execute`, not through MCP.

### W5's refusal to name its model

**The right posture, not evasion.** §5.4e establishes both halves: the rule *is* dispatched
correctly, the harness records the id, and no `MODEL:` line can prove which weights served it —
`COPILOT_MODEL` is the requested id. §5.4e explicitly praises the record that said this plainly
("That is the right epistemic posture, and it is better than the records that state a model
plainly"). Two agents already showed what the alternative produces: `claude-sonnet-4.6`, read out of
the `task` tool's own enum. W5's line cites §5.4e and states the limitation, which is the correct
answer, and my record does the same.

### Mutation hygiene

Fourteen mutations, each applied alone, run, reverted immediately, and `sha256sum`-verified:

```text
$ ./target/review-bak/restore.sh            # after every mutation, before the next
--- restored; verifying ---
crates/engine/src/cmd/comic/{mod,panel,project,lettering,page,panel,project,tests}.rs: OK
crates/comic-model/src/bundle.rs: OK
crates/comic-layout/src/lettering/outline.rs: OK
crates/engine/src/cmd/comic/page.rs: OK
```

Two notes for anyone repeating this, both of which cost me a run:

* **`cp -a` is the wrong restore.** It preserves the backup's *older* mtime, and cargo reads that as
  "not dirtier than the build I just made" and reuses the mutated rlib — I saw a `comic-layout`
  mutation survive into the next mutation's run that way. `restore.sh` now `touch`es every restored
  file. Every result in the table above was re-taken or re-confirmed under the fixed procedure.
* A mutation of a **dependency** crate (`comic-layout`, `comic-model`) rebuilds the engine too, so
  the window is minutes, not seconds. I was told mid-review that three of my windows invalidated
  another owner's `cargo xtask ci` runs; that is a real cost of this method on a shared tree, and it
  is why the windows were kept as short as the rebuild allowed.

## Gate (round 1, on the **pre-fix** revision — kept as the record of that revision)

`cargo xtask ci` — **fail**. The red is W5's inverted evidence test (R1-1); the D4 catalog red W5
reported is fixed and gone.

```text
$ cargo xtask ci
…
thread 'evidence_the_quit_prompts_save_discards_a_model_only_comic_edit' (3542917) panicked at crates/ui-egui/tests/comic_quit_prompt.rs:102:5:
assertion `left == right` failed: the user answered Save and the comic edit is gone anyway: …
  left: "Second brief."
 right: "First brief."
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

error: test failed, to rerun pass `-p vectorcraft-ui-egui --test comic_quit_prompt`

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

Note what the summary means: `fmt` and `clippy` are **green on the whole workspace** (the gate's own
steps), `test` fails, and the four steps after it never ran. The failing test is the **only** red in
the workspace run: the first failing step is `test`, inside `vectorcraft-ui-egui`, and the failing
target is its **integration** target `tests/comic_quit_prompt.rs` — not its lib, which is why W5's
§8 table (which stopped at the i18n lib failure, now fixed) does not show it. The steps the run never
reached, measured directly on the same revision:

```text
$ cargo fmt --all -- --check                              (quoted from the gate above: ok)
$ cargo clippy --workspace --all-targets -- -D warnings   (quoted from the gate above: ok)
$ cargo clippy -p vectorcraft-engine --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 32.91s
EXIT 0
$ cargo xtask layers
OK: 29 crates, no layering violations.
EXIT 0

$ cargo test -p vectorcraft-engine --test comic_acceptance          -> ok. 15 passed; 0 failed
$ cargo test -p vectorcraft-engine --test comic_acceptance_reviewer -> ok.  3 passed; 0 failed   (mine)
$ cargo test -p vectorcraft-mcp --test comic_headless               -> ok.  1 passed; 0 failed
$ cargo test -p vectorcraft-ui-egui --test comic_quit_prompt        -> FAILED. 1 passed; 1 failed
$ cargo test -p vectorcraft-ui-egui --lib i18n::tests::complete_languages_translate_every_message -> ok. 1 passed
```

One process note the gate's own tooling produces: `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh "" M1` reports **this record OK** and 4 of the other M1 records OK, but flags `docs/reviews/M1-acceptance.md` — W5's implementer report — as an incomplete *review* record, because the checker's filter is `<milestone>-*.md` and it requires a `Reviewer:` line and both rounds. It is not a review record and never was; either its name should follow the convention the rule states
(`docs/reviews/<milestone>-<owner>-<slug>.md`, i.e. `M1-verification-engineer-acceptance.md`) or the checker should skip a file that has no `Reviewer:` line. As it stands `check-reviews.sh M1` can never exit 0 while that file has its current name, which will read as a missing review at milestone close. Not mine to rename.

*Round 2: the lead took the second option and made it explicit — the checker now pairs an implementer
report with its `-review.md` sibling (it has neither a `Reviewer:` nor a `Verdict:` line, which is what
makes it an implementer report). `check-reviews.sh "" M1` is **exit 0**, verified in §The `-review.md`
convention. The rename is no longer needed.*

## Round 2 — the post-fix revision, re-measured

Verdict:       approved
Unit tests:    crates/engine/tests/comic_acceptance.rs (15 passed; 0 failed) · crates/mcp/tests/comic_headless.rs (1 passed) · crates/ui-egui/tests/comic_quit_prompt.rs (3 passed) · crates/comic-model all targets (135 passed) · crates/engine/tests/comic_acceptance_reviewer.rs (3 passed, untouched) · cargo fmt --all -- --check exit 0 · cargo clippy -p vectorcraft-engine -p vectorcraft-mcp -p vectorcraft-ui-egui --all-targets -- -D warnings exit 0 · cargo xtask layers: OK (29 crates) · cargo xtask ci: green on all seven steps (below)
Findings:      0 defects, 3 notes — none blocking. Each below as file:line — trigger — expected vs actual — bug|preference
Revision reviewed: the **post-fix working tree** — an uncommitted revision, so its identity is the file hashes, not a commit — crates/engine/tests/comic_acceptance.rs c865499eee5d3a1e31a0ffb2f903e4e81ac0c80d84fed92ab87c6ff366c1e976, crates/mcp/tests/comic_headless.rs a29cc9552717c0c6d638212f1a7e388f66c07265f92d9d3f7c595fcca0782275, crates/ui-egui/tests/comic_quit_prompt.rs a8c9c00d828190e4ca5c9a8dad8c5c9d7881c6823ac8639b1017cc8f9595eb26. The round-1 revision these supersede was c5564cf1… / d95157d8… / b728f213… (§Revision reviewed above).

This is a different revision from round 1's, and the difference is exactly the two defects: the
inverted quit-prompt test (R1-1) and test 12's panel-artwork step (R1-2). One correction to round 1's
revision line: this **is** a git repository (`HEAD` = `16594c58`, the preflight/gate commit), so "no git
repository to name a commit" meant "no commit names this revision" — the three test files are
**untracked** and the production files they exercise are part of the uncommitted working tree, so the
content hashes are the revision's identity and a commit id is not available. That also means
`git diff` cannot witness a restore here, so the **hash check is the only witness** — and it holds:
every production file the round-1 mutation table names, and all five files this round mutated, are at
the hashes the record carried before the round (`page.rs` `b1a1c888…`, `panel.rs` `9e048349…`,
`lettering.rs` `71bfa9f8…`, `comic-layout/.../style.rs` `25eb75c3…`, `io.rs` `6ae00950…`), verified
with `sha256sum -c` against §9's list (§Mutation hygiene, round 2).

### The question, and the answer

Unchanged from round 1 — W5 is test code, so *"is this correct"* means *"would it red if the
behaviour broke"*. The two defects were both *"the test does not exercise the claim"*; the six notes
were one un-pinned half of mitigation 4, three assertions that restated a literal, one self-report,
one unfailable disjunction, one uncovered capability and two non-reproducible quotes. **All eight are
now falsifiable, and I falsified them rather than reading them.** Twelve mutations (ten that red, both
recorded including the two that stayed green), each applied alone, run, restored immediately,
`sha256`-verified.

### Round-2 mutation table

| # | Mutation (file) | The wrong implementation | Acceptance tests that red |
|---|---|---|---|
| 1 | **M-W5-1** `page.rs` — `free_artboards` drops the retirement check | the fast path offers a retired id back | **11** only (`14 passed; 1 failed`) |
| 2 | **M-W5-2** `page.rs` — `claim_artboard`'s loop drops `retired` | the loop hands out a retired id | **11** only — round 1: green |
| 3 | **M-NAMED** `page.rs` — the loop drops `named` | a page takes a number another page resolves to | **11** only — round 1: green |
| 4 | **M-ASSET** `panel.rs` — `replace_art` also drops the previous `Asset` record | §11.1 lineage lost | **6** only, at `asset_kept` (round 1: the literal could not fail) |
| 5 | **M-ORDER** `page.rs` — `reorder` writes the scene, not the document's artboards | the PDF/PNG walk creation order | **10** (the new document read) + **10b** |
| 6 | **M-ROUTE** `io.rs` — the app-level project-save routing is disabled (`&& false`) | ⌘S and the prompt's Save are `document.save` again — the reported revision | **quit-prompt 2 of 3**: arm 1 and the refusal arm; the control stays green |
| 7 | **M-SEL** `panel.rs` — `replace_art` does not select the placed image | `object.cropImage` has nothing to act on | **12** over MCP (`comic_headless.rs:321`) |
| 8 | **M-SFREC** `lettering.rs` — a tailless kind falls back to the frame for `tail_node_id` | a sound effect is written with a tail | **7** (`a sound effect has no tail`) |
| 9 | **M-DLG** `lettering.rs` — `dialogue.update` also rewrites `overflow_policy` | the command reports `typographyTouched: false` and touched typography | **5** (the new derived `typographyTouched`) |
| 10 | **M-RELAY** `page.rs` — `template.apply` re-creates the panels instead of re-laying them | a re-layout does not keep the page's panel list | **2** (its own stability assertion, not its fixture) |

Raw results for the four the lead asked for by name, as printed:

```text
$ python3 target/review-r2/mut.py w5-1 && cargo test -p vectorcraft-engine --test comic_acceptance
test acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path ... FAILED
  panicked at crates/engine/tests/comic_acceptance.rs:892:5:
FAIL (§5.3.1 mitigation 4, the fast path): comic.page.create gave page 8 the retired artboard 2, the id
the deleted page 7 used. A retired number means "never again": rules 4 and 17 make a page that takes one
permanently reported and permanently unfixable
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

$ python3 … mut.py w5-2 && cargo test …
  panicked at crates/engine/tests/comic_acceptance.rs:933:5:
FAIL (§5.3.1 mitigation 4, the loop): the allocation loop handed page 8 the retired artboard 2, which no
page may ever resolve to (rule 4)
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured

$ python3 … mut.py named && cargo test …
  panicked at crates/engine/tests/comic_acceptance.rs:962:5:
assertion `left != right` failed: FAIL (§5.3.1 mitigation 4, a named id): a page took 2, which the second
page still resolves to — one artboard, two pages, and the rule-4 report of the first page's breakage is
deleted
  left: 2 / right: 2
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured

$ python3 … mut.py asset && cargo test …
  panicked at crates/engine/tests/comic_acceptance.rs:522:5:
assertion `left == right` failed: the report is what the registry shows: the replaced asset's record survives
  left: Bool(true) / right: Bool(false)
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured
```

**All three halves of mitigation 4 are now pinned, each by a mutation that reds test 11 and nothing
else in the acceptance suite.** Round 1's partial pin is closed: M-W5-2 and M-NAMED left all fifteen
acceptance tests green before, and each reds test 11 alone now (round 1 measured M-W5-1 across the
whole 1769-test engine suite; I measured the same scope round 1 did for the other two).

### The three round-2 notes

* **N-R2-1** — `crates/ui-egui/src/unsaved.rs:98-101` — trigger: the D1 invariant's last limb
  (`"<title>: not saved, so not closed"`) has **no test**; the implementer's §11 attributes it to "the
  error path", which is `io.rs`'s `?` at `unsaved.rs:84` and a different return site. Expected: a
  data-loss guard is falsifiable. Actual: reachable only from a save that returns `Ok` without clearing
  `is_dirty()` — unreachable post-fix and un-interleavable on one thread — so no driven test can red it
  without a seam. **preference** — see §The refusal arm for what would close it.
* **N-R2-2** — `docs/comiccraft-reuse-report.md:121` and `docs/milestones/M1.md` §5.12 — trigger: both
  say `replace_art` "is unreachable over MCP". Expected: the command *is* reachable over MCP; what M1
  has no command for is *creating* the `Linked` `Asset` record it needs, and test 12 now drives the
  command. Actual: the round-1 wording survived, and it now reads as "test 12 does not cover this" when
  test 12 does. **preference** — the lead's file, not mine to edit.
* **N-R2-3** — `crates/engine/tests/comic_acceptance.rs:606` and `crates/comic-layout/src/lettering/{style,plan}.rs`
  — trigger: test 7's `a sound effect has no tail` is **double-guarded** (`SoundEffect`'s style
  `tail: None` and `tail_spec`'s `is_spoken()`), so two single-guard mutations of the kind a reader
  would naturally aim at it (M-TAIL, M-SFSTYLE) stay **green**. Expected: an assertion named for a rule
  reds when the rule is broken. Actual: it reds against the composed decision (M-SFREC) and against the
  guards' own unit tests, not against either guard alone. **preference** — recorded so the pin is not
  read as stronger than it is.

None of the three is a test that cannot fail, and none touches a claim the twelve rows rest on, so
none blocks.

### Two mutations that did *not* red, and what that means

I record these because they bound the claim, and a reader should not take the table for more than it is.

* **M-TAIL** `lettering.rs` — `with_tail(tail)` unconditionally, so every kind carries a target:
  **15 passed**. `request.style.tail?` is the first guard and `SoundEffect`'s style has `tail: None`.
* **M-SFSTYLE** `comic-layout/lettering/style.rs` — `SoundEffect`'s style `tail: None` →
  `Some(TailKind::Bubble)`: **15 passed**. `tail_spec`'s `request.kind.is_spoken()` is the second
  guard.

So test 7's `tail_node_id.is_none()` for a sound effect is **double-guarded in production**, and no
*single* change to either guard alone reds it. It is still falsifiable — M-SFREC (the engine's record
writer) reds it, and each guard is separately pinned by `comic-layout`'s own tests — but "test 7 pins
the tail rule" should be read as "test 7 pins the composed decision", which is the honest claim.

### R1-1 — verified, and the test-only revert was **not** sufficient evidence on its own

* **Inverted, both arms, both driven through the paths a user reaches.** Arm 1 drives the **binding**
  (`file.save`, not the `document.save` command the old test called) and asserts the reply names a
  `root`, that the bundle on disk holds `"Second brief."` and that `is_dirty()` is false; arm 2 makes a
  second model-only edit, quits, answers Save, and asserts the disk holds `"Third brief."`, the document
  is gone and `status == "quit"`. The new `a_save_that_cannot_complete_refuses_the_close_and_discards_nothing`
  makes the save fail through the bundle's own §13 rule-13 refusal (`pages/page-06.json` beside
  `pages/page-6.json`) and asserts refusal, still open, still dirty, edit still in the model, pre-edit
  brief still on disk, `status` not `quit`. All three pass, and the third could not have been written
  against the pre-fix revision at all.
* **Could either arm pass while the loss still happens? No, and I checked both directions.** Arm 1
  cannot: `document.save` reports `path` and never `root`, and the disk assertion reads the bundle the
  model write produced, so a document-only write fails on the first line. Arm 2 cannot: "the loss" means
  the edit is in neither the model nor the disk, and the test reads `"Third brief."` off the disk *after*
  the prompt's Save, with the on-disk value before it being `"Second brief."` — so the save is proved to
  have happened. And the close is proved to be *conditional*: `unsaved::confirm` reaches `close_now`
  only past the dirty re-check, so `documents().is_empty()` plus `!dirty` cannot both hold on a lie.
* **The test-only revert is a proxy, not the evidence.** Reverting the *test's* call to
  `document.save` proves the `root` assertion discriminates the two commands; it does **not** prove the
  test reds when *production's routing* regresses, because it also bypasses `io::save`, which is the
  code the D1 fix changed. So I did the production revert myself: **M-ROUTE** disables the routing at
  `crates/ui-egui/src/io.rs:285` (`&& false`), applied alone, and **2 of the 3 tests red** —

```text
⌘S → {"bytes":32330,"format":"vectorcraft","path":"…/document/comic.vectorcraft","warnings":[]}
thread '…' panicked at crates/ui-egui/tests/comic_quit_prompt.rs:133:5:
⌘S with a project open is comic.project.save, not document.save: {…}
---- a_save_that_cannot_complete_refuses_the_close_and_discards_nothing stdout ----
wrote pages/page-06.json, so the next save must refuse
quit asked first: {"pending":"saveChanges"}
the close was refused with: comic.vectorcraft: not saved, so not closed
thread '…' panicked at crates/ui-egui/tests/comic_quit_prompt.rs:170:5:
the refusal is the bundle's own, not a silent close: comic.vectorcraft: not saved, so not closed
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

  `io.rs` was restored to `6ae009503ddcd287a5418c1b8859ca77e2bf09f07ed0d847df2649dc315680ae` immediately.
  That is what round 1 asked for and what the implementer's test-only revert did not supply.

### R1-2 — verified: it is the wire format, not a back door

The prepared bundle is `assets/plate.png` (the same 75-byte plate the document-path step places, its
`sha256` in `content_hash`) plus `assets/index.json` in the §5.5 shape the engine's own
`AssetIndexFile`/`Asset`/`StorageRef::Linked`/`ContentHash`/`MediaMetadata` types decode — I checked
each field against `crates/comic-model/src/{bundle,entities}.rs`, including that `Linked` is
externally tagged (`{"linked":{"relative_path":…}}`). Four things make it a fixture rather than a
back door:

* **No door is used.** The file's imports are `std`, `serde_json`, `vectorcraft_mcp` and
  `vectorcraft_format::base64_decode` — still no `vectorcraft_engine`, so `Session` is not in scope, and
  no bundle writer is called. There is now a fourth import (the base64 helper); it is a codec, not the
  comic model, and it does not weaken the claim.
* **The unit of work is still the tool call.** The record is written while the project is **closed**,
  then the bundle is reopened over MCP (`open_file` + `comic.project.open`) and every step of the
  operation is `tools/call` → `run_command`.
* **The engine accepts and re-writes it.** After `comic.panel.replace_art` the test saves over MCP and
  reads `assets/index.json` **as the save wrote it** — one record, id 1000 — with the panel's
  `artwork_refs` naming it once, and `comic.project.validate` reporting 0 errors both after the reopen
  and after the artwork step (rule 5's hash check is part of that). A hand-built bundle is exactly what
  `comic.project.open` exists to read; round 1 said so, and this is the demonstration.
* **The step has teeth over MCP**: **M-SEL** (no selection) reds
  `comic_headless.rs:321` — `the placed image is selected, so object.cropImage acts on it`.

### R1-3 … R1-8 — verified

* **R1-3 — closed, all three.** M-W5-1, M-W5-2 and M-NAMED each red test 11 and nothing else (table
  above). The two halves round 1 could not red are now red, and part 4's consequence assertion is
  unchanged — `validate.rs` is still `58f23b98…`, the hash round 1's M-RULE4 was applied to, so that
  result still stands.
* **R1-4 — verified, and it bites.** `asset_kept` is now `assets_after.contains(&asset_a)`; M-ASSET
  reds it at `:522`. `typographyTouched`/`geometryTouched` are derived from the record, the frame and
  the tail; M-DLG (a real `overflow_policy` change reported as untouched) reds test 5 at `:463`. This
  also closes round 1's *Unverified* item for test 5: it **is** now falsified against its own command
  (`lettering.rs`), which round 1 could not do while that file was owned elsewhere.
* **R1-5 — verified.** Test 10's order assertion now reads the **document**
  (`artboard_ids(&s)`), and M-ORDER reds it at `:760` — the line that stayed green in round 1 — plus
  10b at `:814`.
* **R1-6 — verified.** The disjunction is gone: `open_file`'s reply is asserted with
  `assert_eq!(opened_doc["title"], json!("comic.vectorcraft"))` and `opened_doc["index"].as_u64().is_some()`.
  A string equality and a typed read both can fail; the substantive `restored != true` line is unchanged.
* **R1-7 — verified.** Test 7 adds the sound effect and pins the no-tail rule, the rule-8 split, the
  closed path frame and editability. **M-SFREC** reds it at `:606`. Two neighbouring mutations stayed
  green and are recorded above, so the claim is bounded honestly: the *composed* decision is pinned.
* **R1-8 — verified.** §3's line number is annotated as moved (and moved again: 892 for that message),
  and §5 D2's file hash is annotated as non-reproducible. Re-run: `manifest sha256 c0089ed8…`, file
  `4eec95c0…` (round 1 measured `42b9c88f…`, W5's round 2 `5e6ed4f7…` — three different values, same
  23622 bytes), the §14 warning and both byte counts reproduce. The six `i18n/*.tsv` hashes verify in
  the tree.

### The refusal arm — is the error path enough for a data-loss invariant?

**Partly: the implementer's characterisation is imprecise, and the branch is genuinely unpinned — but
it does not block approval.** Two precisions:

1. **The branch is not the error path.** `a_save_that_cannot_complete_…` makes `io::save` return `Err`
   through `comic.project.save`'s rule-13 refusal; the test asserts that message (`"two files encode one
   id"`), so it exercises the `?` at `unsaved.rs:84`, **not** the `"<title>: not saved, so not closed"`
   guard at `:98-101`. The guard is not pinned by any test in the tree —
   `grep -rn "not saved, so not closed" --include=*.rs` finds the message and no test.
2. **It is deterministic, not race-only.** My M-ROUTE window reached it: with the routing gone, a
   foreground `document.save` succeeds and leaves the model dirty, and `confirm` refuses with exactly
   `comic.vectorcraft: not saved, so not closed`. So the branch is reachable whenever a supported save
   returns `Ok` without clearing `is_dirty()` — which is the *pre-fix world*, deterministically. What is
   true is the reverse: **post-fix it is unreachable from the command surface**, because every plain
   Save with a project open writes and marks both halves (`project.rs:347-349` calls `mark_saved` and
   `mark_comic_saved`). Only the background-save/edit interleaving round 1 named remains as a route.

**Judgement: it is defence in depth, and defence in depth that is *observed working* is enough to
approve a milestone — provided it is recorded as a residual, which this note does.** I saw the branch
execute and return `Err` ahead of `close_now` (M-ROUTE above); the `return` precedes `close_now` by
construction, and I read the two lines. It narrows the invariant's coverage by exactly one limb.

What would close it, if the lead wants it pinned rather than inspected (this needs a production seam,
so it is a change, not a test): extract the three-line tail of `confirm`
(`wait_all` → re-check `is_dirty()` → `Err`) into a `pub(crate)` helper and unit-test *that* with an
artificially dirty document — no save, no race — so deleting `.filter(|d| d.is_dirty())` reds a test.
Cheaper alternative: it needs no production change at all if `unsaved.rs`'s own `mod tests` is given a
`Services::write_async` that completes after a doc edit — but nothing in the harness can interleave on
one thread, which is why the seam is the honest answer.

### The `-review.md` convention — right call, and I checked it is fail-closed

Pairing is right, and for the same reason M1 §5.4e gives about the reviewer's own test file: the
artifact that must survive must be owned by its author. A `Verdict:` line inside the implementer's own
report would be the implementer certifying itself, and it would let a later edit to the implementer's
prose move the reviewer's verdict. The checker's rule is well-chosen too — "no `Reviewer:` and no
`Verdict:` anywhere" is what makes a file an implementer's report, and it is *fail-closed*: an
implementer report that ever grows a line starting with `Reviewer:` gets parsed as a half-written
review and reported as a GAP, not silently paired. I verified the pairing does not weaken the check:
the sibling is itself held to both rounds, a real verdict, a unit-test result and the model/waiver rule.

Measured, both ways:

```text
$ bash .github/skills/comiccraft-verify/scripts/check-reviews.sh "" M1
Review records (6 file(s) for M1)
  OK   docs/reviews/M1-acceptance.md
         (implementer report; reviewed in M1-acceptance-review.md)
  OK   docs/reviews/M1-acceptance-review.md  … (4 more)
All review records complete.                                                        EXIT 0
```

So `M1-verification-engineer-acceptance.md` is not needed, `docs/reviews/M1-acceptance.md` stays where
its owner put it, and §5.4e's "pair it with the sibling" is the convention the rest of the milestones
should follow. One caveat for the checker's owner: the sibling is found by name only, and the test for
"is this an implementer report" is "no *line starting with* `Reviewer:` or `Verdict:`" — so a future
*reviewer* file that lost its `Reviewer:` and `Verdict:` lines would be classified as an implementer
report and, having no sibling, reported as a GAP saying "no reviewer content" about a file that is a
review. That is fail-closed rather than unsafe, but the message would misdirect; worth a one-line
comment in the script, or a check that the sibling's own round sections exist.

### `replace_art` over MCP — the operation meets the criterion; the *input* is reduced

**It meets the exit criteria's intent, and the reduction is one the milestone report should name.**
The criterion is *"the same essential operations succeed through `mcp --headless`"*, and M1 §1's
capability is *"place and clip raster artwork **inside panels**"*. Test 12 now drives that operation
through `tools/call → run_command` with no engine back door, which is exactly what the criterion asks;
*creating a `Linked` `Asset` record* is not an operation M1 lists, and the record is data a bundle may
legitimately already carry — the same status as opening a `.bundle` a third party wrote.

The honest boundary, which I would record in the milestone report in one line: **an agent starting from
a fresh project over MCP cannot reach `comic.panel.replace_art` at all** — there is no `comic.asset.*`
command, `file.place` links nothing, and the command refuses an `Embedded` or `Package` asset by name —
so test 12's coverage of that capability is "the command succeeds over MCP against a bundle that
already carries the asset". Not a gap in the MCP surface (the command is registered, enabled and
works); a gap in M1's command set, which the plan intends. Two records already carry a version of this
and both are now slightly stale in wording: `docs/comiccraft-reuse-report.md:121` says `replace_art`
"is unreachable over MCP", and `docs/milestones/M1.md` §5.12 repeats it — the precise statement is that
the asset's *record* is unreachable, and test 12 no longer leaves the operation uncovered.

### Mutation hygiene, round 2

The twelve mutations live in `target/review-r2/mut.py` (never in the tree: `target/` is not tracked).
Each case takes a byte-exact snapshot, asserts its anchor appears **once**, applies, prints the hash
before and after, and restores with a `touch` so cargo cannot reuse a mutated rlib — the trap round 1
recorded. Five production files were touched, and all five are back:

```text
$ python3 target/review-r2/mut.py restore
25eb75c3dc0c0aa84098bb971803d110ae657ba05630ffc03242795b79a44ca7  crates/comic-layout/src/lettering/style.rs
71bfa9f83db05d77509391916ed4dbe9d31409748aa009a7a044a4c94ea500f6  crates/engine/src/cmd/comic/lettering.rs
b1a1c888b5257da41cc131064cdeaa8ebb972f4dbca549964674fd576cb152db  crates/engine/src/cmd/comic/page.rs
9e048349406b75d3bdde8008af667bcd861556e3841b6d8861f8905911a170a8  crates/engine/src/cmd/comic/panel.rs
6ae009503ddcd287a5418c1b8859ca77e2bf09f07ed0d847df2649dc315680ae  crates/ui-egui/src/io.rs

$ grep -oE '^[0-9a-f]{64}  [^ ]+$' docs/reviews/M1-acceptance.md | sort -u > record-hashes.txt
$ sha256sum -c record-hashes.txt
… 23 lines … WARNING: 3 computed checksums did NOT match
```

The three mismatches are the **round-1** values of W5's three test files, which §11 supersedes — every
one of §11's round-2 hashes verifies `OK`, and every production hash in §9 verifies, including the two
this round re-mutated (`page.rs`, `panel.rs`, `lettering.rs`, `style.rs`) and `io.rs`. **I changed no
production file**: the twelve mutations are the only edits, and each was undone before the next.

I also did **not** re-run the full gate under a mutation; the gate result below is the post-restore tree.

### Gate, round 2 — green on all seven steps

```text
$ cargo xtask ci
CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)

$ cargo test --workspace | grep '^test result' | <sum>
targets (test-result lines): 146
passed: 5452
failed: 0
```

The workspace total matches the lead's figure exactly (5452 across 146 targets), measured by me on the
restored tree after every mutation was undone. `cargo fmt --all -- --check`, the three packages' clippy
with `-D warnings`, and `cargo xtask layers` (29 crates) were also run individually and are green.


## Unverified

**Round 1's list** (kept as the record of that revision; the two items it names about tests 5 and 7 and
about test 2 are now **closed by round 2** — see §R1-3 … R1-8):

* **~~Tests 5 and 7 were not falsified against their own command.~~** Closed: **M-DLG** reds test 5
  against `lettering.rs`, **M-SFREC** reds test 7's sound-effect claim, and **M-TAIL**/**M-SFSTYLE**
  are recorded as the negatives that bound it.
* **~~Test 2's own assertions likewise rest on a fixture-level mutation.~~** Closed: **M-RELAY** (a
  `template.apply` that re-creates the panels instead of re-laying them) reds test 2's own
  `and the page still lists them in order` at `:310`.
* **`cargo test --workspace` end to end.** I ran the gate but the tree was moving under it (the D4
  fix landed during my review), and another owner was running the same gate concurrently. The
  per-target results above are mine and are repeatable; the aggregate summary is not necessarily
  from a quiescent tree.
* **Windows and the wasm/assets/brands steps** on my clean tree: round 1's gate never reached them.
  Independently, `cargo xtask layers` and `cargo fmt --all -- --check` and engine clippy are green.
* **W5's D3 and D5** (another owner's round-2 test, and the shell debris in the repository root) are
  its observations about other owners' work. D5's file (`ritten.contains(key);|XX|`) is **no longer
  in the tree** — someone removed it — so that note is closed; I did not remove it.

**Round 2's own list** — what this round could not check:

* **The `unsaved.rs:98-101` guard under a driven case.** I saw it fire (M-ROUTE) and read it, but no
  test reds it, and no single-threaded harness can interleave the edit a background save would need.
  N-R2-1 records it; the seam that would close it is named in §The refusal arm.
* **The gate, on the tree as of the moment each mutation was applied.** I restored before every
  verification run, so the gate result below describes the post-restore tree. A mutation window is
  minutes for `comic-layout`/`comic-model` edits, and I kept the two dependency-crate mutations
  (`style.rs`) as short as the rebuild allowed.
* **Windows, and `CRAFT_FONTS`.** Linux only, and the environment's comic faces are still missing, so
  no acceptance test here checks rendered lettering pixels — unchanged from round 1.
* **W5's report for arithmetic I did not re-derive.** I re-ran the mutations and the hashes it quotes,
  and verified its §9/§11 lists against the tree, and I reproduced the workspace total (`146` targets,
  `5452 passed`, `0 failed`) on the restored tree; I did not re-derive the milestone's other counts, and
  I did not run the assets/brands steps under a mutation.

