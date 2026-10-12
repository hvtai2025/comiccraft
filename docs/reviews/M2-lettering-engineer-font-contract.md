# M2 W4 — `lettering-engineer`, the `CRAFT_FONTS`-empty contract and the tail a move preserves

MODEL:         `deepseek-v4.1-flash`. Stated by the environment (`COPILOT_MODEL`) and by nothing
               else: `~/.copilot/session-store.db` › `assistant_usage_events` holds **632** rows for
               other sessions and **0** for this one's `COPILOT_AGENT_SESSION_ID=dee5ba9e-dc29-479f-ad08-9c4ccbf3d307`,
               so the environment's word is the only witness. The same gap M1's records state, stated
               rather than papered over (M1 §5.4e).

Revision:      working tree (base `e26f030d`, "M2 plan: exit criteria, work breakdown, and the three
               genuinely new things"). `sha256sum` of every file this task changed, at the revision
               every gate and every mutation below was measured on:

```
3c6704cea7a32893761bfe2c07028247e483cda08b2ba32b620d0de7b1c71c9a  crates/comic-layout/src/lettering/fonts.rs
d2996b8997d5a655b67e79fa868e22d8bd955a623960da10222035a7c4ffa9fe  crates/comic-layout/src/lettering/plan.rs
773aa68a5decbfa9396335458b1bff74b9aaec9b2adec6a9b11e9dcfce6be3fe  crates/comic-layout/src/lettering/mod.rs
502cfbcf3d88de64f810601a9feadab9d340ccd201aeee4ffee5f890c1a70d38  crates/comic-layout/tests/w4_font_contract.rs
e1cfda1fe37717ff04a53498336977186a0e04e0863d40b082567ba6103c15e7  crates/comic-layout/tests/w4_balloon_move_tail.rs
aa7f7af442327bcce2612a88d19d19eee1dd8cb7015757bc9c329ba24bfe8f33  crates/comic-layout/tests/w4_kind_policy_matrix.rs
```

Scope:         W4's module tree only — `crates/comic-layout/src/{lettering,fit}/**` and three new
               integration files of my own. `src/lib.rs`, `Cargo.toml` and every shared type are the
               lead's and are untouched: the diff is 3 source files (+146/−10) and 3 new test files
               (804 lines). Nothing under `crates/comic-model/**` (the `schema-architect`) and nothing
               under any other `crates/*/tests/**` file, `*_reviewer.rs`, `comic_acceptance*` or
               `crates/engine/tests/comic_commands_r*.rs` was read-and-left or written — my three new
               files are `w4_*`, and the two files that appeared in this crate's `tests/` during the
               session (`m2_panel_cycle.rs`, `m2_reading_order_independence.rs`) are the other owner's
               and only ran, never edited.

Design refs:   design §8.3 (lettering: the overflow policy, "reported, not applied", the missing-font
               report, "a project whose lettering font is missing must surface exactly the missing-font
               report the exporter already produces"), §6.1 (one owner for the words), §12.1 (text
               overflow and missing lettering font are deterministic checks), §4 (determinism);
               M2 §4.3 and §1's criterion table; schema §5.3 (`default_lettering_style`),
               §5.5 (`LetteringKind`, `OverflowPolicy`), §6.4 (`tail_node_id`), §6.6 (per-lettering
               `font`); M1's own record, `docs/reviews/M1-lettering-engineer-lettering.md` (rounds 1–3),
               so nothing settled there is re-litigated here.

---

## 1. What was already true, and what was not

M1's record ends *approved* on W3, and the parts of these three criteria that M1 settled are still
settled: the six-kind table, the unified `tail_spec` predicate, the downward probe for an unusable
target, the exporter's filter terms, the `Measured`-in-the-returned-frame fix (F1), the collision rule.
None of that is re-opened, re-tested or re-worded here.

What M1's own round-3 record names as the gap is exactly what this task closes:

* **Criterion 5 was a habit, not a contract.** The only test that spoke to "`CRAFT_FONTS` empty" was
  `a_missing_lettering_face_is_reported_without_stopping_the_plan` (`plan.rs`), which is *conditional*:
  `assert_eq!(reported, !comic_neue_available)`. In a build with the comic faces embedded it asserts the
  **empty** report and exercises nothing about absence. The reviewer's
  `the_missing_face_report_is_conditional_on_the_face_really_being_absent` is the same shape. M2 §4.3:
  *"M1's `CRAFT_FONTS`-empty record-level verification being mistaken for the real thing … is the exact
  shape of every defect in M1 — a guarantee asserted, not delivered"*.
* **Criterion 3's existing tests were attachments, not the criterion's two halves.** The reviewer's
  `attachment_holds_across_a_document_scale_move` and `the_plan_hands_on_an_editable_closed_outline_and_a_translated_frame_path`
  do move a plan and do assert it stays attached (for `speech`, at huge deltas), and the reviewer's F3
  reproduction pins that the outcome moves with the box. What no test did: assert the **tip preserved**
  *and* the **base updated** as two named facts, check the base's **value** against the geometry, tell a
  re-base on the path apart from a re-base on the box (mutation **C**), or cover all four spoken kinds,
  the degenerate move onto the speaker, and the kinds with no tail. The reviewer's attachment test cannot
  see the path-vs-box difference, for the reason §6 records: the plan it moves has `frame == shape.outline(bounds)`,
  so both derivations give the same point.
* **Criterion 2 had never been checked at its own wording.** No test enumerated **kind × policy**
  through the plan: M1's reviewer tests
  `every_finding_a_plan_or_a_policy_returns_is_a_warning_and_never_a_rule` and
  `fit_outcomes_agree_with_the_engine_and_with_the_box_that_comes_back` iterate the three policies over
  **one** hard-coded style, at the `fit::policy` layer rather than over the six kinds' own styles. Which
  cells of the 6 × 3 matrix were covered, and which of them claim only record level, had never been
  stated.

## 2. The change (production, 3 files)

**`src/lettering/fonts.rs` — the contract gets a name.**

```rust
pub fn lettering_face_report(db: &FontDb, faces: &[Face], subject: &str) -> Vec<Finding>
```

One call, documented as the contract, and re-exported as `lettering::lettering_face_report`. It is
`missing_face_findings(&missing_faces(db, faces), subject)` — **no behaviour change**, which is the
point: the contract already held in the code, and what was missing was a *name the tests and a reviewer
can cite*. The two guarantees the doc states are the two the criterion needs, and a report that produced
only one of them is not the contract:

1. **the report is produced** — the requested family and style, the status and the face shown instead,
   which are the four fields `text.missingFonts` reports; and
2. **the layout still runs** — a missing face is neither an error nor a refusal.

`ruler_findings` (`plan.rs`) now calls this and nothing else for the face half, so a substitute face that
skipped the report would have to skip the contract *by name*.

**`src/lettering/mod.rs` — `LETTERING_KINDS`.** The six kinds in schema order, in one `pub const`, so
"every lettering kind" (criterion 2) and "the faces the kinds ask for" (criterion 5) are enumerated where
the tests can reach them instead of a hand-copied array per test file. The compiler's guarantee is
`style_of`'s exhaustive match; the new `every_kind_has_a_style_a_name_and_a_place_in_the_list` test
matches `LetteringKind` with **no `_` arm** and asserts each kind's index in the list, so a seventh kind
stops `mod.rs` compiling until the list, the style table and the matrix have all grown.

**`src/lettering/plan.rs`** — the one-line call site, and four new inline tests (below).

## 3. Criterion 5 — `CRAFT_FONTS` empty is a *tested* contract

### 3.1 The contract, and the report the missing face produces

The gate's own configuration (`CRAFT_FONTS_DIR` unset) reports, for the six kinds' faces, **exactly four**
findings — the six kinds share four distinct faces (raw, from the run below):

```
the lettering face Bangers Regular is not available (missing) and shows in Source Sans 3 Regular; the comic faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there
the lettering face Comic Neue Bold is not available (missing) and shows in Source Sans 3 Bold; the comic faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there
the lettering face Comic Neue Italic is not available (missing) and shows in Source Sans 3 Italic; the comic faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there
the lettering face Comic Neue Regular is not available (missing) and shows in Source Sans 3 Regular; the comic faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there
```

Each is `Finding::warning(None, "lettering <kind>", …)` — a `Warning`, `rule: None`, at most one per
face, deduplicated and sorted. With `CRAFT_FONTS_DIR` set to a craft-fonts checkout the same call reports
**0** for those faces. Both counts were **measured, not assumed**: a temporary probe printed the report in
each configuration and was then deleted.

### 3.2 The test that reds if a substitute face is used

`crates/comic-layout/tests/w4_font_contract.rs`, 7 tests. The load-bearing one is
`a_missing_lettering_face_is_named_in_the_report_and_never_left_to_a_silent_substitute`, and it is
**not conditional**: it asks for `No Such Lettering Face` in style `Bold`, a family no build can have, so
the assertions hold with the comic faces present *and* absent. It asserts:

* the report is **exactly one** finding, a `Warning`, subject `lettering speech`, rule `None`;
* it **names the requested family and style** — the two things a silent substitution replaces — and the
  status string and the face shown instead, i.e. the exporter's four fields;
* the same `Finding`s are what `lettering_face_report` returns directly, and `MissingFace.resolved` names
  the substitute *as* the substitute;
* through **both entry points** (`new_balloon`, `refit_balloon`) the plan reports it **and** still lays
  the text out: `plan.fits()`, a closed frame, a positive finite `text_bounds`, the tail still there, and
  no other finding.

`the_layout_is_the_fallback_face_is_layout_and_the_report_is_the_only_difference` closes the other half of
the same idea: two plans asking for two *different* absent families have **byte-identical geometry**
(`bounds`, `frame`, `text_frame`, `text_bounds`, `outcome`, the tail path) and differ in exactly one
thing, the finding naming the family each asked for. A silent substitution collapses that test to two
empty reports.

`every_kind_names_its_own_absent_face_through_both_entry_points` runs the same contract over
`LETTERING_KINDS`, asking for the absent family **in each kind's own style**, and asserts that style is
the one named (`Italic` for `narration`, `Regular` for `sound_effect`), so a report that named a constant
cannot pass.
`every_kind_face_that_is_not_available_exactly_named_is_reported_and_no_other_is` is the
**biconditional** — reported ⟺ not available exactly, with the left side read from the finding text and
the right side from `FontDb::resolve` — which keeps the empty case asserted in the gate configuration and
not silently skipped in the other. `a_family_that_is_present_without_the_style_asked_for_is_a_substitute_and_not_an_absence`
pins the exporter's `FontMatch::Style` half at the contract's own door.
`the_contract_keeps_the_exporters_own_exclusions_at_its_own_door` pins the CSS-generic and `MAX_NAME`
exclusions there too, plus the deliberate non-filtering of a blank style (M1 round 2, R2-F3).

Two inline tests carry the contract in the implementer's own suite
(`fonts.rs`: the requested-name assertion and the substitute/absence distinction; `plan.rs`:
`the_face_contract_holds_for_a_face_no_build_can_have`), and `a_moved_balloon_still_says_which_face_is_missing`
joins criteria 5 and 3: a drag does not lose the message.

**What reds it:** mutations **A** and **B** below. A (the report dropped entirely — the silent fallback)
reds **5 of the 7** new tests, **2** inline tests and the M1 reviewer's conditional test; B (*the
substitute face reported in place of the requested one*) reds 4 of the 7 and the same inline pair. Both
are exactly the failure modes M2 §4.3 names, and both are caught by name.

