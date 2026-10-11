# M1 — code-reviewer round 1, `crates/comic-model`

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. The environment states
               `COPILOT_MODEL=deepseek-v4.1-flash` (an in-environment statement, not only a harness
               label), and this session's event log attaches `"model": "deepseek-v4.1-flash"` to my
               own `tool.execution_complete` records. `~/.copilot/session-store.db ::
               assistant_usage_events` still has **0 rows** for this session
               (`7f4af39f-7989-4abe-aaac-88b31e9d05bb`), so there is no third corroboration. Recorded
               as the model I ran on, with the verification gap stated rather than papered over.
Revision:      working tree (base `16594c58`)   Files:
               `crates/comic-model/src/{ids,entities,bundle,json,validate,document,error,lib,scratch}.rs`,
               `crates/comic-model/tests/{fixture_v1,round1_reviewer}.rs`,
               `crates/comic-model/tests/fixtures/v1/minimal.comiccraft/` (all 13 files),
               `crates/comic-model/Cargo.toml`, `Cargo.lock` (comic-model block),
               `crates/comic-layout/tests/layer_rules_contract.rs`, `ASSETS.md`
Design refs:   design §4.2 (crate layers), §4.3 (two structural rules), §5.1.1 (a page is an
               artboard), §5.2/§5.2.1 (entities, one owner per link), §5.3/§5.3.1 (identity, the
               artboard-id hazard), §6.1 (ownership), §7.1–§7.5 (bundle, save, assets), §8.3
               (lettering inputs vs measured output), §9.3 (ADR-015 consent), §12.1 (deterministic
               checks); schema §2–§15 (frozen format, save order, the 18 rules)
Toolchain:     `cargo 1.99.0` / `rustc 1.99.0`
               (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`). The system `cargo` is
               1.75 and cannot build this workspace (edition 2024, resolver 3), as the M0 records note.

## Round 1 — correctness

Verdict:       **changes-requested**

Unit tests:
- Added (new file): `crates/comic-model/tests/round1_reviewer.rs` — **19 tests: 12 pass, 7 fail**.
  The seven failures are the `finding_*` tests; each encodes behaviour the frozen schema requires
  and is the reproduction for a finding below, not a test that needs weakening.
- Command:
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-comic-model --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/fixture_v1.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round1_reviewer.rs
test result: FAILED. 12 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/schema_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests vectorcraft_comic_model
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
  The implementer's own 78 tests (59 + 10 + 8 + 1) are green and stay green; only the new file fails.
- `cargo clippy -p vectorcraft-comic-model --all-targets -- -D warnings` → **0 diagnostics**
  (`Finished dev profile … in 1.17s`), including the new test file.
- `cargo xtask layers` → `OK: 29 crates, no layering violations.` (the `vectorcraft-doc` edge
  `comic-model` now declares is a downward L2→L1 edge, and the comic-* rows are unchanged).

Findings:      **8 — 4 bug, 4 preference.**

**F1 — `bug`** — `crates/comic-model/src/validate.rs:189` (`Index::build`), used at `:231`, `:272`, `:293`.
Trigger: a `Scene.page_ids` entry naming a *panel* (`9` in the fixture, where 9 is `Panel` 9), or a
`Lettering.page_id` naming a *lettering* (`14`). Expected: a rule-1 finding, "9 does not resolve to a
Page" — rule 1 is the only rule that governs `Scene.page_ids`, and `Lettering.page_id` is governed by
nothing else. Actual: `Index::build` collects page, panel, dialogue **and** lettering ids into the
one set it calls `pages` (`:175-189`), so both references pass. For the `Scene` case the only signal
is a rule-14 *warning* about the unrelated page file; for the `Lettering` case `validate` returns
`[]` — nothing at all. Reproductions:
`finding_1_a_scene_page_id_that_names_a_panel_is_not_a_page_and_must_be_reported`,
`finding_1_a_lettering_page_id_that_names_a_lettering_is_not_a_page_and_must_be_reported`.
(`Panel.page_id` uses the same set at `:272`, but rule 11 cross-checks it against the containing
file, so that instance is covered.)

**F2 — `bug`** — `crates/comic-model/src/bundle.rs:441-453`.
Trigger: a bundle holding both `pages/page-8.json` and `pages/page-008.json` (both mean page 8 —
§3 says padding is cosmetic and rule 13 parses the number) with `manifest.files` listing both, i.e. a
*coherent* hand-edited bundle. Expected: a rule-2 duplicate report, or a refusal — §11 requires a
migration to "validate its input and refuse rather than guess". Actual: `load` keys `pages` by the
parsed id and the second `insert` silently replaces the first; with a coherent manifest
`loaded.findings` is `[]` and one file is ignored. (Without the manifest entry the mismatch is
reported as a *warning* about an unlisted file, so only the coherent case is silent.)
Reproduction: `finding_2_two_page_files_for_one_page_id_must_not_be_silently_collapsed`.

**F3 — `bug`** — `crates/comic-model/src/bundle.rs:510` (the only call site of
`is_canonical_timestamp`, `entities.rs:52`).
Trigger: a loaded bundle whose `manifest.created` is `2026-10-11 06:00:00`. Expected: §5.7 — "a
**writer** must produce this form … Timestamps in the §5.5 canonical form"; refuse or canonicalise.
Actual: only `SaveOptions.now` is checked, so every other `Timestamp` in the model
(`manifest.created`, `ProjectMetadata.created/updated`, `Provenance.created`, `Consent.granted`,
`ReviewDecision.timestamp`) is written exactly as loaded, and the save emits a non-canonical
timestamp. Reproduction: `finding_3_the_writer_must_not_emit_a_non_canonical_timestamp`.

**F4 — `bug` (low severity)** — `crates/comic-model/src/bundle.rs:512-524` and `:558`, `:534`, `:573`.
Trigger: `save` with `DocumentPayload { relative_path: "../escape.vectorcraft", .. }`. Expected: a
refused save leaves the caller's model and the directory as it found them. Actual: `save` derives and
overwrites `manifest.schema_version/comic_id/title/document/updated`, `project.schema_version`,
`project.metadata.updated` and `project.visual_document` *before* the first fallible operation, so
`project.visual_document.relative_path` is left as `../escape.vectorcraft`; `.save-tmp/` (created at
`:534`, removed only on the success path at `:573`) is left behind with the staged files.
Reproductions: `finding_4_a_refused_save_does_not_leave_the_model_mutated`,
`finding_4_a_refused_save_cleans_up_its_staging_directory`.

**F5 — `preference`** — `crates/comic-model/src/entities.rs:495-503` (`FileEntry.unknown`) with
`crates/comic-model/src/bundle.rs:567`.
Trigger: an unknown key inside a `manifest.files` entry. Expected: the preserve-unknown-fields
promise the `#[serde(flatten)] unknown` on `FileEntry` declares (§12; §11 states the same for
migrations — "never silently discard them"). Actual: `save` replaces the whole `files` map with
`scan_tree`'s output, whose entries are built with `Unknown::new()`, so the key is dropped on every
save and the field can never round-trip. The alternative fix is to remove the field — §12 does not
name `FileEntry` among the shapes that carry one, and `files` is derived data. Reproduction:
`finding_5_an_unknown_key_in_a_manifest_file_entry_is_discarded_by_a_save`.

**F6 — `preference`** — `crates/comic-model/src/validate.rs:446` and `:449`.
Trigger: one `jobs/job-18.json` with `schema_version: 2` in a bundle with several job files. Expected:
a rule-3 finding that names the file. Actual: the subject is the literal template string
`jobs/job-<JobId>.json` (and `pages/page-<PageId>.json`) for *every* file of that kind, so a creator
cannot tell which file to fix. The finding exists and is correct otherwise — see
`rule_3_checks_the_version_every_jobs_file_declares`.

**F7 — `preference`** — `crates/comic-model/src/bundle.rs:642` vs `:626`.
Trigger: a load in the window between the prune and the manifest write, where the previous manifest
names a file the prune has removed. Expected: §14 calls the interrupted-save window one benign,
expected cause that "is **reported as an inconsistency** and the load continues" — so both halves of
the mismatch should be reported at the same, non-blocking severity. Actual: the "declared file
absent" half is `Finding::error`, while the "file not listed" half is `Finding::warning`, even though
the schema treats them as one cause. Reproduction (reporting only, no severity assertion):
`the_window_between_the_prune_and_the_manifest_write_is_survivable`.

