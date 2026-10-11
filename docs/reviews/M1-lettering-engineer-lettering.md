# M1 — code-reviewer, `crates/comic-layout/src/{fit,lettering}/**` (W3)

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. The environment states
               `COPILOT_MODEL=deepseek-v4.1-flash` — an in-environment statement, not only a harness
               label. (`~/.copilot/session-store.db :: assistant_usage_events` has no row for this
               session, so there is no second corroboration; the sibling `crates/comic-model` record
               notes the same gap. Recorded as the model I ran on, with the gap stated rather than
               papered over.)
Revision:      working tree (base `16594c58`) — `crates/comic-layout/src/{fit,lettering}/` are new,
               untracked files; `crates/comic-layout/Cargo.toml` is modified.   Files:
               `src/fit/{mod,sizing,policy}.rs`, `src/lettering/{mod,outline,tail,style,fonts,plan}.rs`,
               `crates/comic-layout/Cargo.toml` (the `vectorcraft-text` line),
               `crates/comic-layout/tests/lettering_round1_reviewer.rs` (new: the reviewer's tests)
Design refs:   design §5.2 (`Dialogue`, `Lettering`), §5.2.1 (link ownership), §6.1 (one owner for
               the text — the visual object is a rendering), §8.3 (lettering: read-back fields, the
               overflow policy, "reported, not applied", the missing-font report), §12.1 (text
               overflow and missing lettering font are deterministic *checks*), §4 (determinism);
               schema §5.5 (`OverflowPolicy`, `LetteringKind`), §6.3, §6.4, §6.6
               (`TextLayoutSettings` is inputs only), §13 rules 7–8 and 12; porting audit §8
               (typography) and §12.5 (no comic lettering face is available).
Toolchain:     `cargo 1.99.0` / `rustc 1.99.0` (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu`).
Scope:         W3's two module trees only. `src/{layout,panel,template,reading_order,validate}/**` is
               W2's (`layout-engineer`) and `src/lib.rs` / `PanelRect` are the lead's; neither is
               reviewed here. The `round1_reviewer.rs` integration file in this crate is the *other*
               reviewer's W2 file, and its three failures are W2 findings, not W3's.

## Round 1 — correctness

Verdict:       **changes-requested**

Unit tests:
- Added (new file): `crates/comic-layout/tests/lettering_round1_reviewer.rs` — **25 tests: 18 pass,
  7 fail**. The seven failures are the `finding_1`–`finding_7` tests; each encodes behaviour the
  design, the frozen schema or the module's own documented contract requires, and each is the
  minimal reproduction for a finding below — none of them needs weakening.
- The implementer's own suite is untouched and green: **180 tests** (155 inline in `src/lib.rs`,
  8 `layer_rules_contract`, 17 `panel_rect_contract`). 89 of the 155 inline tests are W3's own, which
  matches the implementer's report of "3339 lines, 89 new unit tests, 180 in the crate".
- Command, the gate's own configuration (`CRAFT_FONTS_DIR` unset), `--no-fail-fast`:
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/lettering_round1_reviewer.rs
test result: FAILED. 18 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/round1_reviewer.rs
test result: FAILED. 23 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: 2 targets failed:
```
  Every implementer test passes; the two failing targets are the two reviewers' files. The
  `round1_reviewer.rs` target is W2's.
- `env -u CRAFT_FONTS_DIR cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings` →
  **0 diagnostics** (`Finished dev profile … in 0.98s`), including the new test file. Two lint
  failures in my first draft were mine and are fixed (`allow-expect-in-tests` covers `#[test]` bodies
  but not free helpers, so the limits helper uses `unwrap_or`).
- `cargo xtask layers` → `OK: 29 crates, no layering violations.`
- **The other configuration.** `CRAFT_FONTS_DIR=/home/hvt/web/comiclab.io.vn/craft-fonts` (a real
  checkout, Comic Neue Regular/Bold/Italic and Bangers Regular present), in a throwaway
  `CARGO_TARGET_DIR`, run once and then removed:
```
$ CRAFT_FONTS_DIR=/home/hvt/web/comiclab.io.vn/craft-fonts CARGO_TARGET_DIR=…/target/fonts-check \
    cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/lettering_round1_reviewer.rs
test result: FAILED. 16 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```
  So the implementer's 155 inline tests pass in **both** configurations, and every reproduction I had
  written by then (six of them; F7 and the attachment sweep came later) failed identically in both.
  What changes: nothing in the suite's verdict, and — importantly —
  `the_missing_face_report_is_conditional_on_the_face_really_being_absent` takes its *other* branch
  and asserts **no** report for all six kinds, which is the whole point of a conditional test.
- Build failures that were **not W3's**, met and re-run as instructed: `crates/comic-model/src/lib.rs`
  and `entities.rs` failed to compile twice mid-review (`expected ']'`, `unused import: entities::*`)
  and `crates/comic-layout` failed to compile once for the same reason while I was making the
  mutation analysis — the other agent working in `comic-model`. Each cleared on re-run; no W3 defect
  was reported from it.

Findings:      **8 — 5 bug, 3 preference.** (F1–F7 have a failing reproduction; F8 does not.)

**F1 — `bug`** — `crates/comic-layout/src/fit/policy.rs:140` and `:152` (`report_with`, reached from
`resize`, `:102`).
Trigger: a `resize_frame` frame that cannot grow — a blocked grow (an obstacle), or nothing within
the limits fitting. Expected: `FitDecision::measured` is "the measurement behind the decision, **in
`bounds`' frame**" (the field's own doc, `policy.rs:34`) — and the `LetteringPlan` built from it
reports `text_bounds` as "where the text actually landed". Actual: `size_to_text` measured the text
inside **the largest box the limits allow** before returning `Blocked`/`Unfittable`, so
`decision.measured.frame` is the maximum box's frame while `decision.bounds` is the frame the caller
gave. Measured: `bounds = Rect{180,140,228,172}` but `measured.frame = Rect{77.36,50.46,330.64,261.54}`
(the 360×300 box's frame). `Measured::needed_height` is derived from that frame, so a caller reading
either gets geometry for a box nobody is writing; and the plan's `text_bounds` is the text's
position in a box the plan does not describe. Only the blocked/unfittable path is affected; the
`Fits`, `Warned`, `Shrunk` and `Resized` paths hold the invariant, which my test asserts case by case.
Reproduction: `finding_1_the_measurement_must_be_taken_in_the_frame_of_the_box_that_comes_back`.

**F2 — `bug`** — `crates/comic-layout/src/fit/mod.rs:149` (`shrink_percent`), `:170` (`validate`),
`fit/policy.rs:169` (`shrink_message`), `fit/mod.rs` `area_fit`.
Trigger: a `FitTextStyle` whose `shrink_percent` is NaN, or outside `10..=100`. Expected:
`FitTextStyle::validate` is "the numbers a layout call needs, so a bad style is an error rather than
a NaN that spreads" — and the finding must state the policy the engine was actually given. Actual:
`validate` never looks at `shrink_percent`; `area_fit` hands it to `AreaFit::parse`, which clamps into
`10..=100` (`crates/doc/src/text.rs:767-784`) and replaces a non-finite value with 50, while
`shrink_message` quotes the *style's* number as "the smallest size the policy allows":
```
NaN:        "…at 50% — the smallest size the policy allows (NaN) — so it was left as it is"
1.0:        "…at 10% — the smallest size the policy allows (1) — so it was left as it is"
-5.0:       "…at 10% — the smallest size the policy allows (-5) — so it was left as it is"
1_000_000_000: "…at 100% — the smallest size the policy allows (1000000000) — …"
```
  So a user-visible finding can contain `NaN`, and for every out-of-range value the reported policy
  is not the applied one. Reachable from data: `TextLayoutSettings.padding`/`size` come from the
  record, and `LetteringStyle` comes from `ProjectSettings.default_lettering_style` (schema §5.3).
  (Only the *failing* shrink prints the parenthetical: when the engine finds a scale that fits there
  is no finding, and `fit_scale` is then the scale the engine found, not the floor — my test asserts
  the floor only when `fits` is false.)
Reproduction: `finding_2_a_shrink_percent_outside_the_engines_range_is_neither_validated_nor_reported_honestly`.

**F3 — `bug`** — `crates/comic-layout/src/lettering/plan.rs:234` (`moved_by`).
Trigger: `moved_by` on a plan whose outcome is `FitOutcome::Resized { bounds }`. Expected: a moved
plan's public data agrees with itself; `outcome` is documented as "how it resolved" and the plan's
comment justifies copying `findings` on the grounds that "the findings name sizes and policies, never
coordinates". Actual: `outcome` **does** name a rectangle for `Resized`, and it is copied verbatim,
so after a move `plan.outcome == Resized { bounds: pre-move box }` while `plan.bounds` is the moved
box. Measured: outcome `Rect{68.6,118.5,331.4,181.5}` against `bounds Rect{188.6,38.5,451.4,101.5}`
for `delta = (120, −80)`. A caller that re-derives the frame from the outcome (or that renders it)
writes the balloon where it used to be.
Reproduction: `finding_3_a_move_must_not_leave_the_outcome_naming_the_box_it_left`.

**F4 — `bug`** — `crates/comic-layout/src/lettering/plan.rs:329-337` (`ruler_findings`), with
`plan.rs:374` (`assemble`, `style.tail`), reached through `plan.rs:170` (`with_style`).
Trigger: `LetteringRequest::new(LetteringKind::Speech, …).with_style(style_of(LetteringKind::Narration))`
— or any style whose `tail` is `None` for a spoken kind (a project's `default_lettering_style`, which
`with_style` exists for). Expected: either a tail or a finding. The module head states that "`tail`
is present exactly for the kinds that have someone to point at", and schema §6.4 makes
`tail_node_id` "required for kinds that have a tail", so a `speech` plan with `tail: None` is a
record the command layer cannot legally write. Actual: `assemble` uses `style.tail` (the caller's),
`plan.tail` is `None`, and `ruler_findings` reports only the *missing target* (`:331`,
`style.tail.is_some() && tail_target.is_none()`) — never a missing tail *kind*. `findings` is `[]`
apart from the CRAFT_FONTS report. The asymmetry is the point: a balloon with nobody to point at is
reported; a balloon with no tail at all is not.
Reproduction: `finding_4_a_spoken_kind_with_a_tailless_style_must_be_reported`.

**F5 — `bug` (low impact)** — `crates/comic-layout/src/fit/mod.rs:232` (`Measured::bottom`).
Trigger: a frame too narrow for one character — the state this module's own doc names ("a frame too
narrow for even one character drops the line … with no lines at all in the result"), and the one
`Measured::fits` relies on. Expected: `needed_height()`, documented as "the frame height this block
needs", is a height. Actual: with no lines, `bottom()` falls back to `layout.bounds.y1`, which is
`0.0` in text space rather than the frame's own top, so `needed_height()` returns a **negative**
number: `−293.0` for a 400×300-origin frame. No plan is affected — `size_to_text` clamps `needed`
into the limits, and `heights_from` starts at the minimum — but `Measured` and `needed_height` are
public, and the naive caller reads nonsense. (The empty string is *not* affected: the engine places
one empty line for it, so `needed_height()` is positive; my test asserts both.)
Reproduction: `finding_5_the_height_a_frame_that_places_no_lines_needs_is_not_negative`.

**F6 — `bug`** — `crates/comic-layout/src/lettering/tail.rs:136-141` (`attach_point`), used by
`Tail::from_outline` and therefore by `Tail::new`, `after_balloon_move`, `after_outline_move`,
`retarget`.
Trigger: `Tail::new(bounds, shape, kind, Point::new(f64::NAN, f64::NAN))` — a non-finite target, with
a perfectly good outline. Expected: `attach_point` is "The point of `outline` nearest `tip`"; the
fallback is documented for "when the outline has no segments at all, so a degenerate balloon still
yields a connected path". Actual: the fallback is taken for a non-finite tip too, and it returns
`bounds.center()` — which is inside *all four* outlines, not on any of them. So the constructor
builds a tail its own `is_attached_to` calls detached:
```
Ellipse Wedge:  attach (160, 140) is 40.0 from the outline
Cloud   Wedge:  attach (160, 140) is 32.256 from the outline
Box     Wedge:  attach (160, 140) is 40.0 from the outline
Burst   Wedge:  attach (160, 140) is 31.2 from the outline
```
  (same for `Bubble`), against `is_attached_to_outline`'s `distance < 1e-6` — 32 to 40 points out.
  It contradicts the module head ("the base follows the balloon's edge wherever the balloon goes"),
  `is_attached_to`'s doc ("the attachment is a point of the outline to within a hair, so a tail can
  never have been left behind") and the name of the accessor. `plan.rs`'s `assemble` filters a
  non-finite target, so no *plan* is wrong; the defect is in the public `Tail` API that `moved_by`
  and the UI's tail handles build on. The correctly-detached case is worth noting: the implementer's
  `a_nan_target_is_carried_as_a_finite_fallback_not_as_a_broken_path` **asserts the box centre**, so
  the wrong answer is pinned by a test rather than merely produced.
Reproduction: `finding_6_a_tail_built_from_a_non_finite_target_must_still_touch_its_balloon`.

**F7 — `preference`** — `crates/comic-layout/src/lettering/fonts.rs:83-102` (`missing_faces`).
Trigger: a project whose `text_layout_settings.font` is `sans-serif` (schema §6.6 allows any family
name), or a name longer than 256 bytes. Expected: the module's claim is that a project's lettering
report and an export's font warning "name the same thing", and `MissingFace` "field for field what
`text.missingFonts` reports". Actual: the four fields, the `db.resolve` + `FontMatch::Exact` test,
the sort/dedup and the 1000 cap all match the exporter exactly — but the exporter also leaves out the
CSS generic families (`crates/engine/src/cmd/fontfiles.rs:25-37`, `:111`) and names longer than 256
bytes (`:21`), and `missing_faces` filters only blank names. So the lettering reports twelve faces
the exporter stays silent about:
```
the lettering report names ["cursive","emoji","fantasy","math","monospace","sans-serif",
                            "serif","system-ui","ui-monospace","ui-rounded","ui-sans-serif","ui-serif"]
```
Reproduction: `finding_7_the_face_report_leaves_out_what_the_exporter_leaves_out`.

**F8 — `preference`** — `crates/comic-layout/Cargo.toml:19`.
The manifest comment lists `thiserror — the template and plan error types (M1)`. `thiserror` is used
only by W2's trees (`src/template/error.rs:11`, `src/layout/error.rs:12`); W3's `fit::FitError` is a
hand-written `Display` + `std::error::Error` (`fit/mod.rs:109-125`) — which is fine, and arguably
better than a derive here given the variant set — but the comment attributes it to the plan, so the
one manifest line in W3's scope describes W3 wrongly. No test can fail on a comment; this is on the
record so the owner can fix the sentence.
No reproduction test.

Tests that cannot fail: **three, all in the implementer's suite, all positive-only.**
1. `lettering::tail::tests::a_tail_that_would_be_left_behind_is_re_attached_on_every_edge` and
   `a_tail_starts_on_the_balloons_outline_and_ends_at_its_target` assert *only* that tails are
   attached. Nothing in the suite ever asserts that a tail is **not** attached, so
   `is_attached_to_outline` could `return true` and every one of their attachment tests would still
   pass. My `attachment_is_measured_against_the_path_not_the_box` falsifies it (a base 0.25 pt off
   the outline is rejected).
2. `fit::tests::needed_height_is_the_distance_from_the_cell_top_to_the_last_descent` *names* the
   last descent but never asserts it: it checks that two origins agree and that a narrower frame is
   taller. Replacing `bottom()` with `layout.bounds.y1` (the ink bottom) passes it. My
   `a_measurement_quotes_the_frame_it_was_measured_in_and_fits_is_overflow` asserts
   `bottom() == baseline + descent` exactly.
3. `lettering::plan::tests::the_plan_never_carries_the_text` reads the plan's `Debug` output and asks
   whether it contains the words. That is a real check on a `#[derive(Debug)]` struct and it does
   fail if a `String` field is added — but it tests the formatter, not the type. The type-level fact
   is stronger and is stated in the record instead: every field of `LetteringPlan` is a rectangle, a
   `PathData`, an enum, `TextLayoutSettings`, `AreaFit`, `Tail` or `Vec<Finding>` — non-`String`
   throughout except `text_layout.font` (a face name) and `findings` — so there is no place for a
   second copy of the words to live. I did not add a second test for this: the property is a reading
   of the struct, and a test of it would restate the implementation.

Mutation table: **derived by inspection of the implementer's tests, not executed.** The task's
constraint is explicit — test code only, no production edits — so no in-place mutation was run;
every row below is the test that *would* catch it and *why it would*, which the owner can check
against the names given. "Mine" is `tests/lettering_round1_reviewer.rs`.

| # | Mutation | Their suite | Mine |
|---|---|---|---|
| 1 | `text_frame` pre-reduces the padding (padding twice) | caught — `the_text_frame_is_the_inscribed_rectangle_and_the_padding_is_the_layouts_job` pins the exact rectangle | caught — `the_padding_reaches_the_text_area_exactly_once` |
| 2 | `measure` stops setting `LayoutOptions::inset` from the style | **not caught** — no test asserts where the text lands | caught — same test (a left-aligned line starts one padding in) |
| 3 | `needed_box` without `/inner_scale` (the bug the implementer says it fixed) | caught — `the_box_that_comes_back_is_tight_enough_to_matter` (the loose box fails the 20%-smaller probe) | caught — `a_sentence_balloon_does_not_jump_to_the_kinds_maximum_height` (the loose search returns the maximum height) |
| 4 | `needed_box` with the division done twice | **not caught** — `an_empty_string_gets_the_smallest_box_the_kind_allows` clamps a small need to the minimum either way | caught — `the_height_a_short_line_needs_does_not_jump_to_the_kinds_maximum` pins `"Hi!"` at exactly the minimum height and width |
| 5 | `bottom()` = ink bottom (`bounds.y1`) | **not caught** — row of "tests that cannot fail" #2 | caught — `a_measurement_quotes_the_frame_it_was_measured_in_and_fits_is_overflow` |
| 6 | `fits()` = `lines.is_empty()` (or a bounds test) | caught — `a_frame_too_narrow_for_one_character_is_overflow_with_no_lines_at_all`, `empty_text_is_laid_out_and_fits_any_frame` | caught — the same equality `fits() == !overflow` |
| 7 | `overlaps` uses `<=` (a shared edge collides) | caught — `an_edged_touch_is_not_a_collision` | caught — `an_obstacle_that_only_touches_the_frame_does_not_block_a_grow` |
| 8 | `is_attached_to_outline` always `true` | **not caught** — row #1 above | caught — `attachment_is_measured_against_the_path_not_the_box` |
| 9 | `check_source` dropped from `refit_balloon` only | **not caught** — `the_text_source_must_match_the_kind` calls `new_balloon` only | caught — `the_text_source_must_match_the_kind_for_every_kind_through_both_entry_points` |
| 10 | `policy::resize` applies a blocked grow | caught — `resize_frame_that_would_overlap_another_object_is_reported_not_applied` | caught — the same, plus `Blocked { obstacle }` at the `size_to_text` level |
| 11 | `moved_by` does not translate `text_frame` | caught — `moving_a_balloon_moves_its_frame_and_keeps_its_tail_on_it` | caught — `the_plan_hands_on_an_editable_closed_outline_and_a_translated_frame_path` |
| 12 | F1: `measured` taken in the largest box | **not caught** — the blocked/unfittable tests assert `bounds`, `outcome` and findings, never `measured` | caught — `finding_1_…` |
| 13 | F2: `shrink_percent` unvalidated | **not caught** — `shrink_that_cannot_help_…` uses the in-range 90 | caught — `finding_2_…` |
| 14 | F3: `moved_by` copies `outcome` | **not caught** — no test inspects `outcome` after a move | caught — `finding_3_…` |
| 15 | F4: a tail-less style for a spoken kind is silent | **not caught** — the tail tests use the default styles, whose tails all match | caught — `finding_4_…` |
| 16 | F5: the no-lines fallback | **not caught** — `needed_height_…` checks relative properties only | caught — `finding_5_…` |
| 17 | F6: the `attach_point` centre fallback | **caught, in the wrong direction** — `a_nan_target_is_carried_as_a_finite_fallback_not_as_a_broken_path` asserts the centre, so fixing the bug breaks their test | caught — `finding_6_…` |
| 18 | `count_for`'s clamp removed | caught — `counts_are_clamped_and_never_panic_on_hostile_input` | not covered (a panic-free clamp, not a decision about text) |
| 19 | `Limits::within` uses `clamp` instead of `max().min()` | **not caught** — `limits_are_narrowed_…` only uses ordered limits | not covered |

Rows 17 and the "caught, in the wrong direction" cell are the two that worry me most: one of the
implementer's tests locks in a defect, and eleven of their tests assert attachment without ever
falsifying it.

Fitting: the read-back fields are the right ones for the right decisions, and the two engine
behaviours are true and pinned.
- **`overflow`** answers "does it fit" (`Measured::fits`, `fit/mod.rs`), and it is the field design
  §12.1 names for the overflow check — so a balloon's answer and a validation report's answer are the
  same answer rather than two implementations.
- **`bounds`** answers "where is the text" (for the caller and for `text_bounds`) and, at
  `sizing.rs:241-244`, "how wide is the text really" — the preferred width is the natural ink extent
  at the widest allowed frame, which is the only honest way to size a balloon to text without
  writing a layout loop. `bounds` is also what design §8.3 names for the auto-resize path.
- **`fit_scale`** answers "how far did shrink go", read at `policy.rs:90` and surfaced in
  `FitOutcome::Shrunk` — again the field §8.3 and the audit §8 name, and `area_fit` feeds the
  engine's own `shrink_to_fit` rather than scaling type here.
- **each line's `baseline`/`descent`** answer "how tall is the block" (`Measured::bottom`,
  `needed_height`), and the *descent* is the right half of that pair: `crates/text/src/layout.rs:575-585`
  tests its own fit against `baseline + descent > bottom`, so using the ink bottom would call a line
  placed that the engine drops. (F5 is about the no-lines fallback, not about this choice.)
- No layout engine was written: every measurement in the tree goes through
  `vectorcraft_text::layout_with` in the single `measure` function, and the tree contains no
  line-breaking, no font shaping and no advance arithmetic.
- **The first probed behaviour is true and pinned.** Measured, not assumed: a word too long for the
  frame is broken **between characters**. `a_long_word_is_broken_between_characters_rather_than_overflowing`
  ("Unforgettable" in a 40×200 frame) asserts `fits()`, more than one line, and that every line's
  `x1` is inside its own `avail` span — so it is pinned as *spans*, not merely as a flag. My
  independent check adds the consequence: the same word in a wider balloon still widens it
  (`a_long_word_widens_the_balloon_rather_than_reporting_overflow`), and no degenerate input
  (`"Unforgettable"` across all six kinds) produces a non-finite plan.
- **The second is true and pinned.** `a_frame_too_narrow_for_one_character_is_overflow_with_no_lines_at_all`
  asserts `overflow`, `lines.is_empty()` **and** `!fits()` — the three together are what make the
  flag sufficient, and my `finding_5` test re-confirms the `lines.is_empty()` premise before relying
  on it.
- **The two self-reported bugs are genuinely fixed.** (a) *Padding twice*: `text_frame`
  (`fit/mod.rs:304-320`) returns the shape's full inscribed rectangle and says so; the padding is
  applied once, by the layout's inset — asserted at the exact-rectangle level and independently by
  my `line.x0 == frame.x0 + padding` claim. `measure` sets the inset on both the object
  (`object.area.inset`) and the options (`LayoutOptions::inset`) — I followed both engine entry points
  (`crates/text/src/layout.rs:133-147` reads `a.inset`; `:267` reads `opts.inset`) and each path reads
  exactly one, so the double-set is the app's path being served, not a double application.
  (b) *Height need vs box heights*: `sizing.rs:254` divides by `inner_scale`, which is right because
  `inner_rect` scales the box by `inner_scale` in both axes (`fit/mod.rs:293`), so frame = box ×
  `inner_scale`; asserted by my exact-minimum golden and, for the loose direction, by the
  no-jump-to-maximum test.

Tails: **a tail's path cannot drift in normal use, and can be made to disagree by a struct literal —
and one constructor path does make the base disagree with the balloon.** The three claims in order:
- *Derived path.* For every tail built through `Tail::new`/`from_outline`, `path ==
  path_for(kind, attach, tip)` holds for all four shapes, both tail kinds and every direction I tried,
  including tips at ±900, inside the balloon and on the attachment (`a_tail_built_through_its_constructor_has_the_path_its_own_fields_imply`). The three fields are `pub` and there is no
  `#[non_exhaustive]`, so a caller *can* write `Tail { kind, attach, tip, path: something_else }` and
  the invariant is gone — the doc says as much ("build one through `Tail::new` rather than filling the
  fields in"). I judge that acceptable but worth stating: the type does not make the derivation
  structural, and `is_attached_to_outline` validates `attach`, not `path`, so a hand-built tail with a
  wrong path passes every check the crate offers. The move helpers do not propagate the problem:
  `after_balloon_move`, `after_outline_move` and `retarget` all rebuild the path from `kind`, `attach`
  and `tip`, so a move *repairs* a wrong path. **preference, not a finding**: nothing in the design
  requires the field to be private, and `LetteringRequest` takes the same approach deliberately.
- *Attachment after moves.* `after_balloon_move` re-derives `attach` from the **new** outline via
  `PathData::nearest` and keeps the tip, which is exactly design §8.3's rule, and it is the right
  construction rather than a formula that only holds for a circle. `moved_by` translates the frame
  *path* and re-bases on that path, so a balloon whose anchors a creator dragged keeps its shape. I
  checked all four directions and a large drag (`attachment_holds_across_a_document_scale_move`: move
  by (10³, −2·10³), (10⁶, 10⁶) and (−10⁶, 5·10⁵) — the tail stays attached and finite); the
  implementer covers four moves including a shrink to 60×40 and asserts the outward dot product,
  which is the strongest of their tail tests. The one gap is F6: the *degenerate target* path leaves
  the base at the box centre, 31–40 points off the outline.
- *Is `distance < 1e-6` the right test?* Right in kind, and a hair too strict for absolute safety.
  It measures the distance to the real path (so it is exact for a cloud's lobes and a burst's spikes,
  which no formula would be), and it is falsifiable — unlike their suite, mine asserts a *negative*
  (`attachment_is_measured_against_the_path_not_the_box`). It is an **absolute** epsilon, though, and
  the distance is a difference of coordinates: round-off scales with magnitude, so it stops holding
  at roughly `|coord| ≈ 1e9` (≈10⁶ inches). A comic page is ~10²–10⁴ pt and a pasteboard drag is
  ~10⁶, so no document reaches it — acceptable, and stated rather than left as an assumption. A
  relative tolerance would remove the question.

Policy: the collision handling is right and the overflow severity is the right call.
- *Colliding candidates are skipped and the search continues.* `size_to_text` records
  `collision = Some(min(first, index))` and keeps walking widths and heights, and a candidate is only
  ever accepted by `Measured::fits` on the real box. Note that for both anchors the candidate
  boxes are nested (a wider box about the same centre is a superset; a taller `TopCentre` box extends
  downward), so a collision is monotone and the "keep searching" arm cannot rescue a box — it is
  nonetheless the correct shape, because it does not *stop* the search on the first collision and it
  reports the lowest obstacle index, which is the one the caller's slice puts first.
- *`Blocked` is reported and not applied.* `policy.rs:135-141` returns `report_with(request, …)`, and
  `report_with` writes `bounds: request.bounds` — so the frame the caller gave is the frame it gets,
  the message says "reported, not applied", and the finding carries `Blocked { obstacle }` with the
  index. Asserted at both levels (mine at `size_to_text` and through `apply`; theirs through
  `refit_balloon`). `Unfittable` likewise comes back at the largest box allowed with a finding, never
  as an `Err` and never as a silent clamp, which is what "a creator still has a balloon to drag"
  requires.
- *`Unfittable` when nothing fits.* `fit/sizing.rs:269-273` and `plan.rs`'s `sizing_findings` both
  produce it; the search is bounded by `MAX_WIDTH_STEPS` and two heights per width, so termination is
  structural. It is genuinely reachable (a 60×40 limit against "Incomprehensibilities") and tested in
  both trees.
- *Overflow as a `Warning` finding, not an `Err`.* **The right call, for three reasons I checked.**
  (i) Design §12.1 lists "Text overflow" in the *deterministic checks* table, whose remit is "runs
  this set and returns findings as structured data. It never mutates the project", and schema §13
  says every rule "checks these and **reports**". Overflow is a check, and a check reports.
  (ii) `Severity::Error` in this project means a referential inconsistency — a record that cannot be
  resolved — and an overflowing balloon resolves perfectly: the engine draws its overflow marker and
  the page renders (design §8.3 explicitly keeps the marker). An `Err` would make
  `comic.project.validate` unusable on a work in progress, which is the stated reason in the code and
  is correct. (iii) The tree *does* use `Err` where the record would be unrepresentable, and that
  line is drawn in the right place: `FitError::TextSourceMismatch` (rule 8 — a `speech` lettering with
  no `dialogue_id`, or a `narration` with no `text`, is not a record the schema can hold) and
  `NotFinite`/`NonPositive`/`LimitsInverted`/`PaddingTooLarge` (a caller or style bug, not a property
  of the page). So: unrepresentable ⇒ `Err`; representable but imperfect ⇒ `Warning` + a `Finding`.
  F2 (a `NaN` in a message) is a message defect, not a disagreement with that split.
- One further check: the finding is aimed at the right subject (`lettering speech`,
  `lettering narration` — `subject_of` uses `LetteringKindExt::name`, the schema's own string, and
  `the_kind_names_are_the_schemas_own_strings` pins it against `serde_json`), carries `rule: None`
  (it is not one of the eighteen referential rules) and never an `Error`
  (`every_finding_a_plan_or_a_policy_returns_is_a_warning_and_never_a_rule`, which walks all three
  policies against a tight and a wide frame).

Fonts: the `CRAFT_FONTS`-empty path is correct, and the test is genuinely conditional.
- With `CRAFT_FONTS_DIR` unset the lettering lays out and *reports*: every plan carries the
  missing-face finding (one per face the kind's style asks for), `plan.text_bounds.width() > 0.0`
  (the layout ran in the fallback face), the frame is a closed path, and no call fails. Verified for
  all six kinds — the CRAFT_FONTS-empty run of `the_missing_face_report_is_conditional_on_the_face_really_being_absent`
  takes the *absent* branch and asserts exactly one report per kind, naming the face's family and
  style.
- The report mirrors `text.missingFonts` field for field: `family`, `style`, `status`
  (`FontMatch::Missing` vs `Style`), `resolved` — the same `db.resolve` call and the same
  `status != Exact` filter as `crates/engine/src/cmd/fontfiles.rs:100-131`, the same sort by family
  then style, the same dedup, and the same cap (`MAX_FACES = 1000` = the exporter's `MAX_FONTS`).
  The one delta is the *filter*, F7 above.
- **The test is genuinely conditional, and I verified the other branch rather than reading it.** It is
  `reported == !comic_neue_available`, an equality rather than an `if absent { assert }` — so with the
  faces present it does assert *no* report, and it cannot silently pass by skipping. I ran the suite
  with `CRAFT_FONTS_DIR` pointed at the real checkout at
  `/home/hvt/web/comiclab.io.vn/craft-fonts` (Comic Neue Regular/Bold/Italic, Bangers Regular) — at
  which point my file held 22 tests, of which 16 passed and the six reproductions failed — and
  `the_missing_face_report_is_conditional_on_the_face_really_being_absent` was among those that
  passed, which it can only do by asserting the empty report for all six kinds. The
  other implementer test in this area, `a_face_that_is_available_exactly_is_not_reported`, uses the
  bundled `FALLBACK_FAMILY`, which is present in both configurations and so is a weaker check than it
  looks.
- Named, never added: no font file, and no image of one, is added by this change — `crates/comic-layout`
  gains one dependency line and no assets, and the two families are named as ADR-017 defaults in
  `style.rs` and overridable through the request, which is what design §8.3's "boils down to
  `ProjectSettings.default_lettering_style`" requires.

Contract: **clean.** `new_balloon` and `refit_balloon` take `&FontDb` and a request, and return
`Result<LetteringPlan, FitError>`; nothing in `src/{fit,lettering}/` names `Document`, takes `&mut`
anything, or mints a `NodeId` — the plan describes `frame`, `tail.path` and `text_frame_path()` and
leaves identity to the command layer (design §8, §9). The `layers` check and
`no_comic_crate_uses_a_document_type_in_its_code` both pass.
- **The `Dialogue` owns the text.** `TextSource` has exactly two variants and `owner()` maps them to
  `TextOwner::{Dialogue, Lettering}`, which the plan echoes as `text_owner`. `check_source`
  (`plan.rs:310-318`) rejects a mismatch as `FitError::TextSourceMismatch { kind, spoken }` in both
  directions, and it is called from **both** entry points (`new_balloon` `:250`, `refit_balloon`
  `:277`). I checked all six kinds × both sources × both entry points (24 cases) — the four spoken
  kinds accept only `TextSource::Dialogue` and the two carrying their own accept only
  `TextSource::Own`, each with the typed error and the schema's own kind name. This is rule 8 made
  structural rather than commented, which is what the design asks for ("`Lettering.dialogue_id` is
  now the single authoritative link").
- **A plan cannot become a second source of truth for dialogue.** The plan holds no text: its fields
  are the kind, a `TextOwner`, three rectangles, a `PathData`, an `Option<Tail>`, `AreaFit`,
  `TextLayoutSettings`, `OverflowPolicy`, `FitOutcome`, a measured `text_bounds` and `Vec<Finding>`.
  Nothing is a `String` except `text_layout.font` (the face the record asked for) and the findings —
  and `text_bounds` is measured geometry, which design §8.3 and schema §6.6 both require *never* to be
  persisted, so it cannot be written back over the record. The shared record's own fields are the
  only comic meaning a plan carries, and every one of them is an input the command layer will write
  into the record the caller already owns. Combined with the rule-8 gate, there is no path by which a
  dialogue's words acquire a second authority here. (The `Debug`-based test is weak evidence for this;
  the reading of the field list above is the evidence, and it is why I did not add a redundant test.)

Unverified:
- **The mutation table is by inspection, not executed.** The task forbids editing production code, so
  no in-place mutation was run; each "would catch it" claim names the test and the reason, and is
  checkable. If the lead wants an executed table, a reviewer on a branch may run it there — rows 2, 4,
  8, 9, 17 are the ones I would run first, because those are the rows where I predict a gap in the
  implementer's suite and no gap (or the opposite) in mine.
- **The `CRAFT_FONTS`-present run is not the gate's configuration.** It was run once, in a throwaway
  `CARGO_TARGET_DIR` that I then deleted, so the implementer's suite is *known* green in both
  configurations but the recorded gate result is the empty one, which is what the task requires. The
  `craft-fonts` checkout at `/home/hvt/web/comiclab.io.vn/craft-fonts` is the repository's own
  (its `fonts/manifest.txt` pins Comic Neue and Bangers to `google/fonts` commits with SHA-256s); I
  did not verify those hashes.
- **The `is_attached_to_outline` tolerance's failure point** (~`|coord| ≈ 1e9`) is analysis of how the
  distance is computed, not measured — the largest coordinate I actually exercised is 10⁶.
- **A balloon whose frame path a creator has edited** is not reachable through this tree's public API:
  no entry point accepts an existing frame path, so `moved_by`'s translate-not-rebuild claim and
  `after_outline_move` can only be exercised on paths this tree generated. `after_outline_move` is
  called with the translated frame, so the "arbitrary path" case is untested by construction, not by
  oversight. The `plan.rs` doc's "a balloon whose anchors a creator has dragged keeps that shape" is
  therefore a claim about a path this crate cannot yet be handed.
- **RTL and vertical type.** I included Arabic text in the degenerate-input sweep (finite, positive,
  fits) but did not check that `Justify::Center`/`Left`/`Right` interact correctly with the engine's
  own base-direction detection — `justify()` deliberately refuses `Justify::Auto` so that resolving
  alignment against reading direction is the caller's decision, and that decision is not this tree's.
  Vertical type is not exercised at all by the fitting path.
- **Hyphenation and `Composer`**: `measure` leaves `ParaStyle::composer` at its default and sets no
  hyphenation exceptions, so a project that configures either gets a measurement this tree does not
  model. Not in M1's stated scope; noted so it is not mistaken for coverage.
- **One transient, unexplained result.** One intermediate `cargo test` run reported `FAILED. 154
  passed; 1 failed` for the *inline* suite while a neighbouring file in this crate was being written
  by the W2 reviewer; the same command immediately before and after reported `155 passed; 0 failed`,
  and the `CRAFT_FONTS`-present run also reported `155 passed; 0 failed`. I could not attribute it and
  did not chase it, because the tree was moving under two other agents at the time; the stable,
  reproducible result is 155/0.

Gate:          `cargo xtask layers` — **pass** (`OK: 29 crates, no layering violations.`).
               `cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings` — **pass**
               (`Finished dev profile [optimized + debuginfo] target(s) in 0.98s`, 0 diagnostics).
               `env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout` — **fail**, and only
               because of the two reviewers' reproduction files: the implementer's own 180 tests are
               green (`155 + 8 + 17`), this record's seven `finding_*` tests are red by design, and
               the three failures in `tests/round1_reviewer.rs` are W2's record's, not W3's. Not
               summarised as "the gate passes": the exact per-target output is quoted above.

---

## Round 2 — rules, failure paths, edges

Reviewer:      code-reviewer on `deepseek-v4.1-flash` (`COPILOT_MODEL=deepseek-v4.1-flash`; the same
               in-environment statement as round 1, with the same stated gap — no
               `assistant_usage_events` row to corroborate it).
Revision reviewed: **the same base `16594c58` as round 1, at a later working-tree revision**: the
               untracked `src/{fit,lettering}/` trees *after* the F1–F8 fixes. The five files the
               fixes touch, sha256-pinned before any mutation and re-verified after every one:
               `fit/policy.rs 35ae1042…`, `fit/mod.rs 99052833…`, `lettering/tail.rs 73e71d32…`,
               `lettering/plan.rs 9c3aee21…`, `lettering/fonts.rs bd3bad5e…` (full digests in the
               mutation table below). Round 1 reviewed the *pre-fix* revision of the same trees, so
               this line differs from round 1's by construction.

Verdict:       **changes-requested** — one defect remains: the F4 fix has a **third path** it does
               not report, and the fix's own comment claims it has only two.

Unit tests:    `env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast`:
```
     Running unittests src/lib.rs
test result: ok. 165 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/lettering_round1_reviewer.rs
test result: FAILED. 26 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/round1_reviewer.rs
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/round2_reviewer.rs
test result: FAILED. 9 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
- The implementer's own suite is green: **165 inline + 8 + 17 = 190** in the crate's own targets, up
  from round 1's `155 + 8 + 17 = 180` (the +10 inline are the fixes' new tests, W3's and W2's).
  **All seven round-1 `finding_*` reproductions now pass** — the 25 round-1 tests in my file are
  among the 26 that pass, and the single failure is the **new** round-2 reproduction added here.
- My file: `crates/comic-layout/tests/lettering_round1_reviewer.rs` — 27 tests, **26 pass, 1 fails**
  (`r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported`, the F4-hole reproduction).
  The two round-2 tests added are `r2_every_fit_decision_path_measures_in_the_frame_it_returns`
  (passes) and that one (fails by design). `cargo fmt -p vectorcraft-comic-layout` was run on the
  file after the edit and again after removing a temporary probe test.
- `env -u CRAFT_FONTS_DIR cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings` →
  **0 diagnostics** (re-run with the test file touched, so the check is not a stale cache).
- `cargo xtask layers` → **`OK: 29 crates, no layering violations.`**
- **Two failing targets are not W3's.** `tests/round2_reviewer.rs` (9/3) is the *other* reviewer's
  W2 record, failing in both font configurations; the same three shapes of defect show up with
  `CRAFT_FONTS_DIR` set as five more failures in W2's own inline `template`/`validate` tests
  (`the_last_row_of_every_swept_page_height_lands_exactly_on_the_content_box`,
  `the_last_panel_of_every_row_lands_exactly_on_the_content_box`,
  `a_page_whose_rows_do_not_divide_evenly_is_not_reported_as_a_defect`,
  `a_zero_gutter_page_whose_rows_do_not_divide_evenly_is_not_an_overlap`,
  `no_page_geometry_this_crate_can_produce_is_reported_as_a_defect` → `160 passed; 5 failed`).
  Round 1's `CRAFT_FONTS`-present run of the same target was `155 passed; 0 failed`, so those five
  are **new and W2's** (`src/{template,validate}/`, `src/lib.rs`), not a consequence of any W3 fix —
  every W3 test in that run behaves as it does in the empty configuration. Recorded, not reviewed:
  `src/{template,validate,layout,panel,reading_order}/` is out of W3's scope.
- **The scratch-path collision the brief mentions is not in my file.** `lettering_round1_reviewer.rs`
  contains no filesystem use at all — no `std::fs`, no `env::temp_dir`, no `create_dir`/`remove_dir`,
  no hard-coded path, no `CRAFT_FONTS_DIR` write (searched: `fs::`, `temp_dir`, `create_dir`,
  `remove_dir`, `process::`, `option_env`, `MANIFEST`, `"/` and `craft-fonts` all return nothing).
  There is therefore nothing to make unique per process; I have used no scratch paths this round
  either, except one throwaway `CARGO_TARGET_DIR` for the `CRAFT_FONTS`-present run, which I named
  `target/r2-fonts-$PPID-$$` and deleted afterwards. If the brief meant a different artefact, the
  fixed-name collision is not reachable from this file.

Findings:      **3 — 1 bug, 2 preferences.** Only R2-F1 keeps the verdict at `changes-requested`;
R2-F2 and R2-F3 are notes and would not.

**R2-F1 — `bug`** — `crates/comic-layout/src/lettering/plan.rs:396` (`assemble`, the
`(style.tail, request.tail_target)` match) with `:353` (`ruler_findings`' second guard) and `:132`
(`with_tail`).
Trigger: a **spoken** kind whose style *has* a tail and whose target is present but not a finite
point — `LetteringRequest::new(Speech, …).with_tail(Point::new(f64::NAN, 10.0))`, or `…INFINITY`,
through either entry point. Expected: the contract the F4 fix itself states — "the record cannot be
written legally without `tail_node_id` for a kind that has a tail (schema §6.4)" — so a plan with no
tail must be a plan with a finding, or the request must be refused the way a non-finite `at` is
(`validate_point("balloon position", …)`, `plan.rs:257`). Actual: `assemble`'s guard
`if target.x.is_finite() && target.y.is_finite()` falls through to `_ => None`, so `plan.tail` is
`None`; `ruler_findings`' second guard is `tail_target.is_none()`, which is *false* because the
target is present, so **no finding is produced for either limb**. Measured: `plan.kind == Speech`,
`not_font(&plan) == []` (only the CRAFT_FONTS report, which in the empty configuration is the face
report) for both the NaN and the `+∞` target, through `new_balloon` *and* `refit_balloon`. The
command layer writes a `speech` lettering with `tail_node_id: null`, which schema §6.4 does not let
it write and §13 rule 7 would flag — silently. This is round-1 F4's own defect class (a balloon with
nobody to point at is reported; a balloon with no tail is not) reached by a third path, and F4's fix
comment ("Two ways a balloon can end up with no tail, and both are the same defect", `plan.rs:337`)
is false. `tail_target` is the *only* point in `LetteringRequest` that is neither validated nor
reported; `at` is both.
Reproduction: `r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported`.
Either fix satisfies it: refuse the request (`FitError::NotFinite { field: "tail target" }`, matching
`validate_point`), or gate the finding on the assembled tail rather than on `tail_target.is_none()`.

**R2-F2 — `preference`** — `crates/comic-layout/src/lettering/plan.rs:345`.
The F4 fix's comment ends "a caption has no tail by design, and a style that happens to carry one
does not give it one". The second clause is **false**: `assemble` matches only on
`(style.tail, request.tail_target)` and never on the kind, so a `narration` planned with
`with_style(style_of(Speech))` **and** a target comes back with `plan.tail: Some(Wedge)`. Verified,
not inferred (a scratch test printed `narration tail present = true`, then was removed). The
behaviour itself is pre-existing and defensible — the caller asked for a tail explicitly, and §6.4
requires a `tail_node_id` for kinds that have one without forbidding one elsewhere — so this is a
comment that overstates what the code does, not a wrong plan. Worth one sentence of correction.

**R2-F3 — `preference`** — `crates/comic-layout/src/lettering/fonts.rs:112` and `:121`.
F7's fix filters on the exporter's terms and one more: `!face.style.trim().is_empty()`. The
exporter's filter (`crates/engine/src/cmd/fontfiles.rs:111`) is
`!family.trim().is_empty() && !generic(family) && family.len() <= MAX_NAME && style.len() <= MAX_NAME`
— it **does not** test the style for emptiness. So for a face whose style is `""` and whose family is
not, the lettering report stays silent where `text.missingFonts` reports, and the doc's "The filter is
the exporter's, name for name" is one term too strong. Reachability is thin: the crate's own styles
are `REGULAR`/`BOLD`/`ITALIC`, so only a hand-authored document (or a caller's `with_style`) naming an
empty style reaches it, and quietness here cannot produce a wrong plan — hence a note, not a finding.
It is stated because F7 was raised about exactly this kind of one-term filter delta.

Round-1 verification: **F1 fixed — F8 fixed, all eight verified by re-running my reproductions and by
reading the new code.**

| # | Verdict | Evidence |
|---|---|---|
| F1 | **verified fixed** | New `in_the_given_frame(db, request)` (`policy.rs:159-162`) re-measures in `text_frame(request.bounds, …)`, and `resize` uses it for both the `Blocked` and the `Unfittable` arm (`policy.rs:143`, `:147`). My `finding_1_…` passes, and the `report`/`report_with` pair is gone (`grep report_with` → nothing), so there is one report path, not two. |
| F2 | **verified fixed** | `validate` refuses a non-finite `shrink_percent` (`fit/mod.rs:191-194`); `policy::shrink` reads the floor from `area_fit(…).min_scale()` and `shrink_message(request, floor)` quotes it (`policy.rs:99-101`, `:190-196`). My `finding_2_…` passes; the last block asserts `measure(db, "Hi!", &nan_style, …).is_err()`, which couples to the fix. |
| F3 | **verified fixed** | `moved_by` translates `FitOutcome::Resized { bounds }` with `translate_rect` (`plan.rs:236-240`). My `finding_3_…` passes; their `moving_a_grown_balloon_moves_the_outcome_that_names_its_box` now exists and catches the regression. |
| F4 | **partially fixed** | Both named limbs are reported and correctly gated on `kind.is_spoken()` (`plan.rs:346`, `:353`), which is exactly the four kinds `LetteringKind::is_spoken` says have someone to point at — the gate is right, not too narrow and not too wide. Their `a_spoken_kind_wearing_a_tailless_style_is_reported` and my `finding_4_…` both pass and both catch a mutation of the first limb. The third limb is **R2-F1**. |
| F5 | **verified fixed** | `Measured::bottom` falls back to `self.frame.y0 + self.inset` (`fit/mod.rs:244`), so `needed_height() == 2 × padding`. My `finding_5_…` passes and their new `a_block_with_no_lines_needs_its_padding_and_never_a_negative_height` catches the reverted `layout.bounds.y1`. |
| F6 | **verified fixed** | `attach_point` maps a non-finite tip to `downward_probe(bounds)` (`tail.rs:136-141`), and the test that pinned the centre was **rewritten** (see "F6 pressed"). My `finding_6_…` passes, and it catches the *original* behaviour when I restore it by mutation. |
| F7 | **verified fixed, with a residual** | `missing_faces` now filters the generic families, the 256-byte caps and the blank name (`fonts.rs:121-122`), with `MAX_NAME = 256` = the exporter's and the same 12-name list (leaving `fangsong` out for the exporter's stated reason). My `finding_7_…` passes; their `a_css_generic_family_and_an_over_long_name_are_not_reported` catches a mutation of the filter. Residual: R2-F3. |
| F8 | **verified fixed** | `crates/comic-layout/Cargo.toml:19-20` now reads "the template and layout error types (M1, the panel tree; the lettering tree's `FitError` is hand-written)", and `grep -rl thiserror src/` returns exactly `template/error.rs` and `layout/error.rs`. No test, as round 1 said. |

F6 pressed: **the new behaviour is right, and the rewritten test pins the fix rather than a different
wrong answer.** A tail's tip that is not a point has no direction to give, so the only honest choices
are (a) a deterministic, attached, finite path or (b) a refusal; `Tail::new`/`from_outline`/
`after_balloon_move`/`retarget` are infallible by design and every caller builds a path from them, so
(b) would mean changing four signatures to answer a question the module already answers elsewhere —
`reachable_tip` and `direction` both fall back to *straight down* for a target they cannot use.
`downward_probe(bounds) = (bounds.center().x, bounds.y1 + MIN_LENGTH)` aims at exactly that
direction, so the base lands on the balloon's lowest point, which is a point of the outline for all
four shapes by construction (`nearest` returns a point *on* the path, and for a cloud or a burst that
is a lobe or a spike rather than a formula). The result is a 1-point stub hanging down, attached to
within `1e-6`, finite, and identical on every call — degenerate but never wrong, and no *plan* changes
at all, because `assemble` already filters a non-finite target (which is why this is an API-level fix
and not a rendering one). The rewritten
`a_tail_built_from_a_non_finite_target_hangs_from_the_outline_not_from_the_centre` asserts five
things about the new answer — path has bounds, `is_attached_to`, `attach != bounds.center()`, the tip
below the base, a finite length — and then checks `attach_point` *directly* for the three properties
that identify the downward probe (`distance < 1e-6`, `attach.x == center.x`, `attach.y > center.y`).
That is a pin, not a restatement: restoring the old centre fallback by mutation fails it, as does the
weaker `probe = tip` mutation. Two things it does not prove, both worth stating: it pins *this*
fallback, not that some other attached point would be better (none of the alternatives is attached
for all four shapes without the same probe), and `nearest` with a NaN probe is not itself defensive —
with `probe = tip` it returns the outline's **first anchor**, `(100, 140)` for the round-1 box, so the
guard's correctness now rests entirely on the finite-probe branch. That branch is the fix; it is
right; no change requested.

F2 judgement: **the split is defensible, and `min_scale()` is exactly what the engine applied.**
*Refuse non-finite, clamp out-of-range* separates two different things: `AreaFit::clamp_percent`
reads a non-finite value as "no opinion" and substitutes the default 50 (`crates/doc/src/text.rs:768`),
which is a *silent substitution of a policy nobody asked for* — precisely what `FitTextStyle::validate`
exists to prevent, and a `NaN` that reaches a `Finding` is a user-visible defect. An out-of-range
number is not that: it is a number with a meaning, `clamp_percent` documents the clamp as its own
contract ("clamped to 10..100", `:756`), the schema puts no bound on the field, and refusing it would
make a request the engine handles correctly an error. Both halves then say the truth to the user,
because the message quotes the **applied** floor rather than the style's wish. And `min_scale()` is
not a guess: `vectorcraft_text::layout_with` is the only shrink path, and it reads the floor from
`opts.fit.min_scale()` (`crates/text/src/layout.rs:196`) to bound its own bisection
(`let mut lo = (tenths * min).ceil()`, `f = (k / tenths).clamp(min, 1.0)`), so the number the finding
quotes is the number the engine clamped to — the same `AreaFit` value, from the same `area_fit`
call. The only residue is sub-0.1 % arithmetic: when `10 × size × percent / 100` is not an integer,
the engine's floor candidate is the next tenth of a point up (`lo/tenths ≥ min`), so a failing shrink
can report a `fit_scale` a hair *above* `min_scale` — 0.33333 against 0.3333 for a 12 pt style at
33.3 %. The message rounds to a tenth of a percent, so it still names the applied policy; I did not
call it a defect. My `finding_2_…` asserts `(fit_scale - engine_floor).abs() < 1e-9` and passes
because it uses a style whose tenths land exactly.

F5 pressed: **`2 × padding` is the right height, and nothing divides by it.** With no lines placed
the block has zero content, so "the frame height this block needs" is the content (0) plus the two
paddings — one number that is a property of the block plus the frame, not a guess, and the direct
consequence of the same formula that is used when lines *are* placed (`(bottom - top) + 2 × inset`).
Any other fallback would be arbitrary: `frame.y0 + inset` is the frame's own inner top, and a NaN
would be actively harmful (see below). Downstream: `needed_height()` is read at exactly one place,
`sizing.rs:254` (`needed_box = probe.needed_height() / inner_scale`), which *divides by* `inner_scale`
and never by the height, and `heights_from` then clamps it up into `limits.min.height..=limits.max.height`
(so 14 pt of padding against a 32 pt minimum yields `[32, 220]`, the same candidates a positive need
gives). No division, no comparison, and no other user of `needed_height` in the crate or the apps.
Worth recording for the *old* code: a negative need was swallowed by `.max(min)`, so F5 was cosmetic —
but a NaN would not have been, because `heights_from` sends a non-finite need to `max` (`sizing.rs:209`),
which would have jumped every empty balloon to its maximum height.

F4 pressed: **`is_spoken()` is the right gate.** It is `matches!(self, Speech | Thought | Shout |
Whisper)` (`crates/comic-model/src/entities.rs:152-154`), which is exactly the set for which
`style_of` gives a tail (`style.rs:87-146`: `Wedge` for speech/shout/whisper, `Bubble` for thought,
`None` for narration and sound_effect), and the set §6.4 calls "kinds that have a tail". So the gate
does not miss a kind that should have a tail and does not over-report a caption: `narration` and
`sound_effect` are covered by the same predicate the tail defaults are built from, not by a second
list that could drift. The one kind whose tail is *not* a `Wedge` (`Thought`) is spanned by the gate
because `is_spoken` includes it. The comment limb is R2-F2.

F1 walked: **every path agrees.** `FitDecision` is constructed in exactly four functions, all in
`policy.rs` — `apply`'s fitting early return, `report`, `shrink`, `resize`. `Fits` measures in
`text_frame(request.bounds)` and returns `request.bounds`; `Warn`/`Blocked`/`Unfittable` go through
`report`, which forces `bounds: request.bounds` and now receives `in_the_given_frame(request)`, a
measurement in exactly that frame; `Shrunk` (both limbs) measures in `text_frame(request.bounds)` and
returns `request.bounds`; `Resized` returns `sized.bounds` with `sized.measured`, which
`size_to_text` took in `frame_of(`*that*`bounds)`. So `decision.measured.frame ==
text_frame(decision.bounds, inner_scale, padding)` on all seven outcomes, which is what the new
`r2_every_fit_decision_path_measures_in_the_frame_it_returns` asserts case by case (`Fits`, `Warned`,
`Shrunk` fitted, `Shrunk` at the floor, `Resized`, `Blocked`, `Unfittable`), and what the mutation
below falsifies. The `Shrunk`-at-the-floor case is the one where the *fit* differs (the shrunk
measurement is taken with the `AreaFit`, the reported ones with `AreaFit::None`), and it is right:
`Shrunk`'s box is unchanged and its record says `fit_to_frame = true`, so the text really is at the
shrunk size, whereas a reported grow is *not applied* and the text really is at full size.

Mutation table: **executed, one mutation at a time, each restored and re-verified with `sha256sum -c`
before the next.** Baseline (`target/r2-baseline.sha256`, kept in the session folder):
`fit/policy.rs 35ae1042bb9c44bf4dd3ecdc031b65295f5ef05a8ca8f2c37b2a17cf20a0ee0f`,
`fit/mod.rs 99052833defcc2ef764e2a81be63d7a9552903781968171a4a4ba6b414b97531`,
`lettering/tail.rs 73e71d32a0bb2e9a59a9c826707ecc38e3ff22768ac2fbad588b689171851483`,
`lettering/plan.rs 9c3aee2148bb314ad3441d5ffd272bf42d5620682786c242ba1760e21a7cb00e`,
`lettering/fonts.rs bd3bad5e0c4c169db62d6e394cac850a4d2d01a50b9e14f7a25b88f2ac4d3988`. Every run
below is `env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast` in full, and
after every one the five files matched the baseline (`sha256sum -c` → all `OK`). **A mutation was in
the tree during each of these runs and nowhere else** — the tree is shared, so this is stated plainly:
if any reader saw a red inline suite during this window, it was mine and it is gone.

| Mutation (one at a time, restored immediately) | Their suite | My file |
|---|---|---|
| **F1**: `Blocked` arm reverted to `sized.measured` (`policy.rs:135`) | **caught** — `fit::policy::tests::resize_frame_that_would_overlap_another_object_is_reported_not_applied`, `lettering::plan::tests::a_refit_that_would_overlap_another_balloon_is_reported_not_applied` | **caught** — `finding_1_…`, `r2_every_fit_decision_path_measures_in_the_frame_it_returns` |
| **F2**: NaN guard in `validate` disabled (`fit/mod.rs:192`) | **caught** — `an_out_of_range_shrink_percent_is_clamped_and_quoted_as_the_policy_applied` | **caught** — `finding_2_…` |
| **F3**: `moved_by` copies `Resized` verbatim (`plan.rs:236-240`) | **caught** — `moving_a_grown_balloon_moves_the_outcome_that_names_its_box` | **caught** — `finding_3_…` |
| **F4**: missing-tail-style limb disabled (`plan.rs:346`) | **caught** — `a_spoken_kind_wearing_a_tailless_style_is_reported` | **caught** — `finding_4_…` |
| **F5**: `bottom()` back to `layout.bounds.y1` (`fit/mod.rs:244`) | **caught** — `a_block_with_no_lines_needs_its_padding_and_never_a_negative_height` | **caught** — `finding_5_…` |
| **F6a**: non-finite tip → `probe = tip` (the `downward_probe` branch removed) | **caught** — `a_tail_built_from_a_non_finite_target_hangs_from_the_outline_not_from_the_centre` (`the base is under the balloon: (100.0, 140.0)`) | **not caught** — `nearest(NaN)` returns the outline's *first anchor*, which is itself on the outline, so an attachment-only assertion passes; my `finding_6_…` is weaker than theirs here, deliberately — round 1's defect was the centre, and that is F6b |
| **F6b**: non-finite tip → `return bounds.center()` (round 1's behaviour, restored) | **caught** — same test | **caught** — `finding_6_…` (4 shapes × 2 kinds detached) |
| **F7**: `!is_generic_family(…)` removed from the filter (`fonts.rs:122`) | **caught** — `a_css_generic_family_and_an_over_long_name_are_not_reported` | **caught** — `finding_7_…` |

Rows F6a/F6b are the honest pair: their rewritten test is *stronger* than my round-1 reproduction
(it asserts the base is under the balloon, not merely attached), while mine is exactly as strong as
the defect it reproduced. F8 is a comment; no mutation and no test can fail on it.

Semantic defects: **yes — one, R2-F1**, and it is semantic in the bar's sense: a plan whose geometry
nobody writes (`speech` with `tail: None`, which §6.4 makes unrepresentable in the record) produced
silently, plus a comment in the same fix that says the case cannot happen. It is a *hole in a
round-1 fix*, not a new mistake, and it is the only thing standing between this revision and
`approved`. R2-F2 and R2-F3 are preferences. Nothing else in the fixes introduces a wrong predicate,
a wrong frame, or a contradiction: the F1 collapse of `report`/`report_with` removed a duplicated
path rather than a check; `moved_by`'s translate arm leaves every non-coordinate outcome alone
(`Blocked`, `Warned`, `Unfittable`, `Shrunk`, `Fits` are `Copy` and coordinate-free); `needed_height`
stays inside its one clamped consumer; and the `AreaFit::None`-vs-`AreaFit` split between the
reported and the shrunk paths is the right one for what the record will say.

Unverified:
- **`round2_reviewer.rs` and the five W2 inline failures are read, not reviewed.** They are W2's
  trees and W2's reviewer's file; I confirmed only that they are not reachable from anything W3's
  fixes touch (they fail identically with and without the F7 filter mutation, and the five inline
  ones depend on `CRAFT_FONTS_DIR` being *set*, which no W3 code path does).
- **The F7 mirroring claim is checked against the exporter's source, not by running the exporter.**
  `missing_fonts`' filter is quoted from `crates/engine/src/cmd/fontfiles.rs:111` and the two lists
  are compared name for name; I did not build the `engine` crate and compare two reports of the same
  document, so a divergence *above* the filter (for instance in `used_fonts`) would not show up here.
- **`nearest`'s behaviour on a NaN probe** is measured for one shape only (Ellipse, first anchor
  `(100, 140)`); I did not enumerate the other three, because the fixed code never passes a
  non-finite probe.
- **No consumer outside this crate is modelled.** `needed_height`, `attach_point` and `downward_probe`
  have no callers in `crates/` or `apps/` today (grepped), so the changed fallbacks are not exercised
  by any caller this round; the command layer that will consume `LetteringPlan.tail` is M1 step 5
  and does not exist yet, which is also why R2-F1's impact is a *cannot-write* record rather than a
  visible one.
- **The `CARGO_TARGET_DIR`-isolated fonts-present run is not the gate's configuration**, as in round
  1: it is recorded above only for the two facts it establishes (the F7 filter changes nothing about
  the faces-present branch, and my file's conditional font test asserts *no* report for all six
  kinds when the faces are there), and the target directory was deleted afterwards.
- **A tail whose target a document cannot supply** (an edited `tail_node_id` pointing at a node whose
  position is not finite) is exactly R2-F1's trigger and *is* reachable from that direction; I did not
  build the resolver, so I state R2-F1 from this crate's public API only.

---

## Round 3 — verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash` (`COPILOT_MODEL=deepseek-v4.1-flash` — the same
               in-environment statement as rounds 1 and 2, with the same gap stated rather than papered
               over: `~/.copilot/session-store.db` › `assistant_usage_events` holds **632** rows for other
               sessions and **0** for this one's `COPILOT_AGENT_SESSION_ID=240af6e4-0907-4494-a1c8-7c9b0cc54dcc`,
               so the environment's word is the only witness).
Revision reviewed: **rounds 1 and 2's trees at the revision carrying the round-2 fixes** — base
               `16594c58` still, `src/{fit,lettering}/` still untracked. Three of the nine files differ from
               the digests round 2 pinned: `lettering/plan.rs 8f7d6bd1…` (was `9c3aee21…`),
               `lettering/tail.rs 714cc582…` (was `73e71d32…`) and `lettering/fonts.rs 10e9c2b6…` (was
               `bd3bad5e…`). The two files round 2 pinned and the fixes do not touch — `fit/mod.rs
               99052833…` and `fit/policy.rs 35ae1042…` — are byte-for-byte round 2's. `lettering/style.rs`
               (now `25eb75c3…`) carries the fix's own R2-F2 wording in the `tail` field's doc, and round 2
               pinned no digest for it, so its round-2 revision is known only by that marker;
               `lettering/mod.rs 2ec895f0…`, `lettering/outline.rs 1fd2ac8c…` and `fit/sizing.rs f68a4e49…`
               carry no marker of either fix and no fix reaches them. The nine
               concatenated in sorted path order hash to
               `dc0636fd2f9ec1faad5a9c4681226aef1d3ddeeba3582013ad435632930fae49` — the implementer's
               stated `dc0636fd…`, so this round reviewed exactly the revision the report names. Every
               digest was taken before any mutation and re-checked with `sha256sum -c` after every restore
               (`files/r3-baseline.sha256` in the session folder; all nine `OK` at the end, and the
               concatenated digest recomputed as `dc0636fd…` again).   Files:
               `crates/comic-layout/src/lettering/{mod,outline,tail,style,fonts,plan}.rs`,
               `crates/comic-layout/src/fit/{mod,policy,sizing}.rs`,
               `crates/comic-layout/tests/lettering_round1_reviewer.rs` (mine — reached 31 tests with the
               four round-3 tests below).
Verdict:       **approved** — the three round-2 findings are fixed, the unified predicate holds, and what
               remains is three notes (R3-N1…N3), none of which can produce a wrong plan or a wrong record.

Unit tests:    `env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast` (the gate's
               own configuration):
```
     Running unittests src/lib.rs
test result: ok. 172 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/layer_rules_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/lettering_round1_reviewer.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/panel_rect_contract.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/round1_reviewer.rs
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
     Running tests/round2_reviewer.rs
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round3_probe.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round3_reviewer.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
- **Nothing is red.** W3's own targets: the crate's inline suite **172/0** and my file **31/0**. The
  other integration targets are the W2 reviewer's (`round1_reviewer.rs` 27/0, `round2_reviewer.rs` 12/0,
  `round3_reviewer.rs` 11/0) and the frozen contracts (`layer_rules_contract.rs` 8/0,
  `panel_rect_contract.rs` 17/0); `round3_probe.rs` (1/0) was the W2 reviewer's temporary probe, and it
  was deleted by its author after this run — as its own header says it would be — so a re-run of the same
  command now shows these seven targets and not that one.
- **The 263 figure the report quotes is verified, and it was reached before my four new tests.** With my
  file still at round 2's 27 tests and before the W2 reviewer added `round3_reviewer.rs`/`round3_probe.rs`
  (10:16–10:20), the crate ran `172 + 8 + 27 + 17 + 27 + 12 = 263` tests, 0 failures — the implementer's
  count to the test. This round's 279 (`172 + 8 + 31 + 17 + 27 + 12 + 1 + 11`) is 263 plus my four and the
  W2 reviewer's twelve.
- **Four tests added, all in `crates/comic-layout/tests/lettering_round1_reviewer.rs`** (test code only; no
  production line was left changed): `r3_a_kind_with_nobody_to_point_at_gets_no_tail_whatever_its_style_says`,
  `r3_the_missing_tail_finding_names_the_reason_it_is_missing_for`,
  `r3_a_target_with_one_unusable_coordinate_takes_the_same_downward_probe`,
  `r3_the_face_report_filters_on_the_exporters_terms_and_no_more`. `cargo fmt -p vectorcraft-comic-layout`
  was run after the edit and `cargo fmt -p vectorcraft-comic-layout --check` now exits **0** for the whole
  crate.
- `env -u CRAFT_FONTS_DIR cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings` → **0
  diagnostics** (`Finished dev profile [optimized + debuginfo] target(s) in 0.33s`), re-run after the new
  tests and after the last mutation was restored.
- `cargo xtask layers` → **`OK: 29 crates, no layering violations.`**
- **The `CRAFT_FONTS_DIR`-present configuration, run this round rather than assumed.** With
  `CRAFT_FONTS_DIR=/home/hvt/web/comiclab.io.vn/craft-fonts`, my file is **31/0** — including
  `the_missing_face_report_is_conditional_on_the_face_really_being_absent`, which therefore takes its
  *absent-report-is-empty* branch for all six kinds, and the new tests that touch faces or tails. So
  none of the four is configuration-dependent. The gate's own recorded result above is the empty-fonts
  run, as the task requires; the fonts-present run rebuilt `vectorcraft-text` in the shared target
  directory and the empty run at the end rebuilt it back, so the tree is left in the gate's configuration.
- **Two transients, both the other agent's and both cleared.** (i) `tests/round3_reviewer.rs` failed to
  compile mid-round (`TemplateError` is private, a W2 import) and, later, `tests/round3_probe.rs` was
  un-compilable for clippy (`literal out of range for f64`); each cleared on re-run and the final gate run
  above is the complete command with every target green. (ii) `round3_reviewer.rs` also failed once while
  the W2 reviewer was writing it (`the_one_ulp_guarantee_is_attained_and_never_exceeded`, a template-tree
  reproduction, 10/11); it passes now. Neither is reachable from anything W3's fixes touch.
- **The scratch-path question:** `lettering_round1_reviewer.rs` still contains no filesystem use
  (`grep -n "fs::\|temp_dir\|create_dir\|CRAFT_FONTS_DIR"` returns nothing); my one temporary file this
  round was `tests/r3_scratch_probe.rs`, created to *measure* `nearest`'s NaN answer for the four shapes,
  run once, and deleted (`ls tests/` shows it gone). No scratch paths, and nothing written outside the
  repository.

Findings:      **0 defects. 3 notes, all `preference`.** Nothing here is `changes-requested`: a note does
               not hold the verdict, and the three are a doc sentence, a maintenance coupling in a message
               and a deliberate narrowing whose plan is still correct.

**R3-N1 — `preference`** — `crates/comic-layout/src/lettering/plan.rs:97-99` (the `tail_target` field doc)
and `:136-138` (`with_tail`).
Trigger: any request with an unusable target on a kind that has **no** tail to lose — a `narration` or
`sound_effect` reaching `with_tail(Point::new(f64::NAN, f64::NAN))`, which is exactly what a command layer
reading a damaged `tail_node_id` off a caption would do. Expected: the doc it states — "One that is not is
**reported** rather than refused … a finding naming the reason". Actual: nothing is reported for a kind with
no tail, because `ruler_findings`' guard is `kind.is_spoken() && tail_spec(request).is_none()`
(`plan.rs:366-368`) and a caption fails the first conjunct. The *behaviour* is right and is asserted by the
implementer's own `a_tail_target_that_is_not_a_point_is_reported_and_never_silently_dropped` and by my
`r3_…` tests — there is nothing to report, because a caption has no tail to lose — so this is a sentence
that reads as unconditional where the code is conditional. One clause fixes it ("…and, for a kind that has
a tail, a finding naming the reason"); `with_tail`'s own clause ("because a spoken lettering must not be
written without one") already scopes itself, so the field doc is the one to change. `preference`, and the
same class round 2 put on the record as R2-F2.
**R3-N2 — `preference`** — `crates/comic-layout/src/lettering/plan.rs:378-386` (`missing_tail_message`).
The message re-derives *which* reason applies from `(style.tail, tail_target)` instead of being told by
`tail_spec`, whose `None` it is answering for. Today the three arms are exactly `tail_spec`'s three
rejection reasons and all three are pinned by tests (mine walk them through both entry points and assert
each names its reason and not the other two; theirs assert the same phrases), so there is no disagreement
now — but a fourth reason added to `tail_spec` later (a target beyond a limit, say) would be reported as
"the tail target is not a finite point" and the comment's claim to be "exhaustive over the reasons" would
quietly stop being true. `tail_spec` returning the reason (`Option<Result<(TailKind, Point), Reason>>`, or an
enum) would make the mislabelling impossible to write. No change requested: the coupling is stated in the
comment and the three arms are tested.
**R3-N3 — `preference`** — `crates/comic-layout/src/lettering/plan.rs:399-403` (`tail_spec`), with
`lettering/mod.rs:12` and the table at `:16-23`.
Trigger: a caller who wants a tail on a `sound_effect` — a burst pointing at the thing that made the noise,
which comics do draw — by giving it `with_style(style_of(Speech))` and a target. Expected, before this
revision: a wedge, because the caller asked for one and `tail_target` was given. Actual: `tail: None` and no
finding, because presence is the kind's. The change is right on the evidence (schema §6.4 makes
`tail_node_id` *required* for the kinds that have a tail and only *permits* it elsewhere, and the module
head's own table gives `sound_effect` no tail, so the code now matches the contract it documents rather than
the caller's override; `narration` is not even arguable), and it is a narrowing with no override left in the
tree. Stated because round 2's R2-F2 offered it as a *comment* to correct and the implementer corrected the
*code* instead — a defensible reading of "prefer the code over the comment", taken in the direction of the
documented table, and the only cost is a tail this tree can no longer produce for one of six kinds.

Round-2 verification: **all three verified fixed**, each by re-running round 2's reproduction, by reading
the new code, and by a mutation restored to `sha256sum`-clean immediately after.

| # | Verdict | Evidence |
|---|---|---|
| R2-F1 (the bug) | **verified fixed** | `tail_spec` (`plan.rs:399-403`) is the single predicate — the style names the form, the target must be a point, the kind must be spoken — `assemble` builds the tail from it (`:438`) and `ruler_findings` reports when it is `None` (`:366-368`). My `r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported` **passes** (it failed in round 2), and their `a_tail_target_that_is_not_a_point_is_reported_and_never_silently_dropped` walks three non-finite targets (`(NaN,10)`, `(10,∞)`, `(-∞,NaN)`) × both entry points, asserting the balloon is still a balloon (`frame.is_closed()`, `fits()`) as well as the finding. Mutating the guard back to round 2's two-limb form `kind.is_spoken() && (style.tail.is_none() \|\| tail_target.is_none())` turns **both** red (table row 1), so the hole is closed and both files guard it. **Report over refuse is the right call**, and for the reason round 2 did not state: `at` is refused because a placement is *required* — a balloon with no position is not a balloon — while a tail target is optional, so refusing one would trade a usable balloon for an error. Both halves are now on the record: `validate_point("balloon position", …)` refuses, `tail_spec`'s `is_point` reports. |
| R2-F2 | **verified fixed** | The **kind** gates presence and the style only the form: `tail_spec` requires `request.kind.is_spoken()` (`plan.rs:402`), so a `narration`/`sound_effect` wearing `style_of(Speech)` with a target gets `tail: None` **and no finding**. My new `r3_a_kind_with_nobody_to_point_at_gets_no_tail_whatever_its_style_says` asserts exactly that through both entry points, plus the mirror image (a `speech` wearing a caption's style *is* reported) and the form rule (a thought balloon is a bubble, and a wedge when worn with a shout's style). Dropping the conjunct from `tail_spec` is caught by their `the_kind_decides_whether_there_is_a_tail_and_the_style_only_which_form` **and now by mine** — it was not caught by my file before this round (table row 2), which is why the test was added. |
| R2-F3 | **verified fixed** | `fonts.rs:124-125` is the exporter's filter term for term: `!family.trim().is_empty() && !is_generic_family(family) && family.len() <= MAX_NAME && style.len() <= MAX_NAME`, against `crates/engine/src/cmd/fontfiles.rs:111` — `!family.trim().is_empty() && !generic(family) && family.len() <= MAX_NAME && style.len() <= MAX_NAME`. The extra `!face.style.trim().is_empty()` is gone, and the doc says why. Re-adding it fails their `a_blank_family_is_not_a_missing_font_and_a_blank_style_is_reported_as_the_exporter_reports_it` and my new `r3_the_face_report_filters_on_the_exporters_terms_and_no_more` (table row 4) — mine was added because my file did not catch it before this round. Caveat unchanged from round 2: checked against the exporter's *source*, not by running the exporter (see Unverified). |

The unified predicate: **yes, `tail_spec` is the single source of truth, and no caller can reach a plan
whose `tail` and whose finding disagree.**
- One writer, one reader, one input. `LetteringPlan.tail` is set in exactly one place — `assemble`
  (`plan.rs:438`) — and `LetteringPlan.findings` gains a tail finding in exactly one place —
  `ruler_findings` (`:366-368`) — and both call `tail_spec`, a pure function of the same `&LetteringRequest`,
  in the same call. `grep` for `style.tail` / `tail_kind` outside `tail_spec` finds only
  `LetteringKindExt::tail_kind`, which reports the *default* table (`style_of(kind).tail`) and cannot reach
  a plan; `LetteringKindExt`'s tests pin it to `is_spoken()`, and the module head's table is the same
  statement in prose.
- Both entry points. `new_balloon` (`:281`) and `refit_balloon` (`:311`) call `ruler_findings` then
  `assemble` with the same request; nothing between them mutates it (it is `&`). `assemble` can fail on
  `text_frame` — and then there is no plan at all, so there is nothing to disagree with.
- Every kind, both limbs. The five reachable states, walked for all six kinds through both entry points:
  spoken + style tail + finite target → tail `Some`, no tail finding; spoken + style tail + non-finite
  target → tail `None` + "the tail target is not a finite point"; spoken + style tail + no target → tail
  `None` + "no tail target was given"; spoken + a style with no tail → tail `None` + "this style has no tail
  to give a spoken lettering"; a kind with nobody to point at → tail `None` + nothing, whatever the style
  and target say. The `is_spoken()` conjunct in `ruler_findings` is load-bearing, not belt-and-braces: drop
  it and every caption is reported for a tail it does not have — which is what mutation row 3 shows (four
  inline tests and my new one turn red).
- After construction. `moved_by` is the only mutation of an existing plan: `tail` is carried through
  `after_outline_move` when it is `Some` and stays `None` when it is not, `findings` is cloned, and a
  non-finite delta is refused. So the pair cannot drift there either. The one remaining way to hold a
  disagreeing plan is to write `LetteringPlan`'s public fields by hand, which is a property of it being a
  public value type rather than a path through this tree — the same reservation rounds 1 and 2 recorded.
- The message is actionable, and distinguishably so. The three reasons are three distinct sentences
  ("this style has no tail to give a spoken lettering" / "no tail target was given" / "the tail target is
  not a finite point"), each naming the thing to change, and each also naming the consequence in the record
  (`schema §6.4 requires tail_node_id for the kinds that have a tail`), so a creator knows *why* it matters
  and not only *that* it does. My new `r3_the_missing_tail_finding_names_the_reason_it_is_missing_for`
  asserts, per case and per entry point, that exactly one tail finding is produced, that it contains its own
  reason, that it contains **neither of the other two**, and that a spoken kind with a usable target
  produces none — so the three cannot collapse into one message later.

R2-F2's choice: **right, and the one place it could bite is named.**
- Presence as a function of the kind alone is the correct shape, because `is_spoken()` is not this tree's
  opinion: it is the *same predicate* rule 8 uses to decide who owns the words (`comic-model`
  `entities.rs`), and §6.4's "required for kinds that have a tail" is the schema saying its set out loud.
  A tail gate built from any second list could drift from it; this one cannot.
- Form still varies by style, which is where the variety belongs: a thought balloon keeps its bubbles
  because its own style says `Bubble`, and the same balloon is drawn with a wedge when a caller gives it a
  shout's style — both asserted in my new test. So a kind's tail is *not* invariant, only its presence is.
- The case the brief asks for — should a kind's tail ever vary by style *presence*? — is answered by the
  schema, not by taste: a `sound_effect` with a tail would be legal (rule 7 only asks that a non-null
  `tail_node_id` resolve) but is not required, so declining to produce one costs a record nothing. What it
  does cost is the ability to draw a burst with a tail at all through this API (R3-N3): there is no
  override, since `tail_spec` is the only path and it asks the kind. If M1 step 5 or the UI ever needs a
  tailed sound effect, that is a decision for the module head's table and the schema's wording, and it
  should be made there rather than by re-opening this gate. As it stands the code matches the contract it
  documents, which is what round 2 asked for.

The F6 pin: **it holds the property it claims, and it is not a restatement of `downward_probe`.**
- **F6a re-run: caught.** With `attach_point`'s guard removed (`probe = tip`, `tail.rs:145`) both
  `a_non_finite_target_is_answered_by_the_downward_probe_and_not_by_nearest` and
  `a_tail_built_from_a_non_finite_target_hangs_from_the_outline_not_from_the_centre` fail; the pin's own
  message is `Ellipse: a non-finite target did not become the downward probe — left: (100.0, 140.0), right:
  (160.0, 180.0)`, and the older test's is `the base is under the balloon: (100.0, 140.0)`. That is the same
  pair round 2 recorded, reproduced rather than read: the pin asserts the *guard* (a NaN target is
  **replaced** before `nearest` sees it, so `attach_point(outline, NaN, bounds) == nearest(downward_probe(bounds))`)
  and separately that it is **not** `nearest(NaN)`. It cannot pass without `attach_point` consulting
  `downward_probe`, and it fails if the probe is dropped — so it is a property, not a restatement of the
  point `downward_probe` returns.
- **"the path's first anchor" is now measured, not inherited.** A temporary probe test
  (`tests/r3_scratch_probe.rs`, created, run once, deleted) printed, for the 120×80 box at (100,100):
  `nearest(NaN)` equals `anchors().next()` exactly for all four shapes — Ellipse `(100,140)`, Cloud
  `(148.42,108.68)`, Box `(100,100)`, Burst `(160,100)` — while the guarded base is `nearest(downward_probe(bounds))`
  — Ellipse `(160,180)`, Cloud `(156.62,173.88)`, Box `(160,180)`, Burst `(157.22,172.07)`. Different in
  every one, and all four on the outline, which is why an attachment-only assertion (my round-1
  `finding_6_…`) passes under F6a and theirs does not. Round 2's "first anchor" claim is therefore
  verified for all four shapes, not one.
- **The half it does not hold, now held elsewhere.** The guard is `tip.x.is_finite() && tip.y.is_finite()`,
  and `(NaN, NaN)` cannot tell `&&` from `||` (both non-finite is `false || false` → the probe either way).
  Mutating it to `||` leaves all **172** inline tests green — the pin included — and my new
  `r3_a_target_with_one_unusable_coordinate_takes_the_same_downward_probe` is the only test that fails, for
  four partially-unusable targets × four shapes plus both tail kinds. So the pin holds its own property and
  now has a second half.
- One thing stated for accuracy: the pin's third assertion (`attach != nearest(NaN)`) is implied by its
  first two, since `nearest` is deterministic and the two answers differ. It is documentation of the
  distinction rather than an independent check — which is fine, and honest about what the pin is.

Mutation table: **executed, one change at a time, each restored immediately and verified with
`sha256sum -c` against `files/r3-baseline.sha256` before the next.** Baseline:
`lettering/fonts.rs 10e9c2b6…`, `lettering/mod.rs 2ec895f0…`, `lettering/outline.rs 1fd2ac8c…`,
`lettering/plan.rs 8f7d6bd1…`, `lettering/style.rs 25eb75c3…`, `lettering/tail.rs 714cc582…`,
`fit/mod.rs 99052833…`, `fit/policy.rs 35ae1042…`, `fit/sizing.rs f68a4e49…`; all nine `OK` after every
restore. **A mutation was in the tree during each of these runs and nowhere else** — the tree is shared,
so it is said plainly: any red or un-compilable crate state a reader saw between 10:16 and 10:35 was
either mine (restored) or the W2 reviewer's `round3_probe.rs`/`round3_reviewer.rs` transients listed above.

| Mutation (one at a time, restored immediately) | Their suite | My file |
|---|---|---|
| **R2-F1** reverted: the finding's guard back to `is_spoken() && (style.tail.is_none() \|\| tail_target.is_none())` (`plan.rs:366`) | **caught** — `a_tail_target_that_is_not_a_point_is_reported_and_never_silently_dropped` | **caught** — `r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported` |
| **R2-F2** reverted: `request.kind.is_spoken() &&` dropped from `tail_spec` (`plan.rs:402`) | **caught** — `the_kind_decides_whether_there_is_a_tail_and_the_style_only_which_form` | **not caught before this round**; caught now — `r3_a_kind_with_nobody_to_point_at_gets_no_tail_whatever_its_style_says` |
| **R2-F2b**: `kind.is_spoken() &&` dropped from the *finding* guard (`plan.rs:366`) | **caught** — 4 inline tests (`a_caption_is_a_box_with_no_tail_and_its_own_text`, `a_spoken_kind_wearing_a_tailless_style_is_reported`, `a_tail_target_that_is_not_a_point_is_reported_and_never_silently_dropped`, `the_kind_decides_…`) | **caught** — the same new test |
| **R2-F3** reverted: `!face.style.trim().is_empty()` re-added (`fonts.rs:124`) | **caught** — `a_blank_family_is_not_a_missing_font_and_a_blank_style_is_reported_as_the_exporter_reports_it` | **not caught before this round**; caught now — `r3_the_face_report_filters_on_the_exporters_terms_and_no_more` |
| **F6a** re-run: non-finite tip → `probe = tip` (`tail.rs:145`) | **caught** — `a_non_finite_target_is_answered_by_the_downward_probe_and_not_by_nearest` (`left: (100.0, 140.0), right: (160.0, 180.0)`) and `a_tail_built_from_a_non_finite_target_hangs_from_the_outline_not_from_the_centre` (`the base is under the balloon: (100.0, 140.0)`) | **not caught** — `finding_6_…` is attachment-only, exactly as round 2 recorded |
| **F6c** (new): `tip.x.is_finite() && tip.y.is_finite()` → `\|\|` (`tail.rs:145`) | **not caught by any of the 172 inline tests** | **caught** — `r3_a_target_with_one_unusable_coordinate_takes_the_same_downward_probe` |

Rows 2 and 4 are the two gaps in *my* file that this round closes; row 6 is a gap in the implementer's
that neither file had. Row 5 is unchanged from round 2 and is the honest half of the pair: the pin catches
the mutation, my reproduction does not, and no test in either file is weakened to make that untrue. No
mutation was outstanding when this record was written; the nine digests were re-verified and the
concatenated digest recomputed as `dc0636fd…`.

Semantic defects: **no — no remaining finding in this round is semantic, and R2-F1's class is empty.**
- A semantic defect, in this record's sense, is a plan whose data says one thing and whose record cannot be
  written as the schema requires, produced silently. Walking every state of the five above for all six kinds
  through both entry points, and through `moved_by`, finds none: the only state that was ever in that class
  (`speech` with `tail: None`) is now either impossible or reported, and the report and the geometry come
  from one predicate.
- Explicitly, the three notes cannot produce a wrong plan or a wrong record. R3-N1 is a doc sentence whose
  code is correct and tested; R3-N2 is a coupling that today is exactly right and is pinned by tests on all
  three arms; R3-N3 removes a capability the tree's own table never offered, and every plan it declines to
  produce is one `plan.tail: None` plus nothing, which is what §6.4 and the module head both expect of a
  sound effect.
- What the fixes introduced semantically, checked one by one and all correct: the `is_spoken()` gate in
  `ruler_findings` (load-bearing, row 3); the kind gate in `tail_spec` (right, R2-F2 above); the
  `&&` in the finiteness test (right, and now pinned in both directions, rows 5 and 6); the reporting
  route for a *present but unusable* target (right for the reason in the R2-F1 row); the finding's severity
  and subject (`Finding::warning(None, subject_of(kind), …)` — a `Warning`, `rule: None`, `lettering
  speech` — unchanged from what `every_finding_a_plan_or_a_policy_returns_is_a_warning_and_never_a_rule`
  and `the_kind_names_are_the_schemas_own_strings` already pin); and one finding, not two, per plan (the
  `is_spoken()` conjunct keeps the caption path silent, row 3).
- Nothing else moved: `fit/` is byte-identical to round 2, `moved_by`'s translate arm and the
  `AreaFit`-vs-`AreaFit::None` split are untouched, and the layer check still passes, so the contract
  (no `Document`, no `&mut`, no minted `NodeId`, the plan holding no text) is as rounds 1 and 2 found it.

Unverified:
- **The exporter is still read, not run.** `missing_fonts`' filter is quoted from
  `crates/engine/src/cmd/fontfiles.rs:111` and the two filters are compared term for term; I did not build
  `engine` and compare two reports of one document, so a divergence *above* the filter (in `used_fonts`,
  say) would not show up here. R2-F3's fix is verified against the source, as round 2's was.
- **No consumer outside this crate is modelled.** `LetteringPlan.tail` and the new findings have no caller
  in `crates/` or `apps/` yet (the command layer is M1 step 5); `tail_spec`, `is_point` and
  `missing_tail_message` are private, so nothing outside this tree can reach them. R3-N3's cost is therefore
  a cost to a caller that does not exist yet, and the `sound_effect`-with-a-tail question is a design
  decision to be taken when one does.
- **`nearest`'s NaN behaviour is measured, not contracted.** I measured it for all four shapes on one box
  (the temporary probe above); that `nearest` answers a NaN query with the first anchor is the geometry
  crate's behaviour, not a promise this tree makes, which is exactly why the guard replaces the probe rather
  than relying on it. I did not enumerate boxes other than (100,100)–(220,180).
- **The fonts-present run is not the gate's configuration** and was run for the two facts it establishes
  (my four new tests do not depend on `CRAFT_FONTS_DIR`, and the conditional font test still asserts the
  empty report for all six kinds when the faces are present). Nothing else in W3's fixes touches faces, and
  the recorded gate result is the empty-fonts run.
- **`round3_probe.rs` and `round3_reviewer.rs` are read, not reviewed.** Both are the W2 reviewer's
  (`template`/`validate`/`layout`), they pass now, and their subject is out of W3's scope; I checked only
  that neither is reachable from anything W3's fixes touch.
- **A tail whose target a document cannot supply** (an edited `tail_node_id` pointing at a node with a
  non-finite position) is R2-F1's trigger seen from the document side; I state it from this crate's public
  API only, having not built the resolver.