## 4. Criterion 3 — moving a balloon preserves **or updates** its tail without detaching it

### 4.1 Which move, and why

The move a balloon gets in this layer is **`LetteringPlan::moved_by(delta)`** — the drag the command
layer performs on a balloon, which translates the frame *path*, the box, the text frame, the measured
block and the `FitOutcome` that names a box, and re-bases the tail on the path the balloon now has.

The other way a balloon's geometry changes is `comic.panel.move` (`crates/engine/src/cmd/comic/panel.rs`),
which translates every object the panel owns — the tail included — through `d.move_node`. That one moves
the tip *with* the balloon because the speaker moves with the panel, it is the command layer's, and it is
**not** exercised here. There is no `comic.lettering.move` command, so `moved_by` has no engine caller
yet: stated in §7 rather than implied by a test that does not name it.

### 4.2 Preserved, and updated — with the alternatives stated

| Fact | After a move | The alternative, and why it is wrong |
|---|---|---|
| `Tail.tip` | **preserved** — the balloon moved, the speaker did not | moving it with the balloon makes the balloon point at the wrong place, and at nothing in the common case where the speaker is outside the balloon's new position |
| `Tail.attach` | **updated** — the point of the *moved path* nearest the tip | keeping the old base **detaches** the tail (the failure the criterion names); regenerating it from `bounds` attaches it to a shape the balloon does not have, because the frame is the editable path a creator has dragged |

