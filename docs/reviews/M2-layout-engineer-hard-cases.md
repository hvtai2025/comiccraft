# M2 W5 — `layout-engineer`: the two layout criteria at their own wording

Owner:         `layout-engineer` (W5). Module tree `crates/comic-layout/src/{layout,panel,template,reading_order,validate}/**`.
Model:         **`deepseek-v4.1-flash`**. Evidence, not a self-report: this session's environment carries
               `COPILOT_MODEL=deepseek-v4.1-flash`, and the session's own event log
               (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`) carries
               `"model":"deepseek-v4.1-flash"` on **17 316** records (next: `space-bunny` 563,
               `gpt-5.4-mini` 447). M1 §5.4e is why the environment and the log are both quoted: two agents
               self-reported a model the harness never dispatched.
Design refs:   design §8.1 (page layout: inputs, outputs, "Validation warnings"; the deterministic validator
               "must still check overlap, gutter width, minimum panel size, safe margins and reading order
               before anything is written"), §5.1.1 ("Pages are ordered by an explicit comic `reading_order`,
               never by artboard position"), §12.1 (the check→owner table: "Invalid panel geometry |
               `comic-layout` | overlap, gutter width, minimum panel size, containment in the artboard" and
               "Invalid reading order | `comic-layout` | `Page.reading_order` covers every panel exactly
               once"); schema §5.2 (`reading_direction`), §6.2 (`Panel.reading_order_index`, `Page.reading_order`),
               §13 rule 6. M2.md §1 rows 1 and 4 (the criteria, quoted below verbatim) and §3 row W5.
Toolchain:     `cargo 1.99.0` / `rustc 1.99.0` (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`),
               as every M0–M2 record notes: the system `cargo` is 1.75 and cannot build this workspace.
Revision:      the `Revision:` line at the end of this file — it is the citation.

**W5 changed no source file.** The two criteria are already implemented in this tree; what M2 §1 row 1
says they need is *"hardening and a shift test"*, and M2 §1 row 4 says the half that needs work is the
one that is a claim about an **absent input** — *"needs a test that would fail if coordinates leaked
in"*. So W5 is two test files and the evidence that they are doing their job, and the `Revision:` line
below proves the sources are byte-identical to their pre-W5 revisions.

---

## 1. Criterion 1 — *"panels can be created from templates, reordered and resized with validation warnings"*

M1 built the pieces: five templates write panel rectangles (`template::rows`), `panel::place` adds the
narrative index, and five geometry checks read rectangles back (`validate::faults` → `Overlap`,
`GutterTooThin`, `TooSmall`, `OutsidePage`, `InSafeMargin`). What M1 did not have is the **cycle** the
criterion states, on one page, with the warnings asserted on both sides of every threshold.

`crates/comic-layout/tests/m2_panel_cycle.rs` is that cycle, once, in order:

| step | what it does | what must be reported |
|---|---|---|
| 1. apply | `layout::plan(NewPage(612×792), Grid{2,2}, ltr)` — the `comic.page.template.apply` half | **nothing.** The template's own plan is not reported against itself |
| 2. reorder | the page's order becomes `[11, 12, 9, 10]` and every `Panel.reading_order_index` shifts with it (the `comic.reading_order.set` half) | **nothing.** A reorder changes narrative data and no rectangle; rule 6 runs and is satisfied |
| 3a. resize, legal | slot 0 loses 10pt of width: the gutter to its neighbour grows 12pt → 22pt | **nothing** |
| 3b. resize, thin gutter | slot 0 gains 10pt: the gutter falls 12pt → 2pt, below the 6pt minimum | **one warning**: `Page.panels[0] — sits 2pt from Page.panels[1], closer than the 6pt minimum gutter` |
| 3c. resize, safe margin | slot 0's left edge moves to 10pt: inside the page, outside the 18pt margin | **one warning**: `Page.panels[0] — crosses the safe margin; a panel that deliberately bleeds should be confirmed` |
| 3d. resize, too small | slot 0 becomes 40×40pt, below the 48×48pt minimum | **one error**: `Page.panels[0] — is 40x40pt, below the 48x48pt minimum panel size` |
| 4. reload | the same resized page, re-checked through `validate::page` with the frames the command wrote into the document | **the same single warning**, now labelled `Panel(9) — sits 2pt from Panel(10) …`, and the reorder still reports nothing |