**F8 — `preference` (schema erratum, `schema-architect`'s)** —
`docs/comiccraft-project-schema.md` §6.5 and §6.6 list `unknown` in `ContinuityConstraints` and
`TextLayoutSettings`; §12 names both among the seven closed shapes that "omit it deliberately".
`crates/comic-model/src/entities.rs:854-870` and `:906-928` follow §12 and drop extra keys, and
`entities.rs::a_closed_value_type_has_nowhere_to_keep_an_extra_key` pins that reading. Trigger: an
extra key inside `continuity_constraints` / `text_layout_settings`. Expected: one reading of the
frozen document. Actual: two, with the code choosing §12 — a defensible choice, but the frozen
document contradicts itself and the implementer resolved it silently rather than raising it. §1's
tie-break rule covers design-vs-schema, not schema-vs-schema, so this needs an erratum.

### What I checked and found *true* (so it is on the record)

- **The fixture's document is real.** The implementer's `the_fixture_is_a_bundle_and_not_a_document`
  reads `document/comic.vectorcraft` as text, so nothing in this crate proves it parses. I probed it
  through `crates/format` from a temporary test (since deleted — a milestone-level end-to-end test is
  `verification-engineer`'s): `format::load_file` accepts it at `info.version == 4 == format::VERSION`,
  `format::load` yields **exactly one** artboard (id 1, 612×792 pt) and node ids **exactly** 2–13.
  So `document_facts()` in `fixture_v1.rs` states the truth, and rules 4/7/15 are being fed real
  facts rather than convenient ones.
- **The round trip is genuinely byte-identical, and I could not falsify it.** The test copies the
  fixture to a scratch directory, loads, saves with the fixture's own `updated`, and compares every
  file's bytes and the file *set* (`actual.keys() == expected.keys()`), then a second save of a
  reloaded bundle. I attacked it three ways and all three are already covered or pass: a misspelled
  key cannot slip through, because every fixture field is either declared or *required* (a rename
  makes the load fail) — and `the_fixture_has_no_unintended_unknown_keys` (mine) closes the one gap
  that remains, a key that lands in `unknown` and is written straight back out. `Provenance` is the
  one §5.5 shared type the fixture never writes, and
  `a_generated_asset_with_provenance_and_consent_round_trips` (mine) exercises it, including §5.7's
  sorted-keys rule for the `request` map. The fixture's `default_page_size` is written as `612.0`,
  not `612`, which is the f64 shortest round-trip form §5.7 asks for.
- **The fixture is faithful but deliberately minimal.** §11.1's enumeration (1 chapter, 1 scene, 1
  page, 3 panels, 2 balloons, 1 caption, 1 linked asset, 1 job) is met exactly, and it also carries
  a review decision and two unknown keys on the chapter. It does **not** exercise: a
  `generated`/`edited` asset (so rule 10's positive half and `Asset.provenance` are fixture-absent),
  `StorageRef::Embedded`/`Package`, a non-speech/narration `LetteringKind`, a non-`draft`
  `PageStatus`, or a terminal job in `jobs/`. Each is covered by an inline test instead, except
  `Embedded`/`Package` in rule 9 — which my two rule-9 tests now cover. This is a subset, not a
  convenient one: nothing in the fixture is invented or reshaped.
- **Id allocation.** `IdAllocator::allocate` refuses at `u64::MAX` rather than wrapping, and
  `repair_upward` never lowers the counter (both tested). `ComicBundle::repair_id_counter` counts
  *every* kind plus `bundle_id`, and `load` repairs and reports. The artboard hazard is closed the
  way §9.1 requires: `retire_artboard_id` is append-only and de-duplicating, `assignable_artboards`
  filters retired ids out of what a page may take, rule 4 rejects a retired artboard, and rule 17
  reports a repeated retirement. I could not find a path where a page is given a retired number
  (`a_page_is_never_offered_a_retired_artboard`, mine, checks the allocator side).
- **`Cargo.toml`.** `sha2 = { version = "0.10", default-features = false }` is byte-for-byte what
  `crates/pdf/Cargo.toml:34` declares, so §7.2's "the same version, not a new crate" holds; the
  `Cargo.lock` delta is the four new edges on `vectorcraft-comic-model` and nothing else.
- **`xtask layers` and the layer rule.** `comic-model` is L2 and `doc` L1, so the new dependency is
  downward; no same-layer edge was introduced, and the comic-* rows in `xtask/src/layers.rs` are
  untouched by this change.

Tests that cannot fail:
- `crates/comic-model/src/bundle.rs:821` — `a_save_writes_the_manifest_last_and_describes_every_content_file`.
  Its name asserts the ordering §10 fixes; its body checks only the manifest's *contents* (the
  `files` list, `.save-tmp/` gone, a clean reload). **Mutation M7 — write the manifest before the
  content files — leaves this test and all 77 others green.** The ordering claim is unverified by the
  suite; my `a_crash_between_step_4_and_step_5_…` walks the window but does not pin the order either,
  so the order rests on reading §10 against the code.
- `crates/comic-model/tests/fixture_v1.rs:250` — `the_fixtures_severity_vocabulary_is_the_one_the_caller_reports`
  asserts `Severity::Error > Severity::Warning`, which restates a two-variant `derive(PartialOrd)`,
  and compiles a function-pointer coercion. It cannot fail unless the derive is deleted, at which
  point the crate does not build.
- `crates/comic-model/src/validate.rs:1400` — `rule_16_and_rule_17_report_an_unknown_algorithm_and_a_repeated_retirement`
  asserts only that *a* rule-16 and *a* rule-17 finding exist, never on which asset or what it says;
  it would still pass if either rule reported the wrong subject or the wrong message.
- Weak, not vacuous: `entities.rs:1063` `field_order_is_the_order_the_schema_declares` checks 8 of
  `ComicProject`'s 12 declared keys (it omits `character_ids`, `location_ids`,
  `style_profile_ids`), so a reordering among those three would not be caught — the `keys.len() == 8`
  assertion keeps it from skipping silently.

Mutation table:  I mutated production code, ran **the implementer's own suite** (`--lib --test
fixture_v1 --test schema_contract`, i.e. 77 of their 78; the doctest is excluded because `--doc`
cannot be combined with target selection), then restored and forced a rebuild. Restoring via a copy
that preserves mtimes lets cargo reuse the mutated artifact — the first post-restore run was wrong
for exactly that reason — so every row below was re-verified with the source `touch`ed and a fresh
build. Rows 1 and 7 are the two that *pass*, i.e. the two behaviours their suite does not pin.

| # | mutation | their suite | mine |
|---|---|---|---|
| M1 | `Index::build`: `pages` = real page ids only (the F1 fix) | **pass** (59+10+8) | the two `finding_1_*` tests pass |
| M2 | `Index::build`: `pages` = panel ids only | fail — `a_clean_bundle_has_no_findings`, `a_validation_of_a_saved_bundle_matches_the_one_of_the_model` | — |
| M3 | `JobStatus::is_terminal` also true for `Completed` | fail — `only_the_two_terminal_job_statuses_are_terminal` | — |
| M4 | `LetteringKind::carries_own_text` drops `SoundEffect` | fail — `lettering_kinds_split_into_spoken_and_self_texted` | — |
| M5 | `to_canonical_bytes` drops the trailing newline | fail — 2 `json` tests | — |
| M6 | `resolve_path` skips the containment check | fail — `an_escaping_path_is_never_resolved`, `a_symlink_out_of_the_base_is_refused`, `rule_9_reports_a_hash_mismatch_…` | — |
| M7 | `save` writes the manifest **before** the content files | **pass** (59+10+8) | — |
| M8 | the prune step is skipped | fail — `a_save_removes_a_page_file_the_bundle_no_longer_has` | — |
| M9 | `AssetType::requires_provenance` drops `Edited` | fail — `only_generated_and_edited_assets_require_provenance` | — |
| M10 | `is_canonical_timestamp` always true | fail — `canonical_timestamps_are_the_one_form`, `a_save_refuses_a_timestamp_that_is_not_canonical` | — |
| M11 | `repair_id_counter` never repairs | fail — `the_counter_is_repaired_upward_over_every_kind_present` | — |
| M12 | `manifest.files` left empty | fail — `a_save_writes_the_manifest_last_…`, `a_validation_of_a_saved_bundle_…` | — |

Read the table as: the suite is a real suite — ten of twelve mutations are caught, and it is
sensitive to the enums, the canonical writer, rule 9's confinement, the prune, the id counter and
the `files` map. The two it does not catch are exactly the two I report against it: the rule-1 set
that F1 lives in (M1), and §10's ordering (M7).

Rule 9 / save order / rule 2:
- **Rule 9 — correct, and the order is the rule's order.** `resolve_path` (`json.rs:139-171`) joins
  against the base, canonicalises the base, canonicalises the *nearest existing ancestor* of the
  joined path, and only then tests containment. An escape is `Escapes` and is **never opened** — my
  `rule_9_reports_a_symlink_that_leaves_the_bundle` asserts the asset is reported as escaping *and*
  that no "hashes" finding appears, so the file was not read. A `..` that stays inside resolves
  (`assets/../outside.png` → `Resolved`); an absolute path, an empty path and a path that escapes
  with the target absent are all `Escapes`; an absent in-base path is `Missing`, deliberately
  distinct. The three bases are genuinely three: `Linked` → `ValidationInput.root`, `Package` →
  `archive_root` (and an explicit "cannot resolve against the bundle root" finding when there is no
  archive root — `rule_9_package_resolves_against_the_archive_root_and_nothing_else`), `Embedded` →
  no path at all (`rule_9_embedded_is_a_key_not_a_path`). §13.1's two recorded limits hold up: a hard
  link has no path to canonicalise and is accepted, with integrity resting on `content_hash`; the
  TOCTOU window is documented, not hidden. One interpretation to note: §13's rule-9 text says an
  escaped path's asset is "reported unresolved (**rule 1**)", but the implementation reports it under
  rule 9 with a message naming §13.1. That is arguably the better rule to file it under, but a caller
  that filters on rule 1 will not see it — worth a one-line decision from the lead.
- **Save order — correct in substance; the order itself is untested (M7).** The steps are: stage
  every file into `.save-tmp/` → per-file `rename` into place → prune → write the manifest from a
  **fresh `scan_tree`** → remove `.save-tmp/`. Writing `files` from the tree rather than from
  intentions is what makes §4's consistency record able to detect tampering, and it is why M12 is
  caught. I walked the three windows: **before step 4** — staging exists, the previous manifest and
  every content file are untouched, the load ignores `.save-tmp/` entirely and reports nothing
  (`a_crash_before_step_4_leaves_the_previous_generation_untouched`); **step 4↔5** — the content
  files are the new generation under the previous manifest, the load succeeds, exposes the *new*
  content and reports the mismatch (`a_crash_between_step_4_and_step_5_…`), which is what §14
  requires of the one window the schema calls observable; **step 5 itself** — a truncated manifest is
  the single fatal write, and `load` returns `Error::Parse`
  (`a_truncated_manifest_is_the_one_commit_point_that_is_fatal`). The prune belongs to step 4 — it
  is part of "replace the content files", and without it the manifest would describe a page the
  project no longer has — and its own window (a manifest naming a file the prune removed) is
  survivable (`the_window_between_the_prune_and_the_manifest_write_is_survivable`), though reported
  at the severity I question in F7. The prune is correctly narrow: `assets/` is left alone even when
  unreferenced, and a stale page file is removed
  (`the_prune_step_removes_a_deleted_page_and_leaves_assets_alone`, plus their own
  `a_save_removes_a_page_file_the_bundle_no_longer_has`). One thing §14 tolerates that the schema
  does not mention: the prune happens *before* the manifest write, so the window contains a
  *deletion* as well as an addition — still one window, still survivable, but the schema's prose
  describes only the additive half.
- **Rule 2's two exclusions — both are the right reading, and neither hides a duplicate.**
  `manifest.comic_id` vs `project.id`: §4 says `comic_id` is "the `ComicProject.id` in
  `project.json`; **duplicated here** so the manifest alone identifies the project", so counting it
  would make *every* legal bundle a duplicate; rule 3 is what checks the copy agrees
  (`the_manifest_comic_id_is_a_copy_not_a_second_entity`). A job in both `jobs/` and `provenance/`:
  §8.3 says `jobs/` is "the *live* copy" and `provenance/generations.json` the "*record*", and a
  non-terminal job legitimately has both — the frozen fixture itself has `JobId(18)` in both places
  with status `awaiting_review`, so excluding it is required or the anchor fixture would not validate.
  Neither exclusion creates a blind spot for a *real* duplicate: every other kind is counted
  per-kind and across kinds; `load` keys `pages` and `jobs` by id so an intra-kind duplicate is
  caught by rule 2's per-kind count, and the job-file name is separately cross-checked against
  `job.id` (`rule_2_duplicate_ids`, last block). The one hole in that story is **F2**: two *files*
  for one page id are collapsed by the `pages` map before rule 2 ever sees them.

Outside-tree edits:  both reviewed as critically as the code.
- `crates/comic-layout/tests/layer_rules_contract.rs` — **the change did not weaken the substance of
  any rule; it narrowed the *scope* of two checks, and that narrowing should be tightened.**
  - *Test 1 (dependency ban)*: removing `vectorcraft-doc` from `forbidden` while adding
    `declared == "vectorcraft-doc" && crate_name == "comic-ai"` matches design §4.2's own table —
    `comic-model` (L2) depends on `doc`, `comic-layout` (L4) depends on `doc`, and only `comic-ai` is
    required to have "no knowledge of `doc`". `vectorcraft-format`, `engine`, `ui-egui` and `mcp`
    stay banned for all three crates. Not a loosening.
  - *Test 2 (no document type in comic code)*: the six bare words it replaced were genuinely
    unsatisfiable by any schema-faithful implementation — `VisualDocumentRef` contains the substring
    `Document`, and §6.2/§9 mandate `NodeId` and `artboard_id`. The replacement
    (`vectorcraft_doc::Document`, `vectorcraft_doc::Artboard`, `DocumentFacts`, `ArtboardFacts`,
    `geometry_ref`) still forbids holding the document, an artboard or frame geometry. **But** it now
    reads only `entity_source()` = `src/entities.rs` (else `src/lib.rs`), so a record type added in a
    new module is ungoverned; and because every banned token is matched in its qualified form,
    `use vectorcraft_doc::Artboard as Art;` would evade it. The implementer wrote a `sources()`
    helper that walks the whole `src/` tree and then used it only in test 4 — test 2 should use it
    too, minus the files that legitimately name document types (`document.rs`, `json.rs`).
  - *Test 3 (`no_comic_crate_owns_a_second_copy_of_the_page_size_or_a_dialogue`) is unchanged* and
    still reads `source()` = `lib.rs` only. For `comic-model` that file is now a re-export shell, so
    the check is green while covering none of the records it names; it also cannot simply be pointed
    at `sources()`, because its `page_size` token would then trip on the schema-mandated
    `default_page_size`. This is a pre-existing weakness the module split made nominal — worth a
    follow-up, not a blocker on this change.
  - *Test 4 (`declared implies used`)* is **stronger** than the exact-list assertion it replaced: M12
    shows it catches a real unused-dependency class of bug rather than pinning M0's empty crates.
  - Verdict on the outside-tree edit: ratify it, with the test-2 scope tightening as a follow-up for
    the owner. I found no rule that the change lets the *current* code violate.
- `ASSETS.md` — the two rows are accurate and checkable. `assets/panels/panel-1.png` is a real PNG
  (signature and `IHDR` verified: **8×8**, 149 bytes), which is what the row claims, and
  `document/comic.vectorcraft` is the fixture's own document (I verified it loads at v4 with one
  612×792 artboard — see above). Neither row claims a licence or provenance that cannot be checked.

Schema fidelity:  **no field name, type, optionality, order or wire spelling differs from the frozen
schema.** Checked field by field against §4 (`Manifest`), §5.1–§5.4 (`ComicProject`,
`ProjectMetadata`, `ProjectSettings`, the five story files), §5.5 (every shared type and all
fourteen enums), §6.1–§6.6 (`Page`, `Panel`, `Dialogue`, `Lettering`, `ContinuityConstraints`,
`TextLayoutSettings`), §7/§7.1/§7.2 (`Asset`, `StorageRef`, `ContentHash`, `MediaMetadata`),
§8.1–§8.3 (`GenerationJob`, `ReviewDecision`, the `jobs/` wrapper). Specifically: struct fields are
declared in the schema's table order (which §5.7 makes canonical); `Timestamp` is a `String` alias as
§5.5 declares; `PageSize` is `kurbo::Size`, serialising `{width, height}` as f64 in §5.3's order;
`AssetRegistryId` is a `ComicId` alias; every id is `#[serde(transparent)] u64` and serialises as a
bare number; the variant spellings are the table's (`sound_effect`, `awaiting_review`,
`invalid_output`, `resize_frame`, `text_to_image`, `panel_artwork`, `superseded`, …); `Panel` has no
`geometry_ref`, `Page` has no `page_size`/`visual_document_id`, `Dialogue` has no `balloon_id`;
`Lettering`'s `dialogue_id`/`text` are `Option` and present-with-`null`, as §6.4's "Req: yes" means;
`unknown` is flattened on every entity and on no shape §12 names as closed. Four notes rather than
deviations:
1. `unknown` is also flattened onto shapes §5.5's table does not give one — `Provenance`, `Consent`,
   `Progress`, `JobError`, `AppInfo`, `FileEntry`, `VisualDocumentRef`. §12's rule ("Every *entity*
   struct carries it") plus its list of the seven shapes that do not is the only guidance, and none
   of these is in that list, so the choice is defensible — except `FileEntry`'s, which is dead (F5).