**"Still attached"** is asserted as `Tail::is_attached_to_outline(&plan.frame)` — the object
`Lettering.frame_node_id` names — *and* as `Tail::is_attached_to(plan.bounds, shape)`, and every test also
asserts the tail has **not been left behind** by checking it is **not** attached to the outline the
balloon used to have (the move is 700×500pt, farther than the widest of the six maximums, so the two
outlines are disjoint and the assertion is about the move and not about overlap).

`crates/comic-layout/tests/w4_balloon_move_tail.rs`, 5 tests:

* `a_move_keeps_each_spoken_kinds_tail_on_the_balloon_and_its_tip_on_the_speaker` — **all four spoken
  kinds** (`speech`, `thought`, `shout`, `whisper`: ellipse, cloud, burst, ellipse, and both tail forms —
  a wedge and a bubble chain). Asserts the moved plan carries a tail of the same `TailKind`; the tip is
  **preserved exactly**; `attach` equals `attach_point(&moved.frame, tip, moved.bounds)` (the *value* the
  geometry gives, so a different derivation is a different value rather than a different story) and
  **differs from the base it had**; it is on the moved frame and not on the old one; the frame moved and
  only moved (`transformed(Affine::translate(delta))`, same anchor count, still closed); and nothing else
  changed — kind, text owner, policy, `AreaFit`, layout settings, outcome, findings.
* `a_move_re_bases_the_tail_on_the_path_the_balloon_has_and_not_on_its_box` — the discriminating test.
  The plan's `frame` is replaced by a **box** outline (`BalloonShape::Box.outline(bounds)`) while the
  kind's shape is an ellipse — exactly the state a dragged anchor produces. The two candidate bases are
  asserted to **differ** (`assert_ne!`), the tail is asserted to be on the *path* one, not on the box's
  ellipse, and the edited path is asserted to have been translated rather than rebuilt.
* `a_move_onto_the_speaker_leaves_a_tail_that_is_still_attached_rather_than_a_degenerate_path` — the
  balloon dragged onto its own target: the base is still the moved path's nearest point, the tail is
  still attached, its path still has bounds, and the tip is normalised to a reachable one rather than
  left as a zero-length or NaN path.
* `a_move_neither_invents_a_tail_nor_loses_one_a_balloon_never_had` — the two kinds with nobody to point
  at, and a spoken kind whose style the project stripped of a tail: still none after the move, and the
  report about the missing tail is unchanged.
* `a_move_by_a_non_finite_vector_is_refused_rather_than_applied_partially`.