Three things make this the cycle rather than three tests in a row:

* **The steps are dependent.** The page that is resized in step 3 is the page that was reordered in step
  2, and the page re-checked in step 4 is the one resized in 3b — one page, four states, and each
  assertion is made about the state the previous step produced.
* **Every "must not warn" case is asserted, not assumed**, and the sharpest of them is step 1: M1's F1
  was a *spurious warning* on the last row of every 7-row page (and a spurious *error* on the same page
  with a zero margin), on a plan the template had just built. A run that produced the right geometry and
  the wrong warnings is exactly the M1 defect shape, so "empty" is asserted with the same force as the
  exact message.
* **The premise of each warning is measured from the rectangles**, not read off the message: step 3b
  computes `panels[1].x() - panels[0].right() == 2.0` and asserts it is below `Limits::min_gutter` before
  asking the validator, so the test states *why* a warning is due. The message is then pinned in full,
  because that is what a creator acts on.

One documented behaviour is pinned here rather than left to be discovered: after the reorder, the panel
in slot 0 (id 9) is read **second**, and the geometry warning still names it `Page.panels[0]`.
`validate::plan` documents that ("placement order … It is not the reading order"), and the test asserts
both halves — `(9, 2)` for `(id, reading position)` and `Page.panels[0]` for the subject — so a future
change that started labelling geometry findings by narrative position would be caught.

---

## 2. Criterion 4 — *"reading order is explicit, editable and validated independently of panel coordinates"*

`reading_order` derives the order from the template's **row structure** and the project direction
(`rtl` reverses each row, never the page) and `reading_order::check` compares ids and indices only — it
is handed no rectangle at all. So the implementation is right. The difficulty is that **a test which
only checks the resulting order proves nothing**: on the output of every template in this crate, the
row-derived order and a coordinate-derived order are the *same function*, so the two implementations are
indistinguishable on exactly the input a naive test would use.

`crates/comic-layout/tests/m2_reading_order_independence.rs` makes them distinguishable, four ways.

### 2.1 The derivation: same rectangles, different groupings, different orders

The narrative structure *is* the row grouping; the rectangles are only placed by it. So four rectangles
in **identical positions** — one band across the page — grouped three ways must read in three different
orders:

| grouping | `rtl` indices | why |
|---|---|---|
| `[[c0,c1,c2,c3]]` | `[3, 2, 1, 0]` | one row of four reverses wholesale |
| `[[c0,c1],[c2,c3]]` | `[1, 0, 3, 2]` | two rows of two, each reversed on its own |
| `[[c0],[c1],[c2],[c3]]` | `[0, 1, 2, 3]` | four rows of one have nothing to reverse |

A coordinate-derived `place` answers `[3, 2, 1, 0]` **three times** — it sorts by `(y, x)`, calls each
run of equal `y` a row, and cannot see the grouping at all. The test writes that wrong reading out in
the file itself (`wrong_coordinate_order`) and asserts both what it says and that the crate says
something else, which is the M-CLAMP shape: state the wrong answer, then show the code is not it.

And the test ends by asserting the case that makes this construction **necessary** rather than
decorative: on a 2×2 grid and a wide-top, in both directions, `place` and `wrong_coordinate_order`
agree exactly. That is the sentence a reviewer should check first — it is why a template-only test
proves nothing.

### 2.2 The validation: a moved page keeps its order and still validates

One page, four panels, two assignments of the same four cells:

* **ltr.** The order is `[9, 10, 11, 12]`, and the frames are assigned so the page's *geometric* order —
  computed by the test, in `order_by_geometry`, independently of the crate — is `[12, 10, 11, 9]`.
* **rtl.** The order is `[10, 9, 12, 11]` (the manga order: each row reversed), and the frames sit in the
  plain left-to-right sequence `[9, 10, 11, 12]`.

In both, `reading_order::check` and `validate::page` must report **nothing**. The test asserts the
premise before the conclusion (`assert_eq!(order_by_geometry(&moved), vec![12, 10, 11, 9])` and
`assert_ne!(geometric, slots)`), so it cannot silently become a test of a page whose geometry agrees with
its order. This is the test that fails if coordinates leak into the *validator* — the shape a reader
would plausibly write, "the reading order has to match what the page looks like".

