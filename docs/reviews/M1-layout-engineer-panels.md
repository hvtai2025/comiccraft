# M1 — code-reviewer round 1, `crates/comic-layout` (W2: layout, panel, template, reading_order, validate)

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. The environment states
               `COPILOT_MODEL=deepseek-v4.1-flash`, and this session's own event log
               (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`) carries
               `"model":"deepseek-v4.1-flash"` on **7319** records. No waiver needed.
Revision:      working tree (base `16594c58`)   Files:
               `crates/comic-layout/src/{layout/mod.rs,layout/error.rs,panel/mod.rs,template/mod.rs,template/error.rs,reading_order/mod.rs,validate/mod.rs,validate/fault.rs}`,
               `crates/comic-layout/tests/round1_reviewer.rs` (mine),
               `crates/comic-layout/Cargo.toml`, the five `pub mod` lines added to
               `crates/comic-layout/src/lib.rs`
Design refs:   design §5.1.1 (a page is an artboard), §6.1 (ownership: page size is the artboard's,
               panel geometry is the document's), §8.1 (page layout: inputs, outputs, "the plan names
               a rectangle per panel … the layout engine never keeps a second copy"), §8.2, §11.2
               (one undoable operation), §12.1 (the check→owner table, including "Unsupported page
               dimensions | `comic-layout` | … as a warning (schema rule 15)"); schema §5.2
               (`reading_direction`), §5.3 (`ProjectSettings`, with **no** margin or gutter field),
               §6.1 `Page`, §6.2 `Panel`/`ArtworkRef`, §13 rules 4–6 and 13–15
Toolchain:     `cargo 1.99.0` / `rustc 1.99.0`
               (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`), as the M0/M1 records
               note: the system `cargo` is 1.75 and cannot build this workspace (edition 2024).

## Round 1 — correctness

Verdict:       **changes-requested**

Unit tests:
- Added (new file): `crates/comic-layout/tests/round1_reviewer.rs` — **27 tests: 23 pass, 4 fail**.
  The four failures are one defect, F1 (`the_last_row_of_every_page_height_lands_exactly_on_the_content_box`,
  `a_correctly_planned_page_is_never_reported_against_itself`,
  `a_full_bleed_plan_is_not_an_error_against_the_page_it_fills`,
  `a_seven_row_page_is_not_reported_against_the_margin_it_was_laid_out_with`), plus a probe that
  records the `grid-+2x3` leniency found while writing them. The implementer's own 180 tests stay
  green: 155 lib + 8 `layer_rules_contract` + 17 `panel_rect_contract`.
- Commands, verbatim:
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/lettering_round1_reviewer.rs
test result: FAILED. 18 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round1_reviewer.rs
test result: FAILED. 23 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings
    Checking vectorcraft-comic-layout v0.8.0 (/home/hvt/web/comiclab.io.vn/comiccraft/crates/comic-layout)
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.53s