**What reds it:** mutation **C** — `after_outline_move(&frame, bounds)` replaced by
`after_balloon_move(bounds, shape)`: **only this file catches it** (2 tests). The 176 inline tests stay
green and the M1 reviewer's 31 stay green, which is precisely the gap criterion 3's wording is about.
Mutation **D** (the tail copied verbatim, never re-based) reds 3 here, 2 inline and the reviewer's
`attachment_holds_across_a_document_scale_move`; mutation **E** (the tip moved with the balloon) reds 3
here and 1 inline.

## 5. Criterion 2 — every kind × every policy, and which cells are record-level only

`crates/comic-layout/tests/w4_kind_policy_matrix.rs`, 4 tests, iterating `LETTERING_KINDS` ×
`{Warn, Shrink, ResizeFrame}`:

* **renders** — 6 kinds × 3 policies, all 18 cells: a plan exists, the frame is a closed editable path
  with ≥ 4 anchors, the text frame is inside the balloon, the block is finite and positive and laid out
  by the engine, the `AreaFit` is the one the policy asks for (`shrink` → `ShrinkText`, the other two →
  `None`), `text_layout.fit_to_frame` matches the policy, the tail exists exactly for the spoken kinds,
  and there is no finding when the text fits.
* **wraps** — per kind, measured: a frame that holds a word (20 of the kind's own type sizes square,
  which the shape's inner scale turns into a much smaller text frame — 0.51 for a burst, 0.99 for a
  caption) wraps the sentence into 2–4 lines with `overflow == false`, every byte on some line in order
  ending at the last byte, and no line more than one character past the frame — the allowance the
  engine's own documented behaviour needs, because a word too long for the frame is broken **between
  characters**. The plan-level half asserts the same sentence is ≥ 1.5× the height of one line and that
  `text_bounds` is exactly the block the engine measured in the plan's own text frame.
  **The second regime, found while measuring:** a frame *narrower than one of the kind's words* — which
  is what the kind's own minimum box is — places the characters that fit on **one** line and sets
  `overflow`, dropping the rest. That is the state the three policies are decisions about, and it is
  asserted as such for all six kinds rather than described.
* **detects overflow and applies the policy** — 6 × 3 cells with a sentence in the kind's minimum box:
  `warn` → `Warned`, box unchanged, `AreaFit::None`, `!fits()`, exactly one overflow finding;
  `shrink` → `Shrunk { fit_scale, fits }` with `0 < fit_scale ≤ 1`, `AreaFit::ShrinkText`, the box
  unchanged, and a finding **iff** the engine's own `fits` is false; `resize_frame` → `Resized`, the
  outcome naming the box the plan carries, the text fits, no finding, the frame grown around what it was
  and inside the kind's limits.
* **the collision rule** — 6 cells: four objects touching the box on all sides, `resize_frame` →
  `Blocked`, `bounds == box` exactly, `!fits()`, one finding naming "another object", and the report
  measured in the frame of the box that comes back (round 1, F1).

**Which cells are record-level only: all of them, and that is the honest answer.** `comic-layout` computes
a plan and paints nothing, so no cell here claims more than the record: the box, the closed path, the
`AreaFit`, the policy, the findings, and the wrapping/overflow answers **read back from
`vectorcraft_text`** (`TextLayout.overflow`, `.lines`, `.fit_scale`, `.bounds`) rather than restated from
this crate's arithmetic. **Zero cells are verified at pixel level**: whether the faces draw the right
shapes is criterion 6's golden render, it is font-dependent by design (M2 §4.1), and it is W2/W6's. No
kind has a cell that claims less than the others, and no cell claims more.

## 6. Mutation table

Executed one change at a time, each restored immediately and verified with `sha256sum -c` against the six
digests above **after** the last restores (`files/w4-baseline.sha256` in the session folder; all six `OK`
at the end). Baseline `plan.rs` is `d2996b89…` throughout, so every row is a change to the revision this
record names.