### 2.3 The two converse cases

* **Editable.** A 2×2 page whose order is `[11, 9, 12, 10]` — `[2, 0, 3, 1]` in placement slots — with
  every index in step validates clean. The test asserts that order is **not** one any template produces
  (`narrative_order` gives `[0,1,2,3]` for `ltr` and `[1,0,3,2]` for `rtl`), so an implementation that
  re-derived the order from the shape is rejected here.
* **Reported.** With the geometry a perfect left-to-right sequence `[9, 10, 11, 12]`: an order that
  forgets panel 12 is **one error** on `Page(8).reading_order` ("not a permutation"); an order that is a
  permutation but has panels 11 and 12 claiming each other's positions is **one error each** on
  `Panel(11).reading_order_index` and `Panel(12).reading_order_index`. The control then puts the order
  right and asserts **nothing**, on the identical frames — so the findings are the order's and the
  frames had no part in them.

These two are what stop §2.1–2.2 from being satisfied by a validator that has simply stopped checking.

---

## 3. The plausible-but-wrong implementations, and whether the tests reject them

Three mutants, one at a time, restored immediately and `sha256sum`-verified against the recorded
baseline. Each is a *plausible* reading of the criterion, not a strawman.

| # | mutation | where | their new tests | everything else |
|---|---|---|---|---|
| **M-COORD** | the order is read off the rectangles: sort by `(y, x)`, treat each run of equal `y` as a row, reverse each run for `rtl` | `panel::place` | **fail** — 1 of 5 (`the_same_rectangles_grouped_into_different_rows…`) | **all green**: 176 lib + 122 crate integration + **1779 engine** (23 targets, incl. `comic_acceptance` 15, `comic_commands_r1/r2/r3`, `comic_golden_render`) |
| **M-AGREE** | a validator that requires the declared order to agree with the page's geometric order, placed after the artboard skip so it fires only where geometry is really checked | `validate::page` | **fail** — 4 of 5 (both files) | **all green**: 176 lib + 86 crate integration + **1779 engine** |
| **M-NOPOS** | `check` verifies the permutation but not that each panel's own index agrees with its position | `reading_order::check` | **fail** — 1 of 5 (`an_invalid_order_is_reported_although_the_geometry_reads_perfectly`) | **not** all green: 2 lib tests and 2 `round1_reviewer` tests already catch it |

**M-COORD is the decisive one, and it is the M-CLAMP result.** A coordinate-derived reading order passes
the crate's 176 unit tests, every other integration target in the crate (122 tests, including the M1
reviewer's three files and `panel_rect_contract`), and the **entire engine crate** — 1779 tests, all 23
targets, including the 15-test comic acceptance suite and the new golden-render tests. Exactly one test
in the workspace rejects it, and it is the new one. Its failure text:

```
thread 'the_same_rectangles_grouped_into_different_rows_get_different_orders' panicked at
crates/comic-layout/tests/m2_reading_order_independence.rs:176:5:
assertion `left == right` failed: two rows of two reverse one row each
  left: [3, 2, 1, 0]
 right: [1, 0, 3, 2]
```

**M-AGREE is the other half of the criterion**, and it is invisible to every pre-W5 test in the crate
and the engine; it is caught by both of my files (the cycle's step-2 "a legal reorder reports nothing",
and three of the four criterion-4 tests). Its first catch:

```
thread 'applying_a_template_then_reordering_then_resizing_warns_only_where_a_rule_is_broken'
panicked at crates/comic-layout/tests/m2_panel_cycle.rs:141:5:
assertion `left == right` failed: a legal reorder of an unchanged page reports nothing
  left: [Finding { rule: Some(6), severity: Error, subject: "Page(8).reading_order",
          message: "the panels read [9, 10, 11, 12] on the page but the order says [11, 12, 9, 10]" }]
 right: []
```

