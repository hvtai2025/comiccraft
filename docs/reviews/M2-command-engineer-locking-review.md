# M2 W3 — round-1 review: the command layer for panel locking, by `code-reviewer`

Reviewer:      code-reviewer on **deepseek-v4.1-flash** (`COPILOT_MODEL=deepseek-v4.1-flash` in this
               session's environment; no waiver needed)
Revision:      working tree. The sha256 of every file this review measured is in §0.1; the
               production files are the ones `docs/reviews/M2-command-engineer-locking.md` cites, and
               every one of them matched byte for byte before I began.
Files:         read: `crates/engine/src/cmd/comic/{panel,page,project,lettering,mod}.rs`,
               `crates/engine/src/cmd/comic/tests.rs`, `crates/ui-egui/src/{menus.rs,i18n/**}`.
               **Changed by me (test code only):**
               `crates/engine/tests/comic_commands_r1.rs`,
               `crates/ui-egui/tests/comic_labels_are_menu_strings.rs`.
Design refs:   design §8.2 (artwork roles), §8.3 (`default_lettering_style` is a *name*), §10.2 (the
               command contract), §11.1 (the related-entity policy of a delete), §11.2 (one action is
               one undo step); schema §5.3, §5.6 (the v2 delta), §6.2 (`Panel.locked`), §6.6, §11
               (migrations are *reported*); M2 §5 (the risk row), §7.2 (M2-L1), §7.4 (M2-L2),
               §7.7 (the interruption), §7.9 (W3's record).

---

## 0. Provenance, and what this review did and did not do

### 0.1 The revision measured

| file | sha256 | who wrote it |
|---|---|---|
| `crates/engine/src/cmd/comic/panel.rs` | `4cdc79fc6aaa770265202a241e396b6ea211a2e8e623a38ea90246ab1da5739e` | inherited (interrupted agent) |
| `crates/engine/src/cmd/comic/page.rs` | `567d649d0ed4fe4079efff58c29bb25e08c99018eb1cd6ef7740d937c64e8a43` | inherited |
| `crates/engine/src/cmd/comic/project.rs` | `1f242f067ef82b55fb8a192eb32e2024a187c7ea4408f5cda97856e244b506da` | inherited |
| `crates/engine/src/cmd/comic/lettering.rs` | `9388f9fc1765fbd9e1faedb03c97e92eeb6331b7fc4adb8b5a65cc953dd8cf91` | inherited |
| `crates/engine/src/cmd/comic/mod.rs` | `e9244fbec0a5bcb2fdb367cc8eb3f45d909e998114629ec0d4ac898b0a9eb526` | inherited |
| `crates/engine/src/cmd/comic/tests.rs` | `ec269efb09d09c39cb9bff42a7763d86066aaa78fb26805eed6234569fee4927` | inherited + W3 |
| `crates/mcp/tests/comic_headless.rs` | `48777cfa77a152fa2a19e7ce0bd8c0fa8e1a862028d2847208d4e042c737428f` | W3 |
| `crates/engine/tests/comic_commands_r1.rs` | `4bf8f1979eb25068397ab312720adc17badca6e3eae515fe3eded91e70dc9353` | **a previous reviewer; edited by me** |
| `crates/ui-egui/tests/comic_labels_are_menu_strings.rs` | `b11f52ab7bf4806180c7464a37dd69de833b865add6e72d3eac1014459adeb50` | **a previous reviewer; rewritten by me** |
| `crates/ui-egui/src/menus.rs` | `a2113acc6a66bdab6bc3dc6e27ff78ec720a515fb7ce12290258dad60c1f11d0` | not mine; mutated four times and restored |

### 0.2 What I did that the implementer could not

The inheriting agent verified the inherited work **by reading it and running it**, and said so. That
is not a review. This round **drove** the four items — each of the ten lock rows, the dedupe key
through both validators, the L2 reader through a real project and through a name no build can have,
and the migration through open/validate/save — **and mutated production code**, one change at a time,
restoring byte for byte and re-verifying with `sha256sum` after each (§4).

### 0.3 Two files I changed that I did not author

`crates/engine/tests/comic_commands_r1.rs` and `crates/ui-egui/tests/comic_labels_are_menu_strings.rs`
are **a previous `code-reviewer`'s artifacts**, and W3's brief forbade the implementer to touch them.
They are mine now, for two different reasons:

* the **count pin** at `comic_commands_r1.rs:547` was the gate's only red, and a reviewer-owner file
  becomes the obstacle the moment the behaviour legitimately changes (§1);
* the **menu-strings file's central assertion** is what M2 §7.7 handed to this round's reviewer to
  judge, and it could not be judged without editing it (§2).

Changes to both are test-only and are listed in §3. **Attribution follows the file**: the count and
the honest documentation of what that file pins are this round's, not W3's and not M1's.

---

## Round 1 — correctness

Revision reviewed: working tree — the sha256 of every file is in §0.1. The two files this round
               changed are `crates/engine/tests/comic_commands_r1.rs` `4bf8f1979eb25068397ab312720adc17badca6e3eae515fe3eded91e70dc9353`
               and `crates/ui-egui/tests/comic_labels_are_menu_strings.rs` `b11f52ab7bf4806180c7464a37dd69de833b865add6e72d3eac1014459adeb50`.
Verdict:       approved — no defect; six notes (§11), two of which are corrections to *records* rather
               than to code.
Unit tests:    `cargo test -p vectorcraft-engine --test comic_commands_r1` — **17 passed, 0 failed**;
               `cargo test -p vectorcraft-ui-egui --lib i18n::tests` — **34 passed, 0 failed**;
               `cargo test -p vectorcraft-comic-model` — **8 binaries, all ok, exit 0**;
               `cargo test -p vectorcraft-ui-egui` — **5 binaries, all ok, exit 0**.
               Files: `crates/engine/tests/comic_commands_r1.rs` (the `19 → 22` pin, and three new tests —
               `r1_the_lock_table_holds_row_by_row_including_the_rows_that_must_not_refuse`,
               `r1_set_locked_is_never_itself_refused_and_a_missing_panel_is_not_a_lock_error`,
               `r1_a_geometry_fault_has_one_key_whether_the_command_or_the_validator_reports_it`) and
               `crates/ui-egui/tests/comic_labels_are_menu_strings.rs` (the same assertions, honest
               documentation; the `19 → 22` count W3 already changed is kept).
               Full command list and results in §12.
Tests that cannot fail: **none.** W3's F1 recorded one — the `not_required` assertion — and §2
               falsifies the claim: the file's *central* assertion reds under the mutation it was
               written to reject. What is true is narrower: `not_required` is **redundant**, not
               tautological.
Findings:      6 — see §11. Each is a note; none of them is a defect at this revision, so round 1
               requests no changes. I looked for the four silent-failure shapes in the touched code
               (`unwrap_or_default`, an ignored `Result`, an unreachable branch, a `let _ =`) and
               found nothing new: every `unwrap_or*` in `panel.rs`/`page.rs`/`project.rs` is either a
               default that the next line reports properly (a missing page answers "no locked panels"
               and the command then refuses with "no such page") or pre-existing M1 code.

### 1. The count pin: `19 → 22`, pinned and not derived — and I agree

`crates/engine/tests/comic_commands_r1.rs:547` asserted `ids.len() == 19` with the prose *"`has_comic`
gates all nineteen"* / *"Two of the nineteen"*. The surface is **22**: W3 added
`comic.panel.set_locked`, `comic.project.settings.get` and `comic.project.settings.set` to M1's
nineteen. Enumerated from the registrations (`grep '"comic\.' crates/engine/src/cmd/comic/*.rs`):
`comic.dialogue.update`, `comic.lettering.add`, `comic.lettering.set_style`, `comic.lettering.set_text`,
`comic.page.create`, `comic.page.delete`, `comic.page.reorder`, `comic.page.template.apply`,
`comic.panel.add`, `comic.panel.delete`, `comic.panel.move`, `comic.panel.replace_art`,
`comic.panel.resize`, `comic.panel.set_locked`, `comic.panel.set_shot`, `comic.project.create`,
`comic.project.open`, `comic.project.save`, `comic.project.settings.get`, `comic.project.settings.set`,
`comic.project.validate`, `comic.reading_order.set` — and the test prints them on failure. I changed
the number and both prose strings to twenty-two, left the two `ALWAYS`-gated ids alone
(`comic.project.create`, `comic.project.open` — the only ones that must report `BadParams` rather than
`Disabled` with no project), and added the rationale to the test itself.

**I agree with pinning rather than deriving**, and the reason is stronger than "it caught one": the
number is the **catalog obligation**. Every `comic.*` label is a `menu_strings()` entry
(`crates/ui-egui/src/menus.rs:3493-3496`), and `complete_languages_translate_every_menu_string`
requires a row for every entry in each menu-complete language — so the count and the catalog rows are
one obligation in two halves. A derived count (`assert_eq!(ids.len(), comic_ids().len())`) is
`len == len`: it would keep passing while a new command shipped uncatalogued, which is exactly the
failure the pin exists to force a human to notice. Verified after the change (§12).

### 2. M2-W3-F1 — right about `not_required`, wrong as written, and wrong in M2 §7.7

W3 recorded, and M2 §7.7 confirmed, that *"the only assertions in that file that can fail are the
count pins"* and that the `not_required` assertion *"is tautological — it cannot fail"*.

**The second half is false, and I falsified it by running**: I mutated `menus.rs:3493-3496` so that a
command's label is inserted **only when the spec carries a menu path** — literally the `menu: []`
world the file exists to reject — and ran the file. It **FAILED**:

```text
---- a_registered_commands_label_is_a_menu_string_even_with_no_menu stdout ----
thread '…' panicked at crates/ui-egui/tests/comic_labels_are_menu_strings.rs:22:5:
FAIL: `menu_strings()` does not require the labels of menu-less commands, so `menu: []` would have
avoided them: ["Activate Document", "Inspect Document", … "Validate Comic Project"]
```

So the file's **central assertion** — the one that answers the `menu: []` question, `missing` at
line 21 — *does* bite, and it is a real regression guard on `menu_strings()`.

The distinction the brief asks for, stated plainly:

* **`missing` is a guard, not a tautology.** Its two sides are different functions in different
  crates: the labels come from `command_specs()`, the set comes from
  `vectorcraft_ui_egui::menus::menu_strings()`. It fails exactly when `menu_strings()` stops
  requiring the labels of menu-less commands, which is the hypothesis under test.
* **`not_required` (the comic half) is redundant, not tautological.** It can only fail if
  `menu_strings()` stops inserting command labels **at all** — because every `comic.*` command
  carries a `Window › Comic` path, so it is unaffected by the `menu: []` mutation. Anything that
  makes it fail also makes `missing` fail. It adds no discriminating power over `missing`; it is not
  impossible to fail.
* **What neither assertion does is look at the catalogs.** `menu_strings()` is computed from the
  code, so a comic label is in it whether or not `es.tsv` has a row for it. The file's *count pin* is
  all that reaches the catalogs, and only by proxy.

**Can it be made non-vacuous?** Not by strengthening an assertion against `menu_strings()`: the
property *"every comic label is a menu string"* is true by construction of the function under test,
and the property the file *reads* as if it checked — *"a comic label has rows in all 8 catalogs"* —
is a property of the **catalogs**, which this file never touches. Making that assertion here would be
a second copy of a guard that already exists one file over (§3). So the honest fix is what I did:
**keep the assertions and make the file say what it pins and where the real guard is.** The file's
header and the comment on `not_required` now say exactly that, and name the wrong test the record
cites. A reader can no longer over-trust it.

**One correction for the record, and it is a note not a defect:** M2 §7.7 and W3 §9 both attribute
the catalog obligation to `complete_languages_translate_every_message`. That test cannot see a command
label — it scans `MESSAGE_MARKERS` (`.status(`, `Err(`, `#[error(` …) over `MESSAGE_CRATES`
(`crates/ui-egui/src/i18n/tests.rs:880-910`). The guard is
`complete_languages_translate_every_menu_string` (§3).

### 3. What actually guards "a new command's label has rows in all 8 catalogs"

**It is guarded — by `i18n::tests::complete_languages_translate_every_menu_string`
(`crates/ui-egui/src/i18n/tests.rs:350-360`), not by the test the record names.** Its predicate:

```rust
let strings = crate::menus::menu_strings();
for l in LANGUAGES.iter().filter(|l| l.complete_menus) {
    let cat = l.catalog();
    let missing: Vec<_> = strings.iter().filter(|s| !kept(s) && cat.plain(s).is_none()).collect();
    assert!(missing.is_empty(), "{}: {} untranslated menu strings: {missing:#?}", l.code, missing.len());
}
```

Every command label is in `menu_strings()` (that is what §2's mutation *failed* to change for comic
commands, and what `not_required.is_empty()` passing at baseline proves directly), so a comic label
with no row in a `complete_menus` language fails that test. `i18n::tests::menu_catalogs_translate_every_menu_label`
covers the same labels a second time from the menu trees. The obligation is **not** unguarded; the
*record* names the wrong test.

I checked the obligation itself directly as well, because the direct mutation of a catalog row could
not be built in this environment (§7): of the 22 `comic.*` labels enumerated from the source, **all
22 have a `Lock Panel`-shaped row (empty context column) in all eight menu-complete catalogs**
(`es fr it ja ru uk zh-hans zh-hant`). Zero missing.

### 4. The lock table, all ten rows, including the four that must not refuse

The table is `panel.rs:29-41`. I checked it **row by row against the code**, and drove every row in
one table test (§3 of the Unit tests line above). The M1 §11.1 failure mode is a policy applied in
three of five; the *mirror* defect — applied where it must not be — is what six of these ten rows
police, so those are the ones I pressed hardest:

| # | command | table | code | driven by | result |
|---|---|---|---|---|---|
| 1 | `comic.panel.add` | **none** (created `locked: false`) | `add` has no check; the literal sets `false` (`panel.rs:372`) | my `…row_by_row…` (was **claim only**) | **passes**; new panel is unlocked on a page that holds a locked one |
| 2 | `comic.panel.move` | refused | `locked_panel` + refusal before `comic_edit` (`panel.rs:396-399`) | mine + `tests.rs:1027`, `:1147` | refused, naming the panel and the remedy |
| 3 | `comic.panel.resize` | refused | same shape (`panel.rs:443-446`) | mine + `tests.rs:1027` | refused |
| 4 | `comic.panel.replace_art` | refused | same shape, before the asset is looked up (`panel.rs:501-504`) | mine + `tests.rs:1027` | refused |
| 5 | `comic.panel.set_shot` | **ignored** | no check (`panel.rs:606-615`) | mine + `tests.rs:1113` | accepted; the brief is written |
| 6 | `comic.panel.delete` | refused | before `comic_edit` (`panel.rs:810-813`) | mine + `tests.rs:1051` | refused |
| 7 | `comic.page.delete` | refused while the page holds one | `locked_on_page` + refusal (`page.rs:309-312`) | mine + `tests.rs:1051` | refused, naming **that** panel |
| 8 | `comic.page.template.apply` | refused while the page holds one | `page.rs:492-495`, before any write | mine + `tests.rs:1075` | refused on both the re-lay and the `replace: true` path |
| 9 | `comic.reading_order.set` | **ignored** | no check (`page.rs:438-462`) | mine + `tests.rs:1113` | accepted; the locked panel's index really is renumbered |
| 10 | `comic.lettering.add` | **ignored** | no check (`lettering.rs:330-339`) | mine + `tests.rs:1113` | accepted; the panel's frame never moves |

**Granularity, which is the load-bearing half of row 1–4 and 6:** a lock on panel *A* must not stop
panel *B* of the same page. My test drives `move`/`resize`/`replace_art`/`delete` on the **unlocked**
neighbour while the other is locked, and asserts the neighbour's frame really moved, its artwork was
really recorded, and that it was really deletable. That is the assertion a `locked_on_page`-instead-of-
`locked_panel` implementation fails — the plausible-but-wrong mutant §4.2 builds.

**And the refusal is one function** (`locked_refusal`, `panel.rs:318-327`), so wording cannot drift
command by command; my helper asserts the refusal's *content* (names the panel, names
`comic.panel.set_locked`), not merely that some error came back. Two rows I drove beyond the table:

* `comic.panel.set_locked` is **never itself refused** (locking an already-locked panel is idempotent,
  unlocking works while locked) — otherwise the remedy the refusal names would be a dead end;
* a **missing** panel is not a "locked" one: both `set_locked` and `move` on `9999` fail with the
  ordinary error and *not* with the lock's (`is_locked` answers `false`, `panel.rs:297-299`), so the
  two refusals stay distinguishable.

### 5. M2-L1 — the `(rule, subject)` dedupe key, tested by calling both and deduplicating

The implementer's test asserts the command's subject *equals* a `validate` subject. That is the right
shape, but it is not the *contract*: the contract is that a caller merging the two — as
`comic.project.validate` itself does (`project.rs:435-446`, `seen.insert((f.rule, f.subject.clone()))`)
— sees one fault once. So my test does the merge: drive a page until two panels sit 2pt apart, take
`comic.panel.resize`'s findings and `comic.project.validate`'s findings, key both on `(rule, subject)`,
and assert the command's keys are a **subset** of the validator's (`difference` empty) and that the
union adds no key. Passes — and it is the assertion that the `Panel[i]` spelling fails, because then
the command's key is `(None, "Panel[7]")` and the validator's is `(None, "Panel(7)")`.

I also checked the fix is complete: the only remaining `[i]` spelling in production is the
`panels[{i}]` fallback for an index that the label closure cannot answer for (`panel.rs:279`), which
is unreachable for a well-formed `page_frames` and is not `Panel[…]` anyway.

### 6. M2-L2 — a named face is applied, and a nonexistent one is reported

Driven, not read:

* **the project's face reaches the plan and the record** — `the_projects_lettering_face_reaches_the_plan_the_record_and_the_missing_face_report`
  (`tests.rs:1161`) and `the_v1_fixtures_own_named_face_is_applied_to_a_lettering_on_it`
  (`tests.rs:1337`, on W1's real v1 fixture, whose `Comic Neue` differs from `sound_effect`'s kind
  default `Bangers`, so the assertion says *which* of the two the code read);
* **a nonexistent face produces the missing-face report** and the layout still runs, the record
  persists the face, a per-lettering `font` still wins, and clearing the setting restores the kind's;
* both were re-run green in this revision (§6 commands).

The reader is `project_face`/`style_for_project`/`style_from_record` (`lettering.rs:165-193`): the
name is the **family**, the style within it stays the kind's, blank/whitespace is no name, and a
record's own `font` is applied on top. That matches schema §5.3's "names, not fields" and design
§8.3's "a project can choose different faces".

### 7. `settings.set` accepting a nonexistent face — the argument is right

Design §8.3 (`docs/comiccraft-design.md:936-954`) requires that *"a project whose lettering font is
missing must surface exactly the missing-font report the exporter already produces"* rather than fail
to render, and that the comic faces are **defaults a project may replace**. A `settings.set` that
validated the name against the installed faces would make a project that names a `craft-fonts` face
**unsavable on a machine without `craft-fonts`** — the exact configuration the design says must keep
working — and would put a second authority next to the missing-font report. So: the writer validates
the **shape** (absent leaves it, `null` clears, blank or non-string or unknown key or bad size →
`BadParams`, `project.rs:535-565`) and the **reader reports existence** through the plan's finding.
I accept W3's argument, and the consequence the milestone records is the correct one.

### 8. `template.apply`'s frame assertion — correct, and weaker than the record says

The implementer's own mutation moved the lock check after the lay-out block; the test red, but the
assertion that fired was the **`replace` refusal** (the panels had already been deleted, so
`locked_on_page` found nothing and the command returned `Ok`), and the new before/after frame
assertion never ran. W3 said so plainly, and asked whether the frame assertion is good enough.

**Judgement: it is good enough to keep and not good enough to be the evidence it is presented as.**
The reason is structural, and it is worth stating because it applies to every row of the table:
`apply_template`'s whole body runs inside `comic_edit` (`page.rs:614`), and `comic_edit` restores
`doc`, `selection` and `comic` from the snapshot on `Err` (`mod.rs:223-233`). So for any path that
*refuses*, the ordering of the check relative to the writes is **not observable** — a late check is
rolled back and the frames are unchanged either way. The assertion can only fail when the re-lay is
applied **and the command returns `Ok`**, and in that case the `expect_err` on the same call has
already failed. It is therefore a documentation of intent with **zero marginal discriminating power**,
and the honest statement of the property is: *"refused ⇒ nothing was written" is guaranteed by
`comic_edit`'s rollback for every path*, and what the ordering actually protects is the **`replace`**
path, where a late check stops finding the locked panels at all. Keeping the assertion costs nothing
and records the intent; it should not be cited as the proof of "before the write". (Note, §11.4.)

### 9. Migration surfacing — `comic.project.open` and `comic.project.validate`

Driven green in this revision: a copy of W1's v1 fixture opens with one `1 → 2` migration as a
**value** (`migrations` + `migrationsNote`) and **zero findings and zero errors** — so age cannot be
counted as damage; `comic.project.validate` reports the same load's migrations beside
`severityErrorBlocksSave: false`; `comic.project.save` retires them, the manifest then declares the
current version, a reopen reports nothing, and the fixture is byte-stable afterwards (`tests.rs:1293`).
The params strings document the shape and the reason (`project.rs:47`, `:68`). One note in §11.5 about
the *durability* of the retirement.

### 10. Mutations — one change at a time, restored and `sha256sum`-verified

| # | mutation | file | caught by | restored |
|---|---|---|---|---|
| F1 | `menu_strings()` inserts a command's label **only when the spec carries a menu path** (the `menu: []` world) | `crates/ui-egui/src/menus.rs:3493-3496` | `comic_labels_are_menu_strings` — **FAILED**, `missing` at line 22, listing the menu-less labels | `a2113acc…` ✔ |
| A | `comic.panel.add` refuses when the **page** holds a locked panel (over-application of row 1) | `crates/engine/src/cmd/comic/panel.rs` (`add`) | **my new** `r1_the_lock_table_holds_row_by_row…` — FAILED; 16 others in that target **passed** | `4cdc79fc…` ✔ |

Both were applied singly, and each file was restored from a pre-mutation copy and re-hashed before I
went on; the `sha256sum` of the restored `panel.rs` equals the revision the implementer's record
cites. Mutation A is the brief's "plausible but wrong implementation": I judge the **per-panel
granularity of the lock** to be the most load-bearing property in this tree (it is what separates
"a locked panel does not move" from "a page with any locked panel is frozen"), and over-applying it to
the page is one line in the one helper every refusing command shares.

**The gap mutation A exposes, and it is the finding I would keep if I could keep only one:** before
this round, **nothing** in the suite caught it. `panel.add`'s row was recorded as *claim only* by W3,
and I confirmed by reading every test that locks a panel (`grep set_locked`: `tests.rs:1007,1027,1051,
1075,1113,1147` and my own) that none of them adds a panel to a page that already holds a locked one.
The row is now driven, and the mutant reds.

---

## 11. Findings

Six, all notes — the distinction M0/M1 uses: a **defect** (a contradiction, a wrong predicate, a
security hole, something that would produce wrong code) blocks; a note does not. **No defect at this
revision**, so round 1 requests no changes.

* **11.1 — M2-W3-F1 as recorded is wrong in its second half.** *note.*
  `docs/reviews/M2-command-engineer-locking.md:§9` and `docs/milestones/M2.md:§7.7` state that the
  menu-strings file's assertions "*cannot fail*" and that only the count pins can. Trigger: make
  `menu_strings()` insert a command label only when the spec carries a menu path. Expected: the
  recorded claim, i.e. the file still passes. **Actual: it fails**, on `missing` (line 22), which is
  the assertion the file exists for. The claim is true only of `not_required`, and even there the
  accurate word is *redundant*, not *tautological*. Fixed on this side: the file now says what it
  pins and where the real guard is (§2, §3).
* **11.2 — the catalog obligation is attributed to the wrong test.** *note.*
  `M2-command-engineer-locking.md:§7.7` (echoed in §7 of the implementer's record) names
  `complete_languages_translate_every_message` as the automation that catches a missing catalog row
  for a new command. That test cannot see a `cmd!` label: it scans `MESSAGE_MARKERS` over
  `MESSAGE_CRATES` (`crates/ui-egui/src/i18n/tests.rs:880-910`). The guard is
  `i18n::tests::complete_languages_translate_every_menu_string` (§3). Expected: a record that names
  the test it means. Actual: the obligation *is* guarded — only the citation is wrong. The milestone
  doc is the lead's, so I have not edited it.
* **11.3 — `comic.panel.add`'s lock row was claim-only, and nothing caught the over-application.**
  *note.* `panel.rs:29-41` row 1 says **none** (a new panel is created unlocked) and the record
  marked it claim-only. Trigger: make `comic.panel.add` refuse when the page holds a locked panel.
  Expected: the suite rejects it. **Actual: nothing in `--test comic_commands_r1` rejected it** (16
  other tests passed) and, read against every test that locks a panel
  (`tests.rs:1007,1027,1051,1075,1113,1147`; none adds a panel to a page holding a locked one), nothing
  in the lib target did either. Closed here: the row is now driven, and the mutant reds (§10, "A").
* **11.4 — `template.apply`'s frame assertion has no marginal power.** *note.*
  `crates/engine/src/cmd/comic/tests.rs:1081-1102` reads the frame rectangles before and after the two
  refused calls and asserts they are unchanged. Trigger: move the lock check after the lay-out block.
  Expected (per the record): the frame assertion fires. **Actual: it cannot** — `apply_template` runs
  inside `comic_edit` (`page.rs:614`), whose `Err` arm restores `doc` (`mod.rs:223-233`), so any path
  that refuses is rolled back whether the check ran before or after the write. The assertion can only
  fail when the command *succeeds* after writing, and then `expect_err` on the same call has already
  failed. Keep it — it costs nothing and records the intent — but the ordering claim belongs to
  `comic_edit`, and the record should not present this as the proof of "before the write".
* **11.5 — `comic.project.save`'s retirement of the migration record does not survive an undo.**
  *note, read from the code, not driven.* `project.rs:376` clears `oc.migrations` after a successful
  write, "because a later `comic.project.validate` still saying 'read forward' would be asserting a
  fact about the file that is no longer true". Trigger: open a v1 bundle, make one edit (which
  snapshots the whole `OpenComic`, `migrations` included), save, then undo
  (`edit.undo`, `crates/engine/src/cmd/edit.rs:184-194` replaces `st.comic` with the snapshot).
  Expected: the migration stays retired, since the file is v2. **Actual: it comes back**, and
  `comic.project.validate` reports `1 → 2` for a bundle whose manifest says 2. It is defensible — the
  `validate` params string says the value is "the **load's**", and the load did migrate — so this is a
  semantic choice to state rather than a wrong value; what goes stale is the trailing sentence
  "Saving writes schema version 2". Not driven here: the mechanism is a whole-`OpenComic` snapshot and
  a `st.comic.replace`, both visible in the code, and I did not spend a window on it (§12.5).
* **11.6 — `settings.set` stores the face name untrimmed while the reader trims it.** *preference.*
  `project.rs:535-543` accepts any non-blank string and stores it verbatim; `lettering.rs:166` trims
  before use. Trigger: `comic.project.settings.set {"defaultLetteringStyle": "  Comic Neue  "}`.
  Expected: one answer, either way. **Actual: `settings.get` returns `"  Comic Neue  "` while the plan
  and the record carry `"Comic Neue"`.** Self-consistent (a caller that reads back what it set sees
  what it set) and harmless, but the two spellings of one setting are a place for a later comparison to
  go wrong. A one-line `trim` at the writer, or a sentence in the `params` string, closes it.

---

## 12. The commands, and their results

### 12.1 Unit tests (this round's own evidence)

```text
$ cargo test -p vectorcraft-engine --test comic_commands_r1
running 17 tests
test r1_the_lock_table_holds_row_by_row_including_the_rows_that_must_not_refuse ... ok
test r1_set_locked_is_never_itself_refused_and_a_missing_panel_is_not_a_lock_error ... ok
test r1_a_geometry_fault_has_one_key_whether_the_command_or_the_validator_reports_it ... ok
… 14 others ok …
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

### 12.2 The suites the brief names

| command | result |
|---|---|
| `cargo test -p vectorcraft-comic-model` | **exit 0** — 8 test binaries, every `test result: ok` |
| `cargo test -p vectorcraft-ui-egui` | **exit 0** — 5 test binaries, every `test result: ok` (includes `comic_labels_are_menu_strings`, 1 passed) |
| `cargo test -p vectorcraft-engine` | green inside the gate's `cargo test --workspace` (§12.4); a standalone re-run was blocked by another session holding the target lock, which is contention rather than a failure |
| `cargo clippy --workspace --all-targets -- -D warnings` | green — the gate's `clippy` step |
| `cargo xtask layers` | green — the gate's `layers` step |

For the engine crate the gate log gives the per-target numbers, including
`comic_commands_r1.rs` 17 passed / 0 failed, `comic_acceptance.rs` 15, `comic_acceptance_reviewer.rs` 3,
`comic_commands_r2.rs` 5, `comic_commands_r3.rs` 6, `comic_golden_render.rs` 5, `comic_headless.rs` 1,
`comic_quit_prompt.rs` 3, `comic_save_keeps_model.rs` 5. No target anywhere in the run reported a
failure.

### 12.3 Mutations

Four, each one change at a time, each restored from a pre-mutation copy and re-hashed before the next —
the table is §10 and every restore's `sha256sum` is recorded there. `crates/engine/src/cmd/comic/panel.rs`
was `4cdc79fc…` before and after all three of its mutations, and `crates/ui-egui/src/menus.rs` was
`a2113acc…` before and after its mutation; `crates/ui-egui/src/i18n/es.tsv` was `579d8ef6…` before and
after the two `catalog-row` attempts that could not be built (§12.5).

### 12.4 The gate

`cargo xtask ci` — **pass, all 7 steps** (`fmt`, `clippy`, `test`, `assets`, `brands`, `layers`, `wasm`).
Raw output: `docs/ci-m2-w3-r1-review.md`. The one red this round inherited —
`comic_commands_r1.rs::r1_every_comic_command_is_disabled_without_a_project`, the stale `19` pin — is
green after §1's change, which was the only thing standing between this tree and a green gate.

```text
=== ci: fmt ===   ok
=== ci: clippy === ok (`cargo clippy --workspace --all-targets -- -D warnings`)
=== ci: test ===  ok (`cargo test --workspace`; no `test result: FAILED` in the run)
=== ci: assets === ok
=== ci: brands === ok
=== ci: layers === ok
=== ci: wasm ===   ok

CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)
```

### 12.5 Unverified

* **The catalog-row mutation, directly.** I tried five times to delete the `Lock Panel` row from
  `es.tsv` (and, in the last two, to add a synthetic menu string with no row) and run
  `complete_languages_translate_every_menu_string`. Every attempt was killed by this environment
  mid-rebuild — `~/.rustup` and the crate cache were re-created underneath the build twice, and the
  killed compiles left `anon.<hash>.llvm.<hash>` link errors — so **no run of that mutation produced a
  result**. What stands in its place, and is stated as such: the guard's predicate read in full
  (§3), the baseline run of that test green, and the fact — itself mutation-verified — that every
  `comic.*` label is in `menu_strings()`. I also checked the obligation directly: all 22 comic labels
  have rows in all 8 menu-complete catalogs.
* **`is_locked`'s duplicate-panel-id case (M2-W3-F4).** Argued from `panel.rs:297-299`, not driven: I
  did not build a bundle where one `PanelId` sits on two pages. The failure direction is a **false
  refusal**, so it cannot let a write through a lock.
* **`CRAFT_FONTS` set.** Every run here used the gate's empty configuration; §12's L2 evidence is
  therefore the empty configuration only. The tests that pin the reader name faces no build can have
  precisely so that they discriminate in both (W4 measured its own contract with fonts present).
* **Windows**, as every M0–M2 record says.
* **`comic.lettering.set_style`/`.set_text`'s re-fit inside a locked panel** — no test pins it, and
  the table does not claim it (the balloon is not the panel). A claim about scope, not a measured one.
* **The engine crate's standalone `cargo test`.** Blocked by another session's target lock on both
  attempts. The gate's `cargo test --workspace`, which runs the same binary, is green; I did not get a
  separate invocation of my own.

---

## Round 2 — rules, failure paths, edges

**Not run in this session, and deliberately not faked.** This is round 1's record. Round 2 reviews the
**post-fix revision** and must not be a second pass over the same tree
(`.github/instructions/implementation-review.instructions.md`); M2 §3 schedules it after the round-1
findings are answered. Round 1 requested no changes, so no fix moves the revision — but the round-2
pass over the hard rules, the failure paths and the edges has still to be carried out, and
`check-reviews.sh "" M2` will report this file as incomplete until it is.

Gate:          `cargo xtask ci` — **pass** on all 7 steps (§12.4; raw output `docs/ci-m2-w3-r1-review.md`)
Unverified:    §12.5