| Mutation (one at a time, restored immediately) | Inline suite | M1 reviewer file | This task's files |
|---|---|---|---|
| **A** the report dropped: `ruler_findings` returns no missing-face findings — the **silent fallback that renders**, M2 §4.3's failure mode | **caught** — 2 (`a_missing_lettering_face_is_reported_without_stopping_the_plan`, `the_face_contract_holds_for_a_face_no_build_can_have`) | **caught** — 1 (`the_missing_face_report_is_conditional_on_the_face_really_being_absent`) | **caught** — `w4_font_contract` 5 of 7; `w4_kind_policy_matrix` 1 (`every_kind_renders_a_closed_frame_and_a_text_block_under_every_policy`) |
| **B** the **substitute face reported in place of the requested one**: `FALLBACK_FAMILY` + the requested style is passed to `lettering_face_report` | **caught** — the same 2 | **caught** — the same 1 | **caught** — `w4_font_contract` 4 of 7 (incl. the named-contract test and every kind's own style); `w4_kind_policy_matrix` 1 |
| **C** the tail re-based on the **box** instead of **the path the balloon has**: `after_outline_move(&frame, bounds)` → `after_balloon_move(bounds, style_of(kind).shape)` | **not caught** — 176 pass | **not caught** — 31 pass | **caught** — `w4_balloon_move_tail` 2 (`a_move_keeps_each_spoken_kinds_tail_on_the_balloon_and_its_tip_on_the_speaker`, `a_move_re_bases_the_tail_on_the_path_the_balloon_has_and_not_on_its_box`) |
| **D** the tail **not re-based at all**: `tail: self.tail.clone()` | **caught** — 2 (`moving_a_balloon_moves_its_frame_and_keeps_its_tail_on_it`, `moving_a_balloon_onto_its_target_leaves_a_path_behind_not_a_detached_tail`) | **caught** — 1 (`attachment_holds_across_a_document_scale_move`) | **caught** — `w4_balloon_move_tail` 3 |
| **E** the **tip moved with the balloon**: `retarget(bounds, shape, tail.tip + delta)` | **caught** — 1 (`moving_a_balloon_moves_its_frame_and_keeps_its_tail_on_it`) | not caught | **caught** — `w4_balloon_move_tail` 3 |

Row **C** is the row this task exists for: it is the one mutation no test in the tree caught before, and
the two tests that catch it are the criterion-3 move tests. Rows A and B are criterion 5's: the criterion's
own failure mode, caught in both shapes — *saying nothing* and *saying the wrong thing*. Raw output:
`files/w4-mutations.txt`.

## 7. Findings — reported, not fixed (the task's instruction)

Both are reported, not fixed: D1 is outside W4's module tree, D2 is inside it, and the task's instruction
is to *report* what else is found rather than to fix it unasked. Each is stated with its consequence and
the file that would carry a fix.

**M2-W4-D1 — `bug`, reported.** `crates/comic-model/src/entities.rs:581` (`ProjectSettings.default_lettering_style`)
is read by **nothing** in `crates/` or `apps/`: every occurrence but the schema's is the literal `None`
(`crates/engine/src/cmd/comic/project.rs:161`, `crates/comic-model/src/validate.rs:807`,
`crates/comic-model/src/bundle.rs:1206`, `entities.rs:1108`). Schema §5.3 introduces the field as *"what
makes the output comic-styled, and it is a name, not a font file … They are listed so a project created
without an explicit choice uses the comic faces rather than the bundled body font"*, and design §8.3
repeats it: the faces are named there *"rather than hard-coding them, so a project can choose different
faces"*. Actual: `comic.lettering.add`/`set_style` take the **kind's built-in** style (`style_of`) unless
the caller passes an explicit `style` object, and the comic faces **are** hard-coded in `style_of`.
Consequence: (a) a project that chooses a different lettering face gets the built-in one and no message;
(b) criterion 5's report is always about the *kind's* face, never about the project's chosen one — the
half of design §8.3's sentence that says a project's own missing lettering font must surface the report is
unreachable. The consumer would be the command layer (`crates/engine/src/cmd/comic/lettering.rs`, W3) or
the model; `crates/comic-layout` is ready for it (`LetteringRequest::with_style` is the seam, named in W3's
own M1 record).

**M2-W4-D2 — `bug` (a false statement in user-visible text), reported.**
`crates/comic-layout/src/lettering/fonts.rs`, `missing_face_finding`'s message ends *"the comic faces
belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there"*. That clause
asserts a **cause** that is false whenever the missing face is not one of the comic faces — a project
naming a font of its own that is not installed, in a build that *did* embed craft-fonts, and equally in
one that did not, since the sentence names `CRAFT_FONTS` as the reason **this** face is missing. The four
data fields of the report (family, status, resolved, subject) are correct in that case; only the
explanation is wrong, so this is a wording defect rather than a wrong predicate. Not fixed here: the task's
instruction is to report, and the sentence is one the reviewer may want worded once, in one place, with
the exporter's own vocabulary.

**M2-W4-N1 — `note`.** `missing_face_finding` names `FALLBACK_FAMILY` as the face "shown in" when
`MissingFace.resolved` is `None`. `resolve` returns `None` only when no face at all is loaded (the bundled
fonts failed to parse), in which case `Source Sans 3` is not the face shown either. Unreachable in a
working build; stated so the fallback is not mistaken for coverage.

**M2-W4-N2 — `note`.** `LETTERING_KINDS` is a hand-maintained list of six. The structural half is the
compiler's (`style_of`'s exhaustive match) and the new inline test's no-`_`-arm match, which forces the
list to grow when the schema does; the residual half is that a test which iterates the list would still
*pass* with a seventh kind missing from it, since the missing kind is simply absent from the iteration.
The no-`_`-arm match is what turns that into a compile error rather than a silent gap — stated because it
is a coupling, not a guarantee.

**M2-W4-N3 — `note`.** `MAX_NAME` and `MAX_FACES` are reachable as `lettering::fonts::{MAX_NAME, MAX_FACES}`
but are not re-exported at `lettering::`, unlike `missing_faces` and `lettering_face_report`. Purely an
API-shape inconsistency; my integration test imports the module path rather than asking for a re-export.

## 8. Raw gate output

`cargo fmt -p vectorcraft-comic-layout --check` → **exit 0**, no diff. The other three, as run (the gate's
configuration — `CRAFT_FONTS_DIR` unset — `--no-fail-fast`):