Two earlier, discarded drafts of M-AGREE are worth recording because they were **caught for the wrong
reason** and I would not have noticed without running the whole crate: the first was placed before the
artboard skip, so it fired on pages whose geometry is deliberately not checked, and two M1 tests about
*unusable artboards* (`round4_layout_engineer::a_non_finite_artboard_is_skipped_the_way_a_degenerate_one_is`,
`round4_reviewer::every_unusable_artboard_gets_one_skip_not_one_silence_per_cause`) reddened; the second
compared `geometric != declared` when some panel had no frame, so a page with a dangling frame link
reddened the same test. A mutant that is caught by an unrelated M1 test is not evidence that the new
test does anything, so both were fixed and the final M-AGREE is caught **only** by the new tests.

**M-NOPOS is recorded as the honest negative.** It is a real mutation of the converse half, and it shows
that the converse half was *already* covered by M1 — 2 lib tests (`reading_order::tests` and
`validate::tests`) and 2 of the round-1 reviewer's tests catch it — so my converse test is
**redundant** evidence rather than unique evidence. It still does its job (it fails under the mutant),
and it does it at the `validate::page` level with frames present, which is where the "geometry must not
paper over an invalid order" claim lives; but a reviewer should read it as corroboration, not as the
only thing standing between the crate and M-NOPOS.

---

## 4. Findings

### M2-L1 — a `defect` for W3, **reported and not fixed** (`crates/engine/src/cmd/comic/{panel,page}.rs`)

The command layer labels `comic-layout`'s geometry faults `Panel[i]`, where `i` is the **index into the
page file's panel vector** among the panels that have frames — not the panel's id:

* `crates/engine/src/cmd/comic/panel.rs:218-222` — `geometry_findings`, used by `comic.panel.add`
  (`:282`) and `comic.panel.resize` (`:367`), labels `.describe(&|i| format!("Panel[{i}]"))`.
* `crates/engine/src/cmd/comic/page.rs:573-578` — `comic.page.template.apply` labels the same way, over
  the same `filter_map`ped vector.

The same faults on the same page are labelled `Panel(<id>)` by this crate's `validate::page`
(`crates/comic-layout/src/validate/mod.rs:403-409`), and `validate::page`'s own doc claims that subjects are
"this crate's own, so nothing else produces them" and that the dedupe key for a caller running both
validators is the pair `(rule, subject)`. Two consequences, both of which the criterion's own wording
("resized **with validation warnings**") makes matter:

1. A `comic.panel.resize` response is `{panel, frame, rect, findings}` — so a warning reading
   *"sits 2pt from `Panel[1]`, closer than the 6pt minimum gutter"* **cannot be resolved to a panel id
   from the response alone**. The caller knows the panel it resized and not the index→id mapping.
2. A caller that dedupes `(rule, subject)` between `comic.project.validate` (`Panel(9)`) and a command
   response (`Panel[0]`) sees one fault **twice**, which is the M1 F3 double-report by another route.

Verified by reading both call sites; **not** driven end to end (the engine is another owner's tree and
`crates/*/tests/**` is the verification-engineer's, so I added no test there). The fix is presumably one
expression — label by `Panel.id` — but it is W3's call, and a UI-facing convention is worth deciding
rather than patching: `Panel[1]` is also unstable across a `comic.panel.delete`, which renumbers the
vector.

### Notes (no action asked for)

* **A file in my tree was edited by another agent, correctly.** `crates/comic-layout/src/reading_order/mod.rs`
  gained `locked: false` plus a three-line comment in its test-only `Panel` literal — W1's schema v2 field,
  which the lib test literal needs to compile. Left exactly as it is; recorded here because the file is
  mine and the change is not. Its hash is in the `Revision:` line.
* **Contention observed, no action needed.** During one measurement window another agent had a probe
  applied in `crates/comic-layout/src/template/mod.rs` (`let gutter = DEFAULT_GUTTER; // WRONG
  IMPLEMENTATION (M-GUTTER)`), which reddened six lib tests for reasons unrelated to my mutants. I
  waited, re-measured, and the file is back to `f8807972…` (its M1-approved revision) — confirmed by
  hash. Similarly, two of the lettering-engineer's test files (`w4_kind_policy_matrix.rs`,
  `w4_font_contract.rs`) were mid-edit and did not compile for part of the session; they are green in the
  final run. Neither is reported as a failure, per the kickoff rule.