2. §6.5 and §6.6 list `unknown` for `ContinuityConstraints` and `TextLayoutSettings` while §12
   excludes both; the code follows §12 (F8).
3. `StorageRef`'s wire form is serde's externally-tagged `{"linked": {"relative_path": …}}`. The
   schema fixes the variants and their fields but never shows the JSON, so the fixture is what
   actually freezes it — worth one sentence in §7.1.
4. Rule 9 reports an escaped path under rule 9, while §13's rule-9 prose says "(rule 1)".

Unverified:
- `crates/comic-ai`'s duplicate `JobStatus` / `ProviderId` / `is_terminal`. `entities.rs`'s doc
  comments say `crates/comic-ai` "still has its own copy from M0" and that removing it is "the
  lead's, not this crate's". I read the claim and did not verify what `crates/comic-ai/src/lib.rs`
  defines today, nor whether its `is_terminal` still agrees — the two-enums defect §5.2.1 forbids is
  live until that one-line re-export lands. Out of this crate's tree.
- §10 steps 2–3 (serialize the document through `crates/format`, verify by loading it back) are
  delegated to the caller by design, so no test in this crate exercises them; a full round trip
  *through* `crates/format` is `verification-engineer`'s milestone test. I verified only that the
  fixture's document parses (above).
- Windows: `fs::rename` over an existing file and the `.save-tmp/` replacement are the parts §10
  says behave differently there; nothing here was run on Windows.
- Whether a `Severity::Error` finding should block a save is the command layer's policy, and that
  layer does not exist yet — so F7's severity question cannot be settled from this crate alone.
- The `docs/ci-3da4080b.md` and `porting_plan_and_design/` changes in the working tree are not part
  of this review (they are not this change); `docs/milestones/M1.md` is the plan this implements.

---

*Round 2 (the hard rules, failure paths and edges) is not written here: round 1 requested changes, so
round 2 must run on the post-fix revision and name it in its own `Revision reviewed:` line.*

---

## Round 2 — rules, failure paths, edges

MODEL:         `deepseek-v4.1-flash` — `COPILOT_MODEL=deepseek-v4.1-flash` in the environment (an
               in-environment statement, and what the `code-reviewer` agent definition requires).
               Corroboration is thinner than it should be and is recorded rather than assumed: the
               session folder's `events.jsonl` attributes `"model":"deepseek-v4.1-flash"` to events
               in this workspace but carries only `agent_name` values of other agents, and
               `~/.copilot/session-store.db :: assistant_usage_events` still has **0 rows** for this
               session id (632 rows overall, none for `ba68190d-…` or round 1's `7f4af39f-…`). No
               third corroboration exists, exactly as round 1 found.
Reviewer:      code-reviewer on deepseek-v4.1-flash
Revision:      working tree (base `16594c58`)   Files:
               `crates/comic-model/src/{validate,bundle,entities,json,error,ids,lib}.rs`,
               `crates/comic-model/tests/{round1_reviewer,round2_reviewer}.rs` (the second is new),
               `crates/comic-layout/tests/layer_rules_contract.rs`,
               `docs/comiccraft-project-schema.md` §5.7/§6.5/§6.6/§11/§12/§13/§14, `ASSETS.md`
Design refs:   design §4.2/§4.3 (layers), §6.1 (ownership), §9.3 (ADR-015), §12.1; schema §4
               (`manifest.files`), §5.5/§5.7 (timestamps, canonical form), §9, §10 (save protocol),
               §11 (refuse rather than guess), §12 (unknown fields), §13 rules 1–18, §14 (what a
               load tolerates)
Revision reviewed:  working tree after the F1–F8 fixes (post-round-1): `validate.rs` 07:54,
               `bundle.rs` 07:52, `error.rs` 07:48, `ids.rs` 07:50, `layer_rules_contract.rs` +
               the §6.5/§6.6 erratum in the schema. `round1_reviewer.rs` is byte-untouched by the
               fix: its mtime is 07:44:58, *before* every production edit above, its 19 tests keep
               their original names, and all 19 now pass with no assertion changed.
Verdict:       **changes-requested**

Unit tests:
- Added (new file): `crates/comic-model/tests/round2_reviewer.rs` — **18 tests: 16 pass, 2 fail**.
  The two failures are `finding_1_a_stray_json_file_in_pages_is_a_fatal_load_error` and
  `finding_2_a_save_deletes_a_declared_json_file_it_does_not_own`; each encodes behaviour §14/§12
  requires and is the reproduction for a finding below. The other 16 are positive tests of the
  fixes — the rule-1 kind sweep (33 reference slots), the duplicate's report *and* its
  determinism, the complete timestamp table and its two derived exclusions, the refused-save
  whole-model/whole-directory snapshot, the carry-over's non-resurrection, real rule-3 subjects,
  the §14 severity agreement, and §10's ordering.
- Extended (test code only): `crates/comic-layout/tests/layer_rules_contract.rs` — one new test,
  `no_comic_crate_reaches_a_document_type_by_a_brace_a_rename_or_a_space`, closing the evasion
  round 1 asked to be closed. Nothing existing was weakened or deleted.
- Command:
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-comic-model --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/fixture_v1.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round1_reviewer.rs
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round2_reviewer.rs
test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/schema_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests vectorcraft_comic_model
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
  The owner's reported count is confirmed: 65 + 10 + 19 + 8 + 1 = **103**, all green, and the
  seven round-1 `finding_*` reproductions are green **with their bodies unchanged** — the fixes, not
  the tests, made them pass. Run three times to rule out the flake that a shared scratch name
  caused on the first draft of my own file.
- `cargo clippy -p vectorcraft-comic-model --all-targets -- -D warnings` → **0 diagnostics**,
  including both test files.
- `cargo xtask layers` → `OK: 29 crates, no layering violations.`
- `cargo test -p vectorcraft-comic-layout --test layer_rules_contract` → `8 passed; 0 failed`.
- `rustfmt --check --edition 2024` on both files I edited → clean.

Findings:      **5 — 2 bug, 3 preference.**

**R2-F1 — `bug` (low)** — `crates/comic-model/src/bundle.rs:459` (`:479` for jobs).
Trigger: a `*.json` file under `pages/` whose name is not `page-<id>.json` — `pages/notes.json`, a
`page-eight.json` a human typed. Expected: §14 names **two** fatal load failures and says of
everything else that it is "returned as a finding on a usable project, **so a creator can open a
damaged bundle and repair it rather than being locked out**". Actual: `load` returns
`Err(BadIdFileName { name: "notes.json", prefix: "page" })` and the project cannot be opened at all,
so the repair tools are unreachable. The `ok_or_else` sits *after* the `.json`/`/` filter but
*before* any tolerance, and it is on the page- and job-file paths the F2 fix rewrote. Reproduction:
`finding_1_a_stray_json_file_in_pages_is_a_fatal_load_error`.

**R2-F2 — `bug` (low)** — `crates/comic-model/src/bundle.rs:734-752` (`prune_stale`), called at
`:637`.
Trigger: a bundle whose `manifest.files` declares `story/relationships.json` — a file a **newer**
ComicCraft wrote — which `load` reads, hashes and accepts **with no finding** (I assert that).
Expected: §12's promise ("a field written by a newer ComicCraft survives a load-and-save by an
older one") and §4's own definition of `files` ("for every file except the manifest **and except
`exports/` and `.save-tmp/`**", i.e. the manifest is expected to be able to declare files this build
does not name) mean a save must not delete a declared file it does not understand. Actual: step 4
removes every `.json` in `pages/`, `jobs/`, `story/` and `provenance/` that the new generation does
not stage — including that file — and step 5 then drops its `files` entry, so the next load reports
the file as absent and the data is gone with no finding that ever mentioned it. Reproduction:
`finding_2_a_save_deletes_a_declared_json_file_it_does_not_own`.