```
$ env -u CRAFT_FONTS_DIR cargo test -p vectorcraft-comic-layout --no-fail-fast
     Running unittests src/lib.rs (target/debug/deps/vectorcraft_comic_layout-71eed0acaf635fbf)
test result: ok. 176 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s
     Running tests/layer_rules_contract.rs (target/debug/deps/layer_rules_contract-80fe31053d387078)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/lettering_round1_reviewer.rs (target/debug/deps/lettering_round1_reviewer-878c02f2b0e51cef)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/m2_panel_cycle.rs (target/debug/deps/m2_panel_cycle-f8338557f83e68ca)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/m2_reading_order_independence.rs (target/debug/deps/m2_reading_order_independence-883f3c1fb85752f9)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/panel_rect_contract.rs (target/debug/deps/panel_rect_contract-ec5b0194171bb2e7)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/round1_reviewer.rs (target/debug/deps/round1_reviewer-b003a5598ed2874a)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/round2_reviewer.rs (target/debug/deps/round2_reviewer-87daecb0edd2ea8a)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round3_reviewer.rs (target/debug/deps/round3_reviewer-7f1627fb770352f4)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round4_layout_engineer.rs (target/debug/deps/round4_layout_engineer-4e1975be8d550997)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round4_reviewer.rs (target/debug/deps/round4_reviewer-d71d880f430618dc)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/round5_layout_engineer.rs (target/debug/deps/round5_layout_engineer-b874da3d17ee36be)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/w4_balloon_move_tail.rs (target/debug/deps/w4_balloon_move_tail-d19cf424889d5a25)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/w4_font_contract.rs (target/debug/deps/w4_font_contract-48c4c3f37a543679)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/w4_kind_policy_matrix.rs (target/debug/deps/w4_kind_policy_matrix-9387a4eaa6270963)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
   Doc-tests vectorcraft_comic_layout
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**314 tests, 0 failures** (M1 ended at 289: +4 inline and +16 integration are this task's; the remaining
+5 are the other owner's two new files, `m2_panel_cycle.rs` 1 and `m2_reading_order_independence.rs` 4).

```
$ env -u CRAFT_FONTS_DIR cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings
    Checking vectorcraft-comic-model v0.8.0 (/home/hvt/web/comiclab.io.vn/comiccraft/crates/comic-model)
    Checking vectorcraft-comic-layout v0.8.0 (/home/hvt/web/comiclab.io.vn/comiccraft/crates/comic-layout)
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.75s

$ cargo xtask layers
OK: 29 crates, no layering violations.

$ cargo xtask brands
brands: no vendor names in user-visible text
```

**0 clippy diagnostics**, including the three new test files. `clippy::panic` and `clippy::expect_used`
are `deny` workspace-wide with `allow-*-in-tests`, which covers `#[test]` bodies but not free helpers —
so the helpers in the new integration files return `Result<_, FitError>` and the `#[test]` bodies are
where a failure is allowed to be a panic.

**The other configuration.** With `CRAFT_FONTS_DIR=/home/hvt/web/comiclab.io.vn/craft-fonts` in a
throwaway `CARGO_TARGET_DIR` (run and then removed), the three new files are **5 / 7 / 4 all green** and
the crate's inline `lettering::` and `fit::` suites are **66 / 36 green**. So none of the 20 tests this
task adds depends on the configuration: the biconditional in
`every_kind_face_that_is_not_available_exactly_named_is_reported_and_no_other_is` takes its
*no-report* branch, and the impossible-family tests take theirs. The tree is left in the gate's
configuration (empty), which the recorded run above rebuilt and re-ran.

**Two transients from the concurrent owners, both cleared.** (i) `vectorcraft-comic-model` failed to
compile twice mid-session (`no `migrate` in the crate root`, a missing `migrations` field) — the
`schema-architect` writing `bundle.rs`/`lib.rs`; re-run, green. (ii) `crates/comic-layout/src/validate/mod.rs`
hit clippy's `expect_used` and `template`/`validate` tests were red for a stretch — the `layout-engineer`
mid-refactor; both cleared on re-run, and the final gate above is the complete command with every target
green. Neither is reachable from anything this task touched, and nothing here was re-run around a failure
of mine: every red in this record is either a mutation's (restored) or another owner's (cleared).

## 9. Unverified

* **The exporter is read, not run.** `MissingFace` is field-for-field `MissingFont`
  (`crates/engine/src/cmd/fontfiles.rs`) and `lettering_face_report` composes the same filter, but the two
  reports are never compared **of one document**: `vectorcraft-comic-layout` cannot depend on
  `vectorcraft-engine` without a layering violation, so the equivalence is established by reading the
  source, as M1's reviewer established it.
* **No caller outside this crate is modelled.** `comic.lettering.*` does not call `moved_by` (there is no
  `comic.lettering.move`), so criterion 3's move is the layout layer's own; the engine's balloon-moving
  path (`comic.panel.move` → `d.move_node` over the objects the panel owns) is W3's and is not exercised
  here. Whether a *future* caller re-derives the frame from `FitOutcome::Resized` before moving is
  likewise untested from this side.
* **No pixels.** Nothing in this record verifies that a face draws the right shapes — criterion 6's
  golden render is W2/W6's and is font-dependent (M2 §4.1). Every criterion-2 cell is record level, as §5
  says in its own words.