* **`validate::plan` labels geometry findings by *placement* order, not reading order** — documented in
  `crates/comic-layout/src/validate/mod.rs:315-318` and now pinned by criterion 1's step 3b assertion.
  Not a defect; recorded because it is the one place a creator could misread a warning after a reorder,
  and because criterion 1 is the criterion that pairs "reordered" with "warnings".

---

## 5. Commands, as run (raw)

```
$ sha256sum crates/comic-layout/tests/m2_panel_cycle.rs \
            crates/comic-layout/tests/m2_reading_order_independence.rs \
            crates/comic-layout/src/{panel/mod.rs,validate/mod.rs,reading_order/mod.rs,template/mod.rs}
f1baba87de564c4cac7ec6221b8ddf251864df5dfbf8f7b4232991383fb6b03b  crates/comic-layout/tests/m2_panel_cycle.rs
47909f9fe6cd9b0eb816ca9171f8b619b0c11f6d8713c1f5c4b4cf96079c48f5  crates/comic-layout/tests/m2_reading_order_independence.rs
097be6d3a84b61456f621d3c4db6cbece1cbcbc5e5f0140ce0ec2c746cc2d2d4  crates/comic-layout/src/panel/mod.rs
8a71feaea125e94e76cd34e7d90805512e1a097fd2df43ac311c3b9261124f7c  crates/comic-layout/src/validate/mod.rs
c599b0b6267b3ae5b5cd6cae64a34f78f88b70a9650b1f47c7171df7d20d802a  crates/comic-layout/src/reading_order/mod.rs
f8807972ea7f6e28944212c4289646021cf980a8b82704be675c73d6973e3cea  crates/comic-layout/src/template/mod.rs

$ cargo fmt --all --check
exit=0

$ env -u CRAFT_FONTS_DIR cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings
    Checking vectorcraft-comic-model v0.8.0 (/home/hvt/web/comiclab.io.vn/comiccraft/crates/comic-model)
    Checking vectorcraft-comic-layout v0.8.0 (/home/hvt/web/comiclab.io.vn/comiccraft/crates/comic-layout)
    Finished `dev` profile [optimized + debuginfo] target(s) in 2m 35s
exit=0

$ cargo xtask layers
OK: 29 crates, no layering violations.
exit=0

$ cargo xtask brands
brands: no vendor names in user-visible text
exit=0

$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 176 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/lettering_round1_reviewer.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/m2_panel_cycle.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/m2_reading_order_independence.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round1_reviewer.rs
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round2_reviewer.rs
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round3_reviewer.rs
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round4_layout_engineer.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round4_reviewer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round5_layout_engineer.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/w4_balloon_move_tail.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/w4_font_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/w4_kind_policy_matrix.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**315 tests, 0 failed** in this crate at the run pasted above (M1 left 289; W5 adds 5, and W4's count
was 21 at that moment). The three `w4_*` files and `lettering_round1_reviewer.rs` are the
lettering-engineer's, and are included only because `cargo test -p` runs the whole crate — their count
is still moving as W4 lands (a later run of the same command showed 314, `w4_font_contract` 8 → 7). The
numbers that belong to W5 are stable: **all 5 new tests pass, and every pre-W5 test in the crate
passes**.

### The mutants, as measured

```
# M-COORD — panel/mod.rs, place() reads the order off the rectangles
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast --lib \
      --test m2_reading_order_independence --test m2_panel_cycle
test result: ok. 176 passed; 0 failed; ...
test result: ok. 1 passed; 0 failed; ...          # m2_panel_cycle — still green
test result: FAILED. 3 passed; 1 failed; ...      # m2_reading_order_independence
  the_same_rectangles_grouped_into_different_rows_get_different_orders ... FAILED
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast \
      --test layer_rules_contract --test lettering_round1_reviewer --test panel_rect_contract \
      --test round1_reviewer --test round2_reviewer --test round3_reviewer \
      --test round4_layout_engineer --test round4_reviewer --test round5_layout_engineer \
      --test w4_balloon_move_tail
test result: ok. 8 / 31 / 17 / 27 / 12 / 12 / 4 / 5 / 1 / 5 passed; 0 failed      # 122, all green
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-engine
23 targets, 1779 tests, 0 failed

