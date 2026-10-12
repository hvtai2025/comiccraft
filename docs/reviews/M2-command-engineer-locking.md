# M2 W3 — panel commands: `locked`, artwork roles, superseded-artwork history, by `command-engineer`

Owner:         `command-engineer` (W3). Module tree `crates/engine/src/cmd/comic/**`.
Model:         **`deepseek-v4.1-flash`**. Two sources, per M1 §5.4e: this session's environment carries
               `COPILOT_MODEL=deepseek-v4.1-flash`, and the session's own event log
               (`~/.copilot/session-state/bf64cb60-36c7-45ca-ac8c-bbc90d4179d0/events.jsonl`) carries
               `"model":"deepseek-v4.1-flash"` on every `assistant.message` record produced while this
               record was being written (20 275 records across the whole parent session, next:
               `space-bunny` 563, `gpt-5.4-mini` 447).
Milestone:     M2 (porting plan §8, Phase 2) · work item **W3** — `docs/milestones/M2.md` §3
Specification: M2 §3 row W3 — *"Panel commands: honour `locked`, artwork roles, superseded-artwork
               history"*; M2 §5 risk row — *"**`locked` honoured in some commands and not others** | The
               §11.1 class of defect: a policy stated in one place, applied in three of five | W3
               enumerates every command that moves, resizes or deletes a panel and states the lock's
               effect on each"*; M2 §7.5's hand-off — *"`LoadedBundle.migrations` is not surfaced by
               `comic.project.open` … it is in W3's brief"*
Design refs:   design §8.2 (artwork roles: one `primary` plus `background`/`overlay`, and
               `ReviewDecision.superseded_asset_id` as the replacement history), §8.3 (`default_lettering_style`
               is a *name*, not a hard-coded face), §10.2 (the command contract), §11.1 (the related-entity
               policy of a delete), §11.2 (one user action is one undo step); schema §5.3 (`ProjectSettings`),
               §5.6 (the v2 delta), §6.2 (`Panel.locked`), §6.6 (`Lettering.text_layout_settings`), §11
               (migrations are reported)
Toolchain:     `cargo 1.99.0` / `rustc 1.99.0`, as every M0–M2 record notes.
Revision:      the `Revision:` line at the end of this file — it is the citation.

---

## 0. Provenance, and what this record does and does not claim

**This work was written by a previous `command-engineer` agent whose session was interrupted before it
finished. This record was written by a *different* agent that finished the work.** The distinction is
the point of this section, and it is stated first because a record that misattributes who did what is
worse than a thin one.

| | |
|---|---|
| **Inherited** (in the working tree when this session started, uncommitted, authored by the interrupted agent) | `comic.panel.set_locked` and its ten-row lock table; the `locked` refusals in `comic.panel.move`/`.resize`/`.replace_art`/`.delete`, `comic.page.delete` and `comic.page.template.apply`; the M2-L1 fix in `geometry_findings`/`page_frames`; the M2-L2 reader (`project_face`/`style_for_project`) and `comic.project.settings.get`/`.set`; the migration surfacing in `comic.project.open`/`validate`; the 8 catalogs' label rows (and the 6 `@msg` rows that go with them); and the lock/L1/L2/migration tests |
| **Completed by this session** | the `comic_labels_are_menu_strings` count and the two further stale count pins the gate turned out to have (§7); two test assertions that no longer described what they asserted (§5.6, §8); the strengthening of the template-refusal test so it pins *"refused before writing"* rather than *"refused"* (§5.6); this record and `docs/ci-m2-w3.md` |
| **Verified by this session, by reading the code and by running it** | every claim in §1–§5 below: each of the four inherited items, each of the ten rows of the lock table, the `settings.set` validation behaviour, and the refusal order in `comic.page.template.apply` |
| **Taken on trust** | nothing in the four items. What is *not* verified is listed in §9, and it is all outside this tree |

I did not re-derive the inherited work from the previous agent's intent: I read the code and the tests
in the tree, and where the lead's summary and the tree disagreed, the tree wins and §6 says so.

---

## 1. `comic.panel.set_locked` — the twentieth command, and the table that goes with it

`crates/engine/src/cmd/comic/panel.rs:98-106` registers it as the 20th comic command:

```rust
cmd!(
    "comic.panel.set_locked",
    "Lock Panel",
    ["Window", "Comic"],
    None,
    "{panel: PanelId, locked: bool} set or clear `Panel.locked` (§6.2). A locked panel is refused by the
     commands that would **move**, **resize**, **replace the artwork of** or **delete** it — … and by
     nothing else …",
    has_comic,
    set_locked
),
```

The implementation (`panel.rs:620-627`) writes the field through `comic_edit`, so locking and unlocking
are each one undo step, and it is **never itself refused**: it has no lock check, which is what makes
the remedy the refusal names a real remedy rather than a dead end.

The table M2 §5 asks for is `panel.rs:29-41`, in the module doc, and it has **ten rows** — every
command that can move, resize, replace or delete a panel, plus `comic.panel.add`, plus the three
commands the lock deliberately does *not* touch. §5 below walks all ten against the code.

The refusal itself is one function (`panel.rs:318-327`), so the wording cannot drift command by
command:

```
{cmd}: {what}, so {action} — a locked panel does not move, resize, take new artwork or go away.
Unlock it with {remedy}, then retry
```

with `{what}` = `panel 7 is locked` for one panel and `panels 7, 9 are locked` for a pageful, and
`{remedy}` = `comic.panel.set_locked {panel: 7, locked: false}` (or `… for each`). The message names
the panel **and** the fix, which is what makes it actionable rather than merely negative.

## 2. M2-L1 — `Panel[i]` → `Panel(<id>)`

The defect was that this crate labelled a geometry fault `Panel[i]` (an index into a *filtered*
vector) while `comic-layout`'s `validate::page` labels the same fault `Panel(<id>)`, and `validate`'s
own doc offers `(rule, subject)` as the key a caller running both validators deduplicates on — so two
spellings for one fault made that dedupe double-report.

The fix is one helper, `geometry_findings` (`panel.rs:275-281`), plus the type it takes:

```rust
pub(crate) type LabeledFrame = (String, vectorcraft_comic_layout::PanelRect);

pub(crate) fn geometry_findings(page_rect: Rect, panels: &[LabeledFrame]) -> Vec<Value> {
    let rects: Vec<PanelRect> = panels.iter().map(|(_, rect)| *rect).collect();
    validate::faults(page_rect, None, &rects, &Limits::default())
        .iter()
        .map(|f| finding_json(&f.describe(&|i| panels.get(i).map(|(label, _)| label.clone())…)))
        .collect()
}
```

`page_frames` (`panel.rs:287-291`) builds each label as `format!("Panel({})", p.id.0)`, so the
*subject* and the label `describe` substitutes into the *message* are the same spelling — the message
names the other panel of a pair by id too. Both callers go through it: `comic.panel.add`,
`comic.panel.resize` (`panel.rs`) and `comic.page.template.apply` (`page.rs:579-590`), which used to
build its own `Panel[{i}]` vector inline and now calls the same helper. The history is in the helper's
doc comment (`panel.rs:263-274`), including the sentence the M2-L1 report asked for: the index form
survives **nowhere**, because the index it reported was not even the page file's own index.

**Pinned** by `a_geometry_finding_names_its_panel_the_way_the_validator_does` (`tests.rs:959`): it
drives a page resized until two panels sit 2pt apart, asserts the command's subject is `Panel(<id>)`
and contains no `[`, asserts the message names the *other* panel by id, and then finds a finding in
`comic.project.validate` with **the same `(rule, subject)`** — which is exactly the dedupe key. Under
the old spelling both halves fail.

## 3. M2-L2 — `default_lettering_style` now has a reader

Design §8.3 names the comic faces as defaults in `ProjectSettings.default_lettering_style` *"rather
than hard-coding them, so a project can choose different faces"*. The field was written by nothing and
read by nothing, so a project's chosen face never reached a plan.

The reader is `project_face`/`style_for_project` (`lettering.rs:165-177`):

```rust
fn project_face(b: &ComicBundle) -> Option<String> {
    let name = b.project.settings.default_lettering_style.as_ref()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn style_for_project(b: &ComicBundle, kind: LetteringKind) -> LetteringStyle {
    let mut style = style_of(kind);
    if let Some(face) = project_face(b) { style.text.family = face; }
    style
}
```

Three deliberate properties, each visible in the code:

* the name is read as the **family**; the style within it (Regular/Bold/Italic) stays the kind's,
  because one string cannot carry a pair, and a project that wants a different weight per kind says so
  on the record (`comic.lettering.set_style`);
* a **blank or whitespace-only** name is no name (`trim` + `is_empty`), which is what `comic-layout`'s
  face filter does with an empty family;
* a **per-lettering `font`** still wins: `style_from_record` (`lettering.rs:183-193`) applies the
  record's own settings *on top of* the project's default, so the setting is a default and not an
  override.

It reaches the three places the fix promised: `comic.lettering.add` (`lettering.rs:338`) plans in
`style_for_project(...)`, and `comic.lettering.set_text`/`.set_style` re-fit from
`style_from_record(b, l)` — so the face survives a re-fit and is persisted on the record in
`text_layout_settings.font` (schema §6.6).

**Pinned** by `the_projects_lettering_face_reaches_the_plan_the_record_and_the_missing_face_report`
(`tests.rs:1161`) and by `the_v1_fixtures_own_named_face_is_applied_to_a_lettering_on_it`
(`tests.rs:1337`), the second of which uses **W1's real v1 fixture**, whose
`default_lettering_style` is `Comic Neue` while `sound_effect`'s kind default is `Bangers` — so the
record says which of the two the code read. The first names a face no build can have
(`No Such Project Face`), so it is discriminating with `CRAFT_FONTS` empty *and* set.

## 4. The migration reaches the caller

`comic-model` reports a §11 migration as a **value** (`LoadedBundle.migrations`), not as a §14
finding — *a migrated bundle is older, not inconsistent*. W1 could not surface it (it does not own the
command tree); W3 does, in two places:

* `comic.project.open` returns `migrations` + `migrationsNote` (`project.rs:286-303`), and
  `OpenComic` carries the load's migrations (`mod.rs:100-124`);
* `comic.project.validate` returns the **load's** migrations as well (`project.rs:513`), beside
  `severityErrorBlocksSave: false`;
* `comic.project.save` **clears** them (`project.rs:376`), because a save writes this build's
  `schema_version` and a later `validate` still saying "read forward" would assert a fact about the
  file that is no longer true — which is M2-L3's shape (a message explaining a cause it has not
  checked), applied to itself.

`migrations_json`/`migrations_note` (`mod.rs:395-413`) keep the model's own record and add the
sentence: *"the bundle declared an older schema_version and was read forward to 2 (§2, §11); a migrated
bundle is older, not inconsistent, so this is not a `finding`. Saving writes schema version 2"*.

**Pinned** by `opening_an_older_bundle_reports_the_migration_and_a_save_retires_it`
(`tests.rs:1293`): it **copies** W1's v1 fixture (never writes to it), asserts one migration
`1 → 2` with notes, asserts `findings` is empty and `errors` is 0 — so a migration cannot be counted as
damage — then saves and asserts the migration is spent, the manifest declares the current version, and
a reopen reports nothing. It re-hashes the fixture afterwards to prove it was only ever read.

---

## 5. The lock, command by command

M2 §5's risk row is *"a policy stated in one place, applied in three of five"*, so this is the section
that answers it. The table is `panel.rs:29-41`; below, each row is checked against the code, and the
test that pins it is named — or the row says **claim only**.