* **The frame sizes in the wrap test are this task's choice, not a contract.** 20× the kind's own type
  size is big enough for all six kinds to place the whole sentence and small enough that none places it
  on one line; both are asserted, so the number cannot silently stop being right for a *different* kind,
  but a kind whose inner scale were under ~0.25 would need a bigger frame and the test would say so by
  failing rather than by adapting.
* **`vectorcraft_text`'s fallback metrics are another crate's promise.** My tests assert that two plans
  with *different absent faces* resolve to the same face (so their geometry is identical), not that the
  fallback's numbers are stable across machines.
* **Windows untested** — M1's carried note, unchanged, and it is the configuration in which a system
  font could shadow one of the names these tests use. The names chosen to be absent are deliberately not
  plausible font names (`No Such Lettering Face`, `No Such Weight 4f2a`), which is the mitigation
  available without a Windows runner.
* **`CRAFT_FONTS_REQUIRED=1`** (a bad checkout made a build error rather than a warning) is not exercised:
  every run here had the variable unset.

Revision: `crates/comic-layout/src/lettering/fonts.rs` =
`3c6704cea7a32893761bfe2c07028247e483cda08b2ba32b620d0de7b1c71c9a`,
`crates/comic-layout/src/lettering/plan.rs` =
`d2996b8997d5a655b67e79fa868e22d8bd955a623960da10222035a7c4ffa9fe`,
`crates/comic-layout/src/lettering/mod.rs` =
`773aa68a5decbfa9396335458b1bff74b9aaec9b2adec6a9b11e9dcfce6be3fe`,
`crates/comic-layout/tests/w4_font_contract.rs` =
`502cfbcf3d88de64f810601a9feadab9d340ccd201aeee4ffee5f890c1a70d38`,
`crates/comic-layout/tests/w4_balloon_move_tail.rs` =
`e1cfda1fe37717ff04a53498336977186a0e04e0863d40b082567ba6103c15e7`,
`crates/comic-layout/tests/w4_kind_policy_matrix.rs` =
`aa7f7af442327bcce2612a88d19d19eee1dd8cb7015757bc9c329ba24bfe8f33`. Working tree, base
`e26f030d` — an uncommitted revision, so its identity is the file digests and not a commit.

## 10. M2-L3 — the message that asserted a cause, fixed (M2 §7.4's D2, which is §7's M2-W4-D2)

Reported in §7 and unfixed there, because that round's instruction was to report rather than to fix
unasked — and briefly assigned to the wrong owner, which is why it survived a round. It is fixed now, in
the tree it belongs to.

**The change.** `missing_face_finding` branches on one new public predicate, `is_comic_family`, and the
unconditional tail is gone with it:

```
before  … and shows in Source Sans 3 Bold; the comic faces belong in craft-fonts, and CRAFT_FONTS
        being empty is why they are looked for there

after   Comic Neue Bold     → … and shows in Source Sans 3 Bold; Comic Neue is one of ADR-017's comic
                              faces, which are shipped in craft-fonts and reach a build through
                              CRAFT_FONTS_DIR
        No Such Lettering   → … and shows in Source Sans 3 Bold; No Such Lettering Face is not one of
        Face Bold             ADR-017's comic faces (Comic Neue, Bangers), so craft-fonts is not where
                              it belongs: it has to be installed, or another face chosen
```

The front half is untouched, so the family, the style, the status and the face that stands in are reported
exactly as before — and so are the four filters and the cap in `missing_faces`, which no part of this
change goes near.

**The condition it reports on.** `is_comic_family(family)`: trimmed, ASCII-case-insensitive equality with
`DIALOGUE_FAMILY` (`Comic Neue`) or `DISPLAY_FAMILY` (`Bangers`). That is ADR-017's pair, and it is the
pair `vectorcraft_text::craft_fonts::comic_fonts` filters the build input with (`COMIC_FAMILIES`, private
there), so "this face belongs in craft-fonts" is decided by the same two names the faces actually arrive
by — pinned by a cross-crate test, `the_families_craft_fonts_calls_comic_are_the_ones_this_condition_accepts`,
which asserts the constants unconditionally and every family `comic_fonts()` names whenever the build has
one. Nothing is inferred from the status: a comic family with the *style* absent (`substitute`) is still
pointed at craft-fonts, because that is where the family belongs and the status already says which half is
absent; a family of the project's own is pointed at installing the face, in **both** statuses.

**The cause is gone, not corrected.** The alternative considered was to *check* the claim — report
`comic_fonts().is_empty()` in the comic branch, as `crates/engine/tests/comic_golden_render.rs:188` does
for its own skip reason. Rejected: the finding's job is to report the *face*, and that would make the same
missing face produce two messages for two builds on top of the status and the substitute that already
carry what a creator needs. The one fact the message now states about the world is ADR-017's, which this
module is named as knowing; the causal clause is simply not replaced.

**The two cases differ in what the creator is told to do, and in one word about craft-fonts.** Case B
says craft-fonts is *not* where the face belongs — a negative mention, deliberately kept: it is what
preempts the old wrong turn for a reader who has seen the old message, and it is a claim that *was*
checked (the family is not one of the two). No case points a face at a checkout that never carried it.