# M-AGREE — validate/mod.rs, the declared order must agree with the frames' geometric order
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast --lib \
      --test m2_reading_order_independence --test m2_panel_cycle \
      --test round1_reviewer --test round2_reviewer --test round3_reviewer \
      --test round4_layout_engineer --test round4_reviewer --test round5_layout_engineer \
      --test panel_rect_contract --test layer_rules_contract
test result: ok. 176 passed; 0 failed; ...        # lib
test result: FAILED. 0 passed; 1 failed; ...      # m2_panel_cycle
test result: FAILED. 1 passed; 3 failed; ...      # m2_reading_order_independence
test result: ok. 8 / 17 / 27 / 12 / 12 / 4 / 5 / 1 passed; 0 failed    # 86, all green
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-engine --no-fail-fast
23 targets, 1779 tests, 0 failed

# M-NOPOS — reading_order/mod.rs, the permutation is checked, each panel's index is not
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast --lib \
      --test m2_reading_order_independence --test m2_panel_cycle --test round1_reviewer
test result: FAILED. 174 passed; 2 failed; ...    # lib: reading_order::tests, validate::tests
test result: ok. 1 passed; 0 failed; ...          # m2_panel_cycle — still green
test result: FAILED. 3 passed; 1 failed; ...      # m2_reading_order_independence
test result: FAILED. 25 passed; 2 failed; ...     # round1_reviewer — M1 already catches it
```

---

## 6. Unverified

* **M2-L1 was read, not driven.** I verified the two labelling call sites and the crate's own label by
  reading the sources, and I did **not** run a command end to end to capture a `Panel[1]` in a response —
  the engine is W3's tree and `crates/*/tests/**` is the verification-engineer's, so I added nothing
  there. A reviewer who wants the observed response can get it from `comic.panel.resize` on a two-panel
  page.
* **Clippy and the test suite were run for `-p vectorcraft-comic-layout` only** (all targets), plus
  `vectorcraft-engine`'s tests for the mutant measurement. I did not run `cargo xtask ci`, the workspace
  clippy, or the other crates' suites: they are other owners' surfaces and three agents are writing in
  this tree concurrently.
* **`CRAFT_FONTS` empty is irrelevant to these two criteria and was left empty** (`env -u CRAFT_FONTS_DIR`
  on every run). Neither criterion touches lettering; no test here reads a font, so nothing in this file
  changes W4's contract either way.
* **Windows untested**, as every M0–M2 record says. Nothing here is platform-dependent — no paths, no
  wall-clock time, no randomness — but that is an argument, not a measurement.
* **`locked` (W1's schema v2) has no effect on these criteria** and is not exercised by these tests. The
  one place it touches my tree is the test-only `Panel` literal in `reading_order/mod.rs`, which another
  agent updated; the criterion-4 files build their page records from JSON, so they would keep working if
  `locked` gained a serde default or changed its wire name.

Revision:      `crates/comic-layout/tests/m2_panel_cycle.rs` **f1baba87de564c4cac7ec6221b8ddf251864df5dfbf8f7b4232991383fb6b03b**;
               `crates/comic-layout/tests/m2_reading_order_independence.rs` **47909f9fe6cd9b0eb816ca9171f8b619b0c11f6d8713c1f5c4b4cf96079c48f5**.
               **No source file was changed by W5.** Verified byte-identical to the revisions the mutants were
               applied against and reverted from: `crates/comic-layout/src/panel/mod.rs`
               **097be6d3a84b61456f621d3c4db6cbece1cbcbc5e5f0140ce0ec2c746cc2d2d4**,
               `crates/comic-layout/src/validate/mod.rs`
               **8a71feaea125e94e76cd34e7d90805512e1a097fd2df43ac311c3b9261124f7c** (the M1 round-5 revision),
               `crates/comic-layout/src/template/mod.rs`
               **f8807972ea7f6e28944212c4289646021cf980a8b82704be675c73d6973e3cea** (the M1 round-4 revision),
               `crates/comic-layout/src/reading_order/mod.rs`
               **c599b0b6267b3ae5b5cd6cae64a34f78f88b70a9650b1f47c7171df7d20d802a** (M1's file plus W1's one
               `locked: false` line in its test literal, added by another agent — §4).