| Command | Table says | Code | Test |
|---|---|---|---|
| `comic.panel.add` | **none** (created `locked: false`) | `add` has no lock check; the `Panel` literal sets `locked: false` (`panel.rs:372`) | **claim only** — the `Panel` literal is asserted unlocked in `a_new_panel_is_unlocked_and_set_locked_is_the_way_in_and_out` (`tests.rs:1007`), but no test adds a panel to a page that already holds a locked one |
| `comic.panel.move` | **refused** | `locked_panel` + `locked_refusal` before `comic_edit` (`panel.rs:396-399`) | `a_locked_panel_is_not_moved_resized_or_re_arted_and_the_refusal_acts_on_it` (`tests.rs:1027`) |
| `comic.panel.resize` | **refused** | same shape (`panel.rs:443-446`) | same test |
| `comic.panel.replace_art` | **refused** | same shape, before the asset is even looked up (`panel.rs:501-504`) | same test |
| `comic.panel.set_shot` | **ignored** | `set_shot` has no lock check (`panel.rs:606-615`) | `a_lock_does_not_stop_the_brief_the_words_or_the_narrative_order` (`tests.rs:1113`) |
| `comic.panel.delete` | **refused** | before `comic_edit` (`panel.rs:810-813`) | `a_locked_panel_is_not_deleted_and_neither_is_its_page` (`tests.rs:1051`) |
| `comic.page.delete` | **refused while the page holds one** | `locked_on_page` + `locked_refusal` (`page.rs:309-312`) | same test — the *unlocked* panel of the same page is still deletable, so the lock is per panel |
| `comic.page.template.apply` | **refused while the page holds one** | `page.rs:488-495`, see §5.6 | `a_locked_panel_blocks_a_template_that_would_move_or_replace_it` (`tests.rs:1071`) |
| `comic.reading_order.set` | **ignored** | `reading_order_set` has no lock check (`page.rs:438-462`) | same test as `set_shot` — it asserts the locked panel's `reading_order_index` really is renumbered, so the lock is *not* silently absorbing a page-level write |
| `comic.lettering.add` | **ignored** | `add` has no lock check (`lettering.rs:330-339`) | same test — it adds a balloon to the locked panel and asserts the frame never moved |

Two structural properties are worth stating separately, because they are what stops the table from
drifting:

* **the refusal is one function** (`locked_refusal`), so a command cannot invent its own wording;
* **the check is one helper each for the two shapes** — `locked_panel` (one panel) and `locked_on_page`
  (a pageful) — and `is_locked` answers `false` for a panel that does not exist, so "no such panel"
  stays the other command's refusal rather than being swallowed by the lock's.

### 5.6 `comic.page.template.apply` refuses *before* it writes

This is the one row where "refused" and "refused before writing" are different claims, because the
command has three paths that all touch every panel: with no panels it **creates** them, at the same
count it **moves** every frame, and `replace: true` **deletes** them. The check is a single block
before any of them (`page.rs:488-495`):

```rust
let existing: Vec<PanelId> = page_file(&open(s)?.bundle, page)?.panels.iter().map(|p2| p2.id).collect();
// The lock (§6.2), before anything is written. **Every** path below touches every panel: …
let locked = super::panel::locked_on_page(&open(s)?.bundle, page);
if !locked.is_empty() {
    return Err(super::panel::locked_refusal(C, "the template is not applied", &locked));
}
```

and it is asked **once for the page** rather than inside one branch, so a later branch cannot quietly
skip it. Read against the rest of the function, the only writes above it are none: `plan`, `plan`'s
validation and `warnings` are pure, and the `replace` deletion loop, the `create_frame` loop and the
`set_frame_rect` re-lay all come after.

**The test that pins it was weaker than the claim, and this session strengthened it.** As inherited,
the test asserted the refusal and `panels.len() == 3` — which catches a half-applied *replace* but not
a half-applied *re-lay*, because the re-lay path changes no membership and would leave the count at
three after moving every frame. The test now reads the frame rectangles before the two refused calls
and asserts they are unchanged after (`tests.rs:1081-1102`):

```rust
let before = frame_rects(&s);
…
assert_eq!(frame_rects(&s), before, "the refusal came before the write: a lay-out that ran first would have moved these frames");
```