$ cargo xtask layers
OK: 29 crates, no layering violations.
```
  `tests/lettering_round1_reviewer.rs` also exists in this crate — it is the **W3** reviewer's file,
  not mine, and its 7 failures are not W2's. The implementer's claim of "180 green" checks out
  exactly: 155 + 8 + 17 = 180, and clippy is silent on all of them and on my file.
  `tests/panel_rect_contract.rs` is the round-1/2 record for the lead's `PanelRect`, not W2's.

Findings:      **5 — 1 bug, 4 preference.**

**F1 — `bug`** — `crates/comic-layout/src/template/mod.rs:238-258` (`rows`), measured against the
boundary at `crates/comic-layout/src/validate/mod.rs:123` and `:127` (`faults`).
Trigger: a page whose content height does not divide equally by the row count. The row arithmetic is
`row_height = (H - gutter*(R-1)) / R` and the last row's far edge is recomputed as
`content.y0 + (R-1)*(row_height + gutter) + row_height`, which in `f64` is **not** always
`content.y1`. Expected: `validate::plan` reports nothing for a plan the template itself produced,
and `rows` documents exactly that ("the last row's bottom edge is the content box's, so nothing
drifts by rounding"). Actual: the drift is ±1.14e-13 … ±4.55e-13, and because `faults` compares
`panel.bottom() > page.y1 - margin` (and `panel.right() > page.x1 - margin`) strictly, a *correct*
plan is reported against itself.
- Sweep, page heights 600.0 … 2000.0 step 0.1 × 7 row counts, `Limits::default()`:
  **12 629 of 70 005** heights put the last row's bottom edge off the content box;
  **6 313 of 70 005** correctly laid-out pages get a spurious `Warning` —
  `Page.panels[6] — crosses the safe margin; a panel that deliberately bleeds should be confirmed`;
  with the margin at `0.0` the same shape gives a spurious **`Error`** —
  `Page.panels[6] — is not inside the page` — on **6 367** pages.
- Minimal reproduction, no sweep needed: a 612×1000 pt page,
  `Grid { rows: 7, columns: 1 }`, `LayoutSettings::default()`, `Limits::default()` →
  `[Finding { rule: None, severity: Warning, subject: "Page.panels[6]", message: "crosses the safe
  margin; a panel that deliberately bleeds should be confirmed" }]`.
- Why it matters at M1: `comic.project.validate` returns these, and the design (§8.1) says the
  deterministic validator runs "before anything is written" — a caller that refuses to apply a plan
  carrying errors refuses a correct page. It also makes
  `validate::tests::a_planned_three_panel_page_reports_nothing` a statement about 756/3 = 252 rather
  than about the templates.
- The owner's call on the fix (make the far edge the content box's, or compare within one ULP), but
  the *behaviour* the design and the crate's own test assert is: a plan the template produced
  reports nothing.

**F2 — `preference`** — `crates/comic-layout/src/validate/mod.rs:1-20` / decision 1 of the
implementer's two.
Design §12.1's table gives "Unsupported page dimensions" (schema rule 15) to **`comic-layout`**;
this crate does not implement it, on the grounds that `comic-model`'s
`rule_15_page_size_against_the_default` reports it from the same two facts. Judged: **the decision is
the right one and should stand** — one defect, one finding, and §13 has `comic.project.validate`
report each rule once — but it leaves the frozen design table stale, which is a fact about the
document, not about this code. The crate's module docs say so plainly. The lead should either record
the amendment against design §12.1 or note the deviation in the M1 milestone; nothing further is
needed in the code.

**F3 — `preference`** — `crates/comic-layout/src/reading_order/mod.rs:64-68` and
`crates/comic-layout/src/validate/mod.rs:198-206` / decision 2.
The dedupe claim is **true as written** — verified against the real fixture bundle: every
`(rule, subject)` pair this crate emits for rules 6 and 7 is also emitted by `comic-model` under the
same subject. But the claim holds only for that key: the *messages* differ on purpose (this crate
appends `; every panel must appear exactly once` to the permutation message and
`, so the panel has no geometry to check` to the rule-7 message), so a caller that dedupes whole
`Finding`s, or `(rule, subject, message)`, double-reports. Nothing in the crate exports the key or a
dedupe helper. Expected: one sentence saying the messages are deliberately different and the key is
exactly `(rule, subject)`; a mismatch between the two crates' messages would otherwise read as a bug.

**F4 — `preference`** — `crates/comic-layout/src/reading_order/mod.rs:47-52`.
`narrative_indices` documents that the inverse is "computed rather than assumed, so a future template
that is not an involution cannot silently give a panel the wrong index". Every shape this crate can
build is a per-row reversal, which is its own inverse, so `narrative_indices` and `narrative_order`
are the same function for **every** input they can be given. Mutating the inverse into the identity
(M5) survives both suites — the generality is unreachable and unpinnable by construction, i.e. the
comment describes a defence the code can never exercise. Either say so, or drop the second function
and its `page.panels[].reading_order_index` derivation to the one that is tested.

**F5 — `preference`** — `crates/comic-layout/src/validate/mod.rs:194`.
`plan()` passes `Some(plan.margin())` as the safe margin, but a template lays every panel inside the
content box, so the `InSafeMargin` branch cannot fire for a plan — except through F1's drift. No test
in either suite can tell `Some(plan.margin())` from `None` (mutation M12 survives both). Once F1 is
fixed the argument is inert; `Fault::InSafeMargin` would then be reachable only through the public
`faults()`. Decide which it is and say so, rather than leaving a parameter that reads as a check.

### What I checked and found *true* (so it is on the record)

- **The contract holds.** No `&mut Document` anywhere in the five trees: `validate::page` takes
  `&PageFile`, `&DocumentFacts`, `&PanelFrames` and `&Limits`, and `reading_order::check` takes
  `&Page` and `&[Panel]`. `PagePlan` is plain data (`Send + Sync + Clone + Debug + PartialEq +
  'static`, asserted), carrying `Rect`, `Vec<PlannedPanel>`, `LayoutSettings`, `Vec<u16>` and
  `ReadingDirection` — no handle, no interior mutability, no borrow.
- **No geometry reaches a comic record.** Reading every W2 file above its `#[cfg(test)]` line: no
  `Page`, `Panel`, `PageFile`, `Dialogue` or `Lettering` literal is constructed, no field is
  assigned, and `PanelRect` appears only as a value the plan returns. `PanelRect` has no
  `Deserialize` and is a field of no comic entity. `validate::page`'s findings are the only thing it
  returns.
- **`ProjectSettings` really has no margin or gutter.** Schema §5.3 is
  `default_page_size`, `default_template`, `default_lettering_style`, `unknown` — so
  `DEFAULT_MARGIN`/`DEFAULT_GUTTER` and `Limits` being crate policy is correct, and so is
  `page()`'s refusal to check a safe margin a loaded page does not have.
- **The templates' arithmetic, by hand.** US Letter (612×792), default 18/12: content
  (18, 18)–(594, 774) = 576×756. `three-tier` → 244 tall, tops 18 / 274 / 530. `wide-top` → 372 tall,
  282 wide, second row at x 18 and 312. `grid 2x3` → the same cells. A single panel fills the content
  box exactly. A zero-margin, zero-gutter `wide-top` gives 612×396, 306×396, 306×396 and the second
  row's pair is *touching*, not overlapping. All confirmed independently of the implementer's tests.
- **The `PanelRect` invariant.** Every panel every template places is finite, strictly positive, and
  advances both far edges, for page sizes 0.5×100 … 1000×1000 and margins 0.5 … 18 — the invariant
  `PanelRect::new` enforces is not defeated by the layout arithmetic, and the layout cannot overflow
  it into a panic (`PanelRect(NotRepresentable)` is reported as `TemplateError::Rect`).
- **Inverted and `NaN` pages fail cleanly.** `NaN`, `+∞`, `-∞`, zero and negative width/height, and
  both axial inversions (`Rect::new(0, 792, 612, 0)`, `Rect::new(612, 0, 0, 792)`) are refused as
  `LayoutError::Artboard` before anything is computed; a merely small page fails as
  `NoContentArea`, not as a malformed rectangle; a `NaN` `PageSize` (`PageInput::NewPage`) is refused
  the same way. No panic path.
- **The panel-count contract.** Every template, requested 0 / count−1 / count+1 / count+7 /
  `u32::MAX`: `PanelCountMismatch { template, requested, template_count }` with all three fields
  right. Never padded, never truncated.
- **Untrusted input.** See the probes below: refused or normalized, never guessed, never allocated
  for.
- **Reading order is produced from the shape and the direction, not from coordinates.** No module
  under W2 reads a rectangle to decide an order (grep + the `check` bodies). `rtl` is a per-row
  reversal; `wide-top` gives `[0, 2, 1]`; a 2×3 grid gives `[2, 1, 0, 5, 4, 3]`; a tiers page is
  `[0, 1, 2]` either way; `narrative_indices` inverts `narrative_order` exactly for every shape and
  both directions.
- **One defect, one finding.** Overlap beats a negative gutter for the same pair; a panel off the
  page is not *also* a margin finding; a non-permutation order is reported once (the early return),
  not once per panel at two positions; a diagonal pair has no gap; three panels in a column give two
  findings, not three.
- **`Cargo.toml`.** The five dependencies added are the four the four module trees use
  (`comic-model`, `doc`, `geom`, `thiserror`) plus `text` for the lettering tree; `cargo xtask layers`
  is clean and `layer_rules_contract`'s
  `each_comic_manifest_declares_only_what_its_code_uses` passes, so no declared-but-unused edge was
  introduced.

Tests that cannot fail:
- `crates/comic-layout/src/template/mod.rs:314` `every_template_shape_is_well_formed` — its
  `assert_eq!(shape.panel_count(), shape.panels_per_row().iter().sum::<u32>())` restates
  `Shape::panel_count`'s body. The `>= 1` assertions beside it are real; that one cannot fail.
- `crates/comic-layout/src/template/mod.rs:441` `laying_a_shape_twice_gives_the_same_rectangles` and
  `crates/comic-layout/src/layout/mod.rs:266` `planning_the_same_page_twice_gives_the_same_plan` —
  determinism of a pure function with no `rand`/`SystemTime` in the tree. Weak, not vacuous (a global
  counter would be caught), but they would pass on a `rows` that returned a constant.
- `crates/comic-layout/src/validate/fault.rs:132`
  `severity_is_split_by_what_breaks_not_by_what_is_suspicious` restates the `match` it tests: it
  cannot fail unless an arm is deleted, at which point the crate does not build.
- `crates/comic-layout/src/validate/fault.rs:110`
  `every_fault_is_phrased_for_a_creator_with_the_panels_it_names` pins only `Overlap` and
  `GutterTooThin`; the `TooSmall`, `OutsidePage` and `InSafeMargin` messages are unpinned and could
  be swapped or emptied with the suite green. My
  `the_two_fault_messages_the_implementation_never_pinned_say_what_is_wrong` pins them.
- Unpinnable rather than vacuous: `reading_order/mod.rs:175`'s
  `assert_eq!(indices[slot as usize], position as u16, "the inverse must agree")` half of
  `the_order_is_a_permutation_of_the_placement_slots` cannot distinguish the inverse from the order
  itself (mutation M5 survives both suites), because no input exists where the two differ.
- Blind spots their suite does not cover, which my file now does (mutations M9, M13 in the table):
  a panel exactly flush with the page edge is *inside* it; a present-but-degenerate artboard is
  skipped rather than reported; a page with a missing artboard is left to rule 4 (that one *is*
  covered by them).

Mutation table:  I mutated production code in a scratch driver, ran **the implementer's own suite**
(`--lib --test layer_rules_contract --test panel_rect_contract`, i.e. their 180) and **mine**
(`--test round1_reviewer`, with F1's four failures skipped so a row means "my other 23 tests"), then
restored each file and re-verified the restored text by grep. Two rows were retried because another
agent was mid-edit in `crates/comic-model` and the build broke in a file that is not W2's; those
retries are the results below, and every remaining row built from W2's sources.

| # | mutation | their suite | mine |
|---|---|---|---|
| M1 | `rows`: the panel gutter term uses `panels`, not `panels - 1` | fail | fail |
| M2 | `rows`: the row gutter term uses `row_count`, not `row_count - 1` | fail | fail |
| M3 | `narrative_order`: `rtl` mirrors the whole page instead of each row | fail | fail |
| M4 | `narrative_order`: the direction is ignored | fail | fail |
| M5 | `narrative_indices`: returns the order instead of its inverse | **pass** | **pass** |
| M6 | `grid` bound: `>=` where it is `>` | fail | fail |
| M7 | `from_name`: `_` is no longer folded to `-` | fail | fail |
| M8 | `from_name`: an unparseable grid silently becomes `three-tier` | fail | fail |
| M9 | `faults`: `>=` on the page edges, so a flush panel is outside | **pass** | fail |
| M10 | `faults`: the margin is reported for an off-page panel too | fail | pass |
| M11 | `check`: the permutation test compares lengths, not the sets | fail | pass |
| M12 | `plan`: the safe margin is `None` (the parameter is inert) | **pass** | **pass** |
| M13 | `page`: the degenerate-artboard skip is removed | **pass** | fail |
| M14 | `page`: a dangling frame is rule 8, not rule 7 | fail | fail |
| M15 | `Fault::severity`: errors and warnings swapped | fail | fail |
| M16 | `checked_rectangle`: the page rectangle is never validated | fail | fail |

Geometry:  checked by hand, independently of the implementer's tests —
`content_box` on US Letter (18, 18, 594, 774 / 576×756), `three-tier` (244 tall, tops 18/274/530),
`wide-top` (576×372 over 282×372 at x 18 and 312), `grid 2x3` (same cells), a single panel filling
the content box exactly, a zero-margin/zero-gutter `wide-top` (touching, not overlapping), a page
offset to (100, 50) so the panels follow the origin, and a 595.276×841.89 (A4) page. All correct.
The one arithmetic the implementation gets wrong is at the **edge**: the last row/column's far edge
is `content.y1 ± 1e-13` rather than `content.y1`, which is F1.

Untrusted input:  `from_name` probed with, and the result —

| name | result |
|---|---|
| `grid-0x0`, `grid-0x2`, `grid-2x0` | `UnusableGrid { rows, columns, max: 64 }` |
| `grid-64x64` | `Ok(Grid { 64, 64 })`, 64 rows of 64, 4096 panels — bounded |
| `grid-1x99999999`, `grid-65x1`, `grid-1x65`, `grid-4294967295x4294967295` | `UnusableGrid`, and `Grid { 1, 99999999 }.shape()` is `Err` too, so the bound is checked **before** `vec![columns; rows]` |
| `""`, `" "`, `"\t"`, `three`, `three-tiers`, `tiers-3`, `grid`, `grid-`, `grid-2`, `grid-2x`, `grid-2x3x4`, `grid-twoxthree`, `grid- 2x3`, `grid-2.0x3`, `grid--2x3` | `UnknownTemplate { name }` with the name as written |
| `"  three-tier  "`, `"  three-tier\t"`, `THREE_TIER`, `Three-Tier`, `three_tier` | `Ok(ThreeTier)` |
| `grid_2x3`, `GRID-2X3`, `"grid-2x3 "` | `Ok(Grid { 2, 3 })` |
| `grid-+2x3` | `Ok(Grid { 2, 3 })` — Rust's `u32::from_str` accepts a leading `+`. Harmless leniency, recorded |
| `"three-tier\0"`, `"three-tier\u{200b}"`, `"thr\u{435}\u{435}-tier"` (Cyrillic), `"three\u{2010}tier"` | `UnknownTemplate` — `to_ascii_lowercase` does not fold non-ASCII, and the last two are the names a foreign tool would spell with a Unicode hyphen |
| `"  Thré-Tier  "` | `UnknownTemplate { name: "  Thré-Tier  " }` — the untouched original, so a creator can find the line |

No panic, no unbounded allocation, no silent wrong template: a name that does not resolve is
reported, and `grid-64x64` is a *page too small to hold it* (`NoRoomDown`), not a parse failure.
Note the two refusal kinds are distinguishable (`UnknownTemplate` vs `UnusableGrid`), which is what
a creator needs.

Reading order:  **`rtl` reverses each row, and that is the correct reading of a right-to-left comic
page.** A manga page is read top-to-bottom and right-to-left *within a tier*, so a global mirror
(reading the bottom-right panel first) is wrong; a page of full-width tiers is read `[0, 1, 2]`
either way, and `wide-top` is `[0, 2, 1]` — the top panel, then the bottom-right, then the
bottom-left. Verified: `narrative_order(&[1,2], Rtl) == [0,2,1]`, `&[3] → [2,1,0]`, `&[2,3] → [1,0,4,3,2]`,
`&[3,2] → [2,1,0,4,3]`, and the plan for `wide-top`/`rtl` gives `panels[].reading_order_index ==
[0, 2, 1]` with the rectangles unmoved. `narrative_indices` is the exact inverse for every shape and
both directions (`indices[order[i]] == i`), and for these shapes it equals the order itself — see F4.
Nothing in the module reads a coordinate.

Deliberate decisions:
- **Rule 15 not re-implemented — judged correct, with a documentation consequence.** Design §12.1
  assigns "Unsupported page dimensions" to `comic-layout`, so this *is* a deviation from the frozen
  table; but the check runs once, in the crate that owns `ProjectSettings.default_page_size` and the
  artboard facts, and §13 requires `comic.project.validate` to report each rule once. Keeping it
  would double-report. Accepted as the right call; the design table now reads stale and the lead
  should record the amendment (F2).
- **`validate::page` re-reports rules 6/7 — judged correct, and the dedupe claim is true.** Design
  §12.1 does assign "Invalid reading order" to this crate, and the geometry check is only half of
  what a caller wants from a plan/page check. I verified the claim against the real fixture rather
  than trusting it: with the fixture's page 8 mutated three ways (a disagreeing
  `reading_order_index`, a non-permutation `reading_order`, a dangling `frame_node_id`), **every**
  `(rule, subject)` pair this crate reports is also reported by `comic_model::validate` under the
  same subject — `Page(8).reading_order`, `Panel(9).reading_order_index`,
  `Panel(9).frame_node_id` — and the clean fixture produces nothing from either crate.** The claim
  survives the strictest reading, with two caveats on the record: the messages differ deliberately
  (F3), and on a *non-permutation* order the two crates report different **counts** — this crate's
  early return gives one finding, `comic-model` continues its loop and can add a
  `Panel(…).reading_order_index` finding for a panel that is at two positions at once. That is this
  crate being the better of the two (one defect, one finding), and the dedupe still works because
  the superset is on `comic-model`'s side; a caller must not compare counts.

Contract:  **clean.** No `&mut Document` — the trees take only `&` references and plain data
(`PageInput::Artboard(Rect)` / `NewPage(PageSize)`), so "compute and return a plan" holds by
signature; `PagePlan` is plain data (`Send + Sync + Clone + Debug + PartialEq + 'static`, asserted).
No panel geometry in a comic record: no comic record is constructed or assigned anywhere above a
`#[cfg(test)]` line in the five trees, `PanelRect` is a field of no comic entity and has no
`Deserialize`, and the plan's rectangles are returned to the caller, never stored. The one thing the
crate reads from a loaded record is `Panel.reading_order_index`, `Panel.frame_node_id`,
`Page.panel_ids` and `Page.reading_order`, all by reference.

Unverified:
- `cargo xtask ci` end to end (the gate the record template asks for) was **not** run; only the three
  commands this review was given were. `docs/ci-*` is another agent's territory and the workspace
  build spans crates with agents mid-edit in them.
- Anything below the command layer: no engine/command/MCP wiring exists for a plan yet at M1, so
  "the command creates one frame object per panel and writes `Panel.frame_node_id`" (design §8.1) is
  a claim about future code, not about this revision. W2 cannot put geometry in a record because
  nothing here writes a record at all.
- `crates/comic-layout/src/{lettering,fit}/**` and `crates/comic-layout/tests/lettering_round1_reviewer.rs`
  — W3's, reviewed separately; `tests/lettering_round1_reviewer.rs` currently has 7 failures and they
  are not mine. `src/lib.rs`'s `PanelRect` and the five `pub mod` lines are the lead's; I read
  `PanelRect` (it is the one invariant the templates must not defeat) but did not review it as a
  change, and I did not mutate `lib.rs`.
- Whether the F1 drift has a *visible* effect today: no caller refuses on a warning yet, so the
  impact is a false positive in `comic.project.validate`'s output and a falsified invariant, not a
  refused page. It becomes a refused page the moment a command treats a `Severity::Error` finding as
  a reason not to apply.

---

## Round 2 — rules, failure paths, edges

Reviewer:      code-reviewer on `deepseek-v4.1-flash` (`COPILOT_MODEL=deepseek-v4.1-flash`; every
               `"model"` record in this session's own event log says the same). No waiver.
Revision reviewed: **working tree (base `16594c58`) after the remainder + edge-to-edge fix** —
               distinguishable from round 1's `working tree (base 16594c58)` by content:
               `src/template/mod.rs` sha256 `4b14fb44c0009f026f208ba55adb234ae2ae2d2f2707408d8f949a3311bcff94`,
               `src/validate/mod.rs` sha256 `51bd82ee131f88d686189c882d2155468d2fc47f546c78704b93d692518dc875`,
               `src/panel/mod.rs` sha256 `097be6d3a84b61456f621d3c4db6cbece1cbcbc5e5f0140ce0ec2c746cc2d2d4`,
               `src/reading_order/mod.rs` sha256 `0097b0217dad7266a21eb5ab455967774c470c812eee7cc147df2dc22c6934ac`,
               `src/layout/mod.rs` sha256 `aa3cd638c1f697952e3bf55b63b13fc1c831cc1535e85adac61739b90821561f`,
               `src/lib.rs` sha256 `fd5491e03d03ed5568f3c9f3ce90ff02b8de89af645262fa92a84c5c01ecd8af`.
               Files: the eight W2 sources above, `crates/comic-layout/tests/round2_reviewer.rs` (mine,
               sha256 `4db4e7aeab6a8b325433949a5d273a054153701cef1ab3bd7cd958829a4f1103`), round 1's
               `tests/round1_reviewer.rs` (unchanged), `docs/comiccraft-design.md` §12.1 (read only).

Verdict:       **changes-requested** — 5 findings backed by 3 failing tests; 2 bug, 3 preference.

Unit tests:    added `crates/comic-layout/tests/round2_reviewer.rs` — **12 tests: 9 pass, 3 fail**
               (the three failures are R1, R2, R3). Verbatim:

```
$ cargo fmt --all --check          → exit 0 (clean)
$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.91s
$ cargo xtask layers
OK: 29 crates, no layering violations.
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 165 passed; 0 failed
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed
     Running tests/lettering_round1_reviewer.rs
test result: FAILED. 26 passed; 1 failed          (W3's file, mid-edit; not W2's)
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed
     Running tests/round1_reviewer.rs
test result: ok. 27 passed; 0 failed              (round 1's file, unmodified, now green)
     Running tests/round2_reviewer.rs
test result: FAILED. 9 passed; 3 failed
    r1_the_remainder_lands_on_the_content_box_wherever_the_page_is
    r2_a_page_far_from_the_origin_is_not_reported_against_itself
    r3_the_room_guard_covers_the_quantity_the_last_row_actually_uses
```

Findings:      **5 — 2 bug, 3 preference.**

**R1 — `bug` — `crates/comic-layout/src/template/mod.rs:237-257` (the `rows` doc) against `:271`
(the remainder).**
Trigger: `Rect::new(0.1, 0.3, 50.6, 33.6)`, `LayoutSettings::new(3.3, 0.0)`, `grid-1x3` — a 50.5×33.3pt
page. The single row's bottom edge is `3.02999999999999972e1`; the content box's is
`3.02999999999999936e1` — **one ULP past it** (`+3.553e-15`). And
`Rect::new(-1000.5, -2000.25, -388.5, -1208.25)` (US Letter, negative origin), margin 3.3, gutter 0,
`two-tier`/`wide-top`: every row's **right** edge is `5.684e-14` past `content.x1`. Expected, in the
doc's words: "the far edges **are** the content box's … by construction: `last_top + (content.y1 -
last_top)` is `content.y1`", with an error "at most half an ULP of `content.y1`". Actual: `a + (b - a)
== b` is not an f64 identity — the remainder is `content.y1 - top` and `fl(top + fl(content.y1 - top))`
is not `content.y1` in general — so the far edge can land **one ULP outside** the content box, which is
round 1's F1 symptom at a smaller magnitude. 33 of the sweep's far edges are off. The drift is masked
by `GEOMETRY_TOLERANCE` (1e-9) at these magnitudes, so `validate::plan` is quiet and the *behaviour* is
fine here; what is broken is the claim and the test name that assert it. Reproduced by
`round2_reviewer.rs::r1_the_remainder_lands_on_the_content_box_wherever_the_page_is` (**fails**).

**R2 — `bug` — `crates/comic-layout/src/validate/mod.rs:137` (an *absolute* `GEOMETRY_TOLERANCE`)
with `:254` (`plan` judges the plan against its own margin).**
Trigger: `Rect::new(1e7, 1e7, 1e7 + 612.0, 1e7 + 792.0)`, `LayoutSettings::new(0.0, 3.3)`,
`three-tier`, `Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: 3.3 }` →
`validate::plan` returns two findings **on the plan the template just built**:
`GutterTooThin` — `"sits 3.299999998882413pt from Page.panels[1], closer than the 3.3pt minimum
gutter"`. Expected: `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect`'s own claim, and
round 1's F1 ("a plan the template produced reports nothing"), for **every legal page**. Actual: a
correct plan is reported once `ulp(coordinate) > 1e-9`, i.e. `|coordinate| ≳ 4.5e6`pt — the crate
bounds a panel's *size* (`PanelRect::new`) but never a page's *position*, so an absolute epsilon cannot
be "right" as the doc asserts. Swept ±1e6/1e7/1e8, four templates, margins 0/3.3/18, gutters 3.3/12:
**0** reported at 1e6, **24** at 1e7, **24** at 1e8. Only *warning*-level — no `OutsidePage` or
`InSafeMargin` appears at those magnitudes, so nothing would refuse a command — but it is F1's class,
not a note. Reproduced by
`round2_reviewer.rs::r2_a_page_far_from_the_origin_is_not_reported_against_itself` (**fails**).

**R3 — `preference` — `crates/comic-layout/src/template/mod.rs:260-262` (the room guard) against
`:271`/`:275` (the quantity actually used).**
Trigger: `Rect::new(0.0, 0.0, 100.0, 10.05)`, `LayoutSettings::new(0.0, 0.15952380952380796)`,
`grid-64x1` → `Err(TemplateError::Rect(NonPositive { field: "height" }))`; `grid-1x64` on
`Rect::new(0.0, 0.0, 10.05, 100.0)` → `NonPositive { field: "width" }`. Expected: the guard is the
"no room" predicate (`NoRoomDown`/`NoRoomAcross`, with rows/available/gutter), so a page it lets past
must lay out. Actual: the guard measures `nominal_row_height`, but the last row is built with the
remainder `content.y1 - top`, which the guard never sees and which a page just above
`(rows-1)*gutter` pushes to ≤ 0 — a path the fix introduced, untested, refusing with a *rectangle*
error that reads as a caller bug ("panel height must be greater than zero") rather than a page too
small. No bad rectangle and no panic, hence preference, not defect. Reproduced by
`round2_reviewer.rs::r3_the_room_guard_covers_the_quantity_the_last_row_actually_uses` (**fails**).

**R4 — `preference` — `crates/comic-layout/src/template/mod.rs:459-489`**
(`the_last_panel_of_every_row_lands_exactly_on_the_content_box`, the width-axis sweep added so that "a
height-only fix would have passed your sweep").
Trigger: mutation M-C — place the column lefts by `content.x0 + column*(nominal_panel_width + gutter)`
instead of edge-to-edge, keeping the remainder for the last column. Expected: the sweep fails.
Actual: **M-C survives the entire 165-test lib suite**, that sweep included; only my
`round2_reviewer` tests fail it. Every swept combination (origins 0 and (100, 50); widths 611.0,
612.3, 595.7, 1000.1; margins 0/3.3/18; gutters 0/3.3/12) happens to satisfy
`left + (content.x1 - left) == content.x1` even under the index form, so the guard cannot fail for the
mutation it was written for. (The sibling claim *is* now backed: M-A fails round 1's sweep, M-C does
not — "a height-only fix would have passed your sweep" is true.)

**R5 — `preference` — `crates/comic-layout/src/validate/mod.rs:108, 126-127` (the rewritten
`GEOMETRY_TOLERANCE` doc).**
Trigger: read against the arithmetic. Expected: a doc that is true. Actual: (a) "`1e-9`pt is four
orders of magnitude below the smallest mark a printer can place (~0.01pt)" — it is **seven**
(`1e-2 / 1e-9 = 1e7`); (b) "and five above the drift above" — the drift stated above is "a few
`1e-13`", so it is **four** above; (c) "It is a safety net for geometry this crate did not compute …
The tolerance is kept because a comparison against a threshold is still a comparison between two
roundings: a caller may hand `faults` frame rectangles read out of a document" — but mutation M-E
(`GEOMETRY_TOLERANCE = 0.0`) makes `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect`
**fail**, so the tolerance is load-bearing for the crate's **own** output too, not only for a caller's
rectangles (and R2 shows it is insufficient there). The purpose the doc names *is* reachable — pinned
by my `the_tolerance_is_reachable_from_geometry_the_crate_did_not_compute` and by
`a_gap_one_ulp_under_the_minimum_is_not_reported` — but the claim about *whose* geometry needs it is
not true. No failing test: it is a doc claim, evidenced by M-E and R1/R2.

### Round-1 verification

- **F1 — verified fixed** (its own reproductions), *with a residual that is R1/R2.* The 70 005-height
  sweep at the origin is 12 629 → 0; `612x1000pt` `grid-7` at margin 18 and at margin 0 reports
  nothing; all four of round 1's failing tests now pass **by name, unmodified** — and they *bite*:
  mutation M-A (restore the index-multiplied form) fails exactly the four of mine that F1 named plus
  the implementer's two new sweeps and its two validate tests. The residual is that the same symptom
  (a far edge one ULP *past* the content box) reproduces on legal pages away from the origin, which no
  sweep probes (R1), and is reported again once the coordinates outgrow the absolute tolerance (R2).
- **F2 — partially; the contradiction is now *inside* §12.1.** The decision still stands and the
  crate still implements no rule-15 check (grep: no rule-15 predicate anywhere under `src/`). The owner
  column *was* corrected by **adding** a row — `docs/comiccraft-design.md:1292` now says "Page size
  differs from the default | `comic-model` | … `comic-layout` deliberately does not repeat it — two
  findings for one fact is noise" — but the **stale row was left in place** at `:1300`:
  "Unsupported page dimensions | `comic-layout` | the artboard `rect` against
  `ProjectSettings.default_page_size`, as a warning (schema rule 15)". §12.1 therefore now assigns one
  predicate to two owners in adjacent rows and contradicts itself. That is the lead's document, not
  W2's code.
- **F3 — verified fixed.** The dedupe sentence exists in both places (`validate::page`'s doc and
  `reading_order::check`'s), and it is true: the key is exactly `(rule, subject)`, the messages differ
  on purpose, and round 1's caveat (a non-permutation order makes the two crates report different
  *counts*, never different *keys*) is compatible with "unique per defect in both validators".
- **F4 — verified fixed, and the new pin bites.** `narrative_indices` is gone (grep: one occurrence,
  in round 1's own test comment); the inverse is `panel::position_of` (private), used by `panel::place`
  and pinned by `a_slots_position_is_looked_up_in_the_order_not_read_off_it` with the non-involution
  order `[1, 2, 0]`. Mutation M-D (read the order off instead of looking the slot up) fails **that**
  test and nothing else — the mutation round 1 called M5 no longer survives.
- **F5 — verified fixed, and the branch is now pinned.** `plan()`'s margin branch is live for a
  hand-built `PagePlan` (design §8.1's optional fixed panel geometry): a panel inside the page but
  crossing the margin is reported as a `Warning`, and a panel exactly on the boundary is not (my
  `a_panel_exactly_on_the_safe_margin_is_inside_it`). Mutation M-F (`Some(plan.margin())` → `None`)
  fails the implementer's `a_plan_whose_panel_crosses_the_safe_margin_is_reported` **and** mine —
  round 1's M12 no longer survives.
- The four tests round 1 called unable to fail are gone: `assert_eq!(shape.panel_count(),
  sum(panels_per_row))` is out of `every_template_shape_is_well_formed`;
  `laying_a_shape_twice_gives_the_same_rectangles`, `planning_the_same_page_twice_gives_the_same_plan`
  and `severity_is_split_by_what_breaks_not_by_what_is_suspicious` no longer exist (grep).

### The arithmetic (the new code — per template, per axis)

**The remainder is right in *intent* and wrong as an *identity*.** `content.y1 - top` and
`content.x1 - left` make the last row/panel's far edge the boundary rather than `index*(nominal +
gutter)`, which is strictly better and removes the *class* at ordinary origins; but
`fl(a + fl(b - a)) == b` is not a theorem, so the far edge is `content.…1 ± ≤1 ULP`, not `content.…1`.
My probe swept 11 936 far edges (origin, size, margin, gutter, template): 196 are off, every one on a
page away from the origin. Per template:

- **Equal rows / uniform grids** (`two-tier`, `three-tier`, `four-tier`, `grid-2x3`, `grid-3x3`,
  `grid-7x1`, `grid-9x3`): exact at the origin and at (100, 50) for every combed margin/gutter — the
  implementer's claim is true *there*.
- **`wide-top` (unequal rows `[1, 2]`)**: the hand-check is right (576×372, then 282/282 at x 18 and
  312; last row bottom = `content.y1`, second panel right = `content.x1`), so the remainder is not
  "only for the equal-row case" — it is right whenever the subtraction is exact. At a negative origin
  its right edges drift like every other template's (R1).
- **A single row** (`grid-1x3`): this is the case that shows the identity is false — the last row *is*
  the first, so `top == content.y0` with no accumulation, and *still* `y0 + (y1 - y0) != y1` on
  `0.1/0.3 + 50.5×33.3` (R1's first reproduction).
- **A single column** (`grid-3x1`, `grid-7x1`): exact at the origin; the width-axis drift appears on
  `two-tier` at `(-1000.5, -2000.25)` (`5.68e-14` past `content.x1`).
- **A content box that is not a whole number of points**: R1. Fractional *page* sizes whose content
  box is round (`612.3×964.3` at margin 3.3, `595.7×841.9` at 0.1) stay exact — it is the *content
  box* origin, not the page size, that decides.

**Edge-to-edge is equivalent to the old form wherever the old form was right, and strictly better
everywhere else.** At the origin with an even division the two give the same numbers (round 1's
hand-checked 244/274/530, 372/402/18/312, 184/282 are unchanged); the only difference is the last
row/panel's far edge, which is the point. The equivalence is not "close": `PanelRect::right()` *is*
`x + width` and `left += width + gutter` *is* `left + (width + gutter)`, so **at a zero gutter
`left += width` and `top += height` reproduce the neighbour's `right()`/`bottom()` bit for bit**. My
sweep asserts that on both axes, for every template, at four origins including
`(-1000.5, -2000.25)` and `7777.75` — it passes, and M-B/M-C make it fail — so the zero-gutter half of
the doc is *true by construction*, unlike the remainder half.

### Its own guards

- **The sliver detector — now right, and it does detect a multi-column sliver.** The fix compares
  `planned.panels[row * columns]` (the first panel of row `row`) with
  `planned.panels[row * columns - 1]` (the *last* panel of row `row - 1`) — a row boundary, not two
  panels of one row. Verified by mutation: restore the remainder but index-multiply the tops (M-B) and
  the sweep's `drifted` counter fires and `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect`
  fails; the printed drift lines are `3x2`, `3x3`, `4x2`, `4x3`, … — **multi-column grids**, which is
  exactly what the old consecutive-panel comparison could not see (in a multi-column row, consecutive
  panels share a top edge, so a row sliver leaves the count healthy). My
  `a_zero_gutter_makes_neighbours_meet_exactly_on_both_axes_wherever_the_page_sits` and
  `no_template_overlaps_itself_or_leaves_the_content_box_on_any_axis` also fail under M-B, so a
  remainder-only fix no longer passes any suite I know of. One gap remains: the detector measures rows
  only — a *column* sliver could not be seen — but no column sliver is constructible (`left += width +
  gutter` at `g > 0` keeps the gap `≥ g - 1 ULP`, and at `g == 0` columns touch exactly), so the gap is
  not reachable.
- **The zero-gutter guard — now right, and the exact form is the truth.** The assertion is
  `pair[1].rect.y() == pair[0].rect.bottom()`, and it is *not* a tolerance in disguise: because
  `bottom()` is `y + height` and the next top is `y + height`, the meeting is bit-exact at any origin.
  My sweep reproduces it over four origins and three templates; a tolerance there would have been
  weaker than the truth.

### Tolerance

Still reachable, and its purpose is real: `faults` is public and takes rectangles, so a frame the
creator sized to *exactly* the minimum, read back one ULP low, must not be reported — pinned by my
`the_tolerance_is_reachable_from_geometry_the_crate_did_not_compute` (a panel one ULP under
`min_panel_width` is silent; a hundredth of a point under is reported) and by the implementer's
`a_gap_one_ulp_under_the_minimum_is_not_reported`. But its **rewritten doc is not true**: two orders of
magnitude are wrong, and "a safety net for geometry this crate did not compute" is contradicted by
M-E — with the tolerance at `0.0` the crate reports its **own** plans over its **own** sweep, so the
tolerance is still doing work its arithmetic should not need (R5, R1, R2).

### Semantic defects

The fix introduces **no** overlap, no escaped panel, and no truncated or padded page: my sweeps over
nine templates × margins 0/0.1/3.3/18 × gutters 0/0.1/3.3/12 × five page sizes find every panel inside
the content box and **no** pair overlapping under the strict `PanelRect::overlaps` (and M-A/M-B/M-C
make those sweeps fail, so they are not decorative). The only semantic residue is R1, and it is one
ULP: a far edge *past* the content box rather than on it. R2 is a false *report*, not a misplaced
panel; R3 is an error-kind on a degenerate page. Whether that 1-ULP overshoot should block is the
lead's call; what blocked this round is the *claim* that it cannot happen and the reproduction at
1e7pt, not a page a creator can see.

### Mutation table (round 2)

Production files were edited, run, restored, and verified byte-for-byte — `sha256sum` on all nine
files above still matches, and the record was written after the restore. `M-B`'s and `M-C`'s suites
are the implementer's 165 + 8 + 17 (their own) plus round 1's 27 plus my 12.

| # | mutation | their suite | round 1's | mine |
|---|---|---|---|---|
| M-A | `rows`: back to the round-1 index form on both axes and a nominal last row/panel | fail (5) | fail (1) | fail (6) |
| M-B | `rows`: remainder kept, row tops index-multiplied | fail (2) | **pass (27)** | fail (2) |
| M-C | `rows`: column lefts index-multiplied (width axis only) | **pass (190)** | pass (27) | fail (2) |
| M-D | `position_of` reads the order off instead of looking the slot up | fail (1) | pass | pass |
| M-E | `GEOMETRY_TOLERANCE = 0.0` | fail (2) | pass | fail (2) |
| M-F | `plan()` passes `None` as the safe margin | fail (2: the margin test + one W3 lettering test that failed in that run) | pass | fail (1) |

M-B is the one to read: round 1's whole file passes on a row-arithmetic that shares a 1e-13 sliver —
round 1 could not have found the second cause, which is why the implementer's corrected detector is
the only thing standing between that regression and a green suite. M-C is the one to worry about: the
width-axis guard the implementer *added* does not bite (R4).

### What I tried to falsify and could not

- An overlap, an escaped panel, a truncated or padded page, a direction that changes the geometry, an
  index that disagrees with `Page.reading_order`, a `NaN`/infinite page, a hand-built `Plan` whose
  panels contradict its own reading order: all sound in this revision.
- That a zero gutter ever leaves a sliver: it does not, on either axis, at any origin (the one part of
  the new arithmetic that *is* exact by construction).
- That the tolerance masks a real defect: a hundredth of a point under the minimum is still reported,
  and an overlap 50pt deep is still an overlap.

Unverified:
- `cargo xtask ci` end to end was **not** run — only the commands this review was given plus
  `fmt --check`. `ci` also runs `assets`/`brands`/`wasm` and the whole workspace's tests, and other
  agents are mid-edit in `crates/comic-model` and in this crate's `tests/lettering_round1_reviewer.rs`.
- W3's trees: `src/lettering/**`, `src/fit/**` and `tests/lettering_round1_reviewer.rs`, whose result
  moved between runs of this review (25/2, then 26/1) while another agent edited it. Not W2's, not
  reviewed here, and its failures are excluded from every count above.
- Whether the lead wants 1e7pt treated as legal input at all. Nothing in the crate or the design
  bounds a page's *position*; if that is a deliberate refusal to make, R1/R2 become doc-only, and the
  verdict turns on R1's wording alone.
- `PanelRect` and `src/lib.rs` are the lead's: read (it is what makes the zero-gutter meeting exact),
  not reviewed as a change, and not mutated.

---

## Round 3 — verification of the round-2 fixes, and the new relative tolerance

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash`, the role
               file `.github/agents/code-reviewer.agent.md` expects that model, and this session's own
               event log (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`)
               carries `"model":"deepseek-v4.1-flash"` on the overwhelming majority of its 9 188 model
               records. No waiver needed.
Revision reviewed: **working tree (base `16594c58`) after the remainder + relative-tolerance + guard
               fixes** — distinguishable from round 1's `working tree (base 16594c58)` and from round 2's
               `… after the remainder + edge-to-edge fix` by content, and by hash: round 2 recorded
               `src/template/mod.rs` `4b14fb44…` and `src/validate/mod.rs` `51bd82ee…`, and both moved —
               `template/mod.rs` `0f8436407e42c367f3c43c3838d94982d0eacc5d7198666f4409e147f580bf74`,
               `validate/mod.rs` `396f0af65991909c8318f07d0a3d808d5aed0dd83c59c3cdca08a47ca2aeaecc`.
               `src/{panel,reading_order,layout}/mod.rs` and `src/lib.rs` are byte-identical to round 2
               (`097be6d3…`, `0097b021…`, `aa3cd638…`, `fd5491e0…`), so the revision moved exactly where
               the two bug fixes are.
               Files: `crates/comic-layout/src/{template/mod.rs,validate/mod.rs,panel/mod.rs,reading_order/mod.rs,layout/mod.rs,lib.rs}`,
               `crates/comic-layout/tests/round1_reviewer.rs` (round 1's, unchanged,
               `d9385c0c…`), `crates/comic-layout/tests/round2_reviewer.rs` (mine, edited by the lead —
               see below — now `724357f1…`), `crates/comic-layout/tests/round3_reviewer.rs` (mine, new,
               `a23a85b8…`), `docs/comiccraft-design.md` §12.1 (read only).

Verdict:       **approved** — 4 findings, **none a defect**: 4 preference/note. All five round-2
               findings are verified fixed, each by a mutation that makes its own fix fail; the two
               round-2 bugs are gone and the retracted claim's replacement holds at exactly the strength
               it states.

Unit tests:    added `crates/comic-layout/tests/round3_reviewer.rs` — **12 tests, 12 pass**. Every
               one of them is a *verification* of a round-2 fix or a measurement of the new tolerance,
               not a restatement: the one-ULP guarantee is pinned as a claim that is both attained and
               never exceeded, and each round-2 finding has a test that fails when its fix is reverted.
               Verbatim:

```
$ cargo fmt --all --check          → exit 0 (clean, re-checked after every test edit)
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 172 passed; 0 failed
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed
     Running tests/lettering_round1_reviewer.rs
test result: ok. 31 passed; 0 failed        (W3's file, grew 27 → 31 mid-review; not W2's)
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed
     Running tests/round1_reviewer.rs
test result: ok. 27 passed; 0 failed
     Running tests/round2_reviewer.rs
test result: ok. 12 passed; 0 failed
     Running tests/round3_reviewer.rs
test result: ok. 12 passed; 0 failed
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed

$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.42s

$ cargo xtask layers
OK: 29 crates, no layering violations.
```
               279 green in this crate, of which 172 + 8 + 17 are the implementer's and 27 + 12 + 12
               are the three review files. `tests/round1_reviewer.rs` is unchanged and green; its
               round-1 rationale is intact.

### Round-2 verification, finding by finding

- **R1 (bug: the remainder's exactness was overstated) — verified fixed.** The claim is now "the far
  edges are the content box's **to within one ULP of the page's coordinate magnitude**, and **exact**
  when the page's rectangle starts at the document origin" (`template/mod.rs:246-256`), which is what
  round 2 asked for — the guarantee stated at the strength `f64` delivers, and the identity's failure
  explained rather than papered over. Verified independently of the implementer's sweep, over **294 420
  far edges**: 14 origins including `(0.1, 0.3)`, `(1/3, 1/7)`, `(-1000.5, -2000.25)`, `±1e8`, `1e10`
  and `(-3.7e9, 5.1e8)`; page sides 50.5pt … 1e6pt; margins 0 … **49.9% of the page**; gutters
  0/0.1/3.3/12; nine templates. **0 of 294 420 exceed one ULP of the page's magnitude**, and the bound
  is *attained* — the worst is exactly 1 ULP (a `grid-1x3` on a 612×1999.9pt page anchored at
  `(0.1, 0.3)`, margin 305.388). So the replacement claim is neither overstated (R1 again) nor slack.
  Pinned by `the_one_ulp_guarantee_is_attained_and_never_exceeded` (17 088 of those edges), which
  asserts both halves. Its two named reproductions are pinned as *positive* facts by
  `the_documented_guarantee_is_the_strength_f64_delivers_and_not_less`: the 50.5×33.3pt single row lands
  **3.553e-15** (exactly one ULP of 30.3) past the edge, the `(-1000.5, -2000.25)` page's right edges
  **5.684e-14** (0.25 ULP of the page's magnitude) past `content.x1`, and origin-anchored pages are
  exact — so a regression back to "exact everywhere" fails a test. Note the implementer's own
  "measured worst **0.256 ULP** over 47 922 pages" is a *sample*, not the bound: 1 ULP is reachable
  (finding 3 below), and the claim they wrote survives it.
- **R2 (bug: an absolute tolerance cannot cover an unbounded coordinate) — verified fixed.** The
  tolerance is now `tolerance_at(scale) = max(1e-9, 8·|scale|·EPSILON)` with one scale per call. Every
  plan that this sweep can build through the public path — origins `±1e6`, `±1e7`, `±1e8`, three page
  sizes, margins 0/3.3/18, gutters 0/3.3/12, five templates, 400+ of the 810 combinations, the rest
  refused as pages with no room — reports **nothing** against its own gutter as the minimum, and the
  negative sign behaves exactly like the positive one. Mutation **M-REL** (`TOLERANCE_ULPS` 8.0 → 0.1)
  makes those fail, along with the implementer's two `validate` tests and round 2's `r2`, so the fix is
  what makes them pass and not the sweep's shape. And the fix does not blind the validator: at `1e8`pt
  the tolerance is `1.78e-7`pt, and a panel **0.01pt** under the minimum or a gutter 0.01pt under it is
  still reported (`round_two_r2_is_fixed_without_going_blind_to_a_defect_a_creator_could_see`).
- **R3 (preference: the room guard measured the nominal size, not the remainder) — verified fixed, on
  both axes.** The guard now measures `height = content.y1 - top` (and `width = content.x1 - left`) and
  refuses with `NoRoomDown`/`NoRoomAcross` *before* the rectangle is built. Four roomless pages are
  refused as **no room**, never as `Rect(..)`, including the instructive one: a **10.050000001pt** side
  with a 0.1595pt gutter has a *nominal* size of `1.6e-11` — comfortably positive, so only the
  remainder guard can refuse it, and it does. From the other side, a 10.5pt page still lays out all 64
  rows and its last row still lands exactly on the content box. Mutation **M-R3** (the height guard
  reverts to the nominal) fails the implementer's `a_page_too_short_for_its_rows_is_refused_as_no_room`,
  round 2's `r3`, and mine.
- **R4 (preference: the column guard could not fail for the mutation it was written for) — verified
  fixed.** The implementer's `a_zero_gutter_row_has_its_panels_meet_exactly` **fails under M-C**
  (index-multiplied column lefts) — the mutation round 2 showed surviving all 190 of their tests. My
  own wider sweep agrees (`round_two_r4_is_fixed_and_the_column_axis_is_pinned_by_an_exact_meeting`:
  four origins including `(-1000.5, -2000.25)` and `(0.1, 0.3)`, five widths, three margins, six column
  counts), and a companion test pins *why* the earlier sweep could not see it — a far-edge assertion
  alone is satisfied by the index form, so only the edge **between** panels tells them apart
  (`the_zero_gutter_property_is_not_satisfied_by_a_far_edge_alone`). M-C no longer survives any suite I
  know of.
- **R5 (preference: the rewritten tolerance doc was not true) — verified fixed.** All three errors are
  corrected in `validate/mod.rs:106-136`: "**seven** orders of magnitude below … ~0.01pt" (`1e7` ✓),
  "**four** above the `~1e-13` drift" (`1e4` ✓), and the claim that the tolerance is a safety net only
  for geometry this crate did not compute is replaced by "**This floor is absolute, and it is not enough
  on its own**", with the `1e7`/`1e8` measurement that falsified it and the statement that the tolerance
  actually used is `tolerance_at`. The reachability the doc names is still pinned by round 2's
  `the_tolerance_is_reachable_from_geometry_the_crate_did_not_compute` and the implementer's
  `a_gap_one_ulp_under_the_minimum_is_not_reported`, both green. One precision remains (finding 2).
- The four tests round 1 and round 2 called unable to fail are still gone, and the two *new* guards
  round 2 added are now live: I could not make `a_zero_gutter_row_has_its_panels_meet_exactly`,
  `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect` or `a_page_far_from_the_origin_is_not_reported_for_its_own_arithmetic`
  pass on a reverted fix.

### My edit to your test (the lead's change to `round2_reviewer.rs`)

**Honest, and it still bites. I kept it and corrected its comment; I did not loosen or tighten the
assertion.**

- It fails when the arithmetic breaks: under **M-A** (the round-1 index form returned on both axes)
  `r1_the_remainder_lands_on_the_content_box_wherever_the_page_is` **fails**, together with five of my
  other tests, four of round 1's, and seven of the implementer's. The origin half is an exact
  `assert_eq!` and is load-bearing; the elsewhere half is one-sided in the right direction — it accepts
  ≤1 ULP of the content box's magnitude, while the true drift at that magnitude is 0.25 ULP.
- It has not become an assertion that cannot fail: it is not satisfied by a page whose panels stop short
  of the content box (the comparison is two-sided, `|edge - want| ≤ bound`), and its sibling
  `a_page_whose_content_box_is_not_a_whole_number_of_points_keeps_the_far_edges_inside_it` (one-sided,
  `>` only) still fails under M-A.
- It preserves what the test was for. The purpose was "a page whose panels do not reach the content
  box is caught"; that is exactly what it still does, and the two failure messages say which edge of
  which row of which page is off by how much. What it gave up is a claim that f64 cannot keep — which
  round 2 itself found, so the retraction is the reviewer being corrected, not the test being weakened.
- Two things I fixed in *my* file, since I own it: (i) the comment said "within one ULP of the page's
  coordinate magnitude" while the code measures the **content box's** magnitude — the stricter of the
  two, since the content box is inside the page — so the comment now says which one it is, and why the
  tighter reading still holds (0.25 ULP worst with margins to 49.9%); (ii) the sibling test's comment
  referred to "the identity the module doc states", which the corrected module doc no longer states; it
  now quotes the guarantee that is actually there. The rest of the file is untouched: the other
  repeated exact-equality block (the origin-anchored sweep at `round2_reviewer.rs:66,69`, the occurrence
  the lead says he hit first) is intact and still asserts **exactly** — it fails under M-A, so it is
  live. `cargo fmt` was run on the file after the edits.

### The relative tolerance (new code)

**Shape: right. Constant: right, and the doc's name for it under-claims in the safe direction. One
scale per call: right for this crate's own plans, and the justification next to it is not true as
written. Floor: does its job; negatives and zero are handled.**

- **Shape.** The error being covered is a relative (binade) error, so the tolerance must be relative to
  the *magnitude of the operands*, not to the size of the panel or the gap: `a - b` where both are ~1e8
  carries ~`1e8·EPSILON`, whatever the gap between them. `max(floor, k·|scale|·EPSILON)` is the
  standard form and the right one, and `scale` is taken from absolute coordinates rather than from
  widths, which is what makes it cover a page far from the origin.
- **Constant.** Measured worst drift over my 294 420-edge sweep: **1.0 ULP of the page's magnitude**, so
  `8` is ~8× the worst case. It is not measured in whole ULP, and the doc says it is: `8·scale·EPSILON`
  is `8·mantissa` ULP of `scale` with the mantissa in `[1, 2)`, i.e. **between 8 and 16 ULP** — measured
  9.3 … 15.3 over `1e6` … `1e10` (`the_tolerance_grows_with_the_magnitude_and_keeps_the_floor_near_the_origin`
  pins the range). So "eight ULP" is a lower bound and under-promises, which is the harmless direction;
  the other direction would be a false promise (finding 2).
- **One scale per call.** Correct for everything this crate produces: containment means every panel of a
  plan is inside the page, so the page's magnitude is the largest coordinate in play and a page and its
  panels cannot differ in magnitude in a way that matters. It is *not* correct as documented for
  `faults`' second use — `faults` is public and its own doc says a caller hands it rectangles this crate
  did not compute, so a panel may be nowhere near the page (finding 1). The doc's justification ("every
  panel is inside the page (that is what containment checks)") is a statement about this crate's plans,
  not about the function's inputs.
- **Floor.** `tolerance_at(0.0) == tolerance_at(612.0) == 1e-9`, and the crossover is
  `1e-9 / (8·EPSILON) ≈ 5.63e5`pt — computed from the formula, not from a literal, in the test. The
  floor's job has changed with the fix and the doc says so: it is no longer what keeps the crate's own
  output quiet (mutation **M-E**, `GEOMETRY_TOLERANCE = 0.0`, no longer breaks
  `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect` as it did in round 2 — the relative
  term covers those pages now); it is the plain slack for a small page whose relative term is tiny
  (`scale = 1pt` gives `1.8e-15`, and 1e-9 is what keeps a frame read a whisker off from being
  reported). That is exactly what the rewritten doc claims, so R5's correction is complete rather than
  merely reclothed.
- **Negatives and zero.** `scale.abs()` handles negatives, and the tolerance is symmetric: my 270-plan
  sweep runs both signs and round 3's `round_two_r2_is_fixed_and_a_page_far_from_the_origin_is_still_not_a_defect`
  asserts equality of the two. `scale == 0` is the floor, tested. Not obviously handled: `NaN` gives the
  floor (`f64::max` ignores `NaN`) which is the safe answer, and a non-finite magnitude gives a
  non-finite tolerance — finding 4.
- **Ceiling.** There is none, and the consequence is worth stating once: the tolerance is 8–16 ULP of
  the magnitude, while the *smallest representable* length at that magnitude is 1 ULP, so any gap —
  and any overlap up to 8 ULP deep — reads as touching. That is unavoidable for any relative tolerance that
  must not report the crate's own 1-ULP drift (round 1's F1), and at ordinary magnitudes the `1e-9`
  floor is ~8 800 ULP of a 612pt page, i.e. nowhere near the resolution. The two thresholds a user can
  reason about: masking a defect a printer can see (0.01pt) needs a coordinate past **~5.6e12pt**, and
  silences everything past **2.25e307pt**. Both are pinned in the tests.

### Findings

**1 — `preference` — `crates/comic-layout/src/validate/mod.rs:214-221` (`magnitude`), with `:337`
(`page`'s degenerate-artboard guard).**
Trigger: `faults` is public and documented for rectangles this crate did not compute, and the one scale
it takes is the largest coordinate of the page *and every panel*. A caller who hands over one runaway
rectangle inflates the tolerance for all of them: with a page at the origin and a panel 0.001pt past the
margin the panel is a finding, and the same call with one 1e12pt panel alongside it reports only the
runaway one
(`one_scale_per_call_covers_the_pages_own_coordinates_and_every_panel_it_is_given`). Expected: the
tolerance to be sized by what it is defending — the geometry that shares the comparison — or the doc to
say that one runaway rectangle sets the scale for the call. Actual: the doc justifies the single scale
with "every panel is inside the page (that is what containment checks)", which is true of a plan this
crate produced and false of `faults`' inputs. **Not a defect**: what is masked is sub-printer-mark
(0.001pt) and it needs a caller whose own rectangle is inconsistent with its page, so the cost is a
worse doc sentence and a slightly generous tolerance rather than a wrong answer about a real page;
mutation **M-MAG** (the magnitude ignores panels) leaves the implementer's whole suite green, so nothing
in the crate pins the choice either way.

**2 — `preference` — `crates/comic-layout/src/validate/mod.rs:123-129` (`TOLERANCE_ULPS`'s doc).**
Trigger: read against the formula. Expected: "How many ULP of the coordinate magnitude `tolerance_at`
grows by. Eight…" to describe the constant. Actual: it is `8 · scale · EPSILON`, which is `8 × mantissa`
ULP of `scale` — **9.3 to 15.3 ULP** over the magnitudes I measured — so a reader who derives a
tolerance from "eight ULP" gets half the real one. Under-claiming, so no test fails; pinned as a range
in `the_tolerance_grows_with_the_magnitude_and_keeps_the_floor_near_the_origin`.

**3 — `preference` — `crates/comic-layout/src/template/mod.rs:248` (the "worst **0.256 ULP**" claim).**
Trigger: a wider sweep than the one that produced the number. Expected: the worst far-edge error over
legal pages. Actual: an order of magnitude more is reachable — a `grid-1x3` on a 612×1999.9pt page
anchored at `(0.1, 0.3)` with a margin of 49.9% of the page lands **exactly one ULP** past its content
box, `0` of 294 420 other far edges exceed it. The central claim (`≤ 1 ULP`) is *not* affected, which is
why this is a preference and not another R1: the sentence is right and its supporting measurement is
modest. My `the_one_ulp_guarantee_is_attained_and_never_exceeded` asserts the bound is attained, so the
weaker number cannot come back unnoticed.

**4 — `preference` — `crates/comic-layout/src/validate/mod.rs:138-140` (`tolerance_at`) with `:337`
(`page`'s guard).**
Trigger: an artboard whose rect is infinite. `page()` skips an artboard with no area or a `NaN` side
(`width() <= 0.0 || height() <= 0.0 || width().is_nan() || height().is_nan()`) but an **infinite** one
passes both arms, and `faults` then gets a magnitude of `inf`: `TOLERANCE_ULPS * scale.abs() *
f64::EPSILON` multiplies before it scales down, so the relative term overflows to `inf` once `8 · scale`
passes `f64::MAX` (past 2.25e307pt), and every threshold is then satisfied. Expected: the same answer a
finite artboard gives, or the page skipped the way a degenerate one is. Actual: two frames that share 50pt on both axes are an
`Overlap` on a 612×792pt artboard and **nothing at all** on an infinite one
(`an_infinite_artboard_silently_switches_the_geometry_check_off`, asserted as the inconsistency it is:
1 finding vs 0). One line (`is_finite()`) closes it, and the intent at that guard is already visible.
**Not a defect**: nothing in the crate can produce such a page — `plan` refuses a non-finite artboard
cleanly (round 1 pinned that) and `rows` refuses a non-finite content box — so the trigger is a
malformed document or a caller's own bad rectangle, and the fix is defensive consistency rather than a
broken page. It is the strictest thing I found in three rounds and it is still a preference.

### Semantic defects

**None.** Reading for one rather than only testing for one: the guard change is error-kind only — the
rows that previously came back `Err(Rect(NonPositive))` now come back `Err(NoRoom*)`, and *no* page that
returned `Ok` before returns `Err` now, because the guard is evaluated on the same `height`/`width` the
rectangle was built from and returns before the build either way. The relative tolerance changes only
*which* comparisons trip, and at every magnitude a creator can reach it trips the same ones: a 1pt
overlap, a 0.01pt-thin gutter and a panel 0.01pt under the floor are all still reported at 1e8pt (my
round-3 test) and at the origin (round 2's), and the whole class round 2 was about — the crate reporting
its own plan — is gone with 0 spurious findings over those 400+ far-origin plans and 294 420 far edges.
The only new behaviour a user could ever meet is finding 4's silence, and it needs an infinite artboard.
Overlaps, escapes, truncated or padded pages: none introduced (round 2's sweeps still pass, and M-A/M-B/
M-C/M-R3 still fail them).

### Mutation table (round 3)

Production code was edited **one change at a time**, run, restored immediately, and verified — the
`sha256sum` of `template/mod.rs` and `validate/mod.rs` was recorded before the first run and re-checked
after every restore (all six OK), and the "restored" line of every run above is that check. Both files
are byte-identical to the revision I reviewed. Suites: **their** = `--lib` + `layer_rules_contract` +
`panel_rect_contract`; **r1**/**r2**/**r3** = the three review files.

| # | mutation | their suite | round 1's | round 2's | round 3's |
|---|---|---|---|---|---|
| M-C | column lefts index-multiplied (width axis only) | **fail (1)** (+1 unrelated fonts flake) | pass (27) | fail (2) | fail (3) |
| M-A | the round-1 index form on both axes, nominal last row and panel | fail (7) | fail (1) | fail (5) | fail (4) |
| M-R3 | the height guard reverts to `nominal_row_height` | fail (1) | pass (27) | fail (1) | fail (1) |
| M-REL | `TOLERANCE_ULPS` 8.0 → 0.1 | fail (2) | pass (27) | fail (1) | fail (4) |
| M-E | `GEOMETRY_TOLERANCE` 1e-9 → 0.0 | fail (1) | pass (27) | **pass (12)** | fail (1) |
| M-MAG | `magnitude` takes the page's coordinates only | **pass (197)** | pass (27) | pass (12) | **fail (1)** |

Rows to read: **M-C** is R4 — the implementer's own suite now fails the mutation that round 2 showed
surviving all 190 of their tests. **M-A** is R1 — the report is written after the restore, and the
edited round-2 `r1` is among its failures, which is the evidence that the lead's change did not turn
that test into decoration. **M-E** is R5 — with the floor at zero the crate's own sweeps stay green
(and my round-2 reachability test too, since `8·792·EPSILON` is still 1.4e-12), which is precisely the
sense in which the floor is no longer "not enough on its own" but "a floor": the doc's new wording is
the accurate one. **M-MAG** is finding 1 — nothing but my note test pins whether the magnitude includes
the panels.

### What I tried to falsify and could not

- That the one-ULP claim is still an over-claim (round 2's R1 in a new coat): 294 420 far edges, none
  past 1 ULP, one *at* exactly 1 ULP. Falsified the implementer's weaker "0.256 worst" instead.
- That the relative tolerance is too small somewhere: from 1e6 to 1e10, both signs, the crate's own
  plans stay unreported, and M-REL shows the constant is what does it.
- That the relative tolerance is too large somewhere a creator can reach: a 0.01pt violation is
  reported at 1e8, and at 1e10 a zero-gutter page is still not an overlap. Masking needs 5.6e12pt.
- That the new guard refuses a page that previously laid out: it cannot, by construction (same operands,
  earlier return), and a 10.5pt/64-row page still lays out with its last row exactly on the box.
- That the zero-gutter meeting is a tolerance in disguise: `left += width` is `right()` bit for bit; the
  mutation that breaks it fails five tests across three files.
- That any of the four new findings is a defect: each needs a caller or a document that contradicts
  itself (a runaway panel, a non-finite artboard) or is a doc number, not a page.

Unverified:
- `cargo xtask ci` end to end was **not** run — only the three commands this review was given plus
  `fmt --check`. `ci` also runs `assets`/`brands`/`wasm` and the whole workspace's tests, and other
  agents are mid-edit in `crates/comic-model` and in this crate's `tests/lettering_round1_reviewer.rs`
  (which grew 27 → 31 tests during this review, inside one round — its results are excluded from every
  count above).
- One **flaky** failure I could not attribute: `lettering::fonts::tests::a_blank_family_is_not_a_missing_font_and_a_blank_style_is_reported_as_the_exporter_reports_it`
  failed twice inside a full parallel run *under M-C* and passed three full clean runs and nine
  single-test runs; it is W3's module, it is not affected by the column arithmetic, and I did not
  investigate it further.
- Whether the lead wants the four preferences: each is one sentence or one line, and the verdict is
  `approved` with them unaddressed.
- `src/{panel,reading_order,layout}/mod.rs` and `src/lib.rs` are byte-identical to round 2, so they are
  round 2's review, not re-reviewed here; `PanelRect` (the lead's) is read again for the zero-gutter
  meeting and the sub-ULP `NotRepresentable` guard, not reviewed as a change and not mutated.
- W3's trees (`src/lettering/**`, `src/fit/**`, `tests/lettering_round1_reviewer.rs`) — not W2's, read
  only where `validate::plan` and the lettering tests intersect.

## Round 4 — the implementer folds in the four round-3 preferences

Round 3's verdict was `approved` with four `preference` findings, and the lead folded all four in
rather than carrying them: three are the "a stated guarantee that is not the delivered one" pattern,
and the fourth is one `is_finite()`. Nothing else in the crate was touched. Revision measured:

| File | `sha256sum` |
|---|---|
| `crates/comic-layout/src/validate/mod.rs` | `89e1342d565f6bc1a881a889001467b76a3083d142127c765f21910e6818d8e7` |
| `crates/comic-layout/src/template/mod.rs` | `f8807972ea7f6e28944212c4289646021cf980a8b82704be675c73d6973e3cea` |
| `crates/comic-layout/tests/round4_layout_engineer.rs` (new) | `c211b27215ecc6757df519f7c0878924c37b045ad4bf08af4cfd749bafe17e7b` |

**Finding 1 — one scale per call, and the doc now says so.**
The choice is **one scale per the whole call**, kept, because the alternative — one scale per
comparison — contradicts the behaviour `round3_reviewer.rs`'s
`one_scale_per_call_covers_the_pages_own_coordinates_and_every_panel_it_is_given` pins as the recorded
boundary (it asserts the 0.001pt panel *is* masked by a 1e12pt one, and that file is the reviewers' and
is not to be edited). `magnitude`'s doc used to justify the single scale with "every panel is inside the
page", which is a statement about a plan this crate produced and false of `faults`' inputs; it now
states the trade plainly — **"one runaway rectangle sets the tolerance for everything in the call"** —
gives the size of the cost (`1.8e-3`pt at the reviewer's 1e12pt, and ~`5.6e12`pt to mask a defect a
printer can see), and names the test that measures it.
Test: `one_runaway_rectangle_sets_the_tolerance_for_the_whole_call` (`tests/round4_layout_engineer.rs`)
runs the reviewer's case and asserts all three steps — reported alone (`InSafeMargin`), silent with the
runaway (only `OutsidePage { panel: 1 }`), and the boundary from the formula.

**Finding 2 — the constant is described in the unit it is.**
`TOLERANCE_ULPS`' doc said "eight ULP"; the term is `TOLERANCE_ULPS * scale * EPSILON`, i.e. `8 *
mantissa` ULP of `scale`. It now reads: *"**Not "eight ULP".** … so this constant is `8 * mantissa` ULP
of `scale`: **between 8 and 16 ULP**, measured 9.3 … 15.3 over `1e6` … `1e10`pt … the worst drift
measured is **1.0 ULP** of the magnitude, over 294 420 far edges"*. The value is unchanged at `8.0`, and
`tolerance_at`'s own doc was changed from "`TOLERANCE_ULPS` ULP of the magnitude" to "`TOLERANCE_ULPS`
times one ULP of the magnitude" so the wrong unit does not reappear one function away.

**Finding 3 — the bound is reachable, and the old figure was a sample.**
`template::rows`' doc said "Measured worst **0.256 ULP** over 47 922 probed pages". The `≤ 1 ULP`
guarantee stands; the sentence now says the bound is **reachable** (a `grid-1x3` on a 612x1999.9pt page
at `(0.1, 0.3)` with a margin of 49.9% of the page lands **exactly one ULP** past its content box, 0 of
294 420 other far edges past it) and that the 0.256 figure was a **sample**, not a bound. The
reachability is pinned here as well as in the reviewer's file:
`the_one_ulp_bound_is_attained_by_a_legal_page` sweeps the four gutters for that page and asserts a far
edge lands exactly 1 ULP out, and none exceeds it.

**Finding 4 — the non-finite page (the lead overrode the reviewer's `preference` here).**
Two lines, because the silence had two entrances:
1. `magnitude` now takes the largest absolute **finite** coordinate: a coordinate that is not finite
   cannot size a tolerance, since `tolerance_at` has no ceiling and an infinite page made every
   threshold hold. So `faults` reports for an infinite page what it reports for a finite one —
   `an_infinite_page_reports_what_a_finite_page_reports` asserts that equality (a 50pt two-axis overlap
   is one `Overlap` on both pages), and keeps the ceiling visible
   (`tolerance_at(f64::INFINITY).is_infinite()`, `8 * f64::MAX * EPSILON` overflowing).
2. `validate::page`'s degenerate-artboard guard now measures finiteness
   (`!width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0`; `is_finite()` covers
   the `NaN` arm it replaces), so an artboard the geometry check cannot use is **skipped the way a
   degenerate one is** rather than handed to `faults` with a magnitude of `inf`.
   `a_non_finite_artboard_is_skipped_the_way_a_degenerate_one_is` pins that on a page with both a
   dangling frame link (rule 7, which needs no artboard) and a real overlap: the finite artboard reports
   2 findings, and the infinite one reports the rule-7 finding **exactly as the finite page does** and
   nothing else — the same report a zero-area artboard gives.
The residual, stated rather than left implicit: `page` still reports nothing for a non-finite artboard,
because a page whose artboard is not finite has no rectangle to measure panels against, and the
reviewer's `a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults` pins `0` there. What changed
is *why*: a deliberate skip, not a tolerance that overflowed to `inf`.
**Other thresholds reached by the overflow:** none. In `src/validate/**` the only thresholds are in
`faults`, and every one of them is fed by the single `tolerance` computed from `magnitude`; `plan`
delegates to `faults`, `reading_order::check` compares no coordinates, and the rule-7 check is a map
lookup. A page whose *finite* coordinates are past ~`2.25e307`pt still overflows to `inf` and is still
silent — that is the resolution ceiling round 3 recorded and it is left alone: there one ULP is
`~2e292`pt, so no panel comparison inside such a page can represent a defect in the first place. The
non-finite case was different because its panels are finite and the overlap *is* representable.

**Mutation evidence (each change, reverted alone, fails one test and only one):**

| Mutation | `round4_layout_engineer.rs` |
|---|---|
| **M-GUARD** — the artboard guard reverts to `<= 0.0 \|\| is_nan()` | 1 failed (`a_non_finite_artboard_is_skipped_the_way_a_degenerate_one_is`) |
| **M-SCALE** — `magnitude` stops skipping non-finite coordinates | 1 failed (`an_infinite_page_reports_what_a_finite_page_reports`) |

Both mutations were applied to the working tree, the file run, and the file restored; the restored tree
is the revision hashed above.

**Commands, as run** (toolchain `cargo 1.99.0 (5f94df478 2026-08-27)` — the workspace's
`resolver = "3"` needs it; `/usr/bin/cargo` is 1.75 and cannot even parse the manifest):

- `env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout` → **283 passed, 0 failed** across
  172 (lib) + 8 (`layer_rules_contract`) + 31 (`lettering_round1_reviewer`) + 17
  (`panel_rect_contract`) + 27 (`round1_reviewer`) + 12 (`round2_reviewer`) + 12 (`round3_reviewer`) +
  4 (`round4_layout_engineer`), exit 0.
- `cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings` → finished with no output
  after `Checking vectorcraft-comic-layout v0.8.0`, exit 0.
- `cargo xtask layers` → `OK: 29 crates, no layering violations.`
- `cargo fmt --check -p vectorcraft-comic-layout` → clean. `cargo fmt -p vectorcraft-comic-layout`
  touched only the three files above.

**Fifth findings — reported, not fixed.**
None found. The one thing read again and left alone is the finite-huge ceiling in the paragraph above:
it is a property of any relative tolerance at a magnitude where the defect is below one ULP, not a
mismatch between a claim and the code.

**Unverified:** `cargo xtask ci` end to end was not run (it also runs `assets`/`brands`/`wasm` and the
whole workspace, and other agents are mid-edit in `crates/comic-model` and in this crate's lettering
files). The other crates are not re-checked here. The lead's requested test wording — "the infinite
page reporting what the finite page reports" — is satisfied at `faults`; at `validate::page` the
non-finite artboard is skipped, because the reviewer's round-3 file pins `0` findings there and is not
to be edited.

---

## Round 4 — confirmation of the four round-3 preferences, and one residual

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash`, the role
               file expects that model, and this session's own event log
               (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`) carries
               `"model":"deepseek-v4.1-flash"`. No waiver needed.
Revision reviewed: **working tree (base `16594c58`) after the four round-3 preferences were folded
               in** — distinguishable from round 3's `… after the remainder + relative-tolerance + guard
               fixes` by hash: `validate/mod.rs`
               `89e1342d565f6bc1a881a889001467b76a3083d142127c765f21910e6818d8e7`,
               `template/mod.rs f8807972ea7f6e28944212c4289646021cf980a8b82704be675c73d6973e3cea`,
               `tests/round4_layout_engineer.rs` (the implementer's, new)
               `c211b27215ecc6757df519f7c0878924c37b045ad4bf08af4cfd749bafe17e7b`; `panel/reading_order/
               layout/lib.rs` and `tests/round1_reviewer.rs` `d9385c0c…`, `tests/round2_reviewer.rs`
               `724357f1…` are byte-identical to round 3, so round 4 moves exactly where the four
               preferences are. **My own round-3 file changed — by me, this round**: it was
               `a23a85b8…` and is now `23ed989e608b2d0cdc9b0df1c2655334a621471d079ccfac28aae9123f0ab773`
               (one test renamed and re-commented, one assertion pair added); my new
               `tests/round4_reviewer.rs` is `bae176d491afe7eb88ac8645d891ad1833bd7e874d5a3cb736924bf9eaa4790b`.
               Files: `crates/comic-layout/src/{validate/mod.rs,template/mod.rs}` (the changed pair),
               the four byte-identical sources, `tests/round{1,2,3,4}_reviewer.rs`,
               `tests/round4_layout_engineer.rs`, `crates/comic-model/src/validate.rs` (rule 15, read
               only), `docs/comiccraft-design.md` §12.1 (read only).

Verdict:       **approved** — **2 preferences**, no defect. All four round-3 preferences are verified
               fixed, each by a mutation that reddens the test claimed *and* the reviewer's copy of it;
               the residual the implementer recorded is correct as a decision, and the reason its guard
               gives for it is not.

Unit tests:    added `crates/comic-layout/tests/round4_reviewer.rs` — **5 tests, 5 pass** — and changed
               `tests/round3_reviewer.rs`'s `an_infinite_artboard_silently_switches_the_geometry_check_off`
               into `a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults` (same two
               assertions, plus the assertion that `faults` *is* no longer off; see below). `cargo fmt`
               was run on both files after every edit. Verbatim:

```
$ cargo fmt --all --check          → exit 0 (clean, re-checked after every test edit)
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout
     Running unittests src/lib.rs                          → ok. 172 passed; 0 failed
     Running tests/layer_rules_contract.rs                 → ok. 8 passed; 0 failed
     Running tests/lettering_round1_reviewer.rs            → ok. 31 passed; 0 failed
     Running tests/panel_rect_contract.rs                  → ok. 17 passed; 0 failed
     Running tests/round1_reviewer.rs                      → ok. 27 passed; 0 failed
     Running tests/round2_reviewer.rs                      → ok. 12 passed; 0 failed
     Running tests/round3_reviewer.rs                      → ok. 12 passed; 0 failed
     Running tests/round4_layout_engineer.rs               → ok. 4 passed; 0 failed
     Running tests/round4_reviewer.rs                      → ok. 5 passed; 0 failed
   Doc-tests vectorcraft_comic_layout                      → ok. 0 passed; 0 failed
   **288 passed, 0 failed** (the implementer's 283 + my 5)

$ cargo xtask ci
CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)
  — run this round, so "all 7 green" is verified and not taken on report; it also checks
    `vectorcraft-comic-layout` for wasm32 and the whole workspace's tests, all green.
$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings   → clean
$ cargo xtask layers                                                      → OK: 29 crates, no layering violations.
```

### The residual skip — is it the degenerate skip, or a new silence?

**Correct division of labour, and I did not change the assertion. I changed the name and the comment,
because I had described the *cause* rather than the *decision*.**

- It is **the same skip**, proved rather than asserted: `zero area`, `inverted`, `NaN`, `+inf` and
  `-inf` artboards all produce one and the same `Vec` — the rule-7 finding and nothing else — compared
  field by field against each other in
  `every_unusable_artboard_gets_one_skip_not_one_silence_per_cause`, and the rule-7 finding is in the
  list precisely to show the skip is scoped to geometry and does not take the page's other checks with
  it. The implementer's `a_non_finite_artboard_is_skipped_the_way_a_degenerate_one_is` proves the same
  equality independently. A legal artboard beside them reports two findings (the link and the overlap),
  so the skip is a decision about the artboard and not a check that stopped working.
- **A caller cannot distinguish "clean" from "skipped" — and that is not new.** It was already true of
  the zero-area and `NaN` skips, which predate this milestone: a skip is an empty list, and `Finding`
  has no "not checked" channel. What round 4 changed is that the *cause* is now a stated policy
  (`page`'s doc lists it with the missing-artboard and no-area cases) rather than a tolerance that
  overflowed two functions away.
- **It is named one layer up, which is what makes the silence honest — with one condition I checked.**
  `comic-model`'s rule 15 (`crates/comic-model/src/validate.rs:723`) compares `artboard.rect` with
  `ProjectSettings.default_page_size`, and with the real fixture bundle and the real entry point an
  infinite artboard yields **exactly one** rule-15 finding, subject `Page(8).artboard_id`, message
  "artboard 1 is inf×inf pt but `default_page_size` is 612×792 pt — a page may deliberately differ", a
  warning; a legal artboard yields none
  (`the_model_names_a_non_finite_artboard_when_the_project_has_a_default_size`). **The condition:** rule
  15 returns early when `default_page_size` is `None`, and then nothing in the pipeline names the
  artboard — that is pinned in the same test. It is rule 15's own precondition (there is no expected
  size to differ from) and not a hole this crate can close: design §12.1 gives "unsupported page
  dimensions" to `comic-model`, and this crate implements no rule for "the artboard rect is usable".
  If the lead wants unconditional coverage, the rule to widen is in `comic-model`, not here.
- **My pin.** It was **not** pinning the wrong thing: the skip is the answer I offered as the
  alternative and I now judge it right, for the reasons above. What *was* wrong was the label — the
  test was called "silently switches the geometry check off" and its comment explained an overflow that
  round 4 removed, so it described a cause rather than the contract it guards. I renamed it
  `a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults`, rewrote the comment to state the
  skip as a deliberate policy with the two reasons it is defensible (the containment half is meaningless
  for a page that is not a rectangle; the artboard is named by rule 15), and **added** the assertion
  that the switch-off itself is gone (`faults` reports the same `Overlap` for the infinite page it
  skips and for a finite one). It still pins `0`, and it now says in the file that if the `page()`-level
  skip is ever revisited, that assertion is the one to change — a contract, not a hidden prohibition.
  Note the round-4 implementer section still refers to the old test name.

### Is a non-finite artboard reported end-to-end?

**Yes, whenever the project names a default page size — verified with the fixture, not by reading.**
One rule-15 warning with the artboard's own size in the message and `Page(8).artboard_id` as the
subject; nothing for a legal artboard. **When `default_page_size` is `None`, nowhere** — rule 15's early
return — and that is the only silence left in the chain, in a crate that is not W2's. It is the same
condition under which a *legitimately* unusual page size goes unreported, so it is rule 15's shape
rather than a new hole. Beyond the finding: the `faults` primitive now reports for an infinite page what
it reports for a finite one, so the round-3 sentence "the geometry check is silently off" is no longer
true of anything but this deliberate skip.

### Round-3 preferences, one by one

- **1 (one scale per call) — verified closed by documentation, and the numbers are right.**
  `tolerance_at(1e12) = 8·1e12·EPSILON = 1.776e-3`, which the doc calls `1.8e-3`pt; and
  `0.01 / (8·EPSILON) = 5.629e12`, which it calls ~`5.6e12`pt. Both were then *measured through
  `faults`* rather than re-derived: a panel 0.01pt past the safe margin is one `InSafeMargin` finding on
  its own, still reported with a `5e12`pt rectangle in the call, and masked by a `6e12`pt one — one
  runaway rectangle sets the tolerance for every comparison in the call, exactly as the new doc says
  (`the_documented_cost_of_one_scale_per_call_is_the_measured_one`). **Was one scale forced?** Not by
  the arithmetic: per-comparison sizing would have kept every answer for every plan this crate produces
  and would have reported the small panel's breach instead. It was forced only by my own round-3 file,
  which pins the masking — and that file is mine, so the implementer was right to say so and right to
  keep the choice anyway, because it is *not* a correctness requirement: the masking can only bite
  below 0.01pt until a caller's rectangle is 5.6e12pt from its page (a caller contradicting its own
  page), and per-comparison sizing would trade that for a second tolerance to reason about inside one
  call. The doc's old justification — "every panel is inside the page" — is gone; it now says the
  assumption is *stated* rather than relied on, names the cost, and names the test that measures it. My
  pin stays, and it now agrees with the doc: if the lead ever wants per-comparison sizing, that
  assertion in `round3_reviewer.rs` and this paragraph both change together, deliberately.
- **2 (the constant's unit) — verified fixed, and "between 8 and 16" is right at both extremes.**
  `TOLERANCE_ULPS * scale * EPSILON` is `8 · mantissa` ULP of `scale`. At a power of two the mantissa is
  exactly 1, so the term is **exactly 8 ULP** (`term_in_ulp(2^20) == 8.0`, checked bit-wise, with
  `ulp(2^20) == 2^-32`); at the largest `f64` below `2^21` it is **15.999999999999998**, so the top of
  the range is approached and never reached; across the whole binade the multiple stays in `[8, 16)`
  (`the_ulp_multiple_is_eight_at_a_power_of_two_and_stays_under_sixteen_above_it`, 65 points). The
  stated window re-measures at 15.26, 14.11, 9.54, 11.92, 9.31 over `1e6 … 1e10`pt — "9.3 … 15.3" is
  my own figure, to the digit. The one way a reader could still be misled is the *floor* regime, where
  the multiple is *larger* than 16 (a `1e5`pt page gets 68.7 ULP), which is why the doc naming its
  window is load-bearing rather than decorative; the test pins that side too.
- **3 (the 0.256 ULP figure) — verified fixed.** `template::rows`' doc now calls the `≤ 1 ULP` bound
  **reachable**, names the page that attains it (my `grid-1x3` on 612×1999.9pt at `(0.1, 0.3)`, margin
  49.9%), labels 0.256 ULP over 47 922 pages a **sample**, and keeps exactness-at-the-origin. All three
  parts are true against my 294 420-edge sweep, and the implementer added the reachability to their own
  file as `the_one_ulp_bound_is_attained_by_a_legal_page`, which sweeps the four gutters and asserts an
  edge lands exactly 1 ULP out — it is not tautological.
- **4 (the non-finite page) — verified fixed at the primitive, and the skip is the residual above.**
  `magnitude` skips a non-finite coordinate, so the scale is finite (0 when nothing in the call is
  finite, which lands on the `1e-9` floor) and an infinite page reports what a finite one reports. The
  *finite* ceiling is real and is recorded rather than dismissed: `8 · scale` itself overflows past
  `f64::MAX / 8 = 2.247e307`pt, so the same two panels are one `Overlap` on a `1e307`pt page and
  `Vec::new()` on a `1e308`pt one (`the_non_finite_scale_fix_removes_the_overflow_at_infinity_but_not_the_finite_ceiling`).

### Findings

**1 — `preference` — `crates/comic-layout/src/validate/mod.rs:337-343` (`page`'s guard comment), and
the same sentence in the round-4 record.**
Trigger: read the guard's stated reason against the code one function down. Expected: a comment whose
reason is still true. Actual: it says "`faults` cannot size a tolerance from an infinite magnitude, and
handing it one would satisfy every threshold — the geometry check would be silently off". **After this
very change that is false**: `magnitude` now skips a non-finite coordinate, so `faults` *can* be handed
an infinite page and answers correctly — one `Overlap` for two frames sharing 50pt on both axes, no
spurious containment finding (`every_unusable_artboard_gets_one_skip_not_one_silence_per_cause` asserts
the primitive's answer for the same frames `page()` discards; the round-4 prose concedes as much with
"the non-finite case was different because its panels are finite and the overlap *is* representable").
So the skip is now a **choice** — "this crate makes no geometry finding about a page whose artboard is
not a rectangle" — and not something `faults` forces. Since the choice is defensible (the jump the
degenerate skip already made, and the artboard is named by rule 15), the finding is the *reason*, not
the behaviour: state it as a choice, or hand the artboard through, since the fixed primitive would
report the overlap and the size findings and no containment. **Not a defect**: it is a comment whose
justification was removed by the same commit, the behaviour is unchanged from round 3, and no page a
creator can build reaches it. Same family as the three round-3 preferences, and the same one-line kind
of edit.
**Related, also recorded and not a finding:** the sibling sentence in `magnitude`'s comment — "there one
ULP is `~2e292`pt, so no comparison between panels inside it can represent a defect to report" — is too
generous. One ULP at `1e308`pt is `1.995…e292`pt, confirmed, but a `5e299`pt-deep overlap *is* a
representable comparison and *is* not reported: past the ceiling the check is off, not satisfied. What is
true is that a page that size is `3.53e304`m — `3.7e288` light-years, `4e277` observable universes — so
nothing it can hold is a defect a creator could act on. Two sentences, one reason, one edit.

**2 — `preference` — `crates/comic-layout/src/validate/mod.rs:127-129` (`TOLERANCE_ULPS`'s doc).**
Trigger: the sentence that sizes the absurdity — "the relative term stays under a printer's smallest mark
(0.01pt) until the coordinates pass ~`5.6e12`pt, **which is a page wider than the solar system**". The
`5.6e12`pt is right (finding 1's test measures it); the comparison is not. `5.629e12`pt is `1.986e9`m —
**1.43 times the diameter of the Sun**, and about a four-thousandth of the solar system out to Neptune's
orbit (`9.0e12`m). Expected: a comparison that survives a reader doing the arithmetic. Actual: it
overstates the size by ~4500x, in a doc block round 4 rewrote; I quoted the block in round 3 without
checking the astronomy, so it is my miss as much as theirs. **Not a defect**: the point of the sentence (a
page absurdly larger than anything a creator has) holds at either figure, and only the figure is wrong.
The honest version is one word: "wider than the Sun".

### Mutation table (round 4)

Production code was edited **one change at a time**, run, restored immediately, and verified — the
`sha256sum` of `validate/mod.rs` and `template/mod.rs` was recorded before the first run and re-checked
after every restore (both OK), and both are byte-identical to the revision hashed above. Suites:
**theirs** = `--lib` + `layer_rules_contract` + `panel_rect_contract` + `round4_layout_engineer`;
**r3**/**r4** = my two files.

| # | mutation | theirs | round 1's | round 2's | round 3's | round 4's |
|---|---|---|---|---|---|---|
| M-GUARD | `page`'s guard reverts to `<= 0.0 \|\| is_nan()` | **fail (1)** | pass (27) | pass (12) | **fail (1)** | **fail (2)** |
| M-SCALE | `magnitude` stops skipping non-finite coordinates | **fail (1)** | pass (27) | pass (12) | **fail (1)** | **fail (2)** |

Both mutations redden the implementer's file exactly as claimed, and **each also reddens three
reviewer tests** — so "1 failed" is true of their file and understates the guard's and the scale's
coverage: M-GUARD's other failures are mine (`a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults`,
`every_unusable_artboard_gets_one_skip_not_one_silence_per_cause`,
`the_model_names_a_non_finite_artboard_when_the_project_has_a_default_size`) and M-SCALE's are
`the_non_finite_scale_fix_removes_the_overflow_at_infinity_but_not_the_finite_ceiling`,
`every_unusable_artboard…`, and the round-3 test (whose new `faults` assertion was added this round
precisely so it bites here). **Nothing else in either file regressed** — the lib (172),
`layer_rules_contract` (8), `lettering_round1_reviewer` (31), `panel_rect_contract` (17),
`round1_reviewer` (27) and `round2_reviewer` (12) are green in both runs. In particular my round-3 pin
of the `page()`-level skip is what M-GUARD breaks, which is the honest statement of how far that pin
reaches: it holds the *policy*, and the policy is now justified in the file.

### What I tried to falsify and could not

- That the skip is a new silence rather than the degenerate one: five causes produce one identical list.
- That the guard's `is_finite()` arm is not equivalent-or-stronger: `NaN`, `±inf`, zero and inverted
  artboards are all skipped, and a legal one still reports geometry.
- That fixing `magnitude` narrowed the *finite* ceiling: it did not; the same two panels are reported at
  `1e307`pt and silent at `1e308`pt, both verified.
- That the two cost numbers were rounded favourably: `1.776e-3` and `5.629e12`, measured through
  `faults` at `5e12` (reported) and `6e12` (masked).
- That "between 8 and 16" is right only in the middle: it is exactly 8 at a power of two and approaches
  16 from below at the top of the binade.
- That the round-4 change broke a page this crate produces: every `plan`-path sweep from rounds 1–3 is
  still green, and the guard's error-kind equivalence from round 3 is untouched.

Unverified:
- Nothing about the two changed files, their four non-changed companions, or the workspace gate: `cargo
  xtask ci` was run this round and all 7 steps passed.
- `crates/comic-model`'s rule 15 was read and exercised from this crate's test, not reviewed as a
  change; the `default_page_size = None` half of its behaviour is pinned by my test but the rule itself
  is another agent's file.
- W3's trees (`src/lettering/**`, `src/fit/**`, `tests/lettering_round1_reviewer.rs`) — not W2's, read
  only, and green in every run above.
- The `1e308`pt finite ceiling is accepted as a residual on the strength of the arithmetic (a page that
  size, one ULP `1.995e292`pt) rather than on an end-to-end test: no such page exists in any fixture, and
  only its `faults`-level behaviour is measured here.

---

## Round 5 — the implementer closes the *finite* tolerance ceiling

Round 4 came back `approved` with two preferences, and the lead took one of them past the comment it
was raised as: the reviewer's "related, also recorded and not a finding" note under finding 1 is right,
and the residual it describes is a **defect**, not a property of a relative tolerance. Round 3 (and
round 4, which repeated it) argued the finite ceiling at ~`2.25e307`pt away with *"there one ULP is
`~2e292`pt, so no comparison between panels inside it can represent a defect to report"*. **That is
false and it was the wrong diagnosis.** The tolerance did not become useless because one ULP is large;
it became `inf`, because the product is evaluated left to right:

```text
TOLERANCE_ULPS * scale.abs() * f64::EPSILON     ==  (8.0 * scale) * EPSILON
8.0 * 2.25e307                                  ==  1.8e308   >  f64::MAX   →  +inf
```

`(8.0 * scale)` overflows **before** epsilon is ever multiplied in, so `GEOMETRY_TOLERANCE.max(inf)` is
`inf`, every threshold holds, and the geometry check is silently switched off — at a *finite* scale,
which is exactly the failure mode round 4 closed for a non-finite one. On a `2.25e307`pt page a
`5e299`pt overlap is perfectly representable (`5e299 / 4.0e292` is seven orders of magnitude past the
tolerance the reordered product gives) and it was **silently unreported**.

Revision measured:

| File | `sha256sum` |
|---|---|
| `crates/comic-layout/src/validate/mod.rs` | `8a71feaea125e94e76cd34e7d90805512e1a097fd2df43ac311c3b9261124f7c` |
| `crates/comic-layout/tests/round5_layout_engineer.rs` (new) | `23cb21d966711b6cd65f504b32c6331d3f6143871b5cd1159cd636b10bf0aa69` |
| `crates/comic-layout/tests/round4_layout_engineer.rs` (one stale assertion) | `cc7a3967daf5f7d006aa22eebc7d374ff8090cb8ba54640281ac0063a128230b` |
| `docs/reviews/M1-layout-engineer-panels.md` (this section + the nit in mine) | this file — a hash cannot name the revision that contains it; reported to the lead instead |

**The reorder, and why it is load-bearing.** `tolerance_at`'s body is now

```rust
GEOMETRY_TOLERANCE.max(TOLERANCE_ULPS * f64::EPSILON * scale.abs())
```

with a comment above it saying, in the file, that the order is not a style choice: the left-to-right
form overflows to `inf` for a finite `scale` past `f64::MAX / 8` (~`2.247e307`pt) and switches the
checks off silently, so a future reader must not "tidy" it back. The reordered product is
`8 * EPSILON` = `1.776e-15` first, so it is finite for **every** finite `scale` — even `f64::MAX · 8 ·
EPSILON` is `3.19e293`. The two orders are bit-identical for every ordinary page: equal at `1e3`, `1e5`,
`1e6`, `1e7`, `1e8`, `1e12`, and `tolerance_at(1e12)` is still exactly `0.0017763568394002505`, the
value the round-3 masking pin and `round4_layout_engineer.rs` both pin. Nothing ordinary moved.

**The test that reds before the reorder and passes after.**
`round5_layout_engineer.rs::a_representable_overlap_on_a_2_25e307pt_page_is_reported_not_silently_clean`
builds a `2.25e307`pt page (finite, so `magnitude` does not skip it) holding two panels sharing
`5e299`pt on both axes, and asserts the answer is `[Fault::Overlap { first: 0, second: 1 }]` — *not*
`Vec::new()`. It also keeps the mechanism visible (`8.0 * 2.25e307` really is `inf`), asserts the
tolerance is finite at `2.25e307`pt and at `f64::MAX`, and re-pins `tolerance_at(1e12)` to the same
literal. **Mutation evidence:** reverting *only* the reorder (`TOLERANCE_ULPS * scale.abs() *
f64::EPSILON`, everything else untouched) reddens this test and only this test —
`the tolerance at 2.25e307pt must be finite, not inf` — and the file was restored to the hash above
(re-checked after the restore).

**The `magnitude` doc, re-audited.** The residual paragraph no longer claims a ceiling. It used to say
a page past ~`2.25e307`pt "still overflows, which is the ceiling round 3 recorded … so no comparison
between panels inside it can represent a defect to report". It now says **there is no finite ceiling**,
records that the round-3/4 argument was wrong and *why* (`8.0 * scale` overflowed before the multiply by
epsilon, so a representable `5e299`pt overlap went unreported), and states that after round 5 the
tolerance is finite at every finite magnitude — so the only skip `magnitude` makes is for a coordinate
that is not finite at all. The **non-finite** skip itself is unchanged and still approved: it is round
4's, and it is what keeps an infinite page from sizing an infinite tolerance.

**The two false comments.**

1. `TOLERANCE_ULPS`' doc (`validate/mod.rs:132`) said `5.6e12`pt "is a page wider than the solar
   system". `5.629e12`pt is `1.99e9`m — **1.43 Sun diameters**, and about a four-thousandth of the
   solar system out to Neptune. It now reads: *"the relative term stays under a printer's smallest mark
   (`0.01`pt) until the coordinates pass ~`5.6e12`pt — a page `1.99e9`m across, **wider than the Sun**
   (`1.39e9`m across) and still absurd next to anything a creator has."* The reviewer's own miss, and
   it flagged itself.
2. `page`'s guard comment (`validate/mod.rs:367`) justified the non-finite skip with *"`faults` cannot
   size a tolerance from an infinite magnitude, and handing it one would satisfy every threshold — the
   geometry check would be silently off rather than deliberately not made"*. **Round 4 made that
   sentence false**: `magnitude` skips a non-finite coordinate, so `faults` *can* be handed an infinite
   page and answers it correctly (the reviewer's test contrasts the two). The skip is still right, but
   it is now stated as a **deliberate choice resting on consistency with the degenerate skip** (missing
   artboard, zero area, inverted — a page that is not a rectangle has no rectangle to measure a panel
   against) **and on `comic-model` owning the malformed artboard** (rule 15 against `default_page_size`;
   design §12.1 gives "unsupported page dimensions" to the model), rather than on a tolerance
   overflowing two functions away.

**Record nit — fixed.** My own round-4 section below cited the round-3 test's old name
(`an_infinite_artboard_silently_switches_the_geometry_check_off`). It now cites the reviewer's name,
`a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults`. Only my section was edited.

**Mutation evidence (round 5).**

| Mutation | `round5_layout_engineer.rs` |
|---|---|
| **M-ORDER** — `tolerance_at` reverts to `TOLERANCE_ULPS * scale.abs() * f64::EPSILON` | 1 failed (`a_representable_overlap_on_a_2_25e307pt_page_is_reported_not_silently_clean`) |

Applied to the working tree, the file run, the file restored; the restored tree is the revision hashed
above. The other 288 tests are green under M-ORDER, which is the point: the old order was invisible to
every test that existed before this round, because the only page sizes it changes the answer for are
past `2.247e307`pt.

**Commands, as run** (toolchain `cargo 1.99.0 (5f94df478 2026-08-27)`; `/usr/bin/cargo` is 1.75 and
cannot parse `resolver = "3"`, so `~/.cargo/bin/cargo` is the one used):

```
$ cargo fmt --all --check -p vectorcraft-comic-layout                 → exit 0
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     unittests src/lib.rs                          ok. 172 passed; 0 failed
     tests/layer_rules_contract.rs                 ok.   8 passed; 0 failed
     tests/lettering_round1_reviewer.rs            ok.  31 passed; 0 failed
     tests/panel_rect_contract.rs                  ok.  17 passed; 0 failed
     tests/round1_reviewer.rs                      ok.  27 passed; 0 failed
     tests/round2_reviewer.rs                      ok.  12 passed; 0 failed
     tests/round3_reviewer.rs                      ok.  12 passed; 0 failed
     tests/round4_layout_engineer.rs               ok.   4 passed; 0 failed
     tests/round4_reviewer.rs                      FAILED. 4 passed; 1 failed
       └ the_non_finite_scale_fix_removes_the_overflow_at_infinity_but_not_the_finite_ceiling
     tests/round5_layout_engineer.rs               ok.   1 passed; 0 failed
     Doc-tests                                     ok.   0 passed; 0 failed
   **288 passed, 1 failed** — the one failure is the reviewer's round-4 record assertion, below.
$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings  → clean (exit 0)
$ cargo xtask layers                                                     → OK: 29 crates, no layering violations.
```

**Fifth finding — the reviewer's round-4 test records the residual that round 5 closes. Reported, not
fixed (the constraint is explicit that no `round*_reviewer.rs` is to be edited).**
`crates/comic-layout/tests/round4_reviewer.rs::the_non_finite_scale_fix_removes_the_overflow_at_infinity_but_not_the_finite_ceiling`
asserts the *closed* behaviour in two places: the table row `(1e308, false, true, …)` (the overlap is
*not* reported, and the tolerance *is* infinite) and `assert!(tolerance_at(ceiling * 1.01).is_infinite())`.
Both fail under the reorder, by design — the test's own comment already calls the `1e308`pt block
"Recorded, not asserted as correct", so this is the record being overtaken, not a weakened guarantee.
The minimum update for whoever owns that file: rename it (the "not_the_finite_ceiling" half is now
wrong) and change that row to `(1e308, true, false, …)` and the `ceiling * 1.01` line to `is_finite()`.
Nothing else in the file is affected — `every_unusable_artboard_gets_one_skip_not_one_silence_per_cause`
still passes, because its `1e308`pt artboard holds non-overlapping fixture panels, so the geometry
answer there is unchanged; and `the_ulp_multiple_is_eight_at_a_power_of_two_and_stays_under_sixteen_above_it`
and `the_documented_cost_of_one_scale_per_call_is_the_measured_one` are untouched by the reorder. This
is the only thing the two constraints "suite green" and "do not edit a reviewer file" cannot both
satisfy, and it is a coordination item rather than a new defect. The lead can either authorise the
two-line change or leave it to the reviewer's round 5, as round 4 did with `round3_reviewer.rs`.

Beyond that, no other finding. What was read again and left alone is the `TOLERANCE_ULPS * scale` pairing
with a *per-call* scale (round 4's finding 1, unchanged): the reorder does not affect which number is the
scale, only whether the multiplication can overflow.

**Unverified:** `cargo xtask ci` end to end was not run (it also runs the whole workspace and
`assets`/`brands`/`wasm`, and other agents are mid-edit elsewhere); the crate's own 289 tests, its
clippy and `cargo xtask layers` were run. The one non-green target is the reviewer's record assertion
above. No page of this size exists in any fixture, so the defect is measured at `faults` (and through
`page`'s artboard is finite, so the same page *is* handed to `faults`) rather than end to end from a
bundle.

---

## Round 5 — the finite ceiling was a defect, not a residual

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash`, the role
               file expects that model, and this session's own event log
               (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`) carries
               `"model":"deepseek-v4.1-flash"`. No waiver needed.
Revision reviewed: **working tree (base `16594c58`) after round 5's product reorder** — `validate/mod.rs`
               `8a71feaea125e94e76cd34e7d90805512e1a097fd2df43ac311c3b9261124f7c`;
               `template/mod.rs f8807972…`, `panel/reading_order/layout/lib.rs` and `tests/round1..3_reviewer.rs`
               (`d9385c0c…`, `724357f1…`, `23ed989e…`) unchanged from round 4;
               `tests/round4_layout_engineer.rs` moved with the change (`c211b272…` → `cc7a3967…`);
               `tests/round5_layout_engineer.rs` (the implementer's, new) `23cb21d966711b6cd65f504b32c6331d3f6143871b5cd1159cd636b10bf0aa69`.
               **I changed my own `tests/round4_reviewer.rs` this round** — it was `bae176d4…` and is now
               `ab50035073f325ad9e8b77f9e951e06e768e37707b3f381033dfcb60a5f56f4e` (one test renamed, one
               table row and one assertion flipped, the comment rewritten).
               Files: `crates/comic-layout/src/validate/mod.rs`, the four unchanged sources,
               `tests/round{1,2,3,4,5}_reviewer.rs`, `tests/round4_layout_engineer.rs`.

Verdict:       **approved** — **no findings**. Round 4's second preference was a *defect* and I under-called
               it; round 4's first preference was answered by fixing the mechanism rather than the sentence,
               which is the right way round; the reorder is correct, bit-identical where it must be, and
               pinned by three tests in two files, none of which the implementer had to touch in mine.

### What I changed in my file

The one red test was mine, and it asserted the residual this round removes. Its own comment already said
"Recorded, not asserted as correct", so the fix is a correction rather than a weakening:

- **Renamed** `the_non_finite_scale_fix_removes_the_overflow_at_infinity_but_not_the_finite_ceiling` to
  `the_tolerance_is_finite_at_every_finite_scale_and_non_finite_only_for_a_non_finite_one` — the old name
  was false in both halves: the fix is not only about infinity, and there is no finite ceiling any more.
- **Flipped** the table row `(1e308, false, true, "finite and past the overflow: the term is `inf` …")` to
  `(1e308, true, "finite and far past it")`, and `assert!(tolerance_at(ceiling * 1.01).is_infinite())` to
  `is_finite()`. I did not merely invert the boolean: the test now writes **both orders out** and asserts
  they agree at eighteen ordinary scales (including both sides of the `5.63e5`pt crossover) and that the
  old one is `inf` exactly where the new one is finite, so the reorder cannot be "tidied" away without a
  test saying what it costs.
- New assertions in the same test: `tolerance_at(f64::MAX)` is finite and in `3.1e293 … 3.3e293`
  (`3.1933e293` exactly); `tolerance_at(1e12) == 0.0017763568394002505` to the last digit; the `5e299`pt
  overlap is **reported** on `inf`, `1e307`, `2.25e307`, `1e308` and `f64::MAX` pages; `tolerance_at(±inf)`
  is still `inf` and `NaN` still lands on the floor.
- **The comment** now says *why* the ceiling was there: the product parsed left to right, so `8.0 * scale`
  overflowed to `+inf` for a finite `scale` past `f64::MAX / 8` (`2.2471164185778946e307`pt) **before**
  epsilon was multiplied in — a finite input switching the geometry check off, which is round 4's hole by
  another route. Round 3 mistook it for a resolution limit ("one ULP there is `~2e292`pt, so no defect is
  representable"); my round-4 finding said the comment was too generous, and it was the *mechanism* that was
  wrong.

### The reorder, verified rather than taken

- **Bit-identical at ordinary scales.** `8 * EPSILON` is `2^-49` and both products are then single
  power-of-two scalings, so the two orders agree exactly wherever neither overflows: checked at
  `0, 1e-3, 1, 612, 1e3, 1e5, 4.8e5, 5.6e5, 1e6, 3.7e6, 1e7, 1e8, 1e10, 1e12, 1e15, 1e100, 1e307` and the
  threshold itself, and the pinned `0.0017763568394002505` at `1e12` is unchanged.
- **Finite at `f64::MAX`:** `tolerance_at(f64::MAX) == 3.1933444952555513e293`, finite — so no finite page
  has a ceiling.
- **The defect is closed:** a `5e299`pt-deep two-axis overlap is one `Overlap` on pages of `1e307`,
  `2.25e307`, `1e308` and `f64::MAX` pt, where `1e308` and `f64::MAX` used to be silent.
- **Non-finite scales unchanged:** `tolerance_at(±INFINITY)` is still `inf`, which is why round 4's
  `magnitude` skip is still load-bearing — the two halves of the fix are independent and both needed.

### M-ORDER, re-run

| # | mutation | their round-5 | their round-4 | round 3's | round 4's | elsewhere |
|---|---|---|---|---|---|---|
| M-ORDER | the product reverts to `TOLERANCE_ULPS * scale.abs() * f64::EPSILON` | **fail (1/1)** | **fail (1/4)** | pass (12) | **fail (1/5)** | all green |
| M-CLAMP | old order, `inf` clamped to `1e9` instead of reordering | **fail (1/1)** | **fail (1/4)** | **fail (1/12)** | **fail (1/5)** | all green |

One change at a time, restored immediately, `sha256sum`-verified against the recorded baseline both times;
the revision is byte-identical to the hash above. `M-ORDER` reddens exactly the implementer's new test —
which is what they claimed — plus the two reviewer copies of the same claim: their own round-4 test, which
they updated this round from "`8 * f64::MAX * EPSILON` is infinite" to "`tolerance_at(f64::MAX)` is finite"
(so the reorder is now pinned twice in their file), and mine. Nothing else in the crate regressed: the lib
(172), `layer_rules_contract` (8), `lettering_round1_reviewer` (31), `panel_rect_contract` (17),
`round1`/`round2`/`round3_reviewer` (27/12/12) and their round-4 file are green in both runs.
`M-CLAMP` is my own probe of the *test* rather than of the code: it is a plausible wrong fix that keeps the
hole open at every scale past the threshold, and their test catches it — because it pins the tolerance's
**value** just past the threshold (`3.996802888650564e292` at `2.25e307`), not merely that it is finite. My
round-3 test catches it on the other side (`tolerance_at(inf)` must stay `inf`), and mine on a second (the
value at `f64::MAX`).

### Is their round-5 test honest?

**Yes.** `a_representable_overlap_on_a_2_25e307pt_page_is_reported_not_silently_clean` fails for the right
reason under M-ORDER — `tolerance_at(2.25e307)` is `inf` where it asserts finite-and-equal to
`8 · EPSILON · 2.25e307` — and it cannot pass with the hole open at the magnitude it probes, because the
hole *is* an infinite tolerance there and the assertion is on the value, backed by a direct behavioural
check (the `faults` call on a `2.25e307`pt page with a `5e299`pt shared depth). It states the overflow as
arithmetic (`(8.0 * 2.25e307).is_infinite()`) rather than as prose, and it survives M-CLAMP, so it is not
satisfied by a cosmetic fix. Its one soft spot: the equality `tolerance == TOLERANCE_ULPS * f64::EPSILON *
2.25e307` is written in the implementation's own expression, so on its own it is a restatement — it is
carried by the `is_finite()` assertion and the behavioural one next to it, which is why it still bites, and
I would leave it as it is.

### Findings

**None.** One wording note, not a finding and not worth a round: `tolerance_at`'s new doc sentence "A
non-finite `scale` has no ceiling — but a **finite** one always does not, at any magnitude" is a garden path
("a finite one never does" is what it means, and the claim itself is verified). Everything else read this
round is true as written, including `TOLERANCE_ULPS`'s corrected astronomy ("`1.99e9`m across, **wider than
the Sun** (`1.39e9`m across)") — round 4's second preference, fixed to the metre.

### Commands, as run

```
$ cargo fmt --all --check                              → exit 0
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout
     unittests src/lib.rs                     → ok. 172 passed; 0 failed
     tests/layer_rules_contract.rs            → ok. 8 passed; 0 failed
     tests/lettering_round1_reviewer.rs       → ok. 31 passed; 0 failed
     tests/panel_rect_contract.rs             → ok. 17 passed; 0 failed
     tests/round1_reviewer.rs                 → ok. 27 passed; 0 failed
     tests/round2_reviewer.rs                 → ok. 12 passed; 0 failed
     tests/round3_reviewer.rs                 → ok. 12 passed; 0 failed
     tests/round4_layout_engineer.rs          → ok. 4 passed; 0 failed
     tests/round4_reviewer.rs                 → ok. 5 passed; 0 failed
     tests/round5_layout_engineer.rs          → ok. 1 passed; 0 failed
   Doc-tests vectorcraft_comic_layout         → ok. 0 passed; 0 failed
   **289 passed, 0 failed**
$ cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings   → clean
$ cargo xtask layers                                                      → OK: 29 crates, no layering violations.
```

Unverified:
- `cargo xtask ci` was **not** re-run this round (it was run in round 4 and all 7 steps passed); it also
  builds `crates/engine`, which W4 is editing concurrently, so a failure there would not have been this
  crate's.
- `crates/engine` and the other consumers of `tolerance_at` were not re-read: the function's signature and
  its values at every ordinary scale are unchanged, so no caller can see the reorder, but I checked that by
  the reorder's bit-identity rather than by reviewing the callers.
- `crates/comic-model`'s rule 15 — read in round 4, not reviewed as a change.