**R2-F3 — `preference`** — `crates/comic-model/src/bundle.rs:552-554` against `load`.
Trigger: a bundle whose `manifest.created` is `2026-10-11 06:00:00`. Expected: one answer from the
two halves — either `load` reports it (the §14 "usable project, repairable" reading) or the refusal
is documented as save-time-only. Actual: `load` returns `findings == []`, and the *only* signal in
the whole crate is `save` returning `Err(BadTimestamp)` — after the creator has done the work, and
with no rule, no §14 step and no finding that could have warned them. The refusal itself is right
(§11 "validate its input and refuse rather than guess"; canonicalising `2026-10-11 06:00:00` means
inventing a timezone nobody recorded) — it is the one-way-ness that is worth a decision.
Reproduction (characterisation, not a failing test): `the_reader_accepts_a_timestamp_the_writer_refuses`.

**R2-F4 — `preference` (a coverage gap the fix created)** — `crates/comic-model/src/bundle.rs:697-711`.
Trigger: delete the whole asset-provenance branch of `non_canonical_timestamps`, or the job
`updated` and `consent.granted` checks. Expected: the owner's suite, which the fix extended for
exactly this rule, goes red. Actual: **it stays green** (mutations M3a/M3b below), because the new
`the_writer_refuses_a_timestamp_that_is_not_canonical_anywhere_in_the_model` names four fields and
never puts a non-canonical value in an asset's `provenance.created`/`consent.granted` or a job's
`updated`/`consent.granted`. The same is true of four more of the fixes: M2b, M5b, M6 and M7 leave
the owner's suite green while breaking the rule the fix established. None of this is a production
defect — the code is right in every one of those cases — it is the fix's new paths being unpinned.
My `every_timestamp_the_writer_emits_verbatim_is_checked` and the tests named in the table red for
each of them.

**R2-F5 — `preference`** — `crates/comic-layout/tests/layer_rules_contract.rs:174` (the ban),
`:170-172` (its comment).
Trigger: `pub use vectorcraft_doc::{Document, NodeId};` plus a record alias holding `Document` — the
*natural* way to write the violation; or `pub use vectorcraft_doc as vd;` plus `vd::Document`.
Expected: the ban round 1 asked to be widened "wherever it appears" catches a re-export of the doc
crate's types, which is what the comment claims. Actual: **both mutations leave the shipped ban
green** (M-L1/M-L2 below) — the array is four fully-qualified *dotted* literals, so `{`/`,` and a
crate alias slip past, and the comment's claim holds only for the `as`-item form
(`use vectorcraft_doc::Artboard as Art;`). My added test at `:202` is a token-level check on every
line that names the crate: it catches both mutations and keeps the three legitimate uses green
(`entities.rs:28`'s `NodeId` re-export, `document.rs:26`, and comic-layout's text types at
`fit/mod.rs:34`).

Round-1 verification:  all eight, against the code.

- **F1 — verified fixed.** `validate.rs:182-194` now builds ten per-kind sets and `Index::build`'s
  doc comment says why; `rule_1_references` (`:200`) reads only the matching set. Every one of the
  **33 reference slots** the schema declares is checked against its own kind — my sweep
  `rule_1_checks_every_reference_slot_against_its_own_kind` passes, and **M1**, which puts the four
  merged sets back exactly as round 1 found them, reds both the owner's new
  `rule_1_checks_the_kind_of_the_entity_a_reference_names` and my sweep. **No kinds are conflated
  any more**, and none is now over-strict: `Dialogue.speaker_character_id` is still rule 12's and
  not rule 1's (`the_speaker_reference_is_rule_12s_and_not_rule_1s`), and `retired_artboard_ids` is
  still deliberately unwalked. Round 1's test gap is closed.
- **F2 — verified fixed; the deterministic half holds in the code but was not pinned by the new
  test.** `bundle.rs:441-496`: the duplicate is a `Finding::error(Some(2), <losing key>, …)` and the
  winner is the first key in `BTreeMap` order. My `the_duplicate_is_reported_against_the_file_that_loses_and_the_sorted_first_wins`
  makes the two files **differ** and shows the padded `pages/page-008.json` winning and the finding
  naming `pages/page-8.json`; `the_duplicate_resolves_the_same_way_on_every_load` loads twice and
  compares content and findings; `two_job_files_for_one_job_id_are_reported_too` shows `jobs/` did
  not diverge. **M2a** reds both suites; **M2b** — keep the report, let the *last* file win — leaves
  the owner's suite green (its two files are byte-identical, so "which won" is invisible) and reds
  mine. One observation, not a finding: the winner is the lexicographically first key, which need
  not be the file `manifest.files` lists (`jobs/job-018.json` beats the declared `jobs/job-18.json`);
  the schema gives no rule, and it is deterministic, so I only record it.