That assertion is discriminating because the two templates are genuinely different layouts
(`three-tier` is *"three tiers, one panel each"*, `wide-top` is *"a wide panel across the top and two
below it"*, `comic-layout/src/template/mod.rs:47-51`), so a re-lay that ran first would change these
rectangles. **Measured**: with the lock check moved to after the lay-out block, this test reds — see §8
for exactly which assertion fired and what that does and does not prove.

## 6. `comic.project.settings.set` — what it validates

The brief asks whether it validates the face name it is given, or accepts anything. **It validates the
*shape* of the value and deliberately not the *existence* of the face**, and that split is right:

| Input | Behaviour | Where |
|---|---|---|
| key absent | leaves the setting alone | `optional_name`, `project.rs:535-543` |
| `null` | clears the setting | same |
| a non-blank string | **accepted as given**, whether or not any build has that face | same |
| `""` or whitespace | **`BadParams`**: *"`defaultLetteringStyle` is blank: give a face name, or null to clear the setting"* | same |
| a number / object / array | **`BadParams`**: *"must be a face name (a string), or null to clear it"* | same |
| `defaultPageSize` | must be a finite, positive `{width,height}` or `[width,height]` | `page_size_param`, `project.rs:96-110` |
| a key this command does not accept (e.g. `defaultTemplate`) | **`BadParams`** — a `set` that sets nothing is refused, so a mistyped key is not reported as a change that was made | `settings_set`, `project.rs:557-565` |

**A project naming a nonexistent face is accepted, and that is the case M2-L2 exists for.** Design §8.3
requires a project whose lettering font is missing to *"surface exactly the missing-font report the
exporter already produces"* rather than fall back silently — which presupposes that such a project can
be created and opened. A command that refused a name no build carries would make a project that names a
face from `craft-fonts` **unsavable on a machine without `craft-fonts`**, which is the failure §8.3 is
written against. So the reader reports and the writer accepts; the existence half belongs to the
missing-face report, and it is pinned end to end: setting `No Such Project Face` and then adding a
balloon produces a warning naming `No Such Project Face Regular`, its status and the face that stands
in, *while the layout still runs* (`tests.rs:1161`).

The blank-name refusal is not cosmetic: `comic-layout`'s face filter skips an empty family, so `""`
would silently mean the *default* face while looking like a chosen one. `null` says that on purpose.

---

## 7. The gate failure, and the catalog gap the brief described

The brief described the gate as red on one test. It was red on **three**, all of them the same
by-design staleness in the comic-command count, and two of the three are now green. Two things were
said about the first one, and **only one of them was true of this tree**:

**(a) The count had to move from 19 to 22 — true, and it was the whole failure *of that test*.** M1
registered 19 comic commands; W3 added `comic.panel.set_locked`, `comic.project.settings.get` and
`comic.project.settings.set`. `crates/ui-egui/tests/comic_labels_are_menu_strings.rs` pins
`specs.iter().filter(|c| c.id.starts_with("comic.")).count()`, and it read `assert_eq!(comic.len(), 19)`.

I changed it to `22` and rewrote the two-line comment above it, which was also stale (it said *"the
twentieth menu string is the `Comic` group name"*). **This is not a weakening.** The assertion's
purpose is that adding a command is what *forces* its catalog rows to be considered, and that is
exactly what it did here: it is the only automated thing that caught the missing rows in M1 and it is
what the reviewer will be able to point at for W3. I kept it pinned rather than derived — a count
derived from `specs()` would be `len == len` and would let a new command slip in uncatalogued — and I
kept `menu_less.len() > 20` and the `missing`/`not_required` assertions untouched. **I touched another
agent's test file to do this** (it is the verification-engineer's `crates/*/tests/**` surface); the
change is 7 lines of comment and one integer, and the round-1 reviewer should judge whether the exact
count should stay or be made robust instead.

**The count is pinned in three places, and the brief knew of one.** The gate's `test` step found two
more, and both are the same by-design staleness rather than a defect:

* `crates/mcp/tests/comic_headless.rs` — **updated here**, `184` and `200` plus the prose between them
  (`19 → 22`). It is *not* in this brief's `Do not touch` list, the change is the same mechanical one,
  and the lead's own principle from §7.5 applies: *"stop hard-coding the number in tests whose subject
  is not the number"* — this test's subject is M1 acceptance 12, the comic surface working end to end
  over MCP, and its count is incidental to that. It passes now.
* `crates/engine/tests/comic_commands_r1.rs:547` — **left unchanged, deliberately**, because this
  brief forbids touching `crates/engine/tests/comic_commands_r*.rs`. See §11: it is the one thing
  between this tree and a green gate, and it is a contradiction inside the brief rather than an
  oversight in the tree.

**(b) `"Lock Panel"` had zero rows in all 8 catalogs — not true of this tree.** The rows are there. I
checked each catalog individually:

| catalog | `Lock Panel` | `Get Project Settings` | `Set Project Settings` | the lock `@msg` template |
|---|---|---|---|---|
| `es`, `fr`, `it`, `ja`, `ru`, `uk` | 1 | 1 | 1 | 1 |
| `zh-hans`, `zh-hant` | 1 | 1 | 1 | 0 |
| `cs`, `pt-br` | 0 | 0 | 0 | 0 |

The eight with the label rows are exactly `LANGUAGES` with `complete_menus`; the six with the `@msg`
row are exactly `COMPLETE_MESSAGES` (`es, fr, it, ja, ru, uk`), and `zh-hans`/`zh-hant` are not in that
list — so the two counts are consistent with the tests rather than a gap. `cs` and `pt-br` are partial
catalogs and are supposed to have neither.

**Where the brief's claim is true is `HEAD`.** `git show HEAD:crates/ui-egui/src/i18n/es.tsv | grep -c
"Lock Panel"` is `0`; the working tree is `1`. The rows are part of the inherited, uncommitted work —
the diff for `es.tsv` is `+@msg … +Lock Panel +Get Project Settings +Set Project Settings`. So the
catalog half was **already done** by the interrupted agent, and the gate's single red was the count
alone. The catalog key format is the one the other comic labels use: an **empty context column** and
the label as the source (`\tLock Panel\tBloquear viñeta`), not an `@id` row — I did not have to derive
it, because the rows exist; I verified the format rather than guessing it.

## 8. A cross-tree red this session had to reconcile, and what it does and does not prove

While this session was finishing, the `lettering-engineer`'s **M2-L3** work landed in
`crates/comic-layout/src/lettering/fonts.rs` (that file's mtime moved during this session). The new
`missing_face_finding` reports a missing face's origin **only when it knows it**: a comic family is sent
to `craft-fonts`, and any other family gets *"… is not one of ADR-017's comic faces (Comic Neue,
Bangers), so craft-fonts is not where it belongs: it has to be installed, or another face chosen"*.

That message now contains the word `Bangers`, and W3's L2 test asserted the opposite:

```rust
assert!(!messages.iter().any(|m| m.contains("Bangers")), "the kind's default face is not what was asked for any more: {out}");
```

which went red with a message that is *correct* — it mentions `Bangers` as one of ADR-017's comic faces,
not as the face that was asked for. I fixed it in **W3's own tree** (`crates/engine/src/cmd/comic/tests.rs`),
which is the only place it could be fixed from, by pinning the clause that says which face is missing
instead of the absence of a word:

```rust
assert!(messages.iter().any(|m| m.contains("the lettering face No Such Project Face Regular is not available")), …);
assert!(messages.iter().all(|m| !m.contains("the lettering face Bangers")), …);
```

The first is the assertion the original was reaching for — the face the report is *about* is the
project's — and it is stable across any wording of the trailing explanation. The second keeps the
original's *"the kind's default is not what was asked for"* claim in a form the correct message cannot
violate. This is a **strengthening of the clause and a narrowing of the word-match**, and it is the kind
of coupling that two agents editing two trees will keep producing: an assertion on a message's *wording*
belongs in the tree that owns the wording, and this one did not.

### The mutation I ran, and its limit

I moved the lock check in `apply_template` to **after** the lay-out block (from `page.rs:488-495` to
just before `let findings = {` at `page.rs:579`) and ran the
strengthened test. It reds. **The assertion that fired first was the `replace` refusal**, because the
same mutation also un-refuses `replace: true` (the panels are deleted before the check, so
`locked_on_page` then finds nothing) — so the run proves the *test* is sensitive to the ordering, and it
does **not** isolate my new frame assertion, which is the third assertion and never ran. I am stating
that rather than claiming more: the frame assertion's discriminating power is argued from the code path
(the re-lay calls `set_frame_rect` per panel with `wide-top`'s rectangles on a `three-tier` page, and
the two templates are different layouts) and not demonstrated by a run of its own. `page.rs` was restored
and its `sha256` re-checked against the pre-mutation copy.

## 9. Findings

Recorded with M0/M1's distinction: a **defect** blocks; a **note** does not. None of these is a defect
in this tree, and none was fixed unasked.

* **M2-W3-F1 — `comic_labels_are_menu_strings`'s `not_required` assertion can never fail.** It is
  `comic.iter().filter(|label| !strings.contains(**label))`, where `strings` is `menu_strings()` — and
  `menu_strings()` inserts **every** registered command's `label` unconditionally
  (`crates/ui-egui/src/menus.rs:3493-3494`). So the assertion is vacuous by construction: it is
  `filter(|l| !set.contains(l))` over a set that was built from those same labels. The test's real
  content is the count, and the *"would `menu: []` have avoided the rows?"* question it exists to answer
  is answered by the `menu_less` half, not by this one. **Note**, and deliberately not fixed: the file
  is another owner's, and a reviewer should decide whether to delete the assertion or to make it ask the
  catalog rather than `menu_strings()`.
* **M2-W3-F2 — `ProjectSettings.default_template` is read by nothing**, so `comic.project.settings.set`
  deliberately does not accept it and refuses a call that names only it. Recorded in the command's own
  `params` doc, and pinned by `the_project_settings_are_settable_readable_and_independent`
  (`tests.rs:1272-1273`): `{"defaultTemplate": "three-tier"}` is `BadParams` and `{}` is `BadParams`. A
  setting no command reads is worse than one that is absent, and making it settable would be the same
  species of defect as M2-L2 was. **Note** — it is a schema-shaped decision for whoever adds
  `comic.page.create`'s template default.
* **M2-W3-F3 — the comic lock is not mirrored into the document's own node lock.** `crates/doc` has a
  `locked` on `Node` (`crates/doc/src/node.rs`), and nothing in this tree writes it from
  `Panel.locked`; structural rule 1 forbids a comic field in the visual document, and whether the comic
  lock should *drive* the document one is a design question rather than a command one. **Note**, stated
  in `panel.rs:50-55` so the silence is named rather than assumed.
* **M2-W3-F4 — `is_locked` answers by scanning every page.** `panel.rs:297-299` is
  `b.pages.values().flat_map(|f| f.panels.iter()).any(|p| p.id == panel && p.locked)`. In a bundle where
  one `PanelId` appears on two pages — which rule 13 exists to prevent and §14 exists to let a creator
  repair — a lock on either would answer `true` for both. The consequence is a *false refusal*, not a
  missed one, so it cannot let a write through a lock; it is a wrong remedy suggestion in an already
  broken bundle. **Note**, and not fixed: resolving it properly means asking the page the way
  `locate_panel` does, which is a change to a helper five commands share and outside what this session
  was asked to do.

## 10. Unverified

* **`CRAFT_FONTS` set.** Every run here used the gate's empty configuration. The L2 test names a face no
  build can have precisely so that it is discriminating in both, and W4 measured the fonts-present
  configuration for its own contract — but I did not run this tree with `CRAFT_FONTS_DIR` pointed at a
  `craft-fonts` checkout, so "the report is about the project's face, not the kind's" is verified in the
  empty configuration only.
* **Windows**, as every M0–M2 record says. Nothing here is platform-dependent, but that is an argument.
* **The lock's interaction with `comic.lettering.set_style`/`.set_text`'s re-fit**, which resizes the
  *balloon* frame inside a locked panel. The table does not claim it and the code has no check, which is
  consistent — a lock covers the panel as drawn, and the balloon is not the panel — but no test pins it,
  so it is a claim about scope rather than a measured one.
* **`comic.panel.add` on a page that already holds a locked panel** (§5, row 1) — code-verified, no test.
* **The `is_locked` duplicate-id case of M2-W3-F4** — argued from the code, not driven.
* **This session's own gate run is the citation for the acceptance criterion**, not a substitute for
  it: §11 has the raw output, and it is **red on one test**.

## 11. The gate

Full raw output: `docs/ci-m2-w3.md`, with the four steps `ci` skips after a failure run on their own.

| step | result |
|---|---|
| `fmt` | **ok** |
| `clippy` (`--workspace --all-targets -- -D warnings`) | **ok** |
| `test` (`cargo test --workspace`) | **FAIL — one test**, `comic_commands_r1.rs::r1_every_comic_command_is_disabled_without_a_project` |
| `assets` | ok (run separately) |
| `brands` | ok (run separately) |
| `layers` | ok (run separately) |
| `wasm` | ok (run separately) |

`cargo test --workspace --no-fail-fast` confirms there is **no other failure anywhere in the
workspace**: 152 test binaries report `test result: ok`, and the only failing target is
`-p vectorcraft-engine --test comic_commands_r1`.

### The one red, and why it is still red

`crates/engine/tests/comic_commands_r1.rs:547` is the round-1 `code-reviewer`'s pin of the comic
surface — *"the surface is nineteen commands"*, with the same number in the doc comment at `:542` —
written when M1 had nineteen. W3 added three, so it is stale by design, exactly like the two count
pins §7 updates.

**It was left unchanged on purpose.** This brief forbids touching
`crates/engine/tests/comic_commands_r*.rs`, and it also requires `cargo xtask ci` to be green on all
seven steps. Those two cannot both hold, and this is the same shape as M2 §7.7's *Error 1* — a brief
that told W3 to fix M2-L3 while forbidding W3's access to M2-L3's file. **The stronger, more specific
constraint was obeyed and the contradiction is reported rather than silently resolved**, because the
alternative is editing another owner's pinned evidence without being asked to, which is what the
`Do not touch` list exists to prevent. The fix is one integer and one word:

```diff
-/// §3 of the implementation: `has_comic` gates all nineteen, so a comic id is `Disabled` — never
+/// §3 of the implementation: `has_comic` gates all twenty-two, so a comic id is `Disabled` — never
-    assert_eq!(ids.len(), 19, "the surface is nineteen commands: {ids:?}");
+    assert_eq!(ids.len(), 22, "the surface is twenty-two commands: {ids:?}");
```

Nothing else in that test depends on the number: the rest of it asserts that every comic id is
`Disabled` with a reason rather than `Ok(Err(NoDocument))` in a session with no project, which holds
for all 22. Its owner — the `code-reviewer` who wrote it, or the round-3 review M2 §3 schedules after
W3 — is the right place to re-pin it.

---

Revision:      this session changed **three** files (plus this record and `docs/ci-m2-w3.md`); every
               `sha256` below is the working tree at the revision the gate in §11 was measured on.
               `crates/ui-egui/tests/comic_labels_are_menu_strings.rs`
               **710d92d5c70600b707fc2248157616700d5e50f36a1ce81b8e35d917037cc4d8** (the count `19 → 22`
               and the comment above it);
               `crates/engine/src/cmd/comic/tests.rs`
               **ec269efb09d09c39cb9bff42a7763d86066aaa78fb26805eed6234569fee4927** (three assertions:
               the L2 report's clause, and the template test's before/after frame rectangles);
               `crates/mcp/tests/comic_headless.rs`
               **48777cfa77a152fa2a19e7ce0bd8c0fa8e1a862028d2847208d4e042c737428f** (M1 acceptance
               12's two comic-surface counts and the prose between them, `19 → 22`).

               The inherited production revision — the four items of §1–§4, **not written by this
               session** — is cited for the reviewer who wants to check §1–§4 against the exact bytes
               they describe: `crates/engine/src/cmd/comic/panel.rs`
               **4cdc79fc6aaa770265202a241e396b6ea211a2e8e623a38ea90246ab1da5739e**,
               `crates/engine/src/cmd/comic/page.rs`
               **567d649d0ed4fe4079efff58c29bb25e08c99018eb1cd6ef7740d937c64e8a43** (verified by
               `sha256sum -c` after the §8 mutation was reverted),
               `crates/engine/src/cmd/comic/project.rs`
               **1f242f067ef82b55fb8a192eb32e2024a187c7ea4408f5cda97856e244b506da**,
               `crates/engine/src/cmd/comic/lettering.rs`
               **9388f9fc1765fbd9e1faedb03c97e92eeb6331b7fc4adb8b5a65cc953dd8cf91**,
               `crates/engine/src/cmd/comic/mod.rs`
               **e9244fbec0a5bcb2fdb367cc8eb3f45d909e998114629ec0d4ac898b0a9eb526**.
               Catalogs: `es` **579d8ef6994587b44b40d048dd2d070093b7e496be31ba87fda328df9a263c22**,
               `fr` **13cdc4792be15c028ddd1b78288ce738399cdce64fbde76dedf518ab1d549c78**,
               `it` **79b885dd9b960930157937ed39d2e6c96e12544c6053fb25f2abf17f962fd497**,
               `ja` **7c5da4107319f10379c0709696494816f31232367b4986789ee676a5df578bf6**,
               `ru` **b6e87579ba399612bf061b3b2d61301ac39ce581d963636283b9ffb9827f4a86**,
               `uk` **6fd9798521c2188a9a5db5e887d5046761fd07c1b2c45468111cfca95e0bb070**,
               `zh-hans` **44e5c2edb624f22d5e4e2cd693b288a3ddc72650115dcbbe75e1ba11940f1277**,
               `zh-hant` **d1f6adf2ead234bb7dbed960e5cf4bcff001618ed20315d8589f527834db1628**.