**It is caught if it comes back.** Mutation **M-REVERT** (the message literal restored verbatim to the
D2 wording, everything else kept) reds **3** of the 4 new tests —
`a_face_is_sent_to_craft_fonts_only_when_that_is_where_it_belongs` and
`the_message_names_no_cause_but_the_one_it_checked` in `fonts.rs`, and
`the_report_names_craft_fonts_for_a_comic_face_and_never_for_a_face_of_the_projects_own` in
`w4_font_contract.rs` (the fourth pins `is_comic_family` itself, which the mutation does not touch). The
two inline tests build their `MissingFace` records directly, so both branches are asserted in a build with
the comic faces embedded *and* in one without them; the integration test asserts the own-face half
unconditionally and the comic half through `Comic Neue` with a style no file carries, which is reported in
both configurations (`missing` without the faces, `substitute` with them).

**The M1 D4 trap was checked rather than assumed.** `comic-layout` is **not** in `MESSAGE_CRATES`
(`crates/ui-egui/src/i18n/tests.rs:878`), so no `@msg` row is required and none was added; I read the list
and the scanner (`message_literals` walks only `MESSAGE_CRATES`' `src`), then ran the test anyway:

```
cargo test -p vectorcraft-ui-egui --lib i18n   →  test result: ok. 34 passed; 0 failed; 0 ignored; 840 filtered out
```

**Both configurations, as in §3.** In the gate's configuration (`CRAFT_FONTS_DIR` unset) the crate is
**319 tests, 0 failed** — M1's 289 + everything since, 4 of them new. With
`CRAFT_FONTS_DIR=/home/hvt/web/comiclab.io.vn/craft-fonts` and a throwaway `CARGO_TARGET_DIR` the 180
inline tests and the 8 `w4_font_contract` tests are green too, which is the claim the new unconditional
assertions rest on (`Comic Neue` + an absent style reported in either build; `Bangers` likewise through a
planned `sound_effect`). The throwaway target directory was then deleted and the tree left in the gate's
empty configuration: the comic faces were **never** put into the shared `target/`, so nothing this task did
can be the source of a stale-embedding flake for the next runner.

**The scratch-directory fragility from my own W4 record — my tests are clean.** My three `w4_*` files and
the rest of `crates/comic-layout/tests/` create **no scratch directories at all**: no `CARGO_TARGET_TMPDIR`,
no `target/tmp`, no `std::fs` writes (the one file-reading test, `layer_rules_contract.rs`, only reads).
The flaw was real, and it is the **reviewer's**, in another crate:

* `crates/comic-model/tests/round1_reviewer.rs:36` — `scratch(name)` = `CARGO_TARGET_TMPDIR/<name>`, then
  `remove_dir_all`, so a fixed name.
* `crates/comic-model/tests/round2_reviewer.rs:52` — the same function, the same fixed name.
* `crates/comic-model/tests/round3_reviewer.rs:51` — the correct pattern, with the reason written down:
  `CARGO_TARGET_TMPDIR/<name>-<pid>` and a comment naming the concurrent-agent race. `fixture_v1.rs` and
  `fixture_v2.rs` carry it too.

A second concurrent run of `round1_reviewer` or `round2_reviewer` — this workspace routinely has several —
has the other run `remove_dir_all` its tree mid-test, which is a false red indistinguishable from a real
one, and it is a plausible cause of M1's two undiagnosed flakes in this crate. Not touched: those files
are the reviewer's, and their owner is the one to carry the `-<pid>` suffix (the fix is the three-line
`format!` in each `scratch`). Reported here instead, per the task.

**No other file changed**: no catalog row, no `lib.rs`, no `Cargo.toml`, nothing under
`crates/engine/**` or `crates/ui-egui/**`.

```
cargo test -p vectorcraft-comic-layout                       → 319 passed; 0 failed  (15 targets: 180 lib + 139 integration)
cargo clippy -p vectorcraft-comic-layout --all-targets -- -D warnings   → exit 0, no warning
cargo xtask layers                                           → OK: 29 crates, no layering violations
cargo xtask brands                                           → brands: no vendor names in user-visible text
cargo test -p vectorcraft-ui-egui --lib i18n                 → 34 passed; 0 failed
rustfmt --edition 2024 --check <the two files>               → exit 0 (the workspace is style edition 2024;
                                                               fmt was run on the two files only, never --all)
```

Revision: `crates/comic-layout/src/lettering/fonts.rs` =
`6ec212354063d5d41d952e72a235a543dfcda4591ca557ed6f167c285f9d5162`,
`crates/comic-layout/tests/w4_font_contract.rs` =
`af52b5286ac5e1f96bb4f85d21eca23522163af0a8999d148b556db25f211223`. Both digests re-read after the
mutation was reverted, and equal to the pre-mutation values — the file under test is byte-identical to the
file these gates ran on.

**Unverified.** The two `round*_reviewer` false reds are diagnosed from the source, not reproduced (I did
not run two of those suites at once, and the files are not mine to edit); the collision is read, not
observed. Whether M1's two unexplained flakes *are* this is therefore a hypothesis with a mechanism, not a
proven cause. And `CRAFT_FONTS_DIR` pointing at a checkout that holds *some* comic faces — a `Bangers`-less
craft-fonts — is not exercised: the message names craft-fonts for the family either way, which is true of
the family, but I have not run a partial checkout to see the style half reported as `substitute`.