- **F3 — verified fixed, and the refusal is the right call.** `non_canonical_timestamps`
  (`bundle.rs:687-716`) covers **every** timestamp the writer emits verbatim — `manifest.created`,
  `project.metadata.created`, an asset's `provenance.created` and `provenance.consent.granted`, a
  job's `created`/`updated`/`consent.granted` (both copies: `provenance.jobs` and `jobs/`), and a
  decision's `timestamp` — and excludes **exactly the two** the writer derives, `manifest.updated`
  and `ProjectMetadata.updated` (`:575`, `:580`), which my
  `the_two_derived_timestamps_are_replaced_by_now_and_are_not_refused` shows are overwritten with
  `now` and land canonical in both files. I checked the model exhaustively for `: Timestamp` fields
  (entities.rs:347, 437, 561, 563, 911, 913, 935 plus the manifest's two) — the list is complete,
  and `no_non_canonical_timestamp_can_reach_the_disk_at_all` re-derives it from the bytes on disk
  rather than from the fields a test remembered. Refusal over canonicalisation is defensible under
  §11 ("validate its input and **refuse rather than guess**") *and* §5.7 ("a writer must produce this
  form"): the only canonicalisable case that would be lost is an equivalent spelling of the same
  instant, and inventing the timezone for `2026-10-11 06:00:00` is precisely the guess §11 forbids.
  The residue is R2-F3 (one-way reporting) and R2-F4 (four unchecked branches in the owner's test).
- **F4 — verified fixed.** All three fallible checks are before the first mutation: the timestamp
  check (`:552`), `join_relative(&document_key)?` (`:563`) and the document-on-disk check (`:566`);
  everything derived goes into `project`/`manifest` **locals** and reaches `bundle` only in the `Ok`
  arm (`:589-594`); staging is removed on success (`:658`) and on failure (`:597`). Round 1's
  reproduction checked one field; my `every_way_a_save_can_refuse_leaves_the_whole_model_and_the_whole_directory_untouched`
  snapshots the **whole** model (`ComicBundle: PartialEq`) and **every byte** of the directory over
  four kinds of refusal, including a stale `.save-tmp/` a killed run left, which a refused save must
  leave alone because it never created it. **M4a** (derive onto `bundle` early) and **M4b** (no
  staging removal on the error arm) red both suites. The one thing that is *not* atomic, and should
  not be: a failure once step 4 has begun leaves the previous manifest over a partly replaced
  generation — that is §10's window, §14 keeps it survivable, and
  `a_failure_after_staging_still_removes_the_staging_directory_and_stays_loadable` shows the
  staging directory is still cleaned and the bundle still loads with the mismatch reported.
- **F5 — verified fixed, and the carry-over cannot resurrect.** `files_from_tree` (`bundle.rs:668-678`)
  iterates the **fresh** `scan_tree` and only *looks up* each present key in the previous map, so a
  key for a file that is gone cannot come back; my `the_carry_over_cannot_resurrect_a_key_for_a_file_that_no_longer_exists`
  puts `pages/page-99.json` in the manifest and shows it gone from both the written manifest and the
  model, while `project.json`'s key survives and a genuinely new file gets a fresh entry.
  `a_carried_over_entry_key_survives_load_save_load` extends round 1's one-save reproduction to the
  whole cycle, which is what §12 promises. **M5a** (drop the carry-over) reds both suites; **M5b**
  (carry over the entire previous map, the resurrection this finding asks about) leaves the owner's
  suite green and reds mine.
- **F6 — verified fixed.** `rule_3_versions_and_identity` (`validate.rs:441-448`) builds the subject
  from `ComicBundle::page_file_key` / `job_file_key`, so the subject is the real key and two files of
  one kind produce two differently-subjected findings — my
  `rule_3_names_the_file_that_declares_the_wrong_version` shows `pages/page-8.json` and
  `jobs/job-30.json` and asserts no `<…>` template survives anywhere at `:411`'s neighbours. The only
  remaining `job-<JobId>` text is inside a message that is *explaining the convention* under a real
  subject, which is correct. **M6** (restore templates) leaves the owner's suite green — its rule-3
  test asserts message substrings, never subjects — and reds mine.
- **F7 — verified fixed, and the asymmetry is gone everywhere.** Both halves of the `files`/tree
  mismatch are `Finding::warning(None, …)` (`bundle.rs:762`, `:768`, `:792`), and
  `files_mismatch_findings` sorts by subject so the pair is stable. My
  `both_halves_of_the_files_mismatch_are_the_same_non_blocking_severity` puts both halves in one
  load and asserts `Severity::Warning`, `!is_error()` and `rule == None` for each. The severity is
  defensible against §14: the mismatch is "one condition with three causes", the third "benign and
  expected", and §14 requires it not to be fatal — the crate's own `Severity::Warning` doc names
  exactly this case. **M7** (one half back to `error`) leaves the owner's suite green and reds mine.
- **F8 — verified fixed.** The erratum landed: §6.5 and §6.6 now read "A **closed shape**: it
  carries no `unknown` (§12)", matching §12's list, matching `entities.rs:854-870`/`:906-928`, and
  matching the pinned test `a_closed_value_type_has_nowhere_to_keep_an_extra_key`. The frozen
  document no longer contradicts itself.

Mutation table:  I mutated production code, ran **the owner's suite** (`--lib --test fixture_v1
--test schema_contract`, their 83) and **mine** (`--test round1_reviewer --test round2_reviewer`),
then restored each file and verified the md5 against a byte-copy. Every row was re-run after the
restore to confirm the pre-mutation result. "new reds" excludes the two standing reproductions in
`round2_reviewer.rs`.

| # | mutation | the owner's 83 | my suites |
|---|---|---|---|
| M1 | `Index::build`: `pages` = every id under `pages/` (round 1's M1 / the F1 fix reverted) | **fail** — `rule_1_checks_the_kind_of_the_entity_a_reference_names` | fail — `finding_1_*` ×2 + `rule_1_checks_every_reference_slot_against_its_own_kind` |
| M2a | `load`: report no duplicate, keep first-wins | **fail** — `two_files_for_one_page_id_are_reported_rather_than_collapsed` | fail — `finding_2_*`, `two_job_files_…`, `the_duplicate_is_reported_…` |
| M2b | keep the report, let the **last** file win | **pass** (65+10+8 green) | fail — `the_duplicate_is_reported_…`, `the_duplicate_resolves_the_same_way_on_every_load` |
| M3a | drop the whole asset-provenance timestamp branch | **pass** | fail — `every_timestamp_the_writer_emits_verbatim_is_checked` |
| M3b | drop the job `updated` + `consent.granted` checks | **pass** | fail — same |
| M3c | drop the `manifest.created` check | **fail** — `the_writer_refuses_a_timestamp_that_is_not_canonical_anywhere_in_the_model` | fail — `finding_3_*`, `every_timestamp_…`, `the_reader_accepts_…` |
| M4a | derive `bundle.project.metadata.updated` before the fallible checks | **fail** — `a_refused_save_leaves_the_model_and_the_directory_as_it_found_them` | fail — `every_way_a_save_can_refuse_…` |
| M4b | never remove `.save-tmp` on the error arm | **fail** — `a_save_writes_the_manifest_only_after_every_content_file` | fail — `a_failure_after_staging_…` |
| M5a | drop the `FileEntry.unknown` carry-over | **fail** — `an_unknown_key_on_a_manifest_file_entry_survives_a_save` | fail — `finding_5_*`, `a_carried_over_entry_key_…`, `the_carry_over_cannot_resurrect_…` |
| M5b | carry over the **whole** previous map (resurrect a deleted file's key) | **pass** | fail — `the_prune_step_removes_a_deleted_page_and_leaves_assets_alone`, `the_carry_over_cannot_resurrect_…` |
| M6 | rule 3's subjects back to `pages/page-<PageId>.json` / `jobs/job-<JobId>.json` | **pass** | fail — `rule_3_names_the_file_that_declares_the_wrong_version` |
| M7 | the "declared file absent" half back to `Finding::error` | **pass** | fail — `both_halves_of_the_files_mismatch_are_the_same_non_blocking_severity` |
| M8 | **write the manifest before step 4** (round 1's M7) | **fail** — `a_save_writes_the_manifest_only_after_every_content_file` (+3) | fail — `a_failure_in_step_4_leaves_the_previous_manifest_untouched`, `a_failure_after_staging_…` |
| M9 | `is_canonical_timestamp` always true | **fail** — 3 (`canonical_timestamps_are_the_one_form`, `a_save_refuses_a_timestamp_that_is_not_canonical`, `the_writer_refuses_…`) | fail — 5 |
| M-L1 | `pub use vectorcraft_doc::{Document, NodeId};` + a record-side alias holding `Document`, in `entities.rs` | the **layer** test `no_comic_crate_uses_a_document_type_in_its_code` — **pass** (evaded) | my layer test — fail |
| M-L2 | `pub use vectorcraft_doc as vd;` + `vd::Document`, in `entities.rs` | the same shipped ban — **pass** (evaded) | my layer test — fail |

Read the table as: the two test gaps round 1 reported are **closed** — M1 now reds the owner's suite
(it did not before) and M8 does too. What remains is that four of the *fixes* are pinned only by the
reviewer's tests (M2b, M5b, M6, M7) and one of the fix's own branches is pinned by neither (M3a,
M3b) — R2-F4. M-L1/M-L2 are R2-F5.

New-behaviour check:  the fixes as new code, read against the schema.

- **F1 — correct, and complete.** The sets are per-kind and `rule_1_references` was walked field by
  field against §13 rule 1: 33 slots, every one checked against the right set. The two shapes that
  could hide a conflated set are both clean: `Page.reading_order` is checked against `panels` (not
  against `panel_ids`, which would be a tautology) and `review.superseded_asset_id` against
  `assets`. No kind is conflated.
- **F2 — correct. The report is right and the deterministic half holds.** The winner is
  `files.keys()` order on a `BTreeMap`, so it is the same on every load and every platform, and the
  finding names the file that *lost* (`pages/page-8.json` when `page-008.json` wins) with a message
  that names the winner — an inversion of the two roles would be visible to my two tests and is not
  present. The one gap is R2-F4's sibling: the owner's new test compares none of this.
- **F3 — correct, and refusal is the right call.** Coverage is exhaustive and the two derived
  timestamps are excluded *and* overwritten. The write path never emits a non-canonical timestamp:
  `no_non_canonical_timestamp_can_reach_the_disk_at_all` re-derives that from the bytes on disk with
  no knowledge of which fields the check names. Residue: R2-F3 (the load half says nothing) and
  R2-F4 (four branches unpinned).
- **F4 — correct.** Three fallible checks, all before the first mutation; the model reaches `bundle`
  only on `Ok`; `.save-tmp/` removed on both paths; a stale one from an earlier killed run is left
  exactly where it was by a refusal. A post-step-4 failure is the schema's window, not a leak.
- **F5 — correct.** The carry-over is a lookup into the new scan, so it can neither resurrect a
  removed file's key nor invent one; the hash and size stay derived while the unknown keys stay the
  entry's.
- **F6 — correct.** Real subjects for `project.json` and the eight fixed story files, and
  `page_file_key`/`job_file_key` for the per-id ones; no template subject survives in any rule.
- **F7 — correct and defensible.** Both halves `Warning`, both `rule: None`, both sorted by subject.
  §14 calls them one condition and forbids making it fatal; the crate's `Severity` doc already
  describes this exact case as `Warning`.

Layer-rules:  the exemption is justified; the ban as written is evadable, and my added test closes it.

- **The `document.rs` exemption is sound, not a hole.** `crates/comic-model/src/document.rs` is the
  one file whose job *is* the doc crate: it builds `DocumentFacts` from a
  `vectorcraft_doc::Document` so the L2 model does not have to reach `crates/format` (L3). Every
  other legitimate use is `NodeId` — `entities.rs:28`'s re-export — and comic-layout's text types
  at `fit/mod.rs:34`; all three are `vectorcraft_doc::…` paths that the ban's list does not name, so
  the exemption is not what makes them pass. Two residuals, both stated rather than exploited: the
  exemption is by *file name*, so any future `src/x/document.rs` inherits it; and `document.rs`
  could launder the crate with `pub use vectorcraft_doc as vd;`, after which `vd::Document` in
  another module is invisible to *any* textual rule. My test does not close that; nothing textual
  can, and it would need a review-time rule instead.
- **The ban is evadable, and I could still evade it after the tightening.** Two mutations, both
  compiling, both adding a record-side alias that holds the document in violation of design §6.1 —
  `pub use vectorcraft_doc::{Document, NodeId};` (the natural spelling; the ban's array has only
  the dotted literals) and `pub use vectorcraft_doc as vd;` — leave the shipped test **green**
  (M-L1/M-L2). The comment at `:170-172` claims "a re-export of the doc crate's own types evaded a
  ban on bare words — `use vectorcraft_doc::Artboard as Art;` is caught here": that is true of the
  `as`-item form and false of the braced form. My `no_comic_crate_reaches_a_document_type_by_a_brace_a_rename_or_a_space`
  (`:202`) is token-level on every line that names the crate and reds for both mutations while
  keeping all three legitimate uses green — the tightening round 1 asked for, done properly.

Semantic defects:  **yes, two — and neither is introduced by the fixes.** R2-F1 and R2-F2 are
semantic defects in production code (a fatal load where §14 requires a finding; a save that deletes
a declared file), both living in the code the F2 and F4 fixes rewrote, both pre-existing in shape
and both found by the round-2 failure-path walk the role asks for. The fixes themselves contain **no
wrong predicate, no weakened security check and no changed ownership**: `resolve_path` is untouched
and still canonicalises before the containment check and never opens an escape; `join_relative` is
now applied to the *writer's* document path as well as the reader's, which is a strengthening; every
field's owner is unchanged (`Page` still has no size, `Panel` no geometry, `Dialogue` no balloon);
`files` is still written from a fresh `scan_tree`, so it still detects tampering; and the only
newly reachable error is the intended `Error::BadTimestamp`.

Gate:          `cargo xtask ci` — **not run end to end.** Every step that bears on this tree was run
               individually and is quoted above (clippy 0 diagnostics, `cargo test -p
               vectorcraft-comic-model` with the owner's 103 green, `cargo xtask layers` OK 29
               crates, the layer contract tests 8/8). The end-to-end gate was not run because the
               working tree was **moving underneath this review**: `crates/comic-layout/src/fit/*.rs`
               was rewritten between 08:14 and 08:33 by another owner, and `cargo fmt --all -- --check`
               fails right now on `crates/comic-layout/src/fit/sizing.rs` (a file no part of this
               change touches), which would fail the gate's first step for a reason that is not
               attributable to the revision under review.

Unverified:
- **`round1_reviewer.rs` byte-identity cannot be proved.** The file is untracked in git, so there is
  no stored copy to diff against. What I can show is the mtime (07:44:58) preceding every
  post-round-1 production edit, the unchanged 19 test names, the unchanged `finding_*` assertions,
  and all 19 green — consistent with "byte-untouched", not proof of it.
- **The layer contract file changed under me mid-review.** At 08:11 the owner's rewritten test 4
  (`each_comic_manifest_declares_only_what_its_code_uses`) was **red** — `crates/comic-layout
  declares thiserror but no source under src/ names thiserror` — and `crates/comic-layout/src` would
  not compile at 08:16, 08:28 and 08:30. An out-of-tree edit at 08:16 resolved both; the final state
  I report is the 08:35 one, where all 8 layer tests pass including my new one. Any claim about the
  workspace gate belongs to whoever owns `crates/comic-layout` at that moment, not to this change.
- **§10 steps 2–3** (serialize the document through `crates/format` and verify by loading it back)
  are still delegated to the caller by design, so nothing here exercises them; a full round trip
  through `crates/format` remains `verification-engineer`'s milestone test.
- **Windows**: `fs::rename` over an existing file, the `.save-tmp/` replacement and "a directory
  where a file goes" behave differently there; every row above was run on Linux only.
- **`crates/comic-ai`'s duplicate `JobStatus`/`ProviderId`** is still out of this tree and round 1's
  note stands — the two-enums defect §5.2.1 forbids is live until that re-export lands.
- **Whether a `Severity::Error` finding should block a save** is still the command layer's policy,
  and that layer still does not exist, so R2-F3's "who warns the creator" cannot be settled from
  this crate alone.
- **The `docs/ci-3da4080b.md`, `ASSETS.md` and `porting_plan_and_design/` working-tree changes** are
  not part of this review; `docs/milestones/M1.md` is the plan this implements.

---

## Round 3 — verification

MODEL:         `deepseek-v4.1-flash` — `COPILOT_MODEL=deepseek-v4.1-flash` in the environment, **and**
               this session's own event log now corroborates it: `session-state/bf64cb60-…/events.jsonl`
               opens with the `session.start` record whose `sessionId` is that session folder, and
               **2412 of its 2677 `assistant.message` records carry `"model":"deepseek-v4.1-flash"`**
               (the most recent included; the other model names in the file belong to sub-agents and to
               other sessions sharing the folder). That is the model the `code-reviewer` definition
               requires, so this round ran as specified. The residual gap is unchanged from rounds 1
               and 2: `~/.copilot/session-store.db :: assistant_usage_events` still has **0 rows** for
               this session (632 overall), so that third source says nothing either way.
Reviewer:      code-reviewer on deepseek-v4.1-flash
Revision:      working tree (base `16594c58`) — rounds 1 and 2 both reviewed "working tree (base
               `16594c58`)" after their own fix rounds; this is the **third** state of the same
               untracked tree.  Files:
               `crates/comic-model/src/{bundle,validate,entities,error,lib,scratch}.rs`,
               `crates/comic-model/tests/{round1_reviewer,round2_reviewer,round3_reviewer,fixture_v1}.rs`
               (the third is new — mine), `crates/comic-layout/tests/layer_rules_contract.rs`,
               `docs/comiccraft-project-schema.md` §3/§4/§8.3/§13/§14
Design refs:   schema §3 (layout, "padding is cosmetic"), §4 (`manifest.files` covers every file),
               §8.3 (`jobs/` is the live copy), §13 rules 1/2/3/9/12/13/14/18, §14 (the two fatal
               failures and "a finding on a usable project"), §5.7 + §11 (the writer refuses);
               design §4.2/§4.3 (layers), §6.1 (one owner per relationship)

Revision reviewed:  the post-round-2 revision **plus two edits that landed while this review was
               running**: `bundle.rs` 09:22:15 (a `#[cfg(test)]` addition,
               `a_corrupt_collection_file_is_still_fatal`), `src/scratch.rs` and `tests/fixture_v1.rs`
               09:21:31 (per-process scratch directory names). Pinned by md5 —
               `bundle.rs f16fda394d29bd206d9bae51ed2b2ac7`, `validate.rs cc0701bca0e4a8cbb95c8e4420103d4c`,
               `entities.rs 9ee78c1802ec8ffe7bf27a076263a918`, `error.rs 36c6fac4e0672b64bdb69d726da5610d`,
               `lib.rs 5dcf9236814fe2b4a1a2f099e4c78b7c`, `scratch.rs 26b6a5f807e642e37c326651662a7f3e`,
               `ids.rs d268e1d6b5a3b4ee6e4c9ca79c611e36`, `json.rs 300de5a52866c7f30634d456ef2a81dd`,
               `document.rs 92c93098f8706ab75dc78a8a3a1f033e`. A `diff` of `bundle.rs` against the copy
               I took at 09:04 proves the only change since the round-2 fixes is inside
               `#[cfg(test)] mod tests`, so **every production line reviewed here is the one round 2
               verified**; every mutation below was re-run against these hashes, and the reviewed files
               were restored and re-hashed after each.
               **Two pins in this list no longer describe the tree, recorded here so a reader who
               re-runs them is not misled** (annotated by the M1 fix round — `command-engineer`, not
               the reviewer; the same fact is in §R3-8 of
               `docs/reviews/M1-command-engineer-comic-commands.md`):
               `validate.rs` is now **`7f4983caa40e76fc21fc6f907385fcf6`** (was `cc0701bc…`; the
               change is one `#[cfg(test)]` initializer, `files_of_record: BTreeSet::new(),`, that
               `ComicBundle`'s new field forces at `:1093`) and `bundle.rs` is now
               **`926adc8a58d64fc71250d8be563ffc24`** (was `f16fda39…`; not test-only — W4's round-2
               fix changed production lines there: the field, the load seed, the staged return and
               the prune predicate; the same file in the acceptance report's sha256 pins is
               `f928a4d1…`). The other seven pins still match the tree, re-checked with `md5sum` in
               the fix round.
Verdict:       **approved**

Unit tests:
- Added (new file): `crates/comic-model/tests/round3_reviewer.rs` — **8 tests, 8 pass**: the
  two-finding pair and its one-finding mirror for an undeclared file (`:112`, `:146`), the split's
  *other* half — a corrupt collection file (`:168`) and a directory where one belongs (`:319`) are
  fatal rather than an empty collection — all seven collections absent at once (`:194`), the `jobs/`
  stray branch (`:263`), a missing registry governed by rule 1 (`:299`), and a save's effect on nested
  asset bytes (`:340`).
- Not added, deliberately: nothing to `round1_reviewer.rs` or `round2_reviewer.rs`. Both are
  byte-identical to what rounds 1 and 2 left (`round1_reviewer.rs c45c70ae64db49704e81bea65a64f3de`
  — round 2's own hash, `round2_reviewer.rs 1652ed537ef9a36d0e3b02aeed224666` — round 2's own hash),
  so the two earlier rounds' evidence is still sitting on disk unchanged.
- Command:
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-comic-model --no-fail-fast
     Running unittests src/lib.rs
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/fixture_v1.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round1_reviewer.rs
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round2_reviewer.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/round3_reviewer.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/schema_contract.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests vectorcraft_comic_model
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
  **134 tests, all green** (70 + 10 + 19 + 18 + 8 + 8 + 1) — the owner's 125 plus my 8, plus the one
  test the owner added at 09:22 while this review ran. Run six times back to back, plus once with
  `--test-threads=1`, with no failure.
- `cargo clippy -p vectorcraft-comic-model --all-targets -- -D warnings` → **0 diagnostics**.
- `cargo xtask layers` → `OK: 29 crates, no layering violations.`
- `cargo test -p vectorcraft-comic-layout --test layer_rules_contract` → `8 passed; 0 failed` (the
  file is byte-identical to the 09:04 copy: `f90eeba38883467b5c125978d632a15d`).
- `rustfmt --check --edition 2024 crates/comic-model/tests/round3_reviewer.rs` → clean;
  `cargo fmt --all -- --check` → clean (so `xtask ci`'s first step passes and reports no lie about
  the rest).

Findings:      **0 defects — and therefore no `changes-requested`. Six notes (`preference`), all
               recorded below rather than fixed, none of which changes code that would be wrong or
               loses a creator's work.** The bar this round is: a contradiction, a wrong predicate, a
               lockout where §14 wants a finding, a security hole, or a save destroying data. I found
               none in the new code, and the one data-losing behaviour I did find is pre-existing,
               reported at load, and untouched by this revision (note 1).

### Round-2 verification — all five, against the post-round-3 code

- **R2-F1 (a stray `pages/` file must not be a lockout) — verified fixed.** `bundle.rs:513-531` now
  reports `Finding::error(Some(13), key, "… is not `page-<PageId>.json` …")` and `continue`s instead
  of returning, and the `jobs/` twin at `:552-560` reports under `rule: None` ("no *numbered* rule
  governs a `jobs/` file's name"). `Error::BadIdFileName` is **gone from the crate** — a repo-wide
  grep finds the name only in this file's round-2 section and in `docs/milestones/M1.md`'s historical
  row — so the lockout is not merely unreached, it is unrepresentable. Mutation: put the refusal back
  (`Error::Missing` on the unparsable name) → the owner's own
  `bundle::tests::a_stray_json_file_in_pages_is_a_finding_not_a_lockout` **reds**. The `jobs/` branch
  is a second path the owner's suite does not walk: making only *that* one fatal leaves the owner's 70
  lib tests green and reds my `a_stray_job_file_is_a_finding_not_a_lockout_and_a_save_leaves_it_where_it_is`.
- **R2-F2 (a save must not delete a declared file it does not own) — verified fixed.**
  `is_written_by_this_version` (`bundle.rs:850-858`) gates the prune to names this version writes, so
  `story/relationships.json` from a newer build survives and stays declared via `files_from_tree`.
  Mutation: drop the gate (prune by directory, exactly as the finding described) → the owner's
  `bundle::tests::a_save_keeps_a_declared_file_this_version_does_not_own` **reds**, while the older
  `a_save_removes_a_page_file_the_bundle_no_longer_has` still passes — which is the point of the test:
  the *narrowing* is what fails, not the prune's existence.
- **R2-F3 (the one-way timestamp refusal) — verified fixed as the preference it was.** Round 2 asked
  for one of two answers: "either `load` reports it (the §14 'usable project, repairable' reading) or
  the refusal is documented as save-time-only". The second is what landed, and it is more than a
  sentence: `save`'s doc comment says "**Only the writer refuses a timestamp**" and names
  `ComicBundle::non_canonical_timestamps` as the query a caller asks at open time; `error.rs`'s
  `BadTimestamp` doc says the same; and that method is `pub`, is built from the very function the
  refusal is (`timestamps_written_verbatim`), and so cannot drift from it. My round-2 characterisation
  `the_reader_accepts_a_timestamp_the_writer_refuses` still passes **unchanged**, which is the correct
  outcome for a preference resolved by documentation. The residue is real and is recorded, not hidden:
  a caller that never asks gets no warning until the first save fails.
- **R2-F4 (the fix's own branches were unpinned) — verified fixed.** Both mutations now red the
  owner's own suite: **M3a** (drop the whole asset-provenance timestamp branch) →
  `bundle::tests::the_writer_refuses_a_timestamp_that_is_not_canonical_anywhere_in_the_model` reds;
  **M3b** (drop the job `updated` + `consent.granted` checks) → the same test reds. The test's
  doc comment says exactly that ("One case per branch … including the asset-provenance and job ones,
  which the first version of this test left unpinned"), and its eight cases are one per branch plus the
  `jobs/` live copy plus the two derived timestamps.
- **R2-F5 (the document ban as written was evadable) — verified fixed in substance.** Both evasions
  now red the layer suite: `pub use vectorcraft_doc::{Document, NodeId};` + a record-side alias →
  `no_comic_crate_reaches_a_document_type_by_a_brace_a_rename_or_a_space`
  (`layer_rules_contract.rs:217`) fails at `:243` ("names `Document`"); `pub use vectorcraft_doc as vd;`
  + `vd::Document` → the same test fails at `:249` ("aliases the doc crate"). The dotted-substring ban
  beside it (`:174`) is **still** evaded by both — verified, not assumed — and the fix chose to *state*
  that in its own comment ("What it does **not** cover, and cannot: the braced form … and a crate
  alias … Both are the token-level test beside this one … which is the ban this rule actually needs")
  rather than pretend the array covers it. That is the honest reading of a preference, the rule is
  enforced, and one residual stays as round 2 recorded it: the exemption is by *file name*, so
  `document.rs` could still launder the crate with an alias — no textual rule can see that, and it
  needs a review-time rule.

### The open question: **two findings, and the pair is the right shape**

An absent *declared* collection file is reported twice on one subject — the `files`/tree half (§14
step 1, `bundle.rs:925-930`) and the new model half (`bundle.rs:575-585`, "it loaded as an empty
collection"). **I judge that correct, and merging them into one would be worse in both directions:**

1. **They answer different questions from different data at different steps of §14.** Step 1 compares
   `manifest.files` with the tree *as §4 defines it* — for **every** declared file, collection or not,
   page or script. Step 3 is about what the *model* holds afterwards. The first is the consistency
   record's evidence that the manifest is stale (or that a file was deleted, or lost); the second is
   the only statement of what the project opened *as*. A finding that tried to be both would have to
   know which files are collections, which is a coupling §14 step 1 does not have.
2. **Each half is necessary, and I have tests that fail if either is dropped.**
   - Drop the *tree* half for collection files (the obvious "merge"): my
     `an_absent_declared_collection_file_is_reported_by_both_halves_and_they_say_different_things`
     and `every_collection_file_absent_at_once_still_opens_and_no_consequence_is_silent` red, and the
     owner's own suite stays green — the evidence that the manifest no longer describes the tree is
     gone for exactly the seven files they asked to tolerate.
   - Drop the *model* half unless the file is declared (the mirror merge): my
     `an_absent_undeclared_collection_file_is_reported_once_and_only_by_the_model_half` reds. That is
     the case that makes the merge indefensible — with the `files` entry gone (a hand-damaged bundle,
     or a newer version's manifest) there is no tree half *to* keep, so an undeclared absent collection
     file would be reported by nothing at all.
3. **A caller is not left wondering.** They are distinguishable on their face: one names the
   consistency record ("`manifest.files` lists a file that is not in the bundle"), the other names the
   consequence ("it loaded as an empty collection, so the project opens and can be repaired (§14)").
   Both point at the same remedy — restore the file or accept the empty collection — so a creator
   acting on either is doing the right thing, which is the test I would apply to a pair like this.
4. **It is the code's established shape, not a new invention.** A stray file already produces two
   findings on one subject: the §14 step 1 half ("the file is in the bundle but `manifest.files` does
   not list it") plus the name half ("`notes.json` is not `job-<JobId>.json`"). My
   `a_stray_job_file_is_a_finding_not_a_lockout_and_a_save_leaves_it_where_it_is` asserts both, and
   the pair reads naturally.

One residual worth naming, and it is a *caller* question rather than a change to make here: the two
findings share `subject`, `severity` and `rule: None`, so the only thing separating them is the
message. A caller that deduplicates findings by `(subject, rule)` for display would collapse them and
lose the model half; one that keys on the whole `Finding` shows both. If a machine-readable
discriminator is wanted, it belongs in `Finding` (a `kind` or a source enum) for the command layer to
consume — not in a rewrite of this split, and not in message text.

### The role split: every file is classified right, and nothing is now ungoverned

Checked file by file against §3's layout and §14's two fatal failures:

| File | Role | Absent | Right? |
|---|---|---|---|
| `manifest.json` | identity | `MissingManifest` (fatal) | yes — §14 step 1 opens with it, and a bundle *is* its manifest (§3) |
| `project.json` | identity | `Error::Read` (fatal) | yes — it carries `ComicProject`, whose `id` cannot be invented (`ComicId(0)` is refused), so there is nothing to report *against*; §14's "usable project" presupposes it |
| `story/{chapters,scenes,characters,locations,styles}.json`, `assets/index.json`, `provenance/generations.json` | collection | empty + finding | yes — §14 step 3's fatal is "a *comic file* that will not parse"; an absence is the absence of content, and the empty collection is what the file held before anything was added |
| `pages/page-<id>.json`, `jobs/job-<id>.json` | one entity each | *(never fatal — the loader walks the tree)* | yes: a bundle with no pages is legal, and removing one page file is already governed (`files` mismatch + rule 1 for every reference that named it) |
| `document/comic.vectorcraft` | the one visual document | `Error::Missing` (fatal) | yes — §14 step 2 |
| `assets/**/*` content, `exports/` | content / derived | n/a | unchanged, and `exports/` is still never read (§3) |

**Nothing is silently ungoverned.** I walked all seven against the fixture with **all seven absent at
once** (`every_collection_file_absent_at_once_still_opens_and_no_consequence_is_silent`): the load
opens, the model is seven empty collections, and `validate` still reports every reference that pointed
into one — `ComicProject(1).{chapter,character,location,style_profile}_ids`, `Page(8).chapter_id`,
`Page(8).scene_ids`, `Panel(9|11).scene_id`, `Panel(9).artwork_refs[].asset_id`,
`GenerationJob(18).input_asset_ids/result_asset_ids` (rule 1), `Dialogue(12).speaker_character_id`
(rule 12), and `pages/page-8.json` as an orphan (rule 14). Pages and jobs that *are* present still
load, so the tolerance is not contagious.

One correction to the code's own claim, recorded as **note 2**: for an absent `assets/index.json` the
governance is **rule 1, not rule 9** — with an empty registry there is no asset left to resolve a path
for, so rule 9 has nothing to say (asserted in
`a_missing_asset_registry_leaves_every_dangling_asset_reference_to_rule_1`; rule 9 still owns the
registered-asset-with-a-missing-file case that round 1 tested). The governance exists either way; only
the comment's attribution is loose.

`Warning` is the right severity: §14 calls the whole condition one non-fatal report, the crate's
`Severity::Warning` doc names this exact case, and filing the model half as an `Error` while the tree
half for the *same file* is a `Warning` would revive round 1's F7 asymmetry inside a single load.

Taking the **manifest's** `schema_version` for an empty collection is correct, not a hiding place.
Rule 3 compares each file's declared version against `bundle.manifest.schema_version`
(`validate.rs:420`), so the empty collection's value makes it agree by construction — and there is
nothing there to disagree *with*: a file that is not present declared nothing, and reporting it under
rule 3 would be reporting a version disagreement for a file that does not exist. The alternative is
worse: `SCHEMA_VERSION` would manufacture a spurious rule-3 error for all seven files the moment a
newer build loads an older bundle, which is precisely the migration path §11 says must be quiet.
Honest limitation: at v1 **the choice is not observable** — `check_supported` refuses >1 and 0, so no
supported bundle declares anything but 1 — so my test pins the *property* ("rule 3 stays silent for an
absent collection file") rather than the difference between the two candidates. The reasoning above,
not a test, is what makes it right today.

The other half of the split is intact and now pinned twice: `read_collection` (`bundle.rs:949-955`)
answers `None` **only** for `NotFound`, so a *corrupt* collection file is still `Error::Parse` and a
directory sitting where a collection file belongs is still `Error::Read`. Mutation: tolerate *any*
read error as absent → my `a_directory_where_a_collection_file_belongs_is_fatal_and_not_an_empty_collection`
reds (the owner's suite does not). Mutation: tolerate a *parse* error as absent → my
`a_corrupt_collection_file_is_fatal_and_is_not_loaded_as_an_empty_one` reds **and** the owner's new
`a_corrupt_collection_file_is_still_fatal` reds. That second mutation is the one with teeth: with it,
a damaged `story/locations.json` would load as "not in the bundle", and the next save would overwrite
the readable part of the damaged file with an empty collection — the creator's one recoverable copy,
gone, by a save.

### Extra gaps: M2b, M5b, M6 and M7 — all four now bite

Each mutation was re-applied to the reviewed revision and the **owner's own** suite (`--lib
--test fixture_v1 --test schema_contract`, 88 tests) was run:

| gap | mutation | the owner's suite now |
|---|---|---|
| **M2b** | keep the duplicate report, let the *last* file win | **FAIL** — `bundle::tests::two_files_for_one_page_id_are_reported_rather_than_collapsed` (its two files differ, so which won is visible) |
| **M5b** | carry over the whole previous `files` map | **FAIL** — `bundle::tests::an_unknown_key_on_a_manifest_file_entry_survives_a_save` (its third save asserts a key for a deleted file does not come back) |
| **M6** | rule 3's subjects back to `pages/page-<PageId>.json` / `jobs/job-<JobId>.json` | **FAIL** — `validate::tests::rule_3_reports_a_version_or_identity_that_disagrees_with_the_manifest` |
| **M7** | the "declared file absent" half back to `Finding::error` | **FAIL** — `bundle::tests::a_load_reports_a_files_mismatch_rather_than_refusing` |

Round 2 recorded all four as leaving the suite green; none does now. They are tests that bite: each
was added to make a *specific* observable move — a differing pair of files, a third save, a real
subject, a severity — rather than to assert a shape the implementation already had. For completeness,
the three mutations that the owner's suite still does **not** catch are the three my round-3 file pins
(`m_merge` → 2 of mine red; `m_recerr` → 1; `m_jobstray` → 1), and the one it now catches twice is the
corrupt-collection one, where we converged independently.

### Semantic defects: **none in this revision**

No contradiction, no wrong predicate, no lockout where §14 wants a finding, no security hole, and
nothing this change makes lose a creator's work. Specifically re-checked rather than assumed:

- **§14's tolerance is not contagious.** Identity files, the document and unparseable comic files are
  still the only fatal outcomes; `check_supported` still refuses a newer version before anything is
  read; a stray `pages/` file opens the bundle. The one place a creator could still be locked out by a
  *semantic* condition is a `manifest.document.relative_path` that escapes the bundle
  (`join_relative` → `Error::BadBundlePath`), which is §13 rule 9's order applied to the document and
  is unchanged from the revision round 2 approved.
- **Security: unchanged and still the strong way round.** `resolve_path`/`path_within` canonicalise
  before the containment check and never open an escape; `join_relative` is applied to the writer's
  document path as well as the reader's; rule 9 still reports an escaped path rather than resolving it;
  `.save-tmp/` is still never read as input and `exports/` is still never read at all. The new tolerance
  adds no path handling: it only decides *which* of two already-existing files to read.
- **Ownership: unchanged.** `Page` still has no size, `Panel` no geometry, `Dialogue` no balloon; the
  artboard owns the page size and the document owns the frame. `manifest.files` is still written from a
  fresh `scan_tree`, so it still detects tampering (§4).
- **No save-destroying-data path was added.** The prune is strictly *narrower* than before this round
  of fixes, and a save with an absent registry rewrites the empty registry while leaving
  `assets/panels/*` byte-identical (asserted). The one deletion I did find is pre-existing and is
  note 1 below.

Notes (all `preference`; none blocks approval):

1. **`crates/comic-model/src/bundle.rs:891-935` + `:866-882` — the prune deletes the losing file of an
   unresolved duplicate page/job pair, and a save does it before the creator is asked.** Trigger: a
   coherent bundle holding both `pages/page-8.json` and `pages/page-008.json` **with different
   content** (a hand copy, or a tool that padded the name); load; save. Expected: the schema does not
   say which of two files that mean one page survives a save, which is why this is a note rather than a
   defect. Actual (probed, not inferred): the load wins
   with the sorted-first file (`page-008.json`), reports the pair as a rule-2 `error` on the loser and a
   §14 "not listed" warning; the save then stages the canonical key `pages/page-8.json` **from the
   winner's record**, and `prune_stale` removes `pages/page-008.json`, so after the save one file
   remains and the losing file's bytes are gone. The load *does* report the ambiguity before anything is
   written, the winner is deterministic, and the prune behaved identically in the revisions rounds 1 and
   2 approved (round 2 recorded the winner rule as "an observation, not a finding") — so this is
   pre-existing and reported rather than silent, and it is *not* attributable to this revision. Worth a
   decision at the command layer if the lead wants one: the natural place to stop is "do not save while
   an unresolved duplicate page/job exists, ask the creator which file to keep", because a save is the
   one moment the ambiguity becomes destructive.
2. **`bundle.rs:442-449` (the `load` doc comment) — "a missing asset a rule-9 one" is the wrong
   attribution** for the absent-registry case: rule 9 needs a registered asset to resolve, so an empty
   registry leaves rule 1 to report every dangling `asset_id` (asserted). The governance is real; the
   comment is one word off. `preference`.
3. **`bundle.rs:850-858` vs `:704-740` — two lists that must agree.** `is_written_by_this_version`
   enumerates the names this version owns and `write_generation` enumerates the files it stages; a
   future story file added to one and not the other would either leave a stale file unpruned (never
   breaking, always visible in `files`) or prune a file this version still writes. A single table used
   by both would remove the class. `preference`.
4. **`bundle.rs:522` vs `:552` — the same kind of damage carries a rule number in one directory and
   not the other.** A stray `pages/` file is `Some(13)`; a stray `jobs/` file is `None`. The reasoning
   in the comment is sound (no §13 rule governs a `jobs/` file's name, and rule 13 is the page-file-name
   rule), and a caller filtering on `rule == 13` is not misled about the file it names — but the
   asymmetry is a thing a caller can trip over, and it is recorded rather than argued away.
   `preference`.
5. **`crates/comic-model/src/scratch.rs` + the integration `scratch()` helpers — fixed scratch names
   make two concurrent runs of this package fail spuriously.** Reproduction, on the revision I first
   picked up (before the 09:21:31 edit): run one test binary twice at once
   (`round2_reviewer --test-threads=4 & round2_reviewer --test-threads=4 & wait`) → one process reports
   `FAILED. 12 passed; 6 failed`, the other `FAILED. 15 passed; 3 failed`, in tests that pass alone; I
   also hit a `15 passed; 3 failed` run of `round2_reviewer` while `cargo xtask ci` was contending for
   the build lock in this workspace. **Already fixed while this review ran**: `src/scratch.rs` and
   `tests/fixture_v1.rs` now append `std::process::id()` and say why, and my `round3_reviewer.rs` does
   too (verified: two concurrent runs of its binary are both green, while `round2_reviewer.rs` — whose
   18 tests are still byte-identical to round 2's — still loses the race). Recorded so the next run of
   this suite is not mistaken for a regression: **a red `round1_reviewer`/`round2_reviewer` in a
   contended workspace is not evidence about the code.**
6. **`crates/comic-model/src/bundle.rs:575-585` — the two-finding pair shares `(subject, rule, severity)`
   and is separated only by its message.** Not a defect (see the open question above), and the analysis
   says the pair is right; this note exists so the command layer knows a naive `(subject, rule)` dedup
   would hide the model half, and so a future `Finding` discriminator is a deliberate API decision
   rather than an accident of message text. `preference`.

Gate:          `cargo xtask ci` — **not run end to end**, for the same reason round 2 gave and one more.
               The first step (`fmt`) passes on the whole workspace, including every file I touched. The
               `clippy` step fails **in `crates/comic-layout`**, not here:
               `crates/comic-layout/tests/round1_reviewer.rs:17` imports
               `vectorcraft_comic_layout::reading_order::narrative_indices`, which no longer exists
               (`E0432`) — an out-of-tree edit by the other owner mid-flight, which is exactly the case
               the brief told me to attribute rather than report. I re-ran and it is unchanged across
               two attempts. Every step that bears on this crate was run individually and is quoted
               above: clippy on `comic-model` with `--all-targets -D warnings` → 0 diagnostics, the
               134-test suite green, `cargo xtask layers` → OK 29 crates, the layer contract 8/8,
               `rustfmt --check` clean.

Unverified:
- **The `project.json`-is-fatal half of the split has no test of its own beyond the owner's.** I
  checked it by reading (`read_json` → `Error::Read`) and by the owner's
  `a_missing_identity_file_still_refuses`, and I did not add a second copy of that. What is *not*
  pinned anywhere is the choice itself — §14 lists only step 2 and step 3 as fatal, and a missing
  `project.json` is neither; refusing is the lead's decision (and, I think, the right one, since
  `ComicProject.id` cannot be invented), so it is worth the lead's own `§14` sentence if the schema is
  ever revised.
- **The empty collection's `schema_version` choice is unobservable at v1.** `check_supported` refuses
  anything but 1, so no supported bundle can declare a version other than 1 and the manifest-vs-
  `SCHEMA_VERSION` difference cannot be produced in a test. The reasoning is given above; the *test*
  only pins that rule 3 stays silent. It becomes observable at v2, and §11.1's v2 fixture is the place
  to pin it.
- **`asset_registry_id` is cross-checked by nothing, before or after this change.** `AssetIndexFile`
  has no id field, so rule 3 cannot compare it and rule 1 has no registry set. Pre-existing, unchanged
  by the split, and out of scope here — recorded because an absent registry now makes the dangling id
  visible without changing anything about it.
- **The `jobs/` stray path is pinned only by my test.** Worth a line in `M1.md`'s plan if the owner
  wants it in their own file.
- **`crates/comic-ai`'s duplicate `JobStatus`/`ProviderId`** is still out of this tree — round 1's note
  stands, and the two-enums defect §5.2.1 forbids is live until that re-export lands.
- **§10 steps 2–3** (serialize the document through `crates/format` and verify by loading it back) are
  still delegated by design; a full round trip through `crates/format` remains
  `verification-engineer`'s milestone test.
- **Windows**: `fs::rename` over an existing file, the `.save-tmp/` replacement, and "a directory where
  a file goes" behave differently there; every mutation, probe and test above ran on Linux only. Note
  that the *new* `read_collection` distinction (`NotFound` vs anything else) is one place where the
  platform's error for a directory-in-file's-place could differ in kind, so a Windows run of
  `a_directory_where_a_collection_file_belongs_is_fatal_and_not_an_empty_collection` is the one I would
  want to see before trusting that edge there.
- **The tree moved under this review twice** — `bundle.rs` 09:22:15 and `scratch.rs`/`fixture_v1.rs`
  09:21:31, both inside this session — so the hashes above are the contract for what I verified, not the
  working tree as it stands when this is read. The diff proves the movement was test-only, and every
  mutation was re-run against those hashes.
- **`docs/milestones/M1.md`** is the lead's narrative and is not this review's to edit; its §5.3 row
  still quotes round 2's `Err(BadIdFileName)` state, which is history rather than a stale reference to
  code that exists.
