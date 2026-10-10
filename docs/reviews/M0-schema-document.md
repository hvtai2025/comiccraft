# M0 — code-reviewer round 1, `docs/comiccraft-project-schema.md`

Reviewer:      code-reviewer on `opencodego/deepseek-v4.1-flash` (harness record)
Model note:    the harness records `opencodego/deepseek-v4.1-flash`; I **cannot verify it from
               the environment** — `get_current_session` exposes no model field and
               `~/.copilot/session-store.db :: assistant_usage_events` has **0 rows** for this
               session (`7f4af39f-7989-4abe-aaac-88b31e9d05bb`). I record the harness value as
               the model I ran on and flag the verification gap rather than claim proof.
Revision:      working tree (base `3da4080b`; `docs/comiccraft-project-schema.md` is untracked)
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 500 lines)
Files opened to verify a claim:
               `docs/comiccraft-design.md` §4.3, §5.1, §5.1.1, §5.2, §5.2.1, §5.3, §5.3.1, §6.1,
               §7.1–§7.5, §8.3, §12.1, §16; `docs/comiccraft-porting-audit.md` §4, §5, §13 U1/U3;
               `crates/doc/src/links.rs` (22–41, 76–79), `crates/doc/src/lib.rs` (339–349,
               536, 580–583, 628–631, 660–705, 736–746, 1028–1030), `crates/doc/src/node.rs`
               (14–21), `crates/engine/src/cmd/links.rs` (233–239, 627–645),
               `crates/format/src/lib.rs` (49–60, 159–168, 433–465), `crates/pdf/Cargo.toml`
               (line 34), `crates/text/src/lib.rs` (46–65, 155–183), `Cargo.toml`,
               `crates/comic-model/{Cargo.toml,src/lib.rs,tests/schema_contract.rs}`
Files written (test code only): `crates/doc/tests/schema_claims.rs`
Design refs:   design §4.3 (two structural rules), §5.1.1 + §6.1 (artboard owns page size, one
               owner per relationship), §5.2 (entity field lists, "final"), §5.2.1 (link
               ownership), §5.3/§5.3.1 (identity, artboard hazard), §7.1–§7.5 (bundle, save
               order, migrations, assets), §12.1 (deterministic checks), §13 (security),
               §16 ADR-011/012/013

## Round 1 — correctness

Verdict:       **changes-requested**

Unit tests:
- Added: `crates/doc/tests/schema_claims.rs` (7 tests, new) — **7 pass, 0 fail**.
  The change is a specification, so only its *existing-VectorCraft* claims are testable; each test
  pins one of those claims against the code the document cites. The tests are falsifiable by
  construction: `artboard_ids_are_reused_after_the_highest_artboard_is_deleted` asserts the literal
  `2` from `next_artboard_id()`, so moving artboard ids to a persisted counter (schema §9.1
  mitigation 4) breaks it; `the_link_hash_is_fnv1a_64_bit_...` asserts the literal
  `imgcbf29ce484222325`, so replacing the FNV link hash with SHA-256 breaks it;
  `the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle` asserts a
  `../…` value round-trips, so a "no `..` anywhere" confinement rule (finding 10) breaks it.
- Command (the workspace needs ≥1.85; the system `cargo` 1.75 cannot build it, so the rustup
  toolchain was used as in `M0-comic-crates.md`):
```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 7 tests
test artboard_id_is_u32_and_the_rect_is_document_points ... ok
test artboard_ids_are_reused_after_the_highest_artboard_is_deleted ... ok
test the_link_relative_path_is_optional_and_skipped_when_unset ... ok
test document_unknown_and_extra_are_present_and_empty_by_default ... ok
test node_ids_save_as_bare_numbers_and_allocate_upward ... ok
test the_link_hash_is_fnv1a_64_bit_not_a_sha256_integrity_hash ... ok
test the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
- Not tested by unit test, verified by reading instead (documentation-only): §4, §5.1, §5.4, §8.2,
  §11, §12, §13, §14 — the wire format of code that does not exist yet. Round 2 covers fixtures.

### What I checked and found *true* (so it is on the record)

Every code citation in the document resolves, and every claim about existing VectorCraft
behaviour that I could open is accurate except finding 1:

- `crates/doc/src/links.rs` exists; `hash_bytes` is FNV-1a 64 (`links.rs:76-79`) and `LinkInfo.hash`
  is `Option<String>` with the `img` tag — §7.2's "cheap change detector, not an integrity
  guarantee" is right, and audit §13 U3 says the same.
- `crates/pdf/Cargo.toml:34` is `sha2 = { version = "0.10", default-features = false }` — §7.2's
  "already a direct dependency of `crates/pdf` … a workspace dependency entry, not a new crate"
  is right (`sha2` is not yet in `[workspace.dependencies]`, so §7.2's forward statement is the
  correct shape).
- `format::load_file` (`crates/format/src/lib.rs:433-465`) rejects a foreign `format` and
  `version > VERSION` (434–442) and runs exactly `drop_edit_modes`, `migrate_overprint_black`,
  `migrate_color_profiles`, `fix_next_id`, `tidy_saved_selections` in that order (459–464) — §10
  step 3 and §11's migration list are exact. `VERSION: u32 = 4` and the `File { format, version,
  document, … }` struct exist (`lib.rs:49-50`, `159-168`) — §2 is right.
- `Document::unknown` (`crates/doc/src/lib.rs:583`) and `Document::extra` (`:631`) exist, and
  `:629` documents `extra` as separate from `unknown` "which holds foreign data by design" — §1
  and §12's ADR-013 framing is right.
- `Artboard { id: u32, name, rect }` with "Artboard rectangle in document points"
  (`lib.rs:339-349`), `next_artboard_id()` = `max(id) + 1` with `unwrap_or(0) + 1`
  (`lib.rs:1028-1030`), `fix_next_id()` repairs `next_id` upward (`lib.rs:737-746`), and
  `NodeId(pub u64)` at `node.rs:19` — §9, §9.1 and §5.3's identity table are right (the artboard
  hazard is real, not invented; my test demonstrates the reuse).
- `TextLayout { bounds, overflow, fit_scale }` and `LayoutOptions::inset` / `LayoutOptions::fit`
  (`crates/text/src/lib.rs:46-65`, `:159-183`) — §6.6's "the measured result is never persisted"
  is right.
- Audit §13 U1 (artboard-id counter) and §4/§5 (document, format) exist — §9.1 mitigation 4's
  citation is real.
- Internal consistency that *does* hold: §13 rules 3, 5, 6, 7, 10, 11, 12 name fields that exist;
  §2/§14 agree on refuse-greater / migrate-forward; §6.1/§6.2 carry no `page_size` and no
  `geometry_ref`; §5.3 and §6.5 mark `default_page_size` as a default only. No finding.

Findings:      13 (`contradiction` ×9: 1, 2, 4, 5, 6, 7, 9, 11, 12; `under-specified` ×3: 3, 8,
               10, plus the text-node residue recorded in the ownership note; `style` ×1: 13;
               `stale-citation` ×0). One per line below; each is `file:line` — claim — what is
               true — type.

1. `docs/comiccraft-project-schema.md:90-91` (and `:312-313`) — "**All paths are bundle-relative
   with `/` separators**, matching `LinkInfo.relative`" and `StorageRef::Linked.relative_path`
   "Matches `LinkInfo.relative` so the document's own link and this record agree" — **the code says
   the opposite base**: `LinkInfo.relative` is "The path relative to the document's folder"
   (`crates/doc/src/links.rs:25-28`), produced by `relative_path(target, base = the document's
   directory)`, which emits `..` components (`crates/engine/src/cmd/links.rs:627-645`), and
   resolved by joining it onto the document's folder (`crates/engine/src/cmd/links.rs:237-239`).
   With §3's own layout, the same file is `../assets/panels/p1.png` from
   `document/comic.vectorcraft` and `assets/panels/p1.png` from the bundle root, so the two
   records can never hold equal strings. **Which document is wrong:** the schema (§3, §7.1) — and
   the design, whose §7.4 cites `crates/doc/src/links.rs:22-41` while describing those same lines
   as "its bundle-relative path". The schema is the wire-format authority, so it must say the base
   (bundle root *or* document folder) and match `LinkInfo`'s computation. Evidence:
   `crates/doc/tests/schema_claims.rs::the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle`.
   — **contradiction**
2. `:477` §13 rule 4 — "Every `Page.artboard_id` exists in the document, is **not** in
   `retired_artboard_ids`, and **matches the page's file name**" — the page file is
   `page-<PageId>.json` and the name agrees with `Page.id` (§3:85-88; §6.1 `:210` "the file name must
   agree"), *not* with the `u32` artboard id; nothing in `Page` or `Artboard` relates the file
   name to `artboard_id`. A validator written from this rule compares a folio-irrelevant `u32`
   artboard id against a filename that encodes a `u64` comic id, and cannot pass or fail for the
   right reason. Design §12.1's corresponding check is "`Page.artboard_id` against the document and
   `retired_artboard_ids`" — no filename clause. **Wrong:** the schema. — **contradiction**
3. `:386` §9.1 mitigation 3 — validate "verifies every `Page.artboard_id` exists, is not retired,
   and that the artboard's `rect` matches **the project's expectation**" — no field holds an
   expectation: §6.1 gives `Page` no `page_size`, and §5.3 says `ProjectSettings.default_page_size`
   "is a *default for new pages*, never a second copy of a page's size". Design §5.3.1 (mitigation
   3) uses the same words, and §13's rules 1–12 contain **no** rect check at all, so §9.1 and §13
   disagree about what `comic.project.validate` does. Until an expectation exists (a print preset,
   or an explicit `Page.page_size` decision), the check is unimplementable; §13's rule list is the
   place it must land. **Wrong:** both documents, and the design's §5.3.1 sentence is the root.
   — **under-specified**
4. `:125-138` §5.1 `ComicProject` — the frozen field table has no `schema_version`, while design
   §5.2 lists `schema_version: u32` on `ComicProject`, and this document's own per-file headers
   carry one (§6 `:192` page files, §7 `:293` `assets/index.json`, §8 `:331`
   `provenance/generations.json`). §5.4's story files (`:171`) carry none either. Either every
   top-level file has a version (then §5.1 and §5.4 are incomplete) or the reader cannot know
   which files to version-check. **Wrong:** the schema. — **contradiction**
5. `:160` §5.4 `Chapter` row — fields are `id`, `title`, `scene_ids`, `order_index`, `unknown?` —
   design §5.2 `Chapter` is `id`, **`project_id`**, `title`, `scene_ids`, `order_index`. A field the
   design calls final is silently dropped from the frozen wire format, and the row says "all
   required unless marked", so it is not even recorded as dropped. **Wrong:** the schema (or the
   design, if `project_id` was deliberately retired — then §5.2 must say so). — **contradiction**
6. `:395-412` §10 — "Write `manifest.json` **last**" (step 4) and "Because the manifest is written
   last and is the only file declaring the bundle's contents, it can never reference partially
   written data" — step 5 ("Replace the changed files with atomic per-file renames") runs *after*
   the manifest. Between step 4 and the end of step 5 the manifest names new files with new
   sha256s that are still in `.save-tmp/`, so a crash or a concurrent reader in that window sees a
   manifest **newer** than its contents — exactly the state §14 step 1 calls "a visible error".
   The safe order is: publish the file set, then the manifest (which is what "written last" means);
   the design §7.2 says the same as the schema, so both need the correction. — **contradiction**
7. `:462` §12 — "An entity struct may omit `unknown` only if the design marks it closed; **all
   current entities carry it**" — this document's own tables define structs with no `unknown`:
   `PageSize` (`:151`), `ArtworkRef` (`:238`), `MediaMetadata` (`:302`), `ContentHash` (`:320`),
   and neither `VisualDocumentRef` (`:110`) nor `FileEntry` (`:111`) has one. The design marks
   none of them closed, so the sentence is false as written and does not say what a new struct
   must do. — **contradiction**
8. `:110`, `:112-113`, `:144`, `:303`, `:336-341`, `:349` — the document "fixes the **wire
   format** … field types, optionality" yet never shapes `VisualDocumentRef.format_version`
   (type unstated), `Timestamp` (string? epoch? unit?), `Provenance` (§7 gives it one row and
   defers to "§8", which defines *jobs*, not provenance), `JobOperation`, `ProviderId`,
   `Progress`, `JobError`, `Consent`, `ReviewSubject` — all bare names with no fields and no
   JSON shape. §11.1 nevertheless promises a frozen v1 fixture that must "load, validate and
   round-trip byte-identically" and contains "1 job"; that fixture cannot be authored from this
   document alone. — **under-specified**
9. `:482` §13 rule 1 — "Every `*_ids` entry resolves to an entity that exists" — the glob also
   matches `manifest.retired_artboard_ids` (§4 `:109`), which rule 4 and §9.1 mitigation 2 require
   to be **absent** from the document. Implemented literally, rule 1 reports every correctly
   retired artboard as a finding. Scope it to entity fields (or exclude the manifest's retired
   list by name). — **contradiction**
10. `:486-489` §13 rule 9 (with `:316`) — rule 9 confines `StorageRef::Linked`, but
    `StorageRef::Package { relative_path }` is the same kind of path inside an archive that is
    unpacked to disk, and is not covered; the rule is also stated as a textual "inside the bundle…
    `../../etc/passwd` must be rejected", which a **symlink** under `assets/` defeats unless the
    check resolves and canonicalises first and then compares against the canonical bundle root; and
    §14 step 1's "`files` matches the tree" does not say that manifest keys themselves must be
    confined (a manifest key of `../outside` would be opened to hash it), nor that the save
    staging directory (`.save-tmp/`, §10 step 1) is excluded from that tree comparison. Fix the
    rule to: resolve `relative_path` (bundle-relative for `Linked`, the unpack root for `Package`),
    canonicalise, require the canonical result to be a descendant of the canonical bundle root,
    and apply the same confinement to every key in `manifest.files`. — **under-specified**
11. `:235` vs `:339` — `Panel.generation_status` uses `complete`, `GenerationJob.status` uses
    `completed`, for the same state; the two enumerations cannot share a type and a filter written
    for one silently misses the other. Design §5.2 only names `JobStatus`, so the panel-side
    vocabulary is this document's invention. Pick one spelling (or state that they are deliberately
    distinct states and what each means). — **contradiction**
12. `:276`, `:341`, `:303` — the schema adds fields the design's entity lists do not have:
    `ContinuityConstraints.exceptions` (design §5.2 Panel: "characters present, wardrobe, time of
    day, callbacks"), `GenerationJob.consent` (design §5.2 has no such field, and `Consent` is
    never shaped — finding 8), and it makes `Asset.provenance` an `Option` where design §5.2 says
    `provenance: Provenance`. The design is the authority for meaning (§1), so either the design
    gains the fields or the schema drops them; leaving both silent is how a fork drifts.
    — **contradiction**
13. `:111` vs `:402` — the `files` entry is `{ sha256, bytes }` in §4 and "sha256 + size" in §10
    step 4; the field's name in the frozen format is unstated. — **style**

No **stale-citation** findings: every `crate`/file/line citation in the document still resolves
(`crates/doc/src/links.rs`, `crates/doc/src/node.rs`, `crates/doc`, `crates/format`,
`crates/pdf/Cargo.toml`, `crates/comic-model`, `xtask`-adjacent paths, `Document::unknown`,
`Document::extra`, `format::load_file`, `format::VERSION`). The one citation whose *sense* is
wrong is finding 1, and it is recorded as a contradiction because it also contradicts the code's
own doc comment.

Ownership check:  **clean on both structural rules.** Nothing in the schema puts comic data in the
visual document — §1 restates ADR-013, §3 keeps the document at `document/comic.vectorcraft` as a
plain `.vectorcraft` file, §10 step 2/3 saves and verifies it only through `crates/format`, and
§12 explicitly contrasts its own `unknown` map with `Document::unknown`. No duplicated authority:
§6.1 gives `Page` no `page_size` and no `visual_document_id`; §6.2 gives `Panel` no `geometry_ref`;
§5.3 marks `default_page_size` a default only; §6.4 puts dialogue text on `Dialogue` (with
`Lettering.text` only for `narration`/`sound_effect`) and typography on `Lettering`, and §13 rules
5, 6, 8 enforce each one-way link. Two residues, both raised above rather than as new findings:
finding 3 leaves the artboard-rect expectation undefined, and the *rendered* text node that
`Lettering.text_node_id` points at necessarily contains a copy of the text the comic record owns,
which the design §6.1 forbids from becoming "a second source of dialogue truth" — §13 has no rule
comparing that node's content to `Dialogue.text`/`Lettering.text`, so add one (under-specified,
low).

Security check:  the intent of §13 rule 9 is right and firmly worded ("an imported bundle is
untrusted input, and a `relative_path` of `../../etc/passwd` must be rejected, not resolved"), and
§14 steps 1–4 are fatal-on-failure per §2's "Never partially load" — good. It is **not yet firm
enough to implement safely** for four reasons, all in finding 10: confinement covers only
`StorageRef::Linked` and not `StorageRef::Package`; it does not require canonicalising after
resolution, so a symlink inside `assets/` escapes a textual `..` check; `manifest.files` keys are
never said to be confined even though §14 step 1 opens them to compare hashes; and §14's
"`files` matches the tree" does not exclude the save staging directory (`.save-tmp/`) that §10
step 1 creates, so a crash mid-save turns a legitimate bundle into a load error. The
`relative_path` in finding 1 sharpens the risk: because the document's own `LinkInfo.relative`
legitimately carries `..`, `Package`/`Linked` confinement must be *base-relative resolution plus
canonical-prefix*, not a `..` denylist, or the fix will reject valid VectorCraft links. Nothing
else in the document is a path, hash or untrusted-input decision — `content_hash` is SHA-256
lowercase hex (§7.2, correct), and §8.1's "credentials never appear here" is a positive control.

Unverified:
- The model I ran on: unverifiable from this environment (see *Model note*); only the harness
  record names `opencodego/deepseek-v4.1-flash`.
- The comic wire format itself: `crates/comic-model` is a skeleton (`SCHEMA_VERSION`, `ComicId`
  only, `crates/comic-model/src/lib.rs`), so §4–§8, §11.1, §12 and §13 describe types that do not
  exist yet — their fields are checked against the design, not against code. Fixtures are round 2's
  ground, as the parent scope says.
- Whether `comic.page.*` / `comic.project.validate` will actually exist (§9.1 mitigation 1, §13):
  no engine module cites them; they are M1 work, not a false claim.
- Rust-side equivalents of `Timestamp`, `Serde` shape of `manifest.files` and the `PageSize`
  points-vs-pixels open item S1: S1 is correctly recorded as open in §15, so it is not a finding,
  but S1 and finding 3 are the same unresolved question seen twice — S1's owner should close both.

Gate:          `cargo xtask ci` — not run. This round reviewed documentation only and touched no
               production code (the sole new file is `crates/doc/tests/schema_claims.rs`, run
               green by the command above); the parent's round-1 shape asks for the verification
               evidence, not the milestone gate, and round 2 owns the post-fix revision.
               Also clean on the new test target: `cargo fmt -p vectorcraft-doc -- --check` (no
               diff) and `cargo clippy -p vectorcraft-doc --test schema_claims` (0 diagnostics).

Round 2:        **not run — it is a separate, later call on the post-fix revision** (the parent
               scoped this call to round 1, and round 2 covers fixtures and migration rules).
               `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh "" M0` therefore
               reports this record as `GAP — missing Round 2 section`, which is expected while
               round 1 stands at *changes-requested*: the revision must move before round 2, and
               `M0-comic-crates.md` is already `OK`.

---

## Round 2 — rules, failure paths, edges

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. The environment states
               `COPILOT_MODEL=deepseek-v4.1-flash`, which matches the harness record
               `opencodego/deepseek-v4.1-flash`. Unlike round 1 this is an *in-environment* statement
               and not only a harness label; it is still not a cryptographic attestation of the
               serving backend, so the process claim rests on the env var plus the harness record.
               `~/.copilot/session-store.db :: assistant_usage_events` still has **0 rows** for this
               session (`da3af311-32d6-4017-9c8e-aabce5ac3264`), so I cannot corroborate it there.
Revision reviewed:
               **working tree after the round-1 fixes** (path bases, save order, 14 referential
               rules, §5.5/§5.6 added) — base `3da4080b`, `docs/comiccraft-project-schema.md`
               untracked, mtime `2026-10-10 21:57`, **570 lines**. Round 1 reviewed the pre-fix
               500-line draft, so the revision moved and this round is not a re-run of it.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 570 lines), `docs/comiccraft-design.md`
               §4.3, §5.1.1, §5.2, §5.2.1, §5.3, §5.3.1, §6.1, §7.1–§7.5, §12.1,
               `docs/reviews/M0-schema-document.md` (round 1), `Cargo.toml:48-49`,
               `crates/doc/src/links.rs:20-32`, `crates/comic-model/{Cargo.toml,src/lib.rs}`,
               `crates/comic-model/tests/schema_contract.rs`
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: 5 new tests; one round-1 comment
               updated to the post-fix citation; **no assertion of a round-1 test changed**)
Design refs:   §4.3 (two structural rules), §5.1.1/§5.3.1 (artboard ownership and hazard), §6.1
               (one owner per relationship), §7.1–§7.5 (bundle, save order, migrations, assets,
               export), §12.1 (deterministic checks); schema §3, §4, §5.1–§5.6, §6.1–§6.6, §7.1,
               §8.1, §9.1, §10, §11.1, §12, §13, §14, §15

Verdict:       **changes-requested**

One of the 13 round-1 findings is not fixed, six are only partially fixed, and six are verified
fixed; the fixes introduced five contradictions of their own (findings 2, 3, 4, 5 and 13) and
sharpened a sixth (finding 1: §7.1 now directly contradicts the corrected §3). The document is
**close** to implementable — §5.5 genuinely removed most of the guessing — but an agent that builds
`comic-model` from §13/§14 as written would implement a validator that skips two rules, never
enforces a dozen foreign keys, and rejects every bundle whose `exports/` was deleted.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims` — **9 passed; 3 failed** (the 3 failures are findings 1/3/4's reproductions and are red on purpose).

Extended: `crates/doc/tests/schema_claims.rs` — 7 round-1 tests kept unchanged, 5 added.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 12 tests
test artboard_id_is_u32_and_the_rect_is_document_points ... ok
test artboard_ids_are_reused_after_the_highest_artboard_is_deleted ... ok
test document_unknown_and_extra_are_present_and_empty_by_default ... ok
test every_referential_rule_is_numbered_once_and_contiguously ... ok
test node_ids_save_as_bare_numbers_and_allocate_upward ... ok
test the_link_hash_is_fnv1a_64_bit_not_a_sha256_integrity_hash ... ok
test the_link_relative_path_is_optional_and_skipped_when_unset ... ok
test the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle ... ok
test the_round_trip_the_fixture_promises_needs_a_canonical_json_form ... ok
test every_rule_reference_in_the_schema_names_a_rule_that_exists ... FAILED
test the_load_sequence_runs_every_referential_rule ... FAILED
test the_schema_does_not_assert_both_path_bases_at_once ... FAILED
test result: FAILED. 9 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```

**The three failures are the reproductions of findings 1, 3 and 4 below, and they are red on
purpose** — they are the minimal reproduction the rule asks for, and they pass the moment the
document is corrected. Their failure text:

```
every_rule_reference_in_the_schema_names_a_rule_that_exists:
  the schema cites rule 15, which §13 does not define (it defines [1,2,3,4,13,14,5,6,7,8,9,10,11,12])
the_load_sequence_runs_every_referential_rule:
  §14 step 5 runs rules up to 12 but §13 defines up to 14; rules 13..=14 never run on load
the_schema_does_not_assert_both_path_bases_at_once:
  §3 says the bundle-relative base is *not* `LinkInfo.relative`'s base, while §7.1 still says a
  `StorageRef` path matches it — one of the two is wrong
```

The two green additions are the round-2 claims that **do** hold or that pin a real hazard:
`every_referential_rule_is_numbered_once_and_contiguously` (the 14 rule numbers are 1..=14 with no
gap or duplicate, so the *set* is sound even though its order and its citations are not), and
`the_round_trip_the_fixture_promises_needs_a_canonical_json_form`, which demonstrates with the
workspace's own `serde_json` (Cargo.toml:49 — `float_roundtrip`, no `preserve_order`) that
`{"width":595,"height":842}` reloads and re-saves as `{"width":595.0,"height":842.0}` and that
`serde_json::Map` sorts keys, so §11.1's "round-trip byte-identically" is unachievable until the
document fixes a canonical JSON form.

Also clean on the new target: `cargo fmt -p vectorcraft-doc -- --check` (no diff) and
`cargo clippy -p vectorcraft-doc --test schema_claims` (0 diagnostics).

Findings:      19 — 9 `contradiction`, 9 `under-specified`, 1 `style`

Line numbers are the post-fix `docs/comiccraft-project-schema.md` (570 lines).

1. `:359` §7.1 `StorageRef::Linked` — "**The default.** Matches `LinkInfo.relative` so the
   document's own link and this record agree" — **false**. The post-fix `:90-96` says the opposite
   ("**This is not the convention `LinkInfo.relative` uses**"), and the code says the opposite:
   `LinkInfo.relative` is "The path relative to the document's folder" (`crates/doc/src/links.rs:24-28`).
   With §3's own layout the same file is `../assets/panels/p1.png` from `document/` and
   `assets/panels/p1.png` from the bundle root, so the two records can never hold equal strings.
   **contradiction** (finding 1's second half; pinned by a red test).
2. `:432-433` §9.1 mitigation 3 — "`comic.project.validate` verifies … that the artboard's `rect`
   matches **the project's expectation**. A mismatch is a **repairable inconsistency**, never
   auto-corrected" — contradicts `:532` rule 14 ("There is **no** per-page 'expected size' field to
   compare against"; a difference is "a **warning, not an error**") and `:567` S1, which declares
   the item "**Closed** by rule 14". One of the two must change; §9.1 was not touched by the fix.
   **contradiction** (finding 3, not fixed).
3. `:516` §12 — "This is the list **rule 15** checks." §13 defines 14 rules; there is no rule 15.
   A validator cannot implement a citation that does not resolve. **contradiction** (introduced by
   the §12 rewrite; pinned by a red test).
4. `:556` §14 step 5 — "Referential rules **1–12** run and their findings are returned to the
   caller." §13 now has 14 rules, so **rules 13 and 14 never run on load** — including rule 13's
   file-name check and rule 14's rect warning. **contradiction** (introduced by the fix; pinned by
   a red test).
5. `:447-459` §10 — after step 4 "Replace the content files" the **new** generation is on disk
   while the manifest still holds the **previous** `files` hashes; step 5 is the commit point. A
   crash *during* step 4's per-file renames (each rename is atomic, the set is not) or between 4
   and 5 leaves every replaced file with a sha256 that no longer matches `files`, which `:551` §14
   step 1 makes "a visible error" — i.e. a bundle that cannot be loaded. So "it is the only file
   whose truncation can make the bundle inconsistent" and "the previous consistent project intact"
   are false for that window, and "No reader can see the new generation yet" is false as stated
   (the content files *are* the new generation; only the manifest is old). The design still carries
   the **opposite** order (design §7.2: manifest step 4, replace step 5) and §1 makes the design
   authoritative, so the two documents now disagree. **contradiction** (finding 6's fix moved the
   window instead of closing it).
6. `:89` §3 "**`exports/` is never read as input.** It may be deleted at any time." vs `:116` §4
   `files` = "… for **every file** except the manifest itself" and `:551` §14 step 1 "`files`
   matches the tree — a mismatch is a visible error". Deleting `exports/` — explicitly allowed —
   makes every subsequent load fail. The same holds for a crashed save's `.save-tmp/` (`:443`).
   **contradiction**.
7. `:527` §13 rule 1 — "Every `*_ids` entry resolves" is a glob over **plural** lists only, so a
   dozen singular foreign keys are governed by nothing: `Chapter.project_id` (`:181`),
   `Scene.chapter_id` (`:182`), `Page.chapter_id` (`:257`), `Panel.page_id` (`:271`),
   `Panel.scene_id` (`:273`), `Scene.location_id` (`:182`), `Character.style_profile_id` (`:183`),
   `ArtworkRef.asset_id` (`:284`), `GenerationJob.target_panel_id` (`:382`),
   `AssetRegistryId` (`:141`). The entity notes say "must agree with the containing file" for
   `Panel.page_id`, but §13 is the normative list and has no such rule. **under-specified**
   (the glob carve-out fixed the named case; the singular case is unaddressed).
8. `:141` §5.1 `asset_registry_id: AssetRegistryId` is **required**, but `assets/index.json`
   (`:339`) is `{ "schema_version": 1, "assets": […], "unknown": {} }` — no `id` — and no
   `AssetRegistry` entity is defined anywhere. Nothing can resolve or validate the field, and the
   §11.1 fixture must invent a value no rule can check. **under-specified**.
9. `:537` §13 rule 9 — "every storage path — **every `StorageRef` variant** — is resolved against
   its **declared base** (`Linked`/`Package`: the bundle root …)". `Embedded { image_key }`
   (`:360`) is **not a path** and has no declared base, and `Package { relative_path }` (`:362`) is
   "a file inside a packaged archive" that design §7.5 unpacks to a directory, so its base is the
   archive/unpack root, not the bundle root — as does `:93-94` ("A `StorageRef` path resolves
   against the bundle root"). **contradiction**.
10. `:542-543` §13 — "an imported bundle is untrusted input, and a `relative_path` of
    `../../etc/passwd` **must be rejected**, not resolved" — but rule 9 is a §13 rule and `:558`
    §14 says "step 5 returns findings and **still yields a usable project**". So a bundle carrying
    `../../etc/passwd` loads, is handed to the caller, and the escape is only a report. Either
    confinement joins the fatal steps 1–4 or "rejected" must say "reported". **contradiction**
    (security).
11. `:537` §13 rule 9 — canonicalise-then-prefix-check defeats `..` and symlinks but **not a hard
    link**: `assets/x.png` hard-linked to `/etc/passwd` canonicalises to a path inside the bundle
    root, passes, and is then hashed and read. The rule needs an `nlink`/device check, or an
    explicit statement that hard links are out of scope; the same window also has a
    canonicalise-then-open TOCTOU race the rule does not mention. **under-specified** (security).
12. `:510-516` §12 — the closed-type list omits `VisualDocumentRef` (`:200`), `FileEntry` (`:116`),
    `Consent` (`:212`), `Progress` (`:214`), `JobError` (`:215`) and `manifest.app` (`:119`), none
    of which carry `unknown`; and "Every *entity* struct carries it" cannot be checked because the
    document never says which structs are entities. **under-specified** (finding 7's residue:
    `VisualDocumentRef` and `FileEntry` were the two round 1 named).
13. `:219-230` §5.6 — "**Every** such delta is listed here so nothing is an undocumented
    invention" is not true: `StorageRef::Package` is a **third** variant where design §5.2
    `Asset.storage_reference` describes two ("project-relative path, or an embedded key in the
    visual document"), and `MediaMetadata` splits the design's "pixel dimensions" into
    `width`/`height`. The `Asset.provenance` row also cites rule 10 for "**absent** for
    `source`/`reference`", which rule 10 (`:538`) does not say — it only requires presence for
    `generated`/`edited`. **contradiction**.
14. `:531` §13 rule 13 — "every file in `pages/` is reachable from some `Scene.page_ids`" **is**
    checkable (enumerate `pages/*.json`, collect every `Scene.page_ids`), but it makes a page that
    exists with `chapter_id` and an empty `scene_ids` — which §6.1 permits — an error, and the same
    rule requires the file name to parse as an integer while `:87-88` says "the loader must not
    depend on" the file name's form. Say whether reachability is an error or a warning, and what a
    non-numeric file name does. **under-specified**.
15. `:531-532` §13 — the table is not in numeric order (1, 2, 3, 4, **13, 14**, 5 … 12), so rule 4's
    neighbour is rule 13 and `:541`'s "rule 4" back-reference sits above rules 5–12. **style**.
16. `:396` §8.2 — `ReviewDecision` is "… `actor: String`, `timestamp`, `note: Option<String>` …"
    with **no type** for `timestamp`, while `:199` defines `Timestamp`. The fixture must guess
    whether it is `Timestamp`, a string or an integer. **under-specified**.
17. `:199` §5.5 `Timestamp` — "RFC 3339 in UTC, e.g. `2026-10-10T21:52:00Z`" fixes neither
    fractional-second precision nor whether an offset like `+07:00` is accepted, so §11.1's
    (`:494`) byte-identical round trip is unachievable for a timestamp the document does not
    canonicalise (see the green `the_round_trip_…` test, which shows the same problem for numbers
    and object keys). **under-specified**.
18. `:134` §5.1 `schema_version` — "must equal `manifest.schema_version`" is enforced by **no
    rule** (rule 3 `:529` checks only `comic_id` and `document`), and §5.4 (`:181-185`) gives the
    five story files no `schema_version` at all while §6 (`:238`), §7 (`:339`) and §8 (`:377`) all
    carry one. Which files are version-checked is still unstated. **under-specified** (finding 4's
    residue).
19. `:528` §13 rule 2 — "No duplicate id within an entity kind" does not say whether
    `retired_artboard_ids` (`:114`, "Never pruned") may repeat. §9.1's reuse means it **must** be
    able to: delete artboard 2, create a page that reuses 2, delete again, and `2` appears twice.
    A validator implementing rule 2 literally would reject a correctly retired bundle.
    **under-specified**.

Fix verification: the 13 round-1 findings

| # | Round-1 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | `LinkInfo.relative` base claim | **partially** | §3 `:90-96` now states the two bases differ and cites the code — fixed. §7.1 `:359` still says a `StorageRef::Linked` path "Matches `LinkInfo.relative`" — not fixed, and now contradicts §3 (finding 1; red test). |
| 2 | rule 4 conflated `artboard_id`, file name, rect | **verified fixed** | rule 4 `:530` is now exactly design §12.1's check (exists + not retired); the file-name clause moved to rule 13 `:531` and now correctly says "the file name encodes the *page* id, never the artboard id". |
| 3 | rect "matches the project's expectation" | **not fixed** | §9.1 `:432-433` is byte-identical to the pre-fix text, and now contradicts rule 14 `:532` and S1 `:567` (finding 2). Design §5.3.1 mitigation 3 and §12.1 ("the project's print presets") were not corrected either, and §1 makes the design authoritative. |
| 4 | `ComicProject.schema_version` absent | **partially** | added at §5.1 `:134` with the right justification — fixed for `ComicProject`. §5.4 `:181-185` still gives the five story files no version, and no rule enforces the "must equal" clause (finding 18). |
| 5 | `Chapter.project_id` absent | **verified fixed** | present at `:181` as `project_id: ComicId` (design §5.2). Ungoverned by any rule (finding 7), but the field is back. |
| 6 | save-order contradiction | **partially** | §10 `:443-453` now publishes the file set first and the manifest last, so a manifest can no longer describe files that are still staged — the round-1 contradiction is gone. But the new window (crash during/after step 4, before step 5) leaves files newer than the manifest, which §14 `:551` makes fatal, and design §7.2 still has the old order (finding 5). |
| 7 | "all current entities carry `unknown`" | **partially** | the false universal sentence is replaced by an explicit closed-type list `:510-513` — fixed for the seven types it names. `VisualDocumentRef` and `FileEntry` (the two round 1 named) are still absent from the list, and "rule 15" does not exist (findings 3, 12). |
| 8 | nine types left unshaped | **verified fixed** for the named nine | §5.5 `:199-217` shapes `Timestamp`, `VisualDocumentRef.format_version`, `Provenance`, `JobOperation`, `ProviderId`, `Progress`, `JobError`, `Consent`, `ReviewSubject` (plus five more). Residue: `AssetRegistryId`, `manifest.app.schema`, `ReviewDecision.timestamp`, `JobStatus`'s placement (findings 8, 16; §5.5's intro also says "the tables **above**" while its table is below the intro and §8's types come from prose). |
| 9 | rule 1's glob swallowing `retired_artboard_ids` | **verified fixed** | rule 1 `:527` now carries the carve-out: "**`retired_artboard_ids` is not a reference list**". Residue: the singular `_id` keys (finding 7). |
| 10 | rule 9 confined only `Linked` | **partially** | rule 9 `:537` now covers every `StorageRef` variant **and** every `manifest.files` key, requires canonicalisation *before* the check, names symlinks, and says `.save-tmp/` is never a source — three of round 1's four sub-issues fixed. Still missing: §14 `:551`'s "`files` matches the tree" does not exclude `.save-tmp/` (finding 6), `Embedded` is not a path and `Package`'s base is wrong (finding 9), and the rejection is non-fatal (finding 10). |
| 11 | `complete` vs `completed` | **verified fixed** | one spelling everywhere: §5.5 `:204`, §6.2 `:281` and §8.1 `:385` all use `completed`. Residue: two enums for one state (`GenerationStatus` vs `JobStatus`) and no rule ties `Panel.generation_status` to the job that drives it — folded into finding 7's singular-key gap. |
| 12 | undocumented deltas | **partially** | §5.6 `:219-230` documents the three round-1 deltas and the `ContinuityConstraints` shape — the silent-fork risk is gone. But the list is not complete (`Package`, `MediaMetadata` split) and one row mis-cites rule 10 (finding 13). |
| 13 | `files` entry field name unstated | **verified fixed** | `{ sha256, bytes }` at §4 `:116` and again at §10 `:451`. |

New-contradiction check: §5.5/§5.6 walked row by row, then the rules as a set.

**§5.5 walked row by row.** All 19 rows have a shape; none is a bare name. `Timestamp`,
`VisualDocumentRef`, `ReadingDirection`, `ApprovalStatus`, `PageStatus`, `GenerationStatus`,
`LetteringKind`, `OverflowPolicy`, `TextAlign`, `AssetType`, `ArtworkRole`, `Provenance`,
`ProviderId`, `Consent`, `JobOperation`, `Progress`, `JobError`, `ReviewSubject`, `Decision` are all
typed, and the value sets agree with the tables that use them (§6.1 `PageStatus`, §6.2
`GenerationStatus`, §6.4 `LetteringKind`/`OverflowPolicy`, §6.6 `TextAlign`, §7 `AssetType`,
§6.2 `ArtworkRole`, §6.4 `dialogue_id`/`text` vs rule 8). §5.5 does **not** contain `JobStatus`,
which §8.1 `:385` defines inline; §5.5's intro ("the value types the tables above reference") is
therefore both incomplete and mis-scoped, since its own table is below the sentence.

**§5.6 walked row by row.** All four rows match the tables: `ContinuityConstraints.exceptions`
= §6.5 `:320`; `GenerationJob.consent` = §8.1 `:387` and `Provenance.consent` = §5.5 `:210`;
`Asset.provenance: Option<Provenance>` = §7 `:349`; `ContinuityConstraints` shape = §6.5. The
defect is the *completeness claim* and the rule-10 citation (finding 13), not the rows.

**Is anything else still unshaped, so §11.1 cannot be authored?** Yes, four things: the
`AssetRegistryId` value (§5.1 `:141`, finding 8), `manifest.app.schema` (§4 `:119`, no type),
`ReviewDecision.timestamp` (§8.2 `:396`, no type), and the canonical JSON form §11.1's byte-identity
requires (finding 17). Everything else the fixture needs — 1 chapter, 1 scene, 1 page, 3 panels,
2 speech balloons, 1 caption, 1 linked asset, 1 job — can be authored from the document as it now
stands.

**The 14 rules as a set.** The numbers are 1..=14 with no gap or duplicate (green test). Overlaps
are benign and deliberate (rule 1 resolves `Page.panel_ids`, rule 11 checks the same field against
the page file; rule 4 and rule 5 both concern `artboard_id` from different sides). Three defects:
the table is out of numeric order (finding 15); rule 1's plural-only glob leaves the singular
foreign keys ungoverned (finding 7); and §14 runs only 1–12 (finding 4). **Rule 13's
`Scene.page_ids` reachability *is* checkable** — `pages/` is enumerable and `Scene.page_ids` is a
finite list — but as written it is an error for a page that legitimately has no scene yet, and it
shares a rule number with an unrelated file-name check (finding 14).

**The two structural rules.** Still **clean**. Rule 1 (no comic data in the visual document): §1
`:25-27` restates ADR-013; §3 `:70` keeps the document a plain `.vectorcraft` file; §10 steps 2–3
touch it only through `crates/format`; §12 `:509` explicitly contrasts its `unknown` with
`Document::unknown`; §6.1/§6.2/§6.4 carry no `page_size`, no `geometry_ref`, no `balloon_id`. Rule 2
(no duplicated authority): §6.1 `:259` marks `artboard_id` the page's visual home with no
`page_size`; §6.2 `:274` has no `geometry_ref`; §5.3 `:158-159` marks `default_page_size` a default
only, and §13 rules 5, 6, 8 enforce the one-way links. Two residues, neither new: the *rendered*
text node behind `Lettering.text_node_id` necessarily holds a copy of the dialogue text and §13
still has no rule comparing that node's content to `Dialogue.text`/`Lettering.text` (round 1 raised
it; still absent), and rule 14 now uses `default_page_size` as a comparison baseline, which is the
closest the document comes to the "second copy of a page's size" §5.3 forbids — it is a warning, so
it is a smell rather than a breach.

**The security rule.** §13 rule 9 is a real improvement and its central mechanism is right:
resolve against a **declared** base, canonicalise, then compare against the canonical bundle root —
which is exactly what round 1 asked for and what defeats both `..` and symlinks. It is still not
implementable safely as written, for the three reasons in findings 9, 10 and 11 (the `Embedded`
variant is not a path; `Package`'s base is stated three times as the bundle root while §7.1/design
§7.5 place it inside an archive; and confinement failure is non-fatal, so an untrusted bundle with
a traversal path still opens). The hard-link and TOCTOU holes are the residue of a textual check
that has not been tested against a real filesystem yet.

Implementability: see the nine guesses below.

`comic-model` could **not** be built from this document without guessing, but it is close. What is
still a guess:

1. **`AssetRegistryId`** — required, and nothing in the bundle can supply or validate a value
   (finding 8).
2. **Which files carry `schema_version`** — §5.1/§6/§7/§8 say yes, §5.4's five story files say
   nothing, and no rule relates `project.schema_version` to `manifest.schema_version` (finding 18).
3. **The canonical JSON form** §11.1's "round-trip byte-identically" requires — key order, number
   form and `Timestamp` precision are all unstated (finding 17).
4. **`manifest.files` membership** — "every file except the manifest itself" vs `exports/` being
   deletable at any time vs a crashed save's `.save-tmp/` (finding 6).
5. **Which of §13's rules a load runs** — §14 says 1–12, §12 cites 15, the table has 14 (findings
   3, 4).
6. **The rect comparison's baseline and severity** — §9.1 says the project's expectation and a
   repairable inconsistency, rule 14 says `default_page_size` and a warning (finding 2).
7. **Every singular foreign key** — a validator built from §13 alone would check plural `*_ids`
   and let `Chapter.project_id`, `Scene.chapter_id`, `Page.chapter_id`, `Panel.page_id`,
   `Panel.scene_id`, `ArtworkRef.asset_id`, `GenerationJob.target_panel_id` and
   `Character.style_profile_id` point anywhere (finding 7).
8. **`StorageRef::Package`'s base and when `Package` appears in a directory bundle** — stated as
   the bundle root, described as inside an archive (finding 9).
9. **`manifest.app.schema` and `ReviewDecision.timestamp` types** (findings 16 and the §5.5 gap).

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and the review record; it touched
no production code and no crate manifest, and the workspace gate's own doc-state is recorded in
`docs/ci-3da4080b*.md`. Run on the changed target instead:

```
$ cargo fmt -p vectorcraft-doc -- --check        # no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims   # 0 diagnostics
$ cargo test  -p vectorcraft-doc --test schema_claims    # 9 passed; 3 failed (the reproductions)
```

A full `cargo xtask ci` would now **fail at its test step** on the three red tests, which is the
intended evidence, not an accident: they are the reproductions of findings 1, 3 and 4 and they go
green when the document is corrected.

Unverified:

- The serving backend: `COPILOT_MODEL=deepseek-v4.1-flash` is present in the environment and agrees
  with the harness record, but nothing in the environment attests the actual backend, and
  `assistant_usage_events` has 0 rows for this session.
- The comic wire format itself: `crates/comic-model` is still a skeleton (`SCHEMA_VERSION`,
  `ComicId` only), so §4–§8, §11.1, §12, §13 describe types that do not exist; their fields are
  checked against the design, not against code. The v1 fixture does not exist, so its authorability
  is argued from §5.5, not demonstrated.
- Whether the round-1 fixes landed in the **design** as well as the schema: I re-read design §4.3,
  §5.1.1, §5.2, §5.2.1, §5.3, §5.3.1, §6.1, §7.1–§7.5 and §12.1 (where §7.2's save order and
  §5.3.1's rect expectation are both still pre-fix), but I did not re-review the whole design.
- The design's §12.1 "Unsupported page dimensions … against the project's print presets": no
  print-preset field exists in the schema, and §15 S1 declares the item closed by rule 14; whether
  the design's own mechanism is still owed was not settled, only recorded.
- Whether `comic.page.create` / `.delete` / `.reorder` and `comic.project.validate` will exist:
  M1 work, not a false claim.

---

## Round 3 — verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set in
               the environment, which is the same in-environment statement round 2 relied on. New
               corroboration this round: `~/.copilot/session-store.db :: assistant_usage_events` now
               holds 632 rows and records `model = 'deepseek-v4.1-flash'` for the sessions that ran
               concurrently in this window (e.g. `ab0ef86f-9e4c-4174-afc5-b287e064ea4b`, 87 rows).
               My own session (`b1656afb-a5ac-4ac9-9253-7823e1002e89`) still has **0** rows, so the
               attestation is still not a cryptographic one for *this* session — the environment and
               the neighbouring records agree, and I state the gap rather than claim proof.
Revision reviewed:
               **working tree after the round-2 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` untracked, mtime `2026-10-10 22:15`, **604 lines**
               (round 2 reviewed the 570-line post-round-1 text at mtime 21:57). Revision moved; §3
               `jobs/` note, §4's `files` exclusion, §5.1/§5.4 versions, §5.5 `AssetRegistryId` and
               the `Timestamp` canonical form, §5.6's two new rows, §5.7, §9.1 mitigation 3, §13
               rules 1/3/9/13–17, §14 steps 1 and 5 and §15 S1 all changed after round 2.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 604 lines), `docs/comiccraft-design.md`
               §5.2, §5.2.1, §5.3.1, §6.1, §7.1, §7.2, §7.5, §12.1 (the sections the schema cites,
               because §1 makes the design authoritative), `docs/reviews/M0-schema-document.md`
               (rounds 1–2), `crates/doc/src/links.rs:20-32`, `crates/doc/src/lib.rs:1028-1030`,
               `crates/engine/src/cmd/links.rs:624-645`, `crates/format/src/lib.rs:428-470`,
               `crates/pdf/Cargo.toml:34`, `Cargo.toml:49`, `crates/comic-model/src/lib.rs`,
               `crates/comic-model/tests/schema_contract.rs`
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **10 new tests**, one helper each for
               §13's rule table, whitespace-flattened text and §3's store list; **no existing
               assertion changed, none deleted, none weakened**)
Design refs:   §1 of the schema (the design decides the meaning), §4.3, §5.1.1, §5.2, §5.2.1,
               §5.3.1, §6.1, §7.1–§7.5, §8.3, §12.1; schema §3, §4, §5.1–§5.7, §6.1–§6.6, §7.1,
               §8.1, §8.2, §9, §9.1, §10, §11.1, §12, §13, §14, §15

Verdict:       **changes-requested**

The document is close, and closer than round 2: **11 of the 19 round-2 findings are verified fixed**,
**7 are partially fixed** and **one (the rule-9 hard link / TOCTOU residue) is not fixed**. Of the
five contradictions round 2 said the fixes had introduced, **four are resolved** and the fifth
(the §10 save-order window) is resolved in its fatal half and stale in its wording. But the fixes
moved the seam again: the renumbering left §15 S1 citing a rule that no longer says what S1 closes
with, rule 3's rewritten enumeration now omits the five files §5.4 was changed to declare versioned,
and the new rule 17 turns the reuse §9 still permits into a permanent false finding. Two of the
round-2 nine guesses remain (which files rule 3 version-checks; `StorageRef::Package`'s base in §3),
and one blocking gap is new: **§3 declares `jobs/`, and no section of either document says what a
file in it is called, what shape it has, or whether the v1 fixture's "1 job" lives there or in
`provenance/generations.json`** — `manifest.files` must hash every file in it and §14 parses it.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **12 passed; 10 failed**. The 12 are round 1/2's tests, all green now (round 2's
               three red ones — findings 1, 3 and 4 — went green with the fixes, which is the
               evidence that those three are fixed). The 10 failures are this round's reproductions
               of 10 of the 12 findings below — findings 9 and 10 are reading-only, with no
               assertion that could pin them — and each goes green the moment the document is
               corrected.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 22 tests
test artboard_id_is_u32_and_the_rect_is_document_points ... ok
test artboard_ids_are_reused_after_the_highest_artboard_is_deleted ... ok
test document_unknown_and_extra_are_present_and_empty_by_default ... ok
test every_referential_rule_is_numbered_once_and_contiguously ... ok
test every_rule_reference_in_the_schema_names_a_rule_that_exists ... ok
test node_ids_save_as_bare_numbers_and_allocate_upward ... ok
test the_link_hash_is_fnv1a_64_bit_not_a_sha256_integrity_hash ... ok
test the_link_relative_path_is_optional_and_skipped_when_unset ... ok
test the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle ... ok
test the_load_sequence_runs_every_referential_rule ... ok
test the_round_trip_the_fixture_promises_needs_a_canonical_json_form ... ok
test the_schema_does_not_assert_both_path_bases_at_once ... ok
test a_reused_artboard_id_does_not_end_up_permanently_broken ... FAILED
test every_store_the_bundle_layout_declares_has_a_file_the_schema_names ... FAILED
test rule_12_says_what_a_null_speaker_means ... FAILED
test rule_3_names_every_file_that_declares_a_schema_version ... FAILED
test s1_closes_the_page_size_item_with_the_rule_that_states_it ... FAILED
test the_files_tree_comparison_excludes_what_files_excludes ... FAILED
test the_missing_font_claim_cites_a_section_that_contains_the_check ... FAILED
test the_save_order_does_not_claim_content_no_reader_can_see ... FAILED
test the_schema_and_the_design_it_defers_to_do_not_contradict_each_other ... FAILED
test the_schema_does_not_assert_one_storage_base_where_rule_9_splits_them ... FAILED
test result: FAILED. 12 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure text, verbatim (each names the sentence that has to change):

```
every_store_the_bundle_layout_declares_has_a_file_the_schema_names:
  §3 declares ["jobs/"] in the bundle layout, but no file inside them is named anywhere in the
  schema, so `manifest.files`, the v1 fixture and the loader have nothing to work with
s1_closes_the_page_size_item_with_the_rule_that_states_it:
  left: 14, right: 15 — S1 closes the page-size item with rule 14, but the rule that says a
  differing rect is a warning is rule 15
a_reused_artboard_id_does_not_end_up_permanently_broken:
  §9 permits a reused artboard id, rule 4 rejects any page whose artboard id is retired, and
  rule 17 forbids removing it from the list: a reused id is a permanent false finding
rule_3_names_every_file_that_declares_a_schema_version:
  §5.4 declares `story/chapters.json` versioned so rule 3 can check it, but rule 3 does not name
  it: a validator built from the rule's list skips all five story files
the_schema_does_not_assert_one_storage_base_where_rule_9_splits_them:
  rule 9 splits the `StorageRef` bases but §3 still resolves every `StorageRef` path against the
  bundle root
the_files_tree_comparison_excludes_what_files_excludes:
  §4 excludes `exports/` from `files`, but §14 step 1 compares `files` with the whole tree, so
  every bundle containing `exports/` reports an inconsistency that is not one
the_missing_font_claim_cites_a_section_that_contains_the_check:
  §5.3 cites §13 for the missing-font report, but §13's rules mention no font; the check is
  design §12.1, not a referential rule
rule_12_says_what_a_null_speaker_means:
  rule 12 requires every `Dialogue.speaker_character_id` to resolve, but §6.3 makes it
  `Option<CharacterId>` (`null` for narration)
the_save_order_does_not_claim_content_no_reader_can_see:
  §10 publishes the content files in step 4 and the manifest in step 5, yet still claims the new
  generation is not visible until the last step
the_schema_and_the_design_it_defers_to_do_not_contradict_each_other:
  disagree on 2 seam(s): save order (schema §10 vs design §7.2) | rect expectation
  (schema §9.1 vs design §5.3.1, which §9.1 cites as its source)
```

Also clean on the extended target: `cargo fmt -p vectorcraft-doc -- --check` (no diff after
`cargo fmt -p vectorcraft-doc`) and `cargo clippy -p vectorcraft-doc --test schema_claims`
(0 diagnostics).

Findings:      12 — 5 `contradiction`, 5 `under-specified`, 2 `style`

Line numbers are the post-round-2 `docs/comiccraft-project-schema.md` (604 lines). Each is
`file:line` — claim — what is true — type.

1. `docs/comiccraft-project-schema.md:79` (with `:116`, `:581`, `:516-517`) — §3 declares
   `jobs/  # durable in-flight job metadata` in the layout, but **no section of this document or
   the design gives that directory a file name, a shape or an owner**: `manifest.files` must cover
   "every file except the manifest **and except `exports/` and `.save-tmp/`**" (`:116`), §14 step 1
   compares that set with the tree (`:581`), §14 step 3 parses "the comic files", and §11.1's
   fixture has "1 job" (`:516-517`) without saying whether it lives in `jobs/` or in
   `provenance/generations.json` (which already carries `jobs: [ … ]`, `:403`). An implementer of
   `comic-model` cannot write, hash or parse that directory, and cannot author the fixture's job.
   **under-specified** (implementability; pinned by a red test).
2. `:601` §15 S1 — "**Closed** by rule 14: … an artboard rect that differs is a warning" — the rect
   warning is rule **15** (`:568`); rule 14 (`:567`) is the orphan-page warning. The renumbering
   that fixed round 2's finding 15 left the closure pointing at an unrelated rule, so the one piece
   of evidence S1 offers for closing the item is uncheckable. **style** (stale citation; pinned by a
   red test).
3. `:557` rule 4 + `:570` rule 17 vs `:433` §9 and `:445` §9.1 — rule 4 requires every
   `Page.artboard_id` to be **absent** from `retired_artboard_ids`; rule 17 requires that list to be
   **unique and append-only**; §9's identity table says the artboard id "**can be reused — see
   §9.1**" and §9.1 opens "there is no never-reuse guarantee", with mitigation 4 (a persisted
   counter) only *proposed*. Delete the highest page, create another: `max(id) + 1` reuses the id,
   the new page's own `artboard_id` is retired, rule 4 reports it broken, and rule 17 forbids the
   only repair (pruning the id). A legitimate `comic.page.create` therefore manufactures a permanent
   false finding the validator must report. **contradiction** (pinned by a red test).
4. `:556` rule 3 vs `:175-178` §5.4 — rule 3's enumeration of the files whose `schema_version` is
   checked names `project.json`, `pages/page-<id>.json`, `assets/index.json` and
   `provenance/generations.json`; §5.4 was added/changed in the same pass to say each of the **five**
   `story/*.json` files "is `{ "schema_version": 1, … }` **so … rule 3 can check the version against
   the manifest**". The rule's list and §5.4's reason for the field disagree: a validator built from
   the list skips all five. **contradiction** (round-2 finding 18's residue; pinned by a red test).
5. `:93-94` §3 vs `:562` rule 9 — rule 9 splits the bases ("The bases are not one base: `Linked`
   resolves against the **bundle root**, `Package` against the **archive root**") while §3 still says
   flatly "A `StorageRef` path resolves against the bundle root", and §7.1 says `Package` is "a file
   inside a packaged archive". §3 is the paragraph that warns the reader that confusing bases is a
   path-traversal bug, so a reader who follows it resolves archive paths against the wrong root.
   **contradiction** (round-2 finding 9's residue; pinned by a red test).
6. `:581-584` §14 step 1 vs `:116` §4 — §4 excludes `exports/` and `.save-tmp/` from `files`; §14
   step 1 "compares `files` with the tree: a mismatch … is **reported as an inconsistency**". The two
   exclusions are never restated where the comparison is defined, so a bundle that has just been
   exported (or crashed mid-save) reports a spurious inconsistency on every load, forever. Non-fatal
   now (`:590-593`), which is what closed round 2's finding 6, but the noise remains and a future
   reader cannot tell a real mismatch from the designed one. **under-specified** (round-2 finding 6's
   residue; pinned by a red test).
7. `:172` §5.3 — "the lettering then reports a missing font (**§13**)" — §13 is the referential
   rules 1–17 (`:547-570`) and says nothing about fonts; the missing-font check is design §12.1
   ("Missing lettering font | `text` | the missing-font report the exporter already produces").
   Design §13 is "Security and trust boundaries", so the number is wrong whichever document is meant.
   **style** (dangling citation; pinned by a red test).
8. `:565` rule 12 vs `:317` §6.3 — "Every `Dialogue.speaker_character_id` resolves to a
   `Character`", while the field is `Option<CharacterId>` with "`null` for narration". Read literally
   the rule fails every narration line in the book; the same class of optional key is listed in
   rule 1 (`ReviewDecision.superseded_asset_id`), so the omission is not confined to rule 12.
   **under-specified** (pinned by a red test).
9. `:196-197` §5.5 — "Every type a table names appears here; if one is missing, that is a defect in
   this document rather than in the reader's understanding." It is still not true: `ComicId` is named
   in §4's and §5.1's tables and shaped in §9 (`:431`), and `JobStatus` is used by §8.1 (`:410`) and
   shaped inline there; `ProjectMetadata`/`ProjectSettings` come from §5.1's table and are shaped in
   §5.2/§5.3. Nothing is unshaped, so this is a claim-quality defect rather than a gap — but the
   document's own criterion convicts it. **under-specified** (low; not separately tested, it is
   covered by the §5.5 read).
10. `:562` §13 rule 9 — the rule is now resolve-then-canonicalise-then-confine, which defeats `..`
    and symlinks, but it still says nothing about a **hard link** (`assets/x.png` hard-linked to
    `/etc/passwd` canonicalises inside the root, passes, and is then hashed and read) and nothing
    about the **canonicalise-then-open race** a real filesystem introduces. Round 2's finding 11 was
    not addressed; the answer may be "hard links are out of scope", but the document has to say so.
    **under-specified** (security; carried, not re-tested).
11. `:449`, `:457-460` §9.1 and `:474-477` §10 vs design §7.2/§5.3.1/§12.1 — §1 makes the design
    authoritative ("where the two could be read differently, `comiccraft-design.md` decides the
    meaning and this document is corrected"), and the two documents now disagree in three places:
    (a) §10 replaces the content files **before** writing the manifest while design §7.2 still writes
    `manifest.json` as step 4 and replaces the files as step 5 — the order round 1's finding 6 proved
    unsafe; (b) §9.1 mitigation 3 denies any size expectation while design §5.3.1 mitigation 3
    requires "the artboard's `rect` matches **the page size the project expects**" — and §9.1 cites
    "(design §5.3.1)" as the source of the mitigation it contradicts; (c) §15 S1 declares the
    page-dimension item closed while design §12.1 still assigns it ("the artboard `rect` against the
    project's print presets"). The schema is the corrected side of (a) and (b); the design was never
    edited, so a builder reading the pair cannot tell which text governs. **contradiction** (pinned
    by a red test that reads both documents).
12. `:468`, `:475` §10 — "only the last step is visible to a reader" and "No reader can see the new
    generation yet, because the manifest still describes the previous one". Step 4 publishes every
    content file at its final path by atomic rename; the files *are* the new generation from that
    moment, and a reader between step 4 and step 5 sees new content under old hashes — the exact
    window §14 step 1 was rewritten to tolerate. The ordering is safe; the sentence describing it is
    false. **contradiction** (low; pinned by a red test).

Round-2 verification: the 19 findings

| # | Round-2 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | §7.1 "Matches `LinkInfo.relative`" | **verified fixed** | §7.1 `:382-385` now says "the base is **not** `LinkInfo.relative`'s: that is document-folder-relative and may contain `..`". No other place asserts the two agree (grepped `LinkInfo`): §3 `:91-95` and §13 rule 9 `:562` both split them. The round-2 red test `the_schema_does_not_assert_both_path_bases_at_once` is green. |
| 2 | §9.1 rect "matches the project's expectation" | **partially** | §9.1 `:457-460` is rewritten — "**reports** the artboard's `rect` as information. It checks no size expectation, because there is none to hold one (rule 15)" — so the contradiction with rule 15 and S1 is gone. What remains is the other half: §9.1 still cites "(design §5.3.1)" as its source, and design §5.3.1 mitigation 3 still demands the expectation (finding 11b). §1 makes the design authoritative, so the disagreement is live. |
| 3 | §12 "rule 15 checks" | **verified fixed** | §12 `:543` now ends "This is the list **rule 1's** reference walk skips, deliberately." Rule 1 exists and is the walk in question; the round-2 red test is green. |
| 4 | §14 step 5 runs only 1–12 | **verified fixed** | §14 step 5 `:588` is now "Referential rules **1–17** run"; the table has 17 rules. Both round-2 red tests (`every_rule_reference_…`, `the_load_sequence_runs_…`) are green. Note the *set* is sound but one citation into it is still wrong — finding 2 above. |
| 5 | §10's save-order window | **partially** | The order was already published-files-then-manifest, and §14 step 1 `:581-584` now makes the mismatch **non-fatal** ("reported as an inconsistency and the load continues — it must not be fatal, or a crash in the window between step 4 and step 5 of a save would leave a project nobody could open"), which closes the fatal half round 2 named. Residues: §10 `:468`/`:475` still claim the new generation is invisible until the manifest (finding 12), and design §7.2 still carries the opposite order (finding 11a). |
| 6 | §3 `exports/` vs §4 `files` vs §14 step 1 | **partially** | §4 `:116` now excludes both `exports/` and `.save-tmp/`: "for every file except the manifest **and except `exports/` and `.save-tmp/`** — those are never read as input (§3), so listing them would make deleting `exports/` break every load". The fatal consequence is also gone (§14 step 1 is non-fatal). Residue: §14 step 1 does not restate the exclusions (finding 6). |
| 7 | rule 1's plural-only glob | **verified fixed** | Rule 1 `:554` now reads "Every reference resolves: every `*_ids` entry **and** every singular foreign key (`Chapter.project_id`, `Scene.chapter_id`, `Page.chapter_id`, `Panel.page_id`, `Panel.scene_id`, `ArtworkRef.asset_id`, `Lettering.dialogue_id`, `GenerationJob.target_panel_id`, `ReviewDecision.superseded_asset_id`, …)". Ten of round 2's keys are named and the governing phrase plus the terminal "…" covers `Scene.location_id`, `Character.style_profile_id`, `Lettering.panel_id` and the rest. |
| 8 | `AssetRegistryId` unshapeable | **verified fixed** | §5.5 `:203` shapes it: "`ComicId`, allocated once when the project is created and written to `ComicProject.asset_registry_id`; the registry file it names is `assets/index.json`". The fixture can author a value and no rule needs to resolve it. |
| 9 | rule 9 confined `Linked` only / `Package`'s base / `Embedded` | **partially** | Rule 9 `:562` now gives every variant a base and names `Embedded` as "a key in the document's image map, not a path at all"; §7.1 `:388` says `Package` is "a file inside a packaged archive (§7.5 of the design)". Two of the three sub-issues are gone. Residue: §3 `:93-94` still resolves every `StorageRef` path against the bundle root (finding 5). The trailing note `:572` ("confine paths to the bundle root") repeats the same slip. |
| 10 | confinement failure was non-fatal while the rule said "rejected" | **verified fixed** | Rule 9 `:562` now states the safety property and the consequence in one place: "A path that escapes is **never resolved**: that asset is reported unresolved (rule 1) and the rest of the load continues", and §14 `:590-593` enumerates what is fatal (steps 2 and 3 only) and says an escaped path is "returned as a finding on a usable project". The rule no longer promises a rejection it does not perform. |
| 11 | hard link / TOCTOU under rule 9 | **not fixed** | Rule 9 `:562` is unchanged in this respect: no `nlink`, no device check, no statement that hard links are out of scope, no mention of the canonicalise-then-open race. Carried as finding 10. |
| 12 | §12's closed-type list and "every entity" | **partially** | The false universal is gone and the closed list is explicit and longer: `:539-543` names `PageSize`, `TextLayoutSettings`, `ArtworkRef`, `ContinuityConstraints`, `StorageRef`, `ContentHash`, `MediaMetadata`. But it still omits shapes defined in §5.5 with no `unknown` of their own — `FileEntry` `:204`, `AppInfo` `:205`, `VisualDocumentRef` `:206` (round 2's `manifest.app`), `Consent` `:218`, `Progress` `:220`, `JobError` `:221` — and "Every *entity* struct carries it" still has no definition of *entity* anywhere in the document, so the sentence still cannot be checked. |
| 13 | §5.6 deltas incomplete / rule-10 mis-citation | **verified fixed** | §5.6 `:227-239` now lists six rows, including `StorageRef::Package` and the `MediaMetadata` split round 2 named, and the `Asset.provenance` row now cites rule 10 accurately ("rule 10 requires it for `generated` and `edited` and forbids it otherwise") — which rule 10 `:563` now says ("and absent otherwise"). I walked the design's §5.2 lists against §5.1–§8 and found no further unlisted field delta. |
| 14 | rule 13 (reachability + file name in one rule) | **verified fixed** | The two concerns were split: rule 13 `:566` is now the file-name check ("`Page.id` equals the number in its file name … the file name encodes the *page* id, never the artboard id. **Parse the number, not the string**") and rule 14 `:567` is the reachability check as a **warning** ("an orphan is recoverable, and a scene-less page is legal"). Residue, low: a file whose name is not `page-<n>.json` at all still has no stated effect. |
| 15 | §13's table out of numeric order | **verified fixed** | `:554-570` reads 1…17 in order; the round-2 green test that pins contiguity still passes. |
| 16 | `ReviewDecision.timestamp` untyped | **verified fixed** | §8.2 `:422` is "`timestamp: Timestamp`", and §5.5 `:202` gives `Timestamp` its canonical form. |
| 17 | `Timestamp` precision / offset unstated | **verified fixed** | §5.5 `:202` is now "`String`, RFC 3339 **in UTC, second precision, `Z` suffix** … No fractional seconds and no `+00:00` — one canonical form, so a save/reload is byte-identical (§5.7)", and the new §5.7 `:241-254` fixes key order, indentation, number form, escapes and timestamps. Residue, low: §5.7 speaks of "the order the struct declares them" and says nothing about map-valued fields (`unknown`, `request: Value`), whose keys `serde_json` writes sorted (the round-2 green test still demonstrates this) — the fixture author must discover it, not read it. |
| 18 | `schema_version` enforcement unstated | **partially** | Both halves moved: §5.1 `:134-136` justifies the field, §5.4 `:177-178` gives the story files one, and rule 3 `:556` is the first rule to enforce it ("every file that declares `schema_version` declares the manifest's value"). The residue is the enumeration that follows the dash — it names four files and omits the five §5.4 just declared (finding 4). |
| 19 | rule 2 and duplicate retired ids | **partially** | The question is answered — rule 17 `:570` is new and says exactly what round 2 asked for: "`retired_artboard_ids` is **unique and append-only**: an id is never removed once retired". But the answer is only correct if reuse cannot happen, and §9/§9.1 still say it can; rule 4 then makes the reused id broken and rule 17 forbids the repair (finding 3). Round 2 asked for a decision; the decision given contradicts the rest of the document rather than resolving it. |

New-contradiction check: the five round 2 said the fixes had introduced

- **Finding 2 (§9.1 vs rule 15/S1): resolved.** §9.1 mitigation 3 was rewritten in the design's own
  terms and now agrees with rule 15 and S1. The remaining disagreement is with the design
  (finding 11b), not with the schema.
- **Finding 3 (§12 cites rule 15): resolved.** §12 now cites rule 1, which exists and is the walk
  it describes; the round-2 red test is green.
- **Finding 4 (§14 runs 1–12): resolved.** Step 5 runs 1–17; green test.
- **Finding 5 (§10 save-order window): resolved in its fatal half.** §14 step 1 was rewritten to make
  the `files`/tree mismatch non-fatal and to name the window it protects, so a crash between step 4
  and step 5 no longer bricks the bundle. What is left is the false visibility sentence
  (finding 12) and the unedited design order (finding 11a).
- **Finding 13 (§5.6 completeness / rule 10): resolved.** Six rows, all verified against the design,
  and rule 10 now says "absent otherwise".

**What the new fixes moved.** Three places, none of them the same as round 2's five:
(the renumbering, which added rules 15–17, left §15 S1 citing the now-unrelated rule 14 —
finding 2); (rule 3 was rewritten to enforce `schema_version` and its enumeration was not extended
to the five files §5.4 was rewritten to version — finding 4); (the new rule 17 asserts uniqueness
and append-only, which converts §9.1's reuse hazard from a silent drift into a permanent false
finding — finding 3). §3's blanket base (finding 5) and §10's visibility sentence (finding 12) are
older residue that the round-2 fixes did not touch.

The 17 rules as a set: numbers 1..=17, no gap, no duplicate (green test). Every `rule N` citation in
the document resolves to a rule that exists (`:95`, `:178`, `:236`, `:237`, `:384`, `:459`, `:543`,
`:554`, `:562`, `:601`) — but two cite the *wrong* existing rule: §15 S1 cites 14 for the rect
warning (finding 2), and §6.1's "the file name must agree" (`:280`) is still unbacked by a rule
that says *which* name (rule 13 supplies it at `:566`, so the reader has to make the link).
Overlaps are deliberate and benign (rule 1 and rule 11 both constrain `Page.panel_ids`; rule 4 and
rule 5 both constrain `artboard_id`; rule 1 lists `Lettering.dialogue_id` while rule 8 governs its
presence). Coverage gaps: the singular-key class is now governed (rule 1); the four remaining
ungoverned fields are `manifest.bundle_id` (nothing relates it to `project.json` or requires
stability), `manifest.title` vs `project.title` (both required, `:112` calls one "denormalised" but
no rule compares them, and rule 3 compares `comic_id` and `document` only), `ComicProject.settings`
(no rule checks `default_page_size` is a point size or non-negative), and `Page.page_number`
(no rule at all). Every other rule has something to check, and rule 16's "whose `algorithm` the
reader implements" is checkable but vacuous while `ContentHash.algorithm` is the literal `"sha256"`.

The two structural rules: **still clean**. §1 `:21-27` restates ADR-013; §3 `:70` keeps the document
a plain `.vectorcraft` file; §10 steps 2–3 touch it only through `crates/format`; §12 `:536-537`
contrasts its `unknown` with `Document::unknown`; §6.1 `:283` has no `page_size`, §6.2 `:298` has no
`geometry_ref`, §6.4 `:334-335` puts the text on the `Dialogue` and typography on the `Lettering`,
and rules 5, 6, 8 and 11 enforce the one-way links. The two carried residues are unchanged: no rule
compares the *rendered* text node behind `Lettering.text_node_id` to `Dialogue.text` (round 1 raised
it; still absent), and rule 15 now compares the artboard's `rect` to `ProjectSettings.default_page_size`,
which is the nearest the document comes to the "second copy of a page's size" §5.3 `:157-158`
forbids — it is a warning, so it is a smell rather than a breach.

The security rule: rule 9's mechanism is right and unchanged from round 2's assessment — resolve
against a declared base, canonicalise, then confine, which is what defeats `..` and symlinks and
what round 1 asked for. The three things round 2 said were still missing are now: **fixed** for the
`Embedded`/`Package` bases (`:562`), **fixed** for the fatal-versus-reported framing (`:562`,
`:590-593`), **not fixed** for the hard link and the canonicalise-then-open race (finding 10).
The `jobs/` gap (finding 1) is the one *new* security-adjacent item: §14 step 1 opens every named
and every unnamed file to compare hashes, and the document never says what the files in `jobs/`
are, so both the hash comparison and the parser have an undefined input class.

Implementability: which of round 2's nine guesses are gone

| # | Round 2's guess | Status |
|---|---|---|
| 1 | `AssetRegistryId`'s value | **gone** — §5.5 `:201-203` types it and allocates it once at project creation |
| 2 | which files carry `schema_version` | **remains, smaller** — §5.4 now declares the story files versioned, but rule 3's enumeration omits them (finding 4): a validator author still has to choose |
| 3 | the canonical JSON form | **gone** — §5.7 `:241-254`; residual (low) for map-valued fields, whose key order the section does not cover |
| 4 | `manifest.files` membership | **gone in §4** `:116` (both `exports/` and `.save-tmp/` excluded); residual noise in §14 step 1 (finding 6) |
| 5 | which rules a load runs | **gone** — §14 step 5 runs 1–17 |
| 6 | the rect baseline and severity | **gone in the schema** — rule 15 (`default_page_size`, warning) and §9.1 `:457-460` agree; the design still says otherwise (finding 11b) |
| 7 | the singular foreign keys | **gone** — rule 1 `:554` |
| 8 | `StorageRef::Package`'s base | **gone in rule 9/§7.1**, **remains in §3** `:93-94`, which still flattens every `StorageRef` path onto the bundle root (finding 5) |
| 9 | `manifest.app.schema` and `ReviewDecision.timestamp` | **gone** — §5.5 `:205` (`AppInfo { …, schema: u32 }`), §8.2 `:422` |

So of the nine, five are gone outright and three more are gone where the rules are normative and
left as prose residue (§3, §14 step 1, §5.7's maps). Guesses 2 and 8 remain in the text an
implementer reads, and one item **not** on round 2's list is now blocking: **`jobs/` (finding 1)** —
`comic-model` cannot write a bundle whose `manifest.files` must cover a directory the schema never
describes, and §11.1's fixture cannot place its "1 job". That, plus finding 3's made-permanent
reuse finding, is why this round is still `changes-requested` rather than `approved`: the type set
in §5.5 is complete, the rules are numbered and contiguous, and a builder can derive almost every
struct — but the bundle it writes would contain a directory the schema cannot describe, and its
validator would either miss five versioned files or reject a legitimate page.

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest. Run on the changed target instead:

```
$ cargo fmt -p vectorcraft-doc -- --check                # no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims   # 0 diagnostics
$ cargo test  -p vectorcraft-doc --test schema_claims    # 12 passed; 10 failed (the reproductions)
```

A full `cargo xtask ci` would now fail at its test step on the 10 red tests; that is the intended
evidence, not an accident.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set, and the
  session store records that model for the sessions running beside this one, but
  `assistant_usage_events` has **0** rows for `b1656afb-a5ac-4ac9-9253-7823e1002e89`, so nothing in
  the environment attests the backend of the model answering this review.
- The comic wire format: `crates/comic-model` is still `SCHEMA_VERSION` + `ComicId` only
  (`crates/comic-model/src/lib.rs`), so §4–§8, §11.1, §12, §13 describe types that do not exist. The
  v1 fixture does not exist, so its authorability is argued from §5.5–§5.7, not demonstrated — and
  finding 1 is exactly the part of it that cannot be argued from the document.
- Whether the owner intends `jobs/` to hold files at all, or whether it is a leftover of design §7.1
  that should be deleted from both trees. The schema cannot be corrected here without that decision;
  I reported the gap rather than choosing.
- Whether design §7.2, §5.3.1 and §12.1 will be corrected with the schema (findings 11a–c). §1 puts
  the design above this document, so if the design is not edited the schema is the document that is
  "wrong" by its own rule; that decision is the lead's, not mine.
- Whether `comic.page.*` and `comic.project.validate` will exist: M1 work, not a false claim, and
  finding 3 assumes only that they are implemented as described.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: no fixture and no code exercise
  it, so finding 10 is a reading of the rule, not a reproduction.

---

## Round 4 — verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set in
               the environment (`AI_AGENT=github_copilot_vscode_agent`), the same in-environment
               statement rounds 2 and 3 relied on. `~/.copilot/session-store.db ::
               assistant_usage_events` still holds **0** rows for this session
               (`b15e21bd-cf51-4099-9399-86b94a74bbcf`) out of 632 rows in the table (630 of them
               `deepseek-v4.1-flash`), so the model claim rests on the environment plus the harness
               record and not on an attestation of the serving backend.
Revision reviewed:
               **working tree after the round-3 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` untracked, mtime `2026-10-10 22:30`, **644 lines**
               (round 3 reviewed the 604-line post-round-2 text at mtime 22:15);
               `docs/comiccraft-design.md` mtime `2026-10-10 22:30`, **1660 lines** — the design was
               edited in the same pass (§7.2 save order and §5.3.1 mitigation 3, which is round-3
               finding 11's fix; §12.1's page-dimension row still names the old baseline). The
               revision moved; this round is not a re-run of it.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 644 lines), `docs/comiccraft-design.md`
               §5.1.1, §5.2, §5.3.1, §7.1, §7.2, §10.3, §12.1 (the sections the schema cites, and
               the three round 3 found disagreeing), `docs/reviews/M0-schema-document.md`
               (rounds 1–3), `crates/doc/tests/schema_claims.rs`, plus the code the document cites
               that three rounds had not re-opened: `crates/doc/src/links.rs:20-32`,
               `crates/doc/src/lib.rs:1028-1030`, `crates/engine/src/cmd/links.rs:624-645`,
               `crates/format/src/lib.rs:428-470`, `crates/pdf/Cargo.toml:34`,
               `crates/comic-model/src/lib.rs`, `Cargo.toml:48-49`
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **9 new tests**, plus two helpers —
               `word_number` for a spelled count, `heading_numbers`/`number_citations` for
               cross-document citations; **no existing assertion changed, none deleted, none
               weakened**)
Design refs:   §1 of the schema (the design decides the meaning), §4.3, §5.1.1, §5.2, §5.2.1,
               §5.3.1, §6.1, §7.1–§7.5, §8.3, §10.3, §12.1; schema §3, §4, §5.1–§5.7, §6.1–§6.6,
               §7.1–§7.2, §8.1–§8.3, §9, §9.1, §10, §11.1, §12, §13, §14, §15

Verdict:       **changes-requested**

Round 3's twelve findings are addressed: **nine are verified fixed**, **three are partially fixed**
(finding 5, finding 8 and finding 9 — each in the one place the fix did not carry the change), and
**none is untouched**. The rule set is sound as a set (1..=17, no gap, no duplicate, every `rule N`
citation resolves) and the security rule is now the strongest part of the document. What keeps this
at *changes-requested* is that the same seam moved three more times: §9.1's mitigation list now says
"Four" and lists five (finding 1 — and its fallback sentence excludes the mitigation the fix added),
§8.3 cites rule 1 for a check rule 1 cannot perform (finding 2), and rule 9's own trailing note
re-flattens the three bases the rule splits (finding 3). Those three are defects in the sense the
rule cares about — a reader who follows them implements the wrong thing or implements nothing —
however small the edit that fixes each one. Four more are the same classes rounds 1–3 treated as
findings: a citation of a schema section that does not exist (finding 7, a cross-document defect),
rule 1 reporting a legal bundle as broken the way rule 12 did (finding 6), §5.5's completeness
sentence being false about nine of the document's own types (finding 5), and §13's rule table
rendering as prose after rule 9 (finding 4, a formatting defect whose content is nonetheless right).
The last two are M1 notes: design §12.1's page-dimension baseline (finding 8) and §5.7's silence on
the map-valued fields' key order (finding 9).

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **27 passed; 9 failed**. The 27 are rounds 1–3's tests, all green: every round-3
               reproduction went green with the fixes, which is the evidence that *round 3's*
               findings 2, 4, 6, 7, 10 and 12 are fixed. The 9 failures are this round's reproductions
               of the 9 findings below (finding 8's design-side seam and finding 9's map-order gap are
               reading-only for the schema; the design seam is pinned through the design file), and
               each goes green the moment the document is corrected.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 36 tests
test artboard_id_is_u32_and_the_rect_is_document_points ... ok
test artboard_ids_are_reused_after_the_highest_artboard_is_deleted ... ok
test a_reused_artboard_id_does_not_end_up_permanently_broken ... ok
test document_unknown_and_extra_are_present_and_empty_by_default ... ok
test every_directory_in_the_bundle_layout_is_defined_somewhere ... ok
test every_referential_rule_is_numbered_once_and_contiguously ... ok
test every_rule_reference_in_the_schema_names_a_rule_that_exists ... ok
test every_store_the_bundle_layout_declares_has_a_file_the_schema_names ... ok
test node_ids_save_as_bare_numbers_and_allocate_upward ... ok
test no_directory_or_rule_is_defined_twice ... ok
test rule_3_names_every_file_that_declares_a_schema_version ... ok
test rule_three_names_every_file_kind_that_carries_a_schema_version ... ok
test rule_12_says_what_a_null_speaker_means ... ok
test s1_closes_the_page_size_item_with_the_rule_that_states_it ... ok
test section_5_5_does_not_overclaim_completeness ... ok
test the_files_tree_comparison_excludes_what_files_excludes ... ok
test the_link_hash_is_fnv1a_64_bit_not_a_sha256_integrity_hash ... ok
test the_link_relative_path_is_optional_and_skipped_when_unset ... ok
test the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle ... ok
test the_load_sequence_runs_every_referential_rule ... ok
test the_missing_font_claim_cites_a_section_that_contains_the_check ... ok
test the_round_trip_the_fixture_promises_needs_a_canonical_json_form ... ok
test the_save_order_does_not_claim_content_no_reader_can_see ... ok
test the_schema_and_the_design_it_defers_to_do_not_contradict_each_other ... ok
test the_schema_does_not_assert_both_path_bases_at_once ... ok
test the_schema_does_not_assert_one_storage_base_where_rule_9_splits_them ... ok
test the_three_path_bases_are_named_wherever_a_base_is_claimed ... ok
test every_section_cited_across_the_two_documents_exists ... FAILED
test rule_1_carves_out_nullable_keys_like_rule_12 ... FAILED
test section_13s_rule_table_is_one_table ... FAILED
test section_5_5_defines_the_types_it_claims_to ... FAILED
test section_5_7_covers_map_valued_fields ... FAILED
test the_mitigation_list_matches_its_count_and_its_fallback ... FAILED
test the_page_dimension_check_names_a_baseline_the_schema_defines ... FAILED
test the_rule_9_note_does_not_flatten_the_bases_rule_9_splits ... FAILED
test the_stale_jobs_file_is_reported_by_a_rule_that_governs_it ... FAILED
test result: FAILED. 27 passed; 9 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure text, verbatim (each names the sentence that has to change):

```
the_mitigation_list_matches_its_count_and_its_fallback:
  §9.1 says 4 mitigations are mandatory but lists 5: Comic commands own artboard lifecycle. |
  Retired ids are remembered. | Validation cross-checks. | The comic layer never reuses a retired
  id. | Upstream fix proposed.
the_stale_jobs_file_is_reported_by_a_rule_that_governs_it:
  §8.3 says a stale `jobs/` file "there (a terminal status) is an inconsistency rule 1 reports." —
  but rule 1 is "Every reference resolves: every `*_ids` entry **and** every singular foreign key
  (...)", which walks references and cannot report a file's status
the_rule_9_note_does_not_flatten_the_bases_rule_9_splits:
  the note under rule 9 confines every path to the bundle root while rule 9 splits the bases
  (`Package` resolves against the archive root)
section_13s_rule_table_is_one_table:
  §13's table is split by a blank line after rule 9, so rules beyond it render as prose rather than
  as table rows — left: [1..9], right: [1..17]
section_5_5_defines_the_types_it_claims_to:
  §5.5 claims every type this document defines appears in its table, but ProjectMetadata (§5.2),
  ProjectSettings (§5.3), PageSize (§5.3), ArtworkRef (§6.2), ContinuityConstraints (§6.5),
  TextLayoutSettings (§6.6), StorageRef (§7.1), ContentHash (§7.2), MediaMetadata (§7) do not
rule_1_carves_out_nullable_keys_like_rule_12:
  rule 1 requires every singular foreign key to resolve, but the document declares
  Lettering.dialogue_id, GenerationJob.target_panel_id, ReviewDecision.superseded_asset_id as
  `Option<_>`, where `null` is legal
every_section_cited_across_the_two_documents_exists:
  a cross-document citation that does not resolve: design:623 cites schema §5.1.1
the_page_dimension_check_names_a_baseline_the_schema_defines:
  the design's §12.1 compares the artboard `rect` against "the project's print presets", but neither
  document defines a print preset
section_5_7_covers_map_valued_fields:
  §5.7 fixes struct-field order but never mentions the map-valued fields the format carries
  (`unknown`, `request: Value`), whose keys `serde_json` writes sorted
```

Also clean on the extended target: `cargo fmt -p vectorcraft-doc -- --check` (no diff after
`cargo fmt -p vectorcraft-doc`; no tracked file under `crates/doc/` was touched — the only change
is the untracked test file) and `cargo clippy -p vectorcraft-doc --test schema_claims`
(0 diagnostics).

Findings:      9 — 2 `contradiction`, 4 `under-specified`, 3 `style`

Line numbers are the post-round-3 `docs/comiccraft-project-schema.md` (644 lines) unless the file
is named.

1. `:468` (with `:470-487`) §9.1 — "**Four mitigations, all mandatory** (design §5.3.1):" and then
   **five** items: the new mitigation 4 ("The comic layer never reuses a retired id", `:480-485`)
   was inserted without renumbering the copy, and the fifth closes "Until it lands, **1–3 stand
   alone**" (`:487`) — the design's sentence for a four-item list whose fourth item *is* the
   upstream fix (design §5.3.1 has four, and its own "1–3 stand on their own" is correct there).
   Here the sentence excludes mitigation 4, which §9.1 calls mandatory and which is the only thing
   keeping rules 4 and 17 from making §9.1's reuse a permanent finding. **contradiction**
   (introduced by the round-3 fix; pinned by a red test).
2. `:442` §8.3 — "a stale file there (a terminal status) is an inconsistency **rule 1** reports."
   Rule 1 (`:584`) walks references — "every `*_ids` entry **and** every singular foreign key" — and
   cannot report a file's status; no other rule mentions `jobs/` either, so the directory §3
   declares, §4 requires `manifest.files` to cover and §14 hashes is governed by nothing and the
   finding §8.3 promises is never produced. **style** (stale citation — a defect: the number
   resolves, the rule cannot perform the check; introduced by the round-3 fix; pinned by a red
   test).
3. `:610` §13's trailing note — "Rule 9 must **confine paths to the bundle root** before reading …
   must be rejected, not resolved" — re-flattens the bases in the paragraph directly under the rule
   that splits them (`:592`: `Package` resolves against the **archive root**). Round-3 finding 5's
   fix corrected §3 `:91-97` and §7.1 `:386-394` and left this note, the third place the claim
   appears, as §3's pre-fix sentence. An implementer who follows the note rejects valid packaged
   paths, and "rejected" still reads as fatal where rule 9 and §14 `:631` make the escape a finding
   on a usable project. **contradiction** (pinned by a red test).
4. `:592-600` §13 — rule 9's cell now contains blank lines, so the markdown table ends after rule 9:
   rules 10–17 (`:601-608`) do not render as table rows, and rule 9's own last sentence ("A `Linked`
   file must exist and its sha256 must match `content_hash`") is stranded at the end of a prose
   paragraph with a stray trailing `|`. The rule set is the normative list a validator is built
   from; it has to render as one table (an inline `<br>`, or moving the paragraph below the table,
   fixes it without changing a word). **style** (introduced by the round-3 rule-9 fix; pinned by a
   red test).
5. `:199-202` §5.5 — "Every type **this document defines** appears here … A type that is in neither
   place is a defect in this document." Neither sentence is true: `ProjectMetadata` (§5.2),
   `ProjectSettings` (§5.3), `PageSize` (§5.3), `ArtworkRef` (§6.2), `ContinuityConstraints` (§6.5),
   `TextLayoutSettings` (§6.6), `StorageRef` (§7.1), `ContentHash` (§7.2) and `MediaMetadata` (§7)
   are all defined by this document in their own sections and none is a row of §5.5's table — and
   §5.6 counts some of them as shapes this document adds. The round-3 fix removed the false "Every
   type a table names appears here" and its two named counterexamples (`ComicId`, `JobStatus`) but
   replaced it with a **wider** claim, so the residue grew rather than shrank. **under-specified**
   (round-3 finding 9, not fixed; pinned by a red test).
6. `:584` rule 1 — among the "singular foreign key"s that must resolve it names three the document
   declares `Option<_>`: `Lettering.dialogue_id` (`:338`, "**must be `null`** for
   `narration`/`sound_effect`"), `GenerationJob.target_panel_id` (`:414`), and
   `ReviewDecision.superseded_asset_id` (`:429`). Rule 12 was corrected for exactly this class in
   the same pass (`:603`: "that is **not `null`** … so the rule checks the set values, not the
   field's presence") and round 3 recorded that the omission "is not confined to rule 12"; rule 1
   kept its old wording, so a validator built from it reports every narration line, every job with
   no target panel and every non-superseding decision. **under-specified** (round-3 finding 8's
   residue; pinned by a red test).
7. `docs/comiccraft-design.md:623` — design §5.3.1 cites "(**schema §5.1.1**, rule 15)"; the schema
   has **no** §5.1.1 — its page/artboard ownership section is §6.1, and §5.1.1 is the design's own
   number, which the schema cites correctly at `:287`. §1 makes the design decide the meaning, so a
   reader who follows the citation finds nothing to decide it with. **style** (dangling
   cross-reference — a defect; introduced by the round-3 design fix; pinned by a red test that
   resolves every `design §N`, `schema §N` and `schema rule N` citation across the pair).
8. `docs/comiccraft-design.md:1297` — design §12.1 still lists "Unsupported page dimensions |
   `comic-layout` | the artboard `rect` against the project's **print presets**", and `:1300` says
   `comic.project.validate` "runs this set". Neither document defines a print preset: the schema's
   rule 15 (`:606`) compares the rect against `ProjectSettings.default_page_size` and §15 S1
   (`:641`) declares the item closed by rule 15. The round-3 fix rewrote design §5.3.1 `:620-625`
   and §7.2 `:757-785` to the schema's position but not §12.1, so the pair still names two
   baselines for one check and the design is the authoritative text. **under-specified** (round-3
   finding 11c; pinned by a red test).
9. `:250` §5.7 — "Object keys in the order the struct declares them (serde's default), not sorted"
   is the whole key-order rule, while §12 `:558-563` flattens `unknown: Map<String, Value>` into
   every top-level struct and §8.1 embeds `request: Value`. `serde_json` has no `preserve_order`
   here (`Cargo.toml:49`), so `Map` is a `BTreeMap` and those keys are written **sorted** — the
   round-2 green test `the_round_trip_the_fixture_promises_needs_a_canonical_json_form`
   demonstrates it with the workspace's own `serde_json`. §11.1 requires a byte-identical round
   trip, so a fixture author needs this rule and §5.7 does not give it. **under-specified** (low;
   round-3 finding 17's residue, carried; pinned by a red test).

Round-3 verification: the 12 findings

| # | Round-3 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | §3 declared `jobs/` with no file name, shape or owner | **verified fixed** | §3 `:76` names `job-<JobId>.json`; §8.3 `:433-442` gives the file, its shape ("one `GenerationJob` with the same shape as §8.1") and its lifecycle; rule 3 `:586` version-checks it; design §7.1 `:731`/§10.3 `:1185` own it. `every_store_the_bundle_layout_declares_has_a_file_the_schema_names` and `every_directory_in_the_bundle_layout_is_defined_somewhere` are green. Residue, note not finding: §11.1's "1 job" (`:547`) still does not say whether it is a `jobs/` file or an entry in `provenance/generations.json`; §8.3's "empty in a quiescent project" makes the latter the only reading of a stable fixture, but the document could say so in one clause. |
| 2 | §15 S1 cited rule 14 for the rect warning | **verified fixed** | §15 S1 `:641` is "**Closed** by rule 15"; rule 15 `:606` is the rect warning. `s1_closes_the_page_size_item_with_the_rule_that_states_it` is green. |
| 3 | §9's "can be reused" vs rule 4 vs rule 17 | **verified fixed** | §9's table `:450` now says the allocation "**never lets a page take** [an artboard] when it is retired" and the reuse column is "**must not be reused by a page**"; §9.1 `:462-466` and the new mitigation 4 `:480-485` make `comic.page.create` the enforcement point; rule 17 is no longer a rule the document contradicts. `a_reused_artboard_id_does_not_end_up_permanently_broken` is green. Residue is finding 1 (the same fix's count and fallback) and the note below about the mechanism. |
| 4 | rule 3's enumeration omitted the five story files | **verified fixed** | rule 3 `:586` names all five `story/*.json` files (§5.4 `:174-180`) and `every jobs/job-<id>.json`; `rule_3_names_every_file_that_declares_a_schema_version` and `rule_three_names_every_file_kind_that_carries_a_schema_version` are green. |
| 5 | §3 flattened every `StorageRef` onto the bundle root | **partially** | §3 `:91-97` now names all three bases (`Linked`→bundle root, `Package`→archive root, `LinkInfo.relative`→`document/`) and §7.1 `:386-394` agrees; `the_schema_does_not_assert_one_storage_base_where_rule_9_splits_them` and `the_three_path_bases_are_named_wherever_a_base_is_claimed` are green. The claim also appears in the note under rule 9, which the fix did not touch — finding 3. |
| 6 | §14 step 1 did not restate §4's exclusions | **verified fixed** | §14 step 1 `:619-620` compares "**as §4 defines it** — every file except the manifest, `exports/` and `.save-tmp/`". `the_files_tree_comparison_excludes_what_files_excludes` is green. |
| 7 | §5.3 cited §13 for the missing-font report | **verified fixed** | §5.3 `:174` now says "a deterministic check (design §12.1), reported by `comic.project.validate`, **not a referential rule**". `the_missing_font_claim_cites_a_section_that_contains_the_check` is green. The same class recurs at §8.3 `:442`, which was written in the same pass — finding 2. |
| 8 | rule 12 required a value for an `Option` field | **partially** | rule 12 `:603` now reads "that is **not `null`** … `null` is legal and means narration … the rule checks the set values"; `rule_12_says_what_a_null_speaker_means` is green. Round 3 said the omission "is not confined to rule 12"; rule 1 `:584` still names three nullable keys with no carve-out — finding 6. |
| 9 | §5.5's completeness claim was false | **partially** | the old sentence and its two counterexamples are gone and the scope is now "the value types the tables **in this document** reference" (`:196-198`); `section_5_5_does_not_overclaim_completeness` is green. But the replacement sentence ("Every type **this document defines** appears here") is false in a wider way — nine types this document defines are not in the table — finding 5. |
| 10 | rule 9 said nothing about hard links or the canonicalise/open race | **verified fixed** | §13 `:594-600` now states both limits: a hard link "has no path to canonicalise — so it is accepted, and the bundle's integrity rests on `content_hash`", and the reader "opens with no-follow semantics where the platform offers them (`O_NOFOLLOW`), and re-checks that the opened file is inside the base", with the residual window documented and the bundle "treated as locally trusted". That is the answer round 2 asked for. The paragraph's placement is finding 4. |
| 11 | the schema and the design disagreed (a) save order, (b) rect expectation, (c) §12.1 | **partially** | (a) design §7.2 `:757-785` now replaces the content files in step 4 and writes the manifest last in step 5, and even cites schema §14; (b) design §5.3.1 mitigation 3 `:620-625` now reads "It checks no size expectation, because the artboard *is* the page's size and no field holds a second copy to compare against" — both are the schema's position and `the_schema_and_the_design_it_defers_to_do_not_contradict_each_other` is green. (c) design §12.1 `:1297` was **not** updated — finding 8. The same design edit introduced the dangling citation at `:623` — finding 7. |
| 12 | §10 claimed the new generation was invisible until the manifest | **verified fixed** | §10 `:497-505` now describes the window explicitly ("**This opens the window.** … That is expected and survivable … §14 reports a `files` mismatch instead of refusing to load"); both false sentences are gone and `the_save_order_does_not_claim_content_no_reader_can_see` is green. |

Regressions: the five round-3 regressions

- **rule 17 vs §9's reuse — resolved**, and the fix moved the seam once: the count and the fallback
  sentence of §9.1's mitigation list are stale from the design's four-item original (finding 1).
- **rule 3's enumeration vs §5.4 — resolved** (rule 3 `:586` names all five story files).
- **§3 vs §7.1 bases — resolved in both places, and moved to a third**: the note under rule 9
  `:610` still flattens every path onto the bundle root (finding 3).
- **§12's "rule 15 checks" — resolved** (`:543` cites rule 1, which exists and is the walk it
  describes). The *other* completeness claim in the same document, §5.5's, got wider (finding 5).
- **§10's visibility claim — resolved** (`:497-505`).

New seams this round's fixes introduced, none of the same place as before: §9.1's mitigation
count and fallback (finding 1), §8.3's citation of rule 1 for a status check (finding 2), rule 9's
collapsed table cell (finding 4), and design §5.3.1's citation of a non-existent schema section
(finding 7). Finding 8 (design §12.1) is not a new seam but an unmoved one — round 3's finding 11c,
where the fix edited two of the three design places it named. Five seams across four rounds of fixes
is the pattern this document keeps repeating: a correction lands in the place the finding named and
not in the other places the same claim is made. The tests added this round are written to catch that
pattern mechanically — the cross-document citation walk, the §13 table integrity check and the §5.5
definition check are all checks over the whole passage rather than over one sentence.

The 17 rules as a set: still sound. Numbers 1..=17, no gap, no duplicate (`no_directory_or_rule_is_defined_twice`,
`every_referential_rule_is_numbered_once_and_contiguously` green), and every `rule N` citation in the
schema resolves (`:97`, `:180`, `:240`, `:241`, `:388`, `:394`, `:478`, `:481`, `:573`, `:584`,
`:592`, `:641`) — but two cite the *wrong* existing rule: §8.3 `:442` cites rule 1 for a `jobs/`
status check (finding 2), and §13's note `:610` restates rule 9 as one base (finding 3). Rule 12's
null carve-out (`:603`) is correct and rule 1 needs the same (finding 6). Coverage: `jobs/` is the
one directory with no rule (finding 2); the four ungoverned fields round 3 listed are still
ungoverned and are notes, not findings, because none of them can make a legal bundle invalid —
`manifest.bundle_id` (nothing relates it to `project.json`), `manifest.title` vs `project.title`
(rule 3 compares `comic_id` and `document` only), `ComicProject.settings` (no rule makes
`default_page_size` a point size or non-negative), `Page.page_number` (no rule at all), and
`Panel.generation_status` (no rule ties it to the job that drives it).

Security: rule 9 is now the strongest paragraph in the document. Resolve against the declared base,
canonicalise, then confine (`:592`) defeats `..` and symlinks; the escaping path is **never
resolved** and the load continues, which matches §14 `:631`'s "returned as a finding on a usable
project"; the hard-link and TOCTOU limits are stated with the mitigation (`O_NOFOLLOW` plus a
re-check) rather than papered over (`:594-600`); `.save-tmp/` is excluded as a source and §14 step 1
restates §4's exclusions. The two residue risks are not new: (a) the note at `:610` re-collapses the
confinement base (finding 3) — for `Package` a bundle-root confinement is the wrong check, and
following the note would reject valid packaged paths; (b) the table break (finding 4) leaves rule 9's
own "A `Linked` file must exist and its sha256 must match `content_hash`" outside the rule that owns
it, so a validator built from the rendered table misses the hash check. §14's crash window is
correct: only steps 2 and 3 are fatal (`:629-633`), the `files` mismatch is a finding, and the
window §10 opens in step 4 is the one §14 was rewritten to tolerate. §8.1's "credentials never
appear here" (`:417-419`) is unchanged and still a positive control.

Implementability: an agent building `crates/comic-model` from this document would now get the type
set right — §5.5 `:200-241` shapes every type the tables reference and §5.6 `:227-239` accounts for
the six deltas against the design — and could write a bundle whose every store is described. What it
could still get **wrong**:

- the validator's reference walk, if it follows rule 1 literally and reports `null` as unresolved
  (finding 6) — this is the one item in this list that produces a false finding on a legal bundle;
- the confinement base for `Package` if it follows rule 9's note rather than rule 9 (finding 3);
- the `jobs/` check if it implements §8.3 as written and looks for it in rule 1 (finding 2), or
  omits it because no rule names the directory;
- the mitigation list, if it takes "Four mitigations, all mandatory" at face value and picks four of
  the five listed, or drops mitigation 4 because the closing sentence says "1–3 stand alone"
  (finding 1) — dropping it re-opens the reuse hazard rule 17 exists to close;
- the hash check for `Linked` files, if it builds the validator from §13 as rendered (finding 4).

The residual precision gaps (below) would make an implementer pause but not guess wrong, which is
why they are notes: §9.1 mitigation 4 says `comic.page.create` "allocates artboards until it gets an
id that is not retired" without saying whether the skipped artboards are created and kept — with
`max(id) + 1`, discarding them re-derives the same retired id, and keeping them leaves artboards that
are not pages against §3's "artboards are pages"; §11.1's fixture job has no stated home (§8.3's
"empty in a quiescent project" is the only clue); §12 `:569-573`'s closed-type list still omits the
six §5.5 shapes that carry no `unknown`, and "Every *entity* struct carries it" still uses a term the
document never defines; §5.6 `:239` cites "§9.3/Q13" without naming the design as the document those
sections belong to.

Blocking vs note:

- **Defects (blocking, `changes-requested`):** findings 1, 2, 3 (contradictions and a citation that
  cannot perform the check it promises) and 6 (a rule that reports a legal bundle as broken, the
  same defect round 3 found in rule 12) — plus findings 5 and 7, which are internal and
  cross-document false claims of exactly the class the round-3 round treated as findings.
- **Style/formatting (should fix, not a wrong implementation on its own):** finding 4 (the broken
  §13 table). It is listed as a defect in this round only because the table *is* the normative rule
  list; the content is right, the rendering is not.
- **M1 notes, not blockers:** finding 8 (design §12.1's "print presets" — the mechanism is rule 15's
  and an implementer would build that one; the two names should be reconciled, and the design is the
  authoritative text); finding 9 (§5.7's map-valued key order — a fixture author meets it on the
  first fixture); the §9.1 mitigation-4 mechanism; §11.1's fixture job; §12's entity term and its
  six omitted closed shapes; the four ungoverned fields.

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest. Run on the changed target instead:

```
$ cargo fmt -p vectorcraft-doc -- --check                # no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims   # 0 diagnostics
$ cargo test  -p vectorcraft-doc --test schema_claims    # 27 passed; 9 failed (the reproductions)
```

A full `cargo xtask ci` would now fail at its test step on the 9 red tests; that is the intended
evidence, not an accident, and `git status crates/doc/` shows the test file as the only change under
that crate.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set and the
  session store records that model for the sessions running beside this one, but
  `assistant_usage_events` has **0** rows for `b15e21bd-cf51-4099-9399-86b94a74bbcf`, so nothing in
  the environment attests the backend of the model answering this review.
- The comic wire format: `crates/comic-model` is still `SCHEMA_VERSION` + `ComicId` only
  (`crates/comic-model/src/lib.rs`), so §4–§8, §11.1, §12 and §13 describe types that do not exist;
  their fields are checked against the design, not against code, and the v1 fixture does not exist.
- Finding 4 is a rendering judgement: I read the markdown as GitHub-flavoured (a blank line ends a
  table) but did not render it with the repo's own toolchain, which has no Markdown renderer in CI.
  The stray trailing `|` at `:600` and the missing `|` at `:592` are objective regardless.
- Whether the owner wants the `jobs/` staleness check as a new rule 18, a clause of rule 2, or
  nothing (finding 2 names the requirement, not the placement), and whether design §12.1 `:1297`'s
  "print presets" is meant to be `ProjectSettings.default_page_size` or a field M1 must add
  (finding 8). Both are decisions for the owner and the lead; I reported the gaps rather than
  choosing.
- Whether a *pre-existing* (round 3) rendering of §13's table was ever checked in a browser: rounds
  1–3 read the same file with the same tools, so if the split existed before the round-3 fix it was
  missed by all four rounds' reading. What I can say is that the paragraph is new in round 3's
  revision and that round 3's own quoted text of rule 9 ends at "`.save-tmp/` is never a source.",
  which is where the table now stops.
- Exactly which lines the round-3 fix edited: both documents are untracked, so no `git diff` against
  the round-3 revision exists. I verified the seams in the current text (design §7.2 and §5.3.1 now
  state the schema's position; §12.1 does not), not what changed to produce them.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: no fixture and no code exercise
  it; round 3's finding 10 is verified fixed as *text*, not as behaviour.

---

## Round 5 — final verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set in
               the environment (`AI_AGENT=github_copilot_vscode_agent`), the same in-environment
               statement rounds 2–4 relied on. `~/.copilot/session-store.db ::
               assistant_usage_events` holds **632** rows (630 of them `deepseek-v4.1-flash`) and
               **0** rows for this session (`76616a76-f1d0-4707-9c7f-d1ec93730dc2`), so the model
               claim rests on the environment plus the harness record and not on an attestation of
               the serving backend.
Revision reviewed:
               **working tree after the round-4 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` untracked, **658 lines**, md5
               `55bb7edd75deb3a1c7b16ee8ac71fdfc` (round 4 reviewed the 644-line post-round-3 text);
               `docs/comiccraft-design.md` **1660 lines**, md5
               `1a173ad4cddf6ebf3c3061dec27a6bc8`, mtime 22:46 (round 4 recorded the same length at
               mtime 22:30 — the round-4 fix pass edited it, this round did not). The revision
               moved: §13's table is contiguous again (**18 rows**), §13.1 exists, **rule 18**
               exists, §9.1 says "Five", §5.5's intro is rewritten, §5.7 gains the map bullet, and
               design §5.3.1 and §12.1 are corrected. This is not a re-run of round 4. The schema's
               mtime is later than the round-4 revision only because I restored it byte-for-byte
               (md5 unchanged) after the mutation check below.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 658 lines), `docs/comiccraft-design.md`
               §5.3.1 `:600-630` (the four mitigations and the citation), §9.4/§10.3 `:1040-1060`
               and `:1170-1196` (the approval gate and the durable `jobs/` metadata), §12.1
               `:1288-1305` (the page-dimension row) and the bundle tree `:725-735`,
               `docs/reviews/M0-schema-document.md` (rounds 1–4), `crates/doc/tests/schema_claims.rs`,
               and the code the `jobs/` seam turns on: `crates/comic-ai/src/lib.rs` (all 148 lines —
               **new since round 4**), `crates/comic-ai/tests/job_status_contract.rs`,
               `crates/comic-model/src/lib.rs`, `crates/comic-model/tests/schema_contract.rs`,
               `Cargo.toml:49`, `crates/doc/Cargo.toml` (dev-dependencies), and
               `docs/milestones/M0.md` §2.3/§3/§5/§6 (the record this milestone will commit)
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **5 new tests**, plus one helper —
               `schema_section`, a numbered-section body; `word_number`'s vocabulary extended to
               twenty so a spelled "twelve" can be read; **no existing assertion changed, none
               deleted, none weakened**; `cargo fmt -p vectorcraft-doc` reformatted the new block
               only)
Design refs:   §1 of the schema (the design decides the meaning), §4.3, §5.2, §5.3.1, §6.1,
               §7.1–§7.5, §9.3, §9.4, §10.3, §12.1; schema §1–§15, and §8.1/§8.3/§13 rule 18 in
               particular

Verdict:       **changes-requested**

**All nine round-4 findings are addressed**: eight are verified fixed outright, and the ninth
(finding 5's false completeness claim) is gone while the sentence that replaced it carries finding
4 — the fix landed in the place the finding named in every case, and two residues sit beside the
fixes rather than inside them (§9.1's citation now names a design that lists four — finding 5;
§5.5's replacement mispairs `ContentHash` with §7.1 — finding 4). The
structure the round-4 fixes built is sound: §13 is one table again (18 contiguous rows, rule 9's
hash sentence inside rule 9's own cell), §13.1 is correct and does not re-collapse the bases, rule
18 exists and §8.3 cites it, §14 runs 1–18, and every `rule N` citation in both documents
resolves. **Two defects remain, both inside the `jobs/` seam rule 18 was added to close**: the
document never shapes the file rule 18 governs (finding 1 — rule 3 says it declares a
`schema_version`, §8.3 gives it the shape of a bare `GenerationJob` with none), and it never says
which statuses "terminal" names (finding 2 — the workspace's own `JobStatus::is_terminal` is
`Failed | Cancelled`, under which `jobs/` is never empty in a quiescent project and rule 18 never
reports the case §8.3 promises). Those two decide the bytes on disk and the validator's truth
table, so this round is `changes-requested` on them and not on the prose around them. Four notes
remain (findings 3, 4, 5 and 6 — finding 6 is stale counts in the M0 record itself, which is a
record to correct before the commit, not a rule to implement) — each one clause.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **36 passed; 5 failed** (41 total). Rounds 1–4's 36 tests are all green, which is
               this round's evidence that the nine round-4 findings are fixed in the text. The 5
               failures are this round's reproductions of findings 1–4 and 6; each goes green the
               moment the document (or the milestone record) is corrected, and I verified that by
               mutating in place twice: first the schema (adding a `schema_version` clause and the
               status list to §8.3, `manifest.files` to §8.3, and `§7.1/§7.2` to §5.5) — **40 passed;
               0 failed** — then `docs/milestones/M0.md` (`twelve`→`eighteen` referential rules,
               `four`→`five` mitigations) — **37 passed; 4 failed** — and restored both, md5
               `55bb7edd75deb3a1c7b16ee8ac71fdfc` and `03511735ab9b09a7d2de381d4e44c76e` respectively,
               with the suite red again.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 41 tests
... 36 ok ...
test rule_18_names_the_statuses_that_leave_the_jobs_directory_empty ... FAILED
test the_jobs_file_declares_the_schema_version_rule_three_checks ... FAILED
test the_jobs_lifecycle_says_what_a_job_file_does_to_the_manifest ... FAILED
test the_milestone_record_states_the_counts_the_frozen_document_has ... FAILED
test the_shared_types_section_pairs_each_type_with_its_own_section ... FAILED
test result: FAILED. 36 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure text, verbatim except for the quoted section bodies the messages append:

```
the_jobs_file_declares_the_schema_version_rule_three_checks:
  rule 3 lists every `jobs/job-<id>.json` among the files that declare `schema_version`, but §8.3
  gives the file the shape of a bare `GenerationJob` and never gives it a version, where §5.4, §7
  and §8 each show their own file's `schema_version`. The file's bytes are undetermined: §8.3 text:
  "[§8.3 quoted verbatim]"
rule_18_names_the_statuses_that_leave_the_jobs_directory_empty:
  rule 18 says "Every file in `jobs/` holds a job whose `status` is **not terminal** — a finished
  job's record lives in `provenance/generations.json` and its file is removed (§8.3)" and §8.3
  removes the file at a terminal state, but neither names which of the six `JobStatus` values is
  terminal (["queued", "running", "completed", "failed", "awaiting_review", "cancelled"]).
  `completed` and `awaiting_review` are the two `crates/comic-ai`'s `is_terminal` calls
  non-terminal, and under them `jobs/` is never empty in a quiescent project — exactly the case
  §8.3 says rule 18 reports. Name the statuses the rule means
the_jobs_lifecycle_says_what_a_job_file_does_to_the_manifest:
  §8.3 gives `jobs/` a lifecycle but never says what a job file created between saves does to
  `manifest.files`, and §14 step 1 attributes a `files` mismatch only to an interrupted save or
  hand-editing — a stale `files` map after a job starts is a third cause the document never names.
the_shared_types_section_pairs_each_type_with_its_own_section:
  §5.5 sends the reader to a section that does not hold the type it names there: `ContentHash`
  (sent to §7.1, in "`StorageRef` and `ContentHash` §7.1")
the_milestone_record_states_the_counts_the_frozen_document_has:
  the M0 record that will be committed beside the schema describes the frozen document
  incorrectly: M0.md says `twelve referential rules` but the schema has 18; M0.md says `four
  artboard-id mitigations` but the schema has 5
```

Also clean on the extended target: `cargo fmt -p vectorcraft-doc -- --check` (exit 0 after
`cargo fmt -p vectorcraft-doc`; the only file it touched is the untracked test file) and
`cargo clippy -p vectorcraft-doc --test schema_claims` (0 diagnostics).

Findings:      6 — 1 `contradiction`, 2 `under-specified`, 3 `style`

Line numbers are the current `docs/comiccraft-project-schema.md` (658 lines) unless the file is
named.

1. `:443` (§8.3, with rule 3 `:593`) — claims: the job file "hold[s] one `GenerationJob` with the
   same shape as §8.1", and rule 3 enumerates "every `jobs/job-<id>.json`" among the files that
   declare `schema_version`. True: §8.1's field list (`:420-426`) has no `schema_version` and §8.3
   shows no wrapper, while §5.4 `:179`, §6 `:272`, §7 `:373` and §8 `:415` each show their file's
   `{ "schema_version": 1, … }`. A writer cannot tell the file's bytes — a bare job, a job plus a
   version key, or a `{ schema_version, job }` wrapper — and rule 3's check and §8.3's shape cannot
   both hold. **contradiction** (new in the round-4 fix; in a document whose whole subject is the
   frozen wire format; pinned by a red test).
2. `:608` (rule 18, with §8.3 `:446-447`) — claims: a job file is legal iff its `status` "is **not
   terminal**", and the file is removed "when a job reaches a terminal state", "so `jobs/` is empty
   in a quiescent project". True: neither sentence says which of the six `JobStatus` values
   (`:422-423`) is terminal, and the workspace's own definition — `crates/comic-ai/src/lib.rs:61-63`,
   `is_terminal() = matches!(self, Failed | Cancelled)`, whose doc comment freezes it against
   "schema §8.1" — makes a `completed`/`awaiting_review` file legal. Under it a finished generation
   never leaves `jobs/`, so §8.3 `:447`'s "empty in a quiescent project" is false and rule 18 never
   reports the stale file §8.3 `:448` promises it reports; under the other reading (`completed` is
   terminal) it reports a legal bundle, which is rule 1's round-4 defect's class. **under-specified**
   (pinned by a red test).
3. `:446-447` (§8.3) with `:633-640` (§14 step 1) — claims: the file exists for an interrupted
   session to report what was running, and a `files` mismatch "means the bundle was interrupted
   mid-save (§10) or hand-edited". True: a job file is written when the job starts, outside §10's
   save, and §4 `:118` makes `manifest.files` cover every file except the manifest, `exports/` and
   `.save-tmp/` — so a job file created or removed since the last save is a third cause of a
   mismatch that no sentence names, and nothing says whether it joins `files` at once or at the
   next save. The load path stays unambiguous (report and continue), so this is a note.
   **under-specified** (pinned by a red test).
4. `:202` (§5.5) — claims: the types used by one section are defined in that section, in the
   document's own pairing convention, here "`StorageRef` and `ContentHash` §7.1". True: `StorageRef`
   is §7.1 and `ContentHash` is §7.2 (`:402-408`), and §7.1's body never names `ContentHash` — the
   one pair of the five that splits across two subsections carries one number, where §5.3/§6.6,
   §6.2/§6.5 and §8 are right. A reader sent to §7.1 finds §7.2 directly below, so nothing is
   implemented wrongly. **style** (in the round-4 fix's own new sentence; pinned by a red test).
5. `:474` (§9.1, against design `:606-629`) — claims: "Five mitigations, all mandatory (design
   §5.3.1)". True: design §5.3.1 says "**Four** mitigations are mandatory" and folds the schema's
   mitigation 4 into its own item 2 ("Retired ids are remembered, and **never reused**"), numbering
   the upstream fix 4 where the schema numbers it 5. Every guard is in both lists and the schema's
   internal numbering is right (`mitigation 4 below` `:470` resolves to the reuse guard, and the
   fallback `:493-494` covers 1–4), so an implementer builds the same set either way — but the
   citation's count and numbering are not the design's. **style** (round-4 finding 1's residue; not
   pinned by a test: whether the schema splits the design's item 2 in two is the owner's call, and
   a test would decide it).
6. `docs/milestones/M0.md:54-55` (§2.3), with `:19` and `:175` — claims: the frozen schema has
   "the four artboard-id mitigations … the twelve referential rules", and "the schema document
   still needs its two rounds". True: §13 defines 18 rules and §9.1 lists five mitigations, and the
   schema has now had five rounds (this record). The milestone record — not the schema — is what
   will be committed, and it describes a document two revisions old in its own summary of the
   artifact (`:19`'s status row and `:175`'s "Not done" list make the same stale claim). No
   implementation follows from it, so it is a note, but it should be corrected before the commit.
   **style** (pinned by a red test that reads M0.md's spelled counts; the `two rounds` status lines
   are the lead's text, which I report rather than edit).

Round-4 verification: the 9 findings

| # | Round-4 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | §9.1 said "Four" and listed five, and its fallback excluded the added mitigation | **verified fixed** | §9.1 `:474` reads "Five mitigations, all mandatory (design §5.3.1)", items 1–5 are `:476-492`, and the fallback `:493-494` now closes on mitigation 5 ("Until it lands, mitigations 1–4 stand alone: 5 would remove the *cause* … it would not replace the four guards"). `the_mitigation_list_matches_its_count_and_its_fallback` is green: it derives the four guards from the item list minus the upstream fix and requires the range to cover them. Residue is finding 5, and it is the citation, not the list. |
| 2 | §8.3 cited rule 1 for a `jobs/` status check | **verified fixed** | §8.3 `:448` now says "a stale file there is an inconsistency **rule 18** reports"; rule 18 `:608` is the new rule and governs `jobs/`; the round-3 test `every_directory_in_the_bundle_layout_is_defined_somewhere` and round-4's `the_stale_jobs_file_is_reported_by_a_rule_that_governs_it` are green. The rule was added rather than the citation reworded, which is the fix the test implied. **What the fix opened**: the file rule 18 governs has no shape (finding 1) and its predicate is undefined (finding 2). |
| 3 | rule 9's trailing note re-flattened the three bases | **verified fixed** | The note is now `### 13.1` (`:610-615`) and reads "Rule 9 must **confine every path to its own base** before reading — `Linked` to the bundle root, `Package` to the archive root, `LinkInfo.relative` to the document folder", plus "There is no single "the bundle root" rule; a note that collapsed them would undo rule 9". `the_rule_9_note_does_not_flatten_the_bases_rule_9_splits` is green. |
| 4 | §13's table ended after rule 9, so rules 10–17 rendered as prose | **verified fixed** | Rows `:589-608` are one contiguous block — header plus 18 rows, no blank line, no embedded newline, no stray pipe character — and rule 9's own last sentence ("A `Linked` file must exist and its sha256 must match `content_hash`") is inside rule 9's cell `:599`. Rule 9's two caveats moved to §13.1 `:617-620`. `section_13s_rule_table_is_one_table` is green (it compares the table's rows to the rule numbers in table order). |
| 5 | §5.5's completeness claim was false, in a wider way after the round-3 fix | **verified fixed as a claim**, with a defect in its replacement | The old "Every type **this document defines** appears here" and "A type that is in neither place is a defect in this document" are gone; §5.5's intro `:197-204` scopes itself to "the value types the tables **in this document** reference", names the identifier newtypes and inline enums as defined where used, and lists nine types with the section that defines each — eight of the nine pairs are correct. The ninth is finding 4. Note: `section_5_5_defines_the_types_it_claims_to` is now a *guarded* test — it returns early at `:830-832` because the sentence it pinned is gone — so it can no longer fail on this revision and asserts nothing about §5.5; the new test is what pins the replacement sentence. |
| 6 | rule 1 required `null` for nullable keys, like rule 12 before its fix | **verified fixed** | `:591` now reads "A key that is **`null` is not a reference** and is skipped, exactly as rule 12 does for a narration speaker", and adds "**`retired_artboard_ids` is not a reference list**: it holds artboard ids that must *not* resolve (rule 4)". `rule_1_carves_out_nullable_keys_like_rule_12` is green. |
| 7 | design `:623` cited schema §5.1.1, which does not exist | **verified fixed** | design §5.3.1 `:623` now reads "(schema §5.1, rule 15)", and both resolve — `every_section_cited_across_the_two_documents_exists` (the cross-document walk) is green. Residue, not a finding: the ground of the sentence ("no field holds a second copy to compare against") is stated at §5.3 `:179-180` ("never a second copy of a page's size") and §6.1 `:293` ("no `page_size`"), one hop from §5.1 through its `settings` row; a reader following the cite reaches it, so nothing is implemented wrongly. |
| 8 | design §12.1 named "print presets", which neither document defines | **verified fixed** | design `:1297` now reads "the artboard `rect` against `ProjectSettings.default_page_size`, as a warning (schema rule 15)" — one baseline, the schema's — and "print preset" no longer occurs in either file. `the_page_dimension_check_names_a_baseline_the_schema_defines` is green, and rule 15 `:605` and §15 S1 `:655` agree with it. |
| 9 | §5.7 fixed struct-field order but not map-valued fields | **verified fixed** | §5.7 `:258-262` adds "**Map-valued fields** (`unknown`, `provenance`'s `request: Value`, `manifest.files`) are the exception to declared order: their keys are written **sorted**, because `serde_json` sorts a `Map` … A struct field's order is declared; a map's is sorted. Both are canonical." Verified against the workspace: `Cargo.toml:49` enables `float_roundtrip` and not `preserve_order`, and §4 `:118` declares `files` as a `BTreeMap`. `section_5_7_covers_map_valued_fields` is green, and the round-2 test demonstrates the sorted write with the workspace's own `serde_json`. |

Structural check

- **The table.** `| # | Rule |` at `:589` plus 18 rows to `:608`, contiguous, one blank line before
  `### 13.1` at `:610`. `referential_rule_numbers`, `every_referential_rule_is_numbered_once_and_contiguously`
  and `no_directory_or_rule_is_defined_twice` are green: the set is 1..=18, no gap, no duplicate.
  No cell contains a raw newline, a blank line or a literal `|`.
- **Rule 18's interaction with §4 and §14.** §4 `:118` makes `files` "every file except the
  manifest **and except `exports/` and `.save-tmp/`**", so a `jobs/` file is a member by
  construction; §14 step 1 `:633-640` compares the tree "**as §4 defines it**"; §14 step 5 `:642`
  runs "Referential rules **1–18**". The membership rule and the load order are therefore
  consistent with rule 18, and `the_files_tree_comparison_excludes_what_files_excludes` and
  `the_load_sequence_runs_every_referential_rule` are green. What is *not* defined is when the file
  appears and which statuses it may hold — findings 2 and 3.
- **Every rule citation.** `:97` (rule 9), `:180` (3), `:242` (10), `:243` (9), `:394`/`:400` (9),
  `:448` (18), `:484` (15), `:487` (17), `:580` (1), `:591` (12, 4), `:599` (§3), `:615` (9),
  `:655` (15) — all resolve to a rule §13 defines
  (`every_rule_reference_in_the_schema_names_a_rule_that_exists` is green), and the design's
  citations — "(schema rule 17)" `:619`, "(schema §5.1, rule 15)" `:623`, "(schema rule 15)"
  `:1297` — resolve too. The rule that performs the check it is cited for is right in every case
  this round could test; round 4's two wrong-citation classes (F2, F3) are the only ones that ever
  existed and both are closed.
- **§13.1 is correct and complete for the limits it claims.** It names all three bases, states the
  hard-link case as accepted with integrity resting on `content_hash`, states the canonicalise/open
  TOCTOU with `O_NOFOLLOW` plus a re-check and documents the residual window, and explicitly
  refuses the collapsed reading. Its one imprecision — "must be rejected, not resolved" can be
  read as a fatal rejection where rule 9 `:599` and §14 `:644-647` make an escaped path a finding
  on a usable project — is resolved by the sentence before it ("confine every path to **its own**
  base") and by rule 9's own "never resolved … the rest of the load continues", so I do not raise
  it as a finding.
- **§9.1's five mitigations against the design's four** — finding 5; the guard set is identical.

Security

Rule 9 `:599`, §13.1 `:610-620` and §14 `:633-647` are unchanged from round 4 and still the
document's strongest passage: resolve against the **declared** base, canonicalise **before** the
confinement check (so `..` and symlinks are defeated without a denylist), the escaping path is
never resolved and the load continues, `.save-tmp/` is never a source, `Linked` files are hash-checked
against `content_hash`, and hard links and the canonicalise/open race are recorded rather than
papered over. Round 4's two security-shaped residues are closed: rule 9's hash sentence is inside
rule 9's cell again (so a validator built from the rendered table no longer loses it) and §13.1 does
not re-collapse the bases (so `Package` is not confined to the wrong root). §8.1 `:428-429`
("**Credentials never appear here.**") is unchanged, still a positive control. The `jobs/` seam
(findings 1–3) is a wire-format and lifecycle gap — the file's bytes and its status predicate — not
a path-traversal or secret-handling hole; nothing in it lets a path escape or a credential into the
bundle.

Defects vs notes

- **Defects (blocking, `changes-requested`):** **finding 1** — a contradiction inside the frozen
  wire format: rule 3 requires a version on a file whose shape has nowhere to put one, so two
  implementers produce two bundles and the v1 fixture cannot be authored unambiguously; and
  **finding 2** — a normative rule (`rule 18`) whose predicate the document does not define, whose
  only in-repo definition (`crates/comic-ai`'s `is_terminal = Failed | Cancelled`) makes it dead
  for the case §8.3 promises and contradicts §8.3's "empty in a quiescent project". Both are
  decisions a document can and must make; both are one clause, but a reader following the document
  as written builds the wrong bytes or the wrong rule.
- **Notes, not blockers:** finding 3 (a `files`-staleness cause §14 does not name — the load is
  still unambiguous), finding 4 (`ContentHash` paired with §7.1 — one subsection off, resolvable in
  one hop), finding 5 (§9.1's count and numbering against the design's four — the same guard set),
  finding 6 (the M0 record's stale counts and status — a record to correct before the commit,
  outside the schema/design pair but inside the deliverable).
- **Carried notes** (unchanged this round, none blocking): §9.1 mitigation 4's mechanism (a skipped
  artboard discarded re-derives the same id, and kept it is an artboard that is not a page against
  §3's "artboards are pages"); §11.1's "1 job" with no stated home in `jobs/` or `provenance/`;
  §12's undefined term "entity" (`:579-580`) and the six closed shapes it says carry no `unknown`;
  §5.6 `:241`'s "§9.3/Q13" without naming the design as the document those sections belong to; and
  the ungoverned `manifest.bundle_id`, `manifest.title`, `ComicProject.settings`,
  `Page.page_number` and `Panel.generation_status` — none of which can make a legal bundle
  invalid, so none is a finding.

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest (`crates/doc/Cargo.toml` is untouched — the new tests read
the code and the documents as text rather than taking a dependency edge that would put
`comic-ai` behind `doc`). Run on the changed target instead:

```
$ cargo fmt    -p vectorcraft-doc -- --check              # exit 0, no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims    # 0 diagnostics
$ cargo test   -p vectorcraft-doc --test schema_claims    # 36 passed; 5 failed (the reproductions)
```

A full `cargo xtask ci` would fail at its test step on the 5 red tests; that is the intended
evidence, not an accident.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set and the
  session store records that model for 630 of its 632 rows, but **0** rows name
  `76616a76-f1d0-4707-9c7f-d1ec93730dc2`, so nothing in the environment attests the backend of the
  model answering this review.
- The `jobs/` file against any implementation: `crates/comic-model` is still `SCHEMA_VERSION` +
  `ComicId` only, so `GenerationJob`, `ReviewDecision` and the job file do not exist as types and
  there is no bundle to load. Contra rounds 2–4's note, `crates/comic-ai` **does** now hold the
  schema's `JobStatus` with `is_terminal`/`has_result` (M0's schema-derived item, 148 lines) — I
  read it and its seven tests, plus `crates/comic-ai/tests/job_status_contract.rs`, whose round-1
  fix asserts the partition outright (`terminal == [Failed, Cancelled]`,
  `with_result == [Completed, AwaitingReview]`, `pending_no_result == [Queued, Running]`) — and I
  did not run a validator against a bundle, because none exists.
- The M0 record's status lines (`:19`, `:175`, `:214`) that still list the schema's review rounds as
  outstanding: that is the lead's text about the process, not a claim about the format, so I
  reported it in finding 6 without editing it or pinning it with a test.
- Whether the owner wants the job file to be a bare `GenerationJob` plus a version key, a
  `{ schema_version, job, unknown }` wrapper, or something else; and whether "terminal" is meant as
  the code's `Failed | Cancelled` (in which case §8.3's "empty in a quiescent project" and its
  promise about rule 18 need rewording, and the file survives a successful generation) or as "the
  generation has finished" (in which case `crates/comic-ai::JobStatus::is_terminal` and rule 18
  disagree, and M1 needs a second predicate). Both are decisions for the owner and the lead; I
  reported the gaps rather than choosing.
- The design's `jobs/` lifecycle beyond what it says: design `:731`/`:1185` call the directory
  durable metadata for in-flight jobs and `:1049` has a completed job "stay `awaiting_review` until
  a `ReviewDecision` approves or rejects it", but the design defines neither "terminal" nor the
  file's shape, so the document the meaning is deferred to does not decide finding 2.
- The guarded path of the new §5.5 test: if the sentence is rewritten without the
  "that section's own table (…)" construction, the test returns early and stops asserting the
  pairing, the way `section_5_5_defines_the_types_it_claims_to` and
  `the_rule_9_note_does_not_flatten_the_bases_rule_9_splits` now do. Those two cannot fail on this
  revision: they guard a regression rather than assert a claim.
- Exactly which lines the round-4 fix edited: both documents are untracked, so no `git diff`
  against the round-4 revision exists. I verified the current text (and the design's mtime moved
  22:30 → 22:46 at the same 1660 lines), not the delta. The mutation check above is mine and was
  reverted; md5 `55bb7edd75deb3a1c7b16ee8ac71fdfc` confirms the schema is the round-4 revision.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: still no fixture and no code
  exercise it; it is verified as text.

## Round 6 — closing verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set
               in the environment (`AI_AGENT=github_copilot_vscode_agent`), the same in-environment
               statement rounds 2–5 relied on. `~/.copilot/session-store.db ::
               assistant_usage_events` still holds **632** rows (630 of them `deepseek-v4.1-flash`)
               and **0** rows for this session (`2f47f257-8e95-40c0-b39f-09bf6487b2ae`), so the
               model claim rests on the environment plus the harness record, not on an attestation
               of the serving backend.
Revision reviewed:
               **working tree after the round-5 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` untracked, **670 lines**, md5
               `c0968b9bbaaaa023e625ef42e7aa9395` (round 5 reviewed the 658-line text at md5
               `55bb7edd75deb3a1c7b16ee8ac71fdfc`); `docs/comiccraft-design.md` **1662 lines**, md5
               `c1ec2995a41aeb8e91abdef244ee56cb` (round 5 recorded 1660 lines at md5
               `1a173ad4cddf6ebf3c3061dec27a6bc8`); `docs/milestones/M0.md` **373 lines**, md5
               `7bcb20bec2a9b904f62c587fa5e44938`. The revision moved in every place the round-5
               findings named: §8.3 shapes the job file as a wrapper, §8.3 and rule 18 name the
               terminal statuses, §8.3 states the third `files` cause, §5.5 pairs `ContentHash` with
               §7.2, the design §5.3.1 list is five items, and the M0 record states the schema's
               counts. This is not a re-run of round 5.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 670 lines), `docs/comiccraft-design.md`
               (the `jobs/`/`GenerationJob`/`ReviewDecision` prose §9.4/§10.3 `:1040-1060`,
               `:1170-1196`, the bundle tree `:725-735`, §5.3.1 `:598-633` and §20's risk table
               `:1640-1660`), `docs/comiccraft-porting-plan.md` §5.2 `:291-307` and §7 `:383-434`
               (the plan's own mitigation count and M1 acceptance tests),
               `docs/milestones/M0.md`, `crates/doc/tests/schema_claims.rs`,
               `crates/comic-ai/src/lib.rs` (the `is_terminal`/`has_result` predicates),
               `crates/comic-ai/tests/job_status_contract.rs`, `crates/comic-model/src/lib.rs`,
               and `docs/reviews/M0-schema-document.md` (rounds 1–5)
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **4 new tests** plus four helpers —
               `snake_case`, `code_terminal_statuses`, `mitigation_items`, `mitigation_ranges` —
               and the `JOB_STATUSES`/`COMIC_AI_LIB` constants; **no existing assertion changed,
               none deleted, none weakened**; `cargo fmt -p vectorcraft-doc` reformatted the new
               block only, and one helper was relaxed mid-round when it mis-parsed a wrapped list
               item, not when any claim failed)
Design refs:   §1 of the schema (the design decides the meaning), §4, §5.4, §5.5, §8.1, §9.4,
               §10.3; schema §2, §3, §4, §5.5, §8.1, §8.3, §9.1, §13 rules 3/9/18, §13.1, §14,
               §15 S1

Verdict:       **changes-requested**

**All six round-5 findings are addressed.** The two defects are fixed where they were raised and
the four notes are fixed too — §8.3 now fixes the job file's bytes and rule 18's predicate is
decidable and equal to the code's — and I verified each by reading the fix and by a test that
fails when the fix is undone. **The fixes introduced one defect, and it is the same class rounds
4 and 5 caught twice: a count that no longer matches its list.** Round 5's note-5 fix did not just
re-word the design's §5.3.1 — it *split* the old item 2 ("Retired ids are remembered, and never
reused") into item 2 and item 4 and renumbered the list from four items to five. Design §5.3.1,
its closing sentence and the schema's §9.1 are now mutually consistent at five with the guards at
2, 3 and 4, and the upstream proposal at 5. **Design §20's risk table was not updated**: R2's
action row still reads "Mitigations 1–3 are mandatory in M1" (`docs/comiccraft-design.md:1650`),
which after the split names an M1 mandate set that **excludes item 4** — the never-reuse guard
that §9.1 says is what stops a deleted page's artboard id from re-pointing at a new one, and that
both documents call mandatory. Under the *old* numbering R2 was correct (items 1–3 were the
guards; item 4 was the upstream fix), so this is a contradiction the fix created, not one it
inherited. It is one word, but it is the design's own statement of what M1 must build, so it is a
defect and not a note. Everything else round 6 could test on the frozen wire format holds: the
job file's bytes are now determined, rule 18's predicate is decidable and equal to
`JobStatus::is_terminal`, rule 3's check and §8.3's shape agree, and no citation dangles.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **44 passed; 1 failed** (45 total). Rounds 1–5's 41 tests are all green, which is
               this round's evidence that the round-5 fixes are in the text; the 4 new tests are
               round 6's own pins. The one failure is the reproduction of the defect above; it goes
               green the moment R2's range is corrected, and I verified that by mutating in place
               and reverting (md5 `c1ec2995a41aeb8e91abdef244ee56cb`,
               `c0968b9bbaaaa023e625ef42e7aa9395` after restore).

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 45 tests
... 44 ok ...
test every_mitigation_range_covers_the_guards_the_documents_call_mandatory ... FAILED
test result: FAILED. 44 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure text, verbatim:

```
every_mitigation_range_covers_the_guards_the_documents_call_mandatory:
  the mitigation list was renumbered but a reference to it was not:
  docs/comiccraft-design.md:1650 says "| R2 | `Artboard.id` reuse after a delete (§5.3.1) |
  Silent page-identity drift | Mitigations 1–3 are mandatory in M1; push U1 upstream |" —
  every reference has to cover the guards 1–4
```

Mutation checks (each applied in place, run, then reverted; both documents restored byte-for-byte,
md5 confirmed). A test that cannot fail is not evidence, so each new test was shown to fail on the
behaviour it claims to pin:

| Mutation | Test that caught it | Result |
|---|---|---|
| design R2 `1–3` → `1–4` (the fix) | — | **45 passed; 0 failed** |
| rule 18 "`failed` or `cancelled`" → "`failed`" | `rule_18s_terminal_statuses_are_the_ones_the_code_calls_terminal` | FAILED |
| §8.3's wrapper literal → "a bare `GenerationJob`" | `the_jobs_file_wrapper_carries_the_schema_version_rule_three_checks` | FAILED |
| design §5.3.1 "Five mitigations" → "Four mitigations" | `the_design_and_the_schema_state_the_same_mandatory_mitigations` | FAILED |

Also clean on the extended target: `cargo fmt -p vectorcraft-doc -- --check` (exit 0) and
`cargo clippy -p vectorcraft-doc --test schema_claims` (0 diagnostics). The code side of rule 18
was run as well: `cargo test -p vectorcraft-comic-ai` — **7 inline + 8 contract tests, all pass**,
including `the_predicates_partition_every_status` (`terminal == [Failed, Cancelled]`,
`with_result == [Completed, AwaitingReview]`).

Findings:      5 — 1 `contradiction`, 2 `under-specified`, 2 `style`

Line numbers are the current `docs/comiccraft-project-schema.md` (670 lines) unless the file is
named.

1. `docs/comiccraft-design.md:1650` (§20, risk table R2) — claims: "**Mitigations 1–3 are
   mandatory in M1**; push U1 upstream". True: §5.3.1 `:606` says "**Five** mitigations are
   mandatory" and its items are `:610-631` (2 remembered, 3 validation, **4 never reuse**, 5
   upstream), its closing sentence `:631` says "mitigations **1–4** stand on their own", and schema
   §9.1 `:486-505` says the same five and "1–4 stand alone". R2's `1–3` therefore names an M1
   mandate set that omits mitigation 4 — the guard §9.1 says keeps rules 4 and 17 from turning the
   hazard into a permanent finding. Under the pre-fix numbering (four items, item 2 folding
   "remembered" and "never reused") R2 was correct; the round-5 split moved the guard to item 4 and
   left R2 behind, so the fix created this. **contradiction** (pinned by a red test).
2. `docs/comiccraft-porting-plan.md:306` (§5.2) — claims: "The artboard-id reuse hazard and its
   **four** mandatory mitigations are in design §5.3.1". True: the design and the schema now state
   five. The sentence sends the reader to §5.3.1 and implies no set of its own, so a reader who
   follows it builds the five; a stale count, not a scope change. **style** (same one-word fix as
   finding 1; reported rather than pinned, because a red test would make it a blocker rather than
   the note it is).
3. `:645-652` (§14 step 1, with §8.3 `:457-460`) — claims: a `files` mismatch "means the bundle was
   interrupted mid-save (§10) or hand-edited". True: §8.3 now names a **third, benign** cause —
   "A job file is written **while the app runs**, outside the §10 save protocol, so it is in
   `manifest.files` (§4) and a job started since the last save makes those hashes stale by design.
   That is a third, benign cause of the `files` mismatch §14 step 1 reports". §14's own sentence was
   not updated, so the document's two statements about the cause set disagree. The operative rule is
   identical under both — report and continue — so nothing is implemented differently. Round 5's
   finding 3 asked for a sentence that names the cause, and that sentence now exists; this is its
   residue. **under-specified**.
4. `:452-455` (§8.3) — claims: "`jobs/` … is empty once every job has failed, been cancelled, been
   applied or been rejected — and a file whose status *is* terminal is the inconsistency rule 18
   reports". True: rule 18 `:620` forbids only `failed`/`cancelled` files, so a `completed` or
   `awaiting_review` file left in `jobs/` is legal, and `JobStatus` (`:420-423`) has no
   `applied`/`rejected` value — nothing says what status an applied or rejected job carries or
   which command removes its file. Rule 18 is one-directional and stays decidable; the emptiness is
   a writer expectation the document does not state a mechanism for. **under-specified**.
5. `docs/milestones/M0.md:3`, `:19`, `:175`, `:214` — claims: the milestone is "Incomplete … the
   schema-document review is outstanding", criterion 5 is "partial — the schema document's two
   rounds are outstanding", and §5 lists "Both rounds of review for
   `docs/comiccraft-project-schema.md`" as not done. True: the schema has now had **six** recorded
   rounds (this record) and the counts in §2.3 `:54-55` were corrected this round. This is the
   lead's process text about the review, not a claim about the format; it is a record to update when
   M0 is committed, not a rule to implement. **style** (reported, not pinned — round 5 made the same
   call on the same lines).

Round-5 verification: the 6 findings

| # | Round-5 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | **Defect**: the job file's bytes were undetermined — rule 3 required a `schema_version` on a file whose shape had nowhere to put one | **verified fixed** | §8.3 `:441-445`: "One file per job, `jobs/job-<JobId>.json`, holding a wrapper `{ "schema_version": 1, "job": { … }, "unknown": {} }` whose `job` is a §8.1 `GenerationJob`. The wrapper is the same shape every other file uses (§5.4, §6, §7, §8), which is what lets rule 3 check this file's version — §8.1's entity has no `schema_version` of its own." Rule 3 `:605` still enumerates `jobs/job-<id>.json`; §8.1 `:420-427` has no version field; §5.4 `:179`, §6 `:272`, §7 `:373` and §8 `:415` show the same wrapper shape, so the wildcard is honest. `the_jobs_file_declares_the_schema_version_rule_three_checks` is green, and my new `the_jobs_file_wrapper_carries_the_schema_version_rule_three_checks` fails when the wrapper is replaced by a bare job (mutation 3). Two implementers now write the same bytes. |
| 2 | **Defect**: rule 18's predicate ("not terminal") was undefined, and the code's `is_terminal` made it dead for the case §8.3 promised | **verified fixed** | §8.3 `:448-455` and rule 18 `:620` both name it: "Terminal is exactly `failed` or `cancelled` (`JobStatus::is_terminal`, `crates/comic-ai/src/lib.rs`); `completed` and `awaiting_review` are not terminal", and §8.3 now says "`jobs/` is not empty in a quiescent project — it is empty once every job has failed, been cancelled, been applied or been rejected", so the over-promise is gone. `crates/comic-ai/src/lib.rs:61-63` is still `matches!(self, Self::Failed | Self::Cancelled)`; my `rule_18s_terminal_statuses_are_the_ones_the_code_calls_terminal` reads the predicate out of the source, compares it with the statuses rule 18 names (both `[cancelled, failed]`), and fails when rule 18 drops `cancelled` (mutation 2). `rule_18_names_the_statuses_that_leave_the_jobs_directory_empty` is green. |
| 3 | Note: nothing said what a job file created between saves does to `manifest.files`, and §14 step 1 knew only two causes | **verified fixed**, with a residue | §8.3 `:457-460` names the third, benign cause and says §14 step 1 reports it. `the_jobs_lifecycle_says_what_a_job_file_does_to_the_manifest` is green. The residue is finding 3 this round: §14 step 1's own sentence still lists two causes. |
| 4 | Note: §5.5 sent the reader to §7.1 for `ContentHash` | **verified fixed** | §5.5 `:202` now reads "`StorageRef` §7.1, `ContentHash` §7.2, `MediaMetadata` §7", and §7.1 defines `StorageRef`, §7.2 `ContentHash`. `the_shared_types_section_pairs_each_type_with_its_own_section` walks every pair in the sentence and is green — the one split pair now carries two numbers. |
| 5 | Note: design §5.3.1 said "Four" and numbered the guards differently from the schema's five | **verified fixed in §5.3.1**, with a defect outside it | design `:606` reads "Five mitigations are mandatory … (the schema states the same five, rule 4 and rule 17)" and `:610-631` lists five with the schema's numbering (2 remembered, 3 validation, 4 never reuse, 5 upstream); `:631` says "mitigations 1–4 stand on their own". My `the_design_and_the_schema_state_the_same_mandatory_mitigations` checks both documents' stated count against their own list, the item each guard sits at (2 and 4), the upstream item last, and the closing range `[1, 4]` in both — green, and it fails when the design's count is changed (mutation 4). **What the fix opened**: §20's R2 still says "1–3" — this round's finding 1. |
| 6 | Note: the M0 record said "four artboard-id mitigations" and "twelve referential rules" | **verified fixed** | `docs/milestones/M0.md:54-55` now reads "the five artboard-id mitigations … the eighteen referential rules"; `the_milestone_record_states_the_counts_the_frozen_document_has` reads the schema's own counts and M0.md's spelled numbers and is green. Residue is finding 5 this round (the "two rounds outstanding" status lines, which round 5 also left to the lead). |

New-defect check

- **The design's list, its numbering, its closing sentence and the schema's five.** Mutually
  consistent at five: design `:606` count = five, its `:610-631` items = five, schema `:486`
  count = five, its `:488-505` items = five; the guards sit at 2, 3, 4 in both and the upstream
  proposal at 5 in both; both closing sentences say 1–4 stand alone; both cite each other
  correctly (`design §5.3.1` from the schema, `(the schema states the same five, rule 4 and rule
  17)` from the design, and every cited rule exists). The design no longer folds the schema's
  guard 4 into its item 2. **The one site that is not consistent is §20 R2 `:1650`**, which still
  carries the pre-split `1–3` — finding 1. The porting plan's "four" (`:306`) is the same origin,
  finding 2.
- **Rule 18's predicate vs the code, `has_result` and §8.1's status set.** Decidable: parse
  `jobs/job-<id>.json`, read `job.status`, reject `failed`/`cancelled`. Equal to the code:
  `crates/comic-ai/src/lib.rs:61-63` matches exactly those two, and the crate's own contract test
  asserts the partition (`terminal == [Failed, Cancelled]`, `with_result == [Completed,
  AwaitingReview]`, `pending_no_result == [Queued, Running]`). Consistent with `has_result`, not
  contradicted by it: the legal set is `{queued, running, completed, awaiting_review}` ⊇
  `has_result = {completed, awaiting_review}`, and no file rule requires a result to exist, so no
  status is both legal in `jobs/` and impossible. Consistent with §8.1 `:420-423`, which lists the
  same six values, and with §3's tree comment `:79` ("one file per in-flight job") now that
  "in flight" is defined as non-terminal. §14 step 5 runs rules 1–18 and rule 18 is numbered
  contiguously (`referential_rule_numbers` is green), so there is no gap or duplicate.
- **Did anything else move that should not have?** §13's table is still one contiguous 18-row
  table (`section_13s_rule_table_is_one_table` green), §13.1 still refuses the collapsed base
  reading, §5.7 still covers the map-valued fields, and `every_section_cited_across_the_two_documents_exists`
  is green, so nothing the earlier rounds fixed was re-broken by the round-5 pass.

Security

Rule 9 `:599`, §13.1 `:622-638` and §14 `:641-658` are unchanged from round 5 and remain the
document's strongest passage: resolve against the **declared** base, canonicalise **before** the
confinement check, never resolve an escaping path, `.save-tmp/` is never a source, `Linked` files
are hash-checked against `content_hash`, and hard links and the canonicalise/open race are recorded
rather than papered over. §8.1 `:428-429` ("**Credentials never appear here.**") is unchanged. The
round-5 fix is wire-format and lifecycle only: the job file's
wrapper carries no path and no secret, and rule 18's predicate is a status check. The `jobs/` seam
opens no path-traversal or secret-handling hole; findings 3 and 4 this round are imprecisions in
prose whose operative behaviour (report and continue; rule 18 one-directional) is unchanged.

Defects vs notes

- **Defect (blocking, `changes-requested`):** **finding 1** — design §20 R2 `:1650` "Mitigations
  1–3 are mandatory in M1" now excludes mitigation 4, while §5.3.1 `:606`/`:631` and schema §9.1
  `:486`/`:505` say five mitigations are mandatory and 1–4 stand on their own. It is a
  contradiction inside the design's own statement of the M1 scope, introduced by the note-5 fix's
  renumbering, and an M1 implementer who follows R2's row ships without the never-reuse guard that
  §9.1 says is what keeps rules 4 and 17 from turning the hazard into a permanent finding. The fix
  is one word (`1–3` → `1–4`), and the red test goes green when it lands.
- **Notes, not blockers:** finding 2 (the porting plan's stale "four" — the same one-word fix,
  but the sentence points at §5.3.1 and implies no set of its own), finding 3 (§14 step 1's
  two-cause sentence beside §8.3's third cause — the load path is identical), finding 4 (§8.3's
  "empty once … been applied or been rejected" names no status or command — rule 18 is unaffected),
  and finding 5 (the M0 record's "two rounds outstanding" status lines — the lead's text, to
  correct when committing).
- **Carried notes** (unchanged this round, none blocking): §9.1 mitigation 4's mechanism (a skipped
  artboard discarded re-derives the same id, and kept it is an artboard that is not a page against
  §3's "artboards are pages"); §11.1's "1 job" with no stated home in `jobs/` or `provenance/`;
  §12's undefined term "entity" (`:579-580`) and the six closed shapes it says carry no `unknown`;
  §5.6 `:241`'s "§9.3/Q13" without naming the design as the document those sections belong to; and
  the ungoverned `manifest.bundle_id`, `manifest.title`, `ComicProject.settings`,
  `Page.page_number` and `Panel.generation_status` — none of which can make a legal bundle
  invalid, so none is a finding.

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest (`crates/doc/Cargo.toml` is untouched — the new tests read
`crates/comic-ai/src/lib.rs` and the documents as text rather than taking a dependency edge that
would put `comic-ai` behind `doc`). Run on the changed targets instead:

```
$ cargo fmt    -p vectorcraft-doc -- --check              # exit 0, no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims    # 0 diagnostics
$ cargo test   -p vectorcraft-doc --test schema_claims    # 44 pass, 1 fail (finding 1)
$ cargo test   -p vectorcraft-comic-ai                    # 7 inline + 8 contract tests pass
```

A full `cargo xtask ci` would fail at its test step on the one red test; that is the intended
evidence, not an accident.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set and the
  session store records that model for 630 of its 632 rows, but **0** rows name
  `2f47f257-8e95-40c0-b39f-09bf6487b2ae`, so nothing in the environment attests the backend of the
  model answering this review.
- The `jobs/` file against an implementation: `crates/comic-model` is still `SCHEMA_VERSION` +
  `ComicId` only (69 lines), so `GenerationJob`, `ReviewDecision` and the job file do not exist as
  types and there is no bundle to load. I read the schema's counterpart code —
  `crates/comic-ai`'s `JobStatus` with `is_terminal`/`has_result`, its 7 inline tests and the 8
  contract tests, all green — and did not run a validator against a bundle, because none exists.
- Whether the owner *wants* R2 to say "1–4" or to restate the M1 set differently (the sentence
  could also be re-worded rather than renumbered); and whether §8.3's "been applied or been
  rejected" is meant to introduce a status transition the six-value `JobStatus` does not have.
  Both are decisions for the owner and the lead; I reported the gaps rather than choosing, and the
  red test asserts only that a range covers the guards, so any correct wording satisfies it.
- Exactly which lines the round-5 fix edited: both documents are untracked, so no `git diff`
  against the round-5 revision exists. I verified the current text and the size/md5 movement
  (658 → 670 lines in the schema, 1660 → 1662 in the design), not the delta. The mutation check
  above is mine and was reverted; md5 `c0968b9bbaaaa023e625ef42e7aa9395` and
  `c1ec2995a41aeb8e91abdef244ee56cb` confirm both documents are the round-5 revision.
- The guarded paths of `the_shared_types_section_pairs_each_type_with_its_own_section` and
  `section_5_5_defines_the_types_it_claims_to`: both still return early if their sentence is
  rewritten, so on this revision they guard a regression rather than assert a claim. My
  `the_design_and_the_schema_state_the_same_mandatory_mitigations` reads the *level* of each
  document's §5.3.1/§9.1 rather than a fixed line, so a reflow does not disarm it, but a rewrite
  that drops the phrase " mitigations" would.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: still no fixture and no code
  exercise it; it is verified as text.

## Round 7 — closing verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set in
               the environment (`AI_AGENT=github_copilot_vscode_agent`), the same in-environment
               statement rounds 2–6 relied on; `~/.copilot/session-store.db ::
               assistant_usage_events` still holds **632** rows (630 of them `deepseek-v4.1-flash`)
               and **0** rows for this session, so the model claim rests on the environment plus the
               harness record, not on an attestation of the serving backend.
Revision reviewed:
               **working tree after the round-6 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` **675 lines**, md5
               `93b52b9bdd5129fd90d18078101df58d` (round 6: 670 at `c0968b9b…`);
               `docs/comiccraft-design.md` **1662 lines**, md5
               `0bb79ad54251846a3c3a235159edecee` (round 6: 1662 at `c1ec2995…`);
               `docs/comiccraft-porting-plan.md` **1002 lines**, md5
               `44e03ee0566462696b7ff6fd4fb394f3`; `docs/milestones/M0.md` **373 lines**, md5
               `0ea62afc0b2c81e22d7bcc1c619b7564` (round 6: `7bcb20be…`). Every document moved where
               the round-6 findings named: R2's range, §14 step 1's third cause, §8.3's removal
               mechanism, the plan's five, and the M0 status line. This is not a re-run of round 6.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 675 lines), `docs/comiccraft-design.md` (§0's
               section ranges, §1, §3, §4.2/§4.3, §5.2/§5.3/§5.3.1, §6.1, §7.4, §8.3, §9.3/§9.4,
               §10.1/§10.3, §12.1/§12.3, §16/§16.1, §17–§20 including the risk table `:1649-1658`),
               `docs/comiccraft-porting-plan.md` (§3–§7, §15, §17), `docs/milestones/M0.md` (all 373
               lines), `crates/doc/tests/schema_claims.rs`, `docs/comiccraft-porting-audit.md`
               (§13's U1–U8), `crates/comic-ai/src/lib.rs`, `crates/comic-model/src/lib.rs`, and
               `docs/reviews/M0-schema-document.md` (rounds 1–6)
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **4 new tests** and five helpers —
               `count_noun_pairs`, `introduced_items`, `starts_a_block`, `question_citations`,
               `question_items`; **no existing assertion changed, none deleted, none weakened**).
               `cargo fmt -p vectorcraft-doc` reformatted the new block only. One new test was
               repaired twice during the round — a missing `i += 1` on a blank line spun the sweep
               forever (caught by watching the test binary burn CPU, not by a green run), and the
               citation test's first anti-vacuity guard would have rejected a legitimate fix; both
               were fixed before the run recorded below.
Design refs:   §1 of the schema (the design decides the meaning), §1, §3, §5.5, §5.6, §9.1, §13
               rules 1/9/15/17/18, §14; design §0, §1, §3, §4.3, §5.3.1, §6.1, §8.3, §9.3, §12.3,
               §16/§16.1, §19, §20 R1/R2

Verdict:       **changes-requested**

**All five round-6 findings are addressed where round 6 raised them — except note 5, which is not
fixed, and the task statement that says it is.** Round 6's one defect (design §20 R2's range) and
notes 2, 3 and 4 are verified fixed, by reading and by the tests that went green with them. Note 5
is **partially fixed**: `docs/milestones/M0.md:3` and `:19` were reworded to the truth (six rounds,
all defects fixed) but the two sites round 6 named, `:175` ("the schema document still needs its two
rounds") and `:214` ("both rounds for the schema document"), are unchanged, as is `:120`'s
`| pending | pending |` — which round 6 did not name — and `:212`'s "Complete the M1 hand-proof
(§5)", which now contradicts `:3`, §1's criterion 4 and §4a's own "**passed**". The record
therefore says the schema has had six rounds and the hand-proof passed *and* that both are
outstanding. I report that below as a note, not a defect, for the reason rounds 5 and 6 gave on the
same lines: it is the lead's status record, which this verdict requires the lead to rewrite anyway,
so it cannot mislead an implementer.

The sweep round 7 was sent to do found **three more of the drift class**, all of them in the frozen
pair and all now pinned by a test that goes green the moment they are fixed. The design's §16
introduces the ADR table with "ADR-011 … ADR-016 were taken or revised in this document" while that
table marks **seven** ADRs `(new)` — ADR-017, the lettering decision R1 is resolved by, is outside
the range; §16.1's "Decisions taken in this revision" stops there too. Design §19 ends by counting
what still has to be built — "There is still no …, no …, **no comic lettering font**, and no … —
those four are the real gaps" — while §8.3 and §20 R1 record the lettering faces as **Resolved
(ADR-017)**; one of the two statements is stale, and the sentence is the one that tells a reader
what M1 still has to build. And schema §5.6's consent row cites "§9.3/**Q13**": §9.3 exists, `Q13`
exists in no document (the design numbers its open items `R1`–`R10`, the audit `U1`–`U8`; searched
the whole tree, including the historical `comiccraft_design.html`), and §5.5 `:224` 17 lines
earlier cites the same requirement correctly as "design §9.3". Rounds 5 and 6 carried that row as a
note about the missing document qualification; with the target shown to be absent it is the bar's
dangling reference, so I have re-classified it and say so.

Everything else the sweep touched holds, and the sweep is now a test rather than a reading: 10
counted lists across the three documents, the section ranges, the 18 rules, the 5 mitigations, the
3 path bases, the 10 risks, the 9 milestones, the 12 M1 acceptance tests and the 20 top-level
design sections all agree with their contents. The security passage (§13 rule 9, §13.1, §14) is
unchanged from round 6 and remains the document's strongest part; the round-6 fix is wire-format
and lifecycle only. Nothing in this round's findings opens a path-traversal, secret-handling or
RCE hole — all three are prose and citation drift, and the fixes are one word, one clause and one
citation.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **46 passed; 3 failed** (49 total). Rounds 1–6's 45 tests are all green (that is
               this round's evidence that the round-5/6 fixes are in the text) and the 4 new tests
               are round 7's own pins: the generic sweep passes, the three defect reproductions fail.
               Each red test was shown to go green on its fix by mutating in place and reverting —
               the design was restored byte-for-byte (`0bb79ad5…`) after every mutation, including
               once after a mutation script died mid-run.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 49 tests
... 46 ok ...
test every_count_a_document_states_is_the_length_of_the_list_it_introduces ... ok
test every_question_citation_names_a_question_that_exists ... FAILED
test the_design_covers_every_adr_its_own_table_marks_new ... FAILED
test the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap ... FAILED
test result: FAILED. 46 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure text, verbatim:

```
every_question_citation_names_a_question_that_exists:
  a citation that names an item no document defines: docs/comiccraft-project-schema.md:241 cites
  `Q13` — the design numbers its open items `R1`–`R10` (§20) and the audit numbers its `U1`–`U8`,
  so a reader who follows `Q13` has nothing to find. Cite the item that holds the requirement
  (design §9.3, §5.5's own form) or drop the number
the_design_covers_every_adr_its_own_table_marks_new:
  §16's intro says "The audit is done, so these are decisions rather than recommendations.
  ADR-011 … ADR-016 were taken or revised in this document.", but its own table marks 7 ADRs
  "Accepted (new)" (ADR-011, ADR-012, ADR-013, ADR-014, ADR-015, ADR-016, ADR-017) — ADR-017 not
  named by the range, so a reader building the list of decisions this document took drops it
the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap:
  §19 lists "comic lettering font," among the real gaps, but the same document records the lettering
  faces as **Resolved (ADR-017)** — §8.3 "The two faces are now in `craft-fonts`" and §20 R1
  "tests green". A capability recorded as resolved is not still a gap: drop it and correct the
  count, or name where the faces come from
```

Mutation checks (each applied in place, run, then reverted; md5 confirmed after every one). A test
that cannot fail is not evidence, so each new test was shown to fail on the behaviour it claims to
pin, and each fix was shown to turn it green:

| Mutation | Test that caught it | Result |
|---|---|---|
| design §5.3.1 "Five mitigations" → "Six mitigations" | `every_count_a_document_states_is_the_length_of_the_list_it_introduces` | FAILED (the sweep is not vacuous; it reads 10 counted lists) |
| design §16 "ADR-016 were taken" → "ADR-017 were taken" (the fix) | — | `the_design_covers_every_adr_its_own_table_marks_new` → **passes** |
| design §19 drop "no comic lettering font" + "those four" → "those three" (the fix) | — | `the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap` → **passes** |
| schema §5.6 "§9.3/Q13" → "§9.3" (the fix) | — | `every_question_citation_names_a_question_that_exists` → **passes** |
| schema §5.6's consent row emptied of its `§9.3` citation | `every_question_citation_names_a_question_that_exists` (its `§9.3` guard) | FAILED — so the check cannot be disarmed by deleting the citation |

Also clean on the extended target: `cargo fmt -p vectorcraft-doc -- --check` (exit 0) and
`cargo clippy -p vectorcraft-doc --test schema_claims` (0 diagnostics). The code side of rule 18 was
re-run as well: `cargo test -p vectorcraft-comic-ai` — **7 inline + 8 contract tests, all pass**.

Findings:      5 — 3 `contradiction`, 2 `style`

Line numbers are the current `docs/comiccraft-project-schema.md` (675 lines), `docs/comiccraft-design.md`
(1662 lines) or `docs/milestones/M0.md` (373 lines) as named.

1. `docs/comiccraft-design.md:1487-1488` (§16's intro) — claims: "ADR-011 … ADR-016 were taken or
   revised in this document", and `:1510` §16.1's "Decisions taken in this revision" carries
   subsections for ADR-011 … ADR-016 and no other. True: the same section's table marks **seven**
   ADRs `Accepted (new)` — `:1502-1508`, ADR-011 through **ADR-017** — and ADR-017 is the decision
   R1's resolution rests on. A range that stops one short of the set it describes is the exact class
   round 6's finding 1 was, and this is its neighbour: an implementer building the list of decisions
   §16 took drops the one that governs lettering. **The fix is to extend the range** (and give
   §16.1's heading the same span, or say explicitly that ADR-017 is recorded in §8.3), which is what
   the new test accepts. **contradiction** (pinned by a red test).
2. `docs/comiccraft-design.md:1637-1639` (§19) — claims: "There is still no existing command
   registration seam, no networking dependency, **no comic lettering font**, and no storyboard or
   page navigator — those four are the real gaps." True: the same document records the lettering
   faces as **Resolved (ADR-017)** twice — §8.3 `:940` "The two faces are now in `craft-fonts`" and
   §20 `:1649` R1 "**Resolved (ADR-017):** … tests green". One of the two statements is stale. Of
   the four named gaps three are still gaps (`:1021-1022` ADR-015 leaves networking a decision,
   `:243-250` ADR-014 leaves the registration seam a divergence, and there is no storyboard or page
   navigator anywhere), so the count and the list disagree with the document's own record. A reader
   who follows §19 plans lettering-font work M0 has already finished. **The fix is to drop the clause
   and the count, or to say where the faces come from** — the test accepts either. **contradiction**
   (pinned by a red test).
3. `docs/comiccraft-project-schema.md:241` (§5.6's delta table) — claims: "`GenerationJob.consent`
   and `Provenance.consent` | ADR-015 requires informed consent before artwork is sent to a
   provider, and **§9.3/Q13** require it to be recorded." True: §9.3 exists and the design does
   require consent to be recorded there (`docs/comiccraft-design.md:1032`), but **`Q13` exists in no
   document** — the design numbers its open items `R1`–`R10` (§20), the audit numbers its `U1`–`U8`
   (§13), and a whole-tree search finds the string only in this row (and in an unrelated PNG's
   bytes). The schema's own §5.5 `:224` cites the same requirement correctly, as "design §9.3 and
   ADR-015", so the row has a model 17 lines above it. **The fix is to drop `/Q13` or name the item
   that actually holds it.** **contradiction** (a dangling citation; pinned by a red test). Rounds 5
   and 6 carried this row as a note about the missing document qualification; neither verified the
   target, and it does not exist.
4. `docs/milestones/M0.md:19` (§1's criterion 5 row) — claims: "Schema document: **six rounds**
   (13/19/12/9/6 findings)". True: the record this round appends to holds **six** rounds with
   **13/19/12/9/6/5** findings (round 6's five are missing), so the count and its list disagree —
   the same mechanical drift as findings 1–3, one file over. **style** (reported, not pinned: the
   record has to be rewritten on this verdict before the count can be final, and rounds 5 and 6 made
   the same call on the same file).
5. `docs/milestones/M0.md:3`, `:120`, `:175`, `:212`, `:214` — claims: the milestone is "Incomplete
   … crate reviews complete (2 rounds, approved), **M1 hand-proof passed** … the schema document has
   had six adversarial review rounds, all defects fixed" (`:3`), §1's criterion 2 is "**pass** —
   `docs/comiccraft-project-schema.md`, version 1" and criterion 4 "**pass**", and §4a is titled
   "The M1 hand-proof (porting plan §6 task 3) — **passed**". True: the same record says
   "`| pending | pending |`" for the schema's two review rounds (`:120`), "the schema document
   **still needs its two rounds**" (`:175`), "Round 2 of the code review on the post-fix revision;
   **both rounds for the schema document**" (`:214`), "**Complete the M1 hand-proof (§5)**"
   (`:212`), and "it is the last M0 exit criterion that is **unmet**" (`:174`). Round 6's note 5 is
   only half fixed — `:3`/`:19` were corrected and `:120`/`:175`/`:212`/`:214` were not, which the
   task statement's "no longer describes two outstanding rounds" does not match. **style** (the
   lead's status record; reported, not pinned, for the reason rounds 5 and 6 gave).

Round-6 verification: the 5 findings

| # | Round-6 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | **Defect**: design §20 R2's action row said "Mitigations 1–3 are mandatory in M1" after the §5.3.1 renumbering, excluding the never-reuse guard | **verified fixed** | `docs/comiccraft-design.md:1650` now reads "**Mitigations 1–4 are mandatory in M1** (§5.3.1 lists five; 5 is the upstream fix that would relax them); push U1 upstream" — the range covers the guards, and it names why 5 is outside it. `every_mitigation_range_covers_the_guards_the_documents_call_mandatory` is green, `the_design_and_the_schema_state_the_same_mandatory_mitigations` is green, and `the_mitigation_list_matches_its_count_and_its_fallback` is green, so the design's count (five), its items (`:610-631`), its closing range (1–4) and the schema's §9.1 all agree. |
| 2 | Note: `docs/comiccraft-porting-plan.md:306` said "four mandatory mitigations" | **verified fixed** | `:306` now reads "the artboard-id reuse hazard and its **five** mandatory mitigations are in design §5.3.1". The number matches the list the sentence points at, and the count the design states. |
| 3 | Note: §14 step 1 knew only two causes of a `files` mismatch | **verified fixed** | schema `:649-655` now reads "A mismatch means the bundle was interrupted mid-save (§10), hand-edited, **or a `jobs/` file written while the app ran outside the save protocol (§8.3) — the third cause is benign and expected**, so it is reported as an inconsistency and the load continues". §8.3 `:461-464` states the same third cause from the other side, so the two sentences about the cause set now agree; the residue round 6 reported is gone. |
| 4 | Note: §8.3 named statuses that do not exist (`applied`/`rejected`) and stated no removal mechanism | **verified fixed** | §8.3 `:452-459` now reads "A job's file is removed when the job becomes terminal (`failed`, `cancelled`), or when its result is **consumed** — `comic.job.apply` or `comic.job.reject` writes the `ReviewDecision` and removes the file. Rule 18 checks only the first half, because consumption is not visible in the status… A file whose status *is* terminal is the inconsistency rule 18 reports." No `applied`/`rejected` status remains, the two removal paths are stated, and both commands are real: design §10.1 `:1126-1127` lists `comic.job.apply` and `comic.job.reject`. `rule_18_names_the_statuses_that_leave_the_jobs_directory_empty` and `rule_18s_terminal_statuses_are_the_ones_the_code_calls_terminal` are green. |
| 5 | Note: the M0 record still described two outstanding review rounds | **partially fixed** | `docs/milestones/M0.md:3` and `:19` now say "six adversarial review rounds, all defects fixed" / "Schema document: six rounds … every defect fixed"; the two lines round 6 named, `:175` ("the schema document still needs its two rounds") and `:214` ("both rounds for the schema document"), are **unchanged**, as are `:120` (`| pending | pending |`), `:212` and `:174`. The record now contradicts itself about the hand-proof and the review status — finding 5 this round. The task statement's claim that the record "no longer describes two outstanding rounds" is not what the file says. |

New-defect check

- **The design's §16 range against its own table, and §16.1.** The table's seven `(new)` rows are
  contiguous (ADR-011 … ADR-017, no gap, no duplicate); the intro's range and §16.1's heading stop
  at ADR-016. Finding 1 is the only site; nothing else in §16/§16.1 contradicts the table, and
  §16.1's prose covers ADR-011 … ADR-016, which is consistent with its own span (only the *table's*
  status column claims ADR-017 is new, and R1's resolution cites it).
- **The design's gap list against its `Resolved` records.** §20's risk table is one contiguous
  10-row table (R1–R10); only R1 carries "Resolved", and §19 is the only place that still counts the
  lettering font among the gaps. Design §17's acceptance criteria and §12.1's check table name no
  counts, so there is nothing else to disagree with.
- **The schema's citations.** §5.6's consent row is the only `Q<n>` citation in either document, and
  `number_citations` finds no other unresolved `design §`/`schema §`/`schema rule` reference; §13's
  rule numbering is contiguous at 18 and every `rule N` citation resolves; §14 step 5 runs 1–18.
- **Did the round-6 fix break anything?** §14 step 1's new third cause is benign by its own words
  and by §8.3, the load path is identical, and rule 18's one-directional predicate is unchanged;
  `the_files_tree_comparison_excludes_what_files_excludes`, `section_13s_rule_table_is_one_table`
  and `every_section_cited_across_the_two_documents_exists` are all green. Nothing round 5 or 6
  fixed was re-broken.

Count/range sweep

Every count, range and enumeration in the four documents, against its contents. `✓` = agrees; the
three `✗` are findings 1–3. **Guarded** means a test now fails if the two drift apart again — the
new generic sweep covers the counted-list form (10 lists), the existing tests cover the rules,
mitigations, stores and section citations, and the three new tests cover the three sites below.

| Document | Site | States | Contents | |
|---|---|---|---|---|
| schema | §1 `:23` | two rules | 1, 2 | ✓ guarded (sweep) |
| schema | §3 `:79`, §8.3 `:441` | one file per in-flight job | `jobs/job-<id>.json`, lifecycle defined | ✓ guarded (`rule_18_names_the_statuses…`) |
| schema | §3 `:93` | **three** bases | `Linked`→bundle root, `Package`→archive root, `LinkInfo.relative`→document folder; `Embedded` not a path | ✓ guarded (`the_three_path_bases_are_named_wherever_a_base_is_claimed`) |
| schema | §5.3 `:163` | the two faces | Comic Neue, Bangers | ✓ |
| schema | §5.5 `:200-210` | the shared types, each paired with its section | every pair resolves | ✓ guarded (`the_shared_types_section_pairs_each_type_with_its_own_section`) |
| schema | §5.6 `:241` | "§9.3/Q13" | §9.3 ✓, `Q13` ✗ — no document defines a `Q13` | **✗ finding 3** — now guarded (`every_question_citation_names_a_question_that_exists`) |
| schema | §5.7 `:253-261` | six bullets, no count claimed | 6 | ✓ |
| schema | §7.1 `:388-404` | three `StorageRef` variants; "the two bases" a link converts between | 3 variants, 2 bases in that sentence | ✓ |
| schema | §9.1 `:490`, `:510` | **five** mitigations; "the four guards" | 5 items, guards at 1–4 | ✓ guarded (mitigation tests) |
| schema | §10 steps 4/5 `:516` | "Steps 4 and 5" | the protocol has exactly 5 steps | ✓ |
| schema | §11.1 `:568-570` | 1 chapter, 1 scene, 1 page, 3 panels, 2 balloons, 1 caption, 1 linked asset, 1 job | no fixture on disk yet (M1); the "1 job"'s home is the carried note | ✓ as text |
| schema | §12 `:592-595` | the closed shapes that omit `unknown` | 7 types listed, no count claimed | ✓ |
| schema | §13 `:606-624` | 18 rules | 18 contiguous | ✓ guarded (`every_referential_rule_is_numbered_once_and_contiguously`) |
| schema | §13.1 `:633` | **two** limits | hard link, TOCTOU | ✓ guarded (sweep) |
| schema | §14 `:649-659` | steps 1–5, step 5 runs **1–18** | 5 steps, 18 rules | ✓ guarded (`the_load_sequence_runs_every_referential_rule`) |
| schema | §15 `:670-675` | S1–S4 | 4 items, no count claimed | ✓ |
| design | §0 `:10-13` | sections 1–15, 16–18, 19–20 | 20 top-level sections, contiguous, no overlap | ✓ |
| design | §1 `:27` | **five** principles | 1–5 | ✓ guarded (sweep) |
| design | §3 `:89` | **six** subsystems | §3.1–§3.6 | ✓ |
| design | §4.2 `:251`, `:323` | three new crates, not twelve | 3 rows; "one of the three crates" | ✓ |
| design | §4.3 `:293` | two structural rules | 1, 2 | ✓ |
| design | §5.2 `:493` | two owners for one relationship | `Dialogue.balloon_id` + `Balloon.dialogue_id` | ✓ |
| design | §5.3.1 `:606`, `:631` | **five** mitigations; "1–4 stand on their own" | 5 items, guards at 2–4, upstream at 5 | ✓ guarded (mitigation tests) |
| design | §6.1 `:663` | **three** rules | 1–3 | ✓ guarded (sweep) |
| design | §7.4 `:826` | two hashes, two jobs | `LinkInfo.hash` FNV-1a, `Asset.content_hash` SHA-256 | ✓ |
| design | §10.1 `:1106-1129` | the command groups | 8 groups, no count claimed | ✓ |
| design | §12.1 `:1289-1300` | one check per row | 12 rows, no count claimed | ✓ |
| design | §12.3 `:1318` | **five** layers | 1–5 | ✓ guarded (sweep) |
| design | §16 `:1487-1488`, `:1510` | ADR-011 … ADR-016 taken here; §16.1 over the same span | the table marks **seven** new (011–017) | **✗ finding 1** — now guarded (`the_design_covers_every_adr_its_own_table_marks_new`) |
| design | §18 `:1590-1600` | 10 steps | 1–10, no count claimed | ✓ |
| design | §19 `:1637-1639` | "those **four** are the real gaps" | 4 items, but one is recorded Resolved | **✗ finding 2** — now guarded (`the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap`) |
| design | §20 `:1649-1658` | R1–R10 | 10 contiguous rows | ✓ |
| design | §16 `:1492-1508` | ADR-001 … ADR-017 | 17 contiguous rows | ✓ |
| plan | §2.3 `:100` | **three** rules | 1–3 | ✓ guarded (sweep) |
| plan | §3 `:119` | **four** logical layers | Layer A–D | ✓ |
| plan | §4 `:202`, `:234`, `:215-218` | three crates added, two surfaces extended, not twelve | 3 `NEW` crates; engine + ui-egui | ✓ |
| plan | §5.2 `:306` | **five** mandatory mitigations | 5 | ✓ (round 6's fix) |
| plan | §6 `:364`, `:375`, `:6` | three crate skeletons / three empty comic crates | 3 | ✓ |
| plan | §15 `:833-848` | one milestone per phase, M0–M8; M1 gate "the twelve acceptance tests" | 9 milestones; §7's tests are 1–12 | ✓ |
| plan | §13 `:748` | steps 2, 6, 7, 8 are engine features | 4 named steps of the 8-step pipeline | ✓ |
| plan | §17 `:969`, `:972` | three concerns, three module-tree owners, nine agents | 3 owners named; the ownership table holds 9 roles (+ the reviewer) | ✓ |
| M0.md | §1 `:19` | six rounds (13/19/12/9/6 findings) | six rounds: 13/19/12/9/6/**5** | **✗ finding 4** — note, not pinned |
| M0.md | §1 `:18`, §2.3 `:54-55`, §4 `:152` | three crates, five mitigations, eighteen rules, 59 tests | all match the schema/the suite | ✓ guarded (`the_milestone_record_states_the_counts_the_frozen_document_has`) |

What is **not** swept, and why: a claim of the form "every X is listed here" (§5.6's "a delta that
is not here is a defect in this document", §12's "every *entity* struct carries it") is a
completeness claim about the design's prose, not a count, and can only be checked by re-auditing
the design's §5.2 against the schema field by field — that is the §5.5/§5.6 class rounds 3 and 4
worked and is recorded as carried. It is not made harder by this round.

Security

Unchanged from round 6, and re-read: §13 rule 9 `:615` resolves against the **declared** base,
canonicalises **before** the confinement check, never resolves an escaping path, and refuses
`.save-tmp/` as a source; §13.1 `:633-641` keeps the hard-link limit and the canonicalise/open
TOCTOU window visible rather than papered over; §14 `:649-664` makes only a parse failure fatal.
§8.1 `:428-429` ("**Credentials never appear here.**") is unchanged, and §8.3's job file is a
wrapper with no path and no secret. Finding 3 is a citation, not a resolution path, and it opens
nothing: the row is prose about where a *consent record* lives, and the whole-tree search that
proved `Q13` absent touched no code. No finding this round can be turned into an exploit.

Defects vs notes

- **Defect (blocking, `changes-requested`):** findings 1, 2 and 3. Each is in one of the bar's named
  classes and none is a wording preference: 1 is a range that disagrees with the set it describes
  (ADR-017 inside the table, outside the range), 2 is a contradiction between §19's list of what
  must still be built and the two places the same document records the lettering faces as
  **Resolved (ADR-017)**, and 3 is a dangling reference (`Q13` in no document). All three are in the
  pair the milestone commits as frozen, all three are now pinned by a test that goes green on the
  fix, and each fix is one word, one clause or one citation.
- **Notes, not blockers:** finding 4 (the M0 record's six-rounds/five-numbers list) and finding 5
  (the record's status lines, including the half-fixed note 5 and the hand-proof contradiction).
  Both are in `docs/milestones/M0.md`, which this verdict requires the lead to rewrite before the
  commit, and both are reported rather than pinned because rounds 5 and 6 made that call on the same
  lines and because a red test there would block the commit on the record's own prose. **They must
  still be corrected before M0 is committed** — §5 says the hand-proof is unmet while §4a says it
  passed, and §4 says the schema review is pending while §1 says the schema is approved and frozen.
- **Carried notes** (unchanged this round, none blocking): §9.1 mitigation 4's mechanism (a skipped
  artboard discarded re-derives the same id, and kept it is an artboard that is not a page against
  §3's "artboards are pages"); §11.1's "1 job" with no stated home in `jobs/` or `provenance/`;
  §12's undefined term "entity" (`:595`) and the closed shapes it says carry no `unknown` (the list
  holds **seven** types, so round 6's carried note calling them "six" is itself one short — the
  document states no count, so there is nothing to mismatch); §5.6 `:241`'s consent row now also
  carries finding 3; and the ungoverned `manifest.bundle_id`, `manifest.title`,
  `ComicProject.settings`, `Page.page_number` and `Panel.generation_status` — none of which can make
  a legal bundle invalid, so none is a finding.

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest (`crates/doc/Cargo.toml` is untouched — the new tests read
`crates/comic-ai/src/lib.rs` and the documents as text rather than taking a dependency edge that
would put `comic-ai` behind `doc`). Run on the changed targets instead:

```
$ cargo fmt    -p vectorcraft-doc -- --check              # exit 0, no diff
$ cargo clippy -p vectorcraft-doc --test schema_claims    # 0 diagnostics
$ cargo test   -p vectorcraft-doc --test schema_claims    # 46 pass, 3 fail (findings 1-3)
$ cargo test   -p vectorcraft-comic-ai                    # 7 inline + 8 contract tests pass
```

The lead's `docs/ci-3da4080b.md` records "CI summary: all 7 steps passed (fmt, clippy, test, assets,
brands, layers, wasm)" for the revision *before* these three tests. A full `cargo xtask ci` on this
revision would fail at its test step on the three red tests; that is the intended evidence, not an
accident.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set and the
  session store records that model for 630 of its 632 rows, but **0** rows name this session, so
  nothing in the environment attests the backend of the model answering this review.
- Whether the owner *wants* §16's range extended or §16.1 given an ADR-017 subsection instead,
  whether §19 keeps a lettering-font clause qualified by `craft-fonts`, and whether §5.6's `/Q13`
  is dropped or replaced by the item that holds the requirement. All are the owner's calls; the
  tests assert the invariant (a range covers the new set, a resolved capability is not
  simultaneously a gap, a cited item exists), not a particular wording.
- The `cargo xtask ci` run on this revision (see Gate), and any run of the doc-claim suite under
  `--release` or on another platform.
- The §5.6 completeness claim ("a delta that is not here is a defect in this document") as against
  the design's §5.2 prose — a re-audit, not a count, and carried.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: still no fixture and no code
  exercise it; it is verified as text.

---

## Round 8 — closing verification

Reviewer:      code-reviewer on `deepseek-v4.1-flash`. `COPILOT_MODEL=deepseek-v4.1-flash` is set in
               the environment (`AI_AGENT=github_copilot_vscode_agent`), the same in-environment
               statement rounds 2–7 relied on; `~/.copilot/session-store.db ::
               assistant_usage_events` still holds **632** rows (**630** of them
               `deepseek-v4.1-flash`, one `gemini-2.5-flash`, one `gemini-3-flash-preview`) and **0**
               rows for this session (`045b6ced-…`), so the model claim rests on the environment
               plus the harness record, not on an attestation of the serving backend.
Revision reviewed:
               **working tree after the round-7 fixes** — base `3da4080b`,
               `docs/comiccraft-project-schema.md` **675 lines**, md5
               `5a9106745a8af02a8e7a3edf1097dda6` (round 7: 675 at `93b52b9b…` — the §5.6 citation);
               `docs/comiccraft-design.md` **1664 lines**, md5
               `52a5d3c9066c1055cebd38f2e61fa748` (round 7: 1662 at `0bb79ad5…` — §16's range and
               §19's clause, two lines); `docs/comiccraft-porting-plan.md` **1002 lines**, md5
               `44e03ee0566462696b7ff6fd4fb394f3` (unchanged, so round 6's verdict on it stands);
               `docs/milestones/M0.md` **379 lines**, md5
               `8afae9fa5111782fac4f68dfe573c601` (round 7: 373 at `0ea62afc…` — the status lines,
               the not-done list and the next steps). Every document moved where a round-7 finding
               named it, and every one of the four md5s differs from round 7's. This is not a re-run
               of round 7.
Files reviewed:
               `docs/comiccraft-project-schema.md` (all 675 lines), `docs/comiccraft-design.md` (§0,
               §1, §5.2, §5.3.1, §8.3, §9.3/§9.4, §12.1, §13, §16/§16.1, §17, §19, §20),
               `docs/milestones/M0.md` (all 379 lines), `docs/comiccraft-porting-plan.md` (§3–§7,
               §8, §15, §17), `crates/doc/tests/schema_claims.rs`,
               `crates/text/src/craft_fonts.rs`, `crates/text/src/fontdb.rs`,
               `crates/comic-ai/src/lib.rs`, `crates/comic-model/src/lib.rs`,
               `docs/reviews/M0-schema-document.md` (rounds 1–7)
Files written (test code only):
               `crates/doc/tests/schema_claims.rs` (extended: **1 new test** and two helpers —
               `table_column`, `face_family`; **no existing assertion changed, none deleted, none
               weakened**). `cargo fmt -p vectorcraft-doc` and
               `cargo clippy -p vectorcraft-doc --all-targets -- -D warnings` were both run on the
               result (clippy's `collapsible_if` on the first draft of `face_family` was fixed before
               the run recorded below — the gate runs clippy with `-D warnings`, so a warning there
               would have failed it).
Design refs:   §1 of the schema (the design decides the meaning), §1, §3, §5.3, §5.5, §5.6, §6.6,
               §9.1, §12, §13 rules 1/9/17/18, §14; design §0, §1, §5.2, §5.3.1, §8.3, §16/§16.1,
               §19, §20 R1/R2

Verdict:       **approved**

**All five round-7 findings are addressed, and the three defects are pinned green.** Round 7's one
unfinished note (the M0 record's status lines) is now largely fixed — the stale text round 7 named
at `:175` and `:214`, and the `| pending | pending |` row, are gone — but the record it fixed is still
internally inconsistent: `:3` says **six** rounds where `:19`/`:120`/§5/§8 say seven, `:19` lists
**six** counts for seven rounds, and §5 counts **four** ungoverned fields while naming five. I report
those as notes, not defects, for the reason rounds 5–7 gave on the same file: it is the lead's status
record, this verdict requires the lead to rewrite it before the commit, and none of the three can
make an implementer write different code. The frozen pair (schema + design) carries no defect this
round.

The sweep round 8 was sent to do found **no new count/range defect in the frozen pair**, and one
live escapee in the M0 record: the generic sweep round 7 added reads *paragraph-introduced bullet
lists*, so a spelled count followed by an inline parenthesised enumeration is not checked — and
`docs/milestones/M0.md:180` is exactly that ("and four ungoverned fields (`manifest.bundle_id`,
`manifest.title`, `ComicProject.settings`, `Page.page_number`, `Panel.generation_status`)": five
names, "four"). Round 7's note 4 is one number short in the same way. So the class is guarded for the
forms the sweep reads and not for the inline form; §5's answer below says which.

The one thing this round adds to the document/code surface is a **new test that pins the lettering
faces the schema names against the code that loads them**: schema §5.3's faces must be exactly
`craft_fonts::comic_fonts()`'s `COMIC_FAMILIES` and exactly design §8.3's table. It is green on this
revision and was shown to fail on each half by mutating the document in place and restoring it
byte-for-byte (both md5s above re-verified after the mutation, and again after a second mutation for
the clippy fix).

### Round-7 verification: the 5 findings

| # | Round-7 finding | Verdict | Evidence |
|---|---|---|---|
| 1 | **Defect**: design §16's intro ranged "ADR-011 … ADR-016" while its own table marks **seven** ADRs `(new)` | **verified fixed** | `docs/comiccraft-design.md:1487-1488` now reads "ADR-011 … ADR-017 were taken or revised in this document", and the table's new rows run `:1498-1508` — ADR-011 … ADR-017, contiguous, no gap, ADR-017 `Accepted (new)` at `:1508`. `the_design_covers_every_adr_its_own_table_marks_new` is green. **Residue (note 4 below):** §16.1's prose still stops at ADR-016. |
| 2 | **Defect**: design §19 still counted the lettering font among the gaps | **verified fixed** | `docs/comiccraft-design.md:1637-1641` now reads "There is still no existing command registration seam, no networking dependency, and no storyboard or page navigator — those **three** are the real gaps. (Comic lettering faces were the fourth; ADR-017 closed that one by adding Comic Neue and Bangers to `storytold/craft-fonts`.)" The count matches its own list (3), and the fourth is named as closed rather than left out silently, so §19 no longer disagrees with §8.3 `:940` and §20 R1 `:1651`. `the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap` is green. |
| 3 | **Defect**: schema §5.6's consent row cited "§9.3/**Q13**", and no document defines a `Q13` | **verified fixed** | `docs/comiccraft-project-schema.md:241` now reads "ADR-015 requires informed consent before artwork is sent to a provider, and **design §9.3** requires it to be recorded. The design says the consent must be recorded but does not place it" — the same form §5.5 `:224` uses. A whole-tree grep for `Q[0-9]` over the four documents returns nothing, and the design §9.3 `:1032` requirement the row points at exists. `every_question_citation_names_a_question_that_exists` is green, and its anti-vacuity half (the row must still cite `§9.3`) is what stops the citation being deleted instead of corrected. |
| 4 | **Note**: M0 §1 criterion 5 said "six rounds (13/19/12/9/6 findings)" | **partially fixed** | `docs/milestones/M0.md:19` now reads "**seven rounds** so far (13/19/12/9/6/5 findings)" — the round count moved, and the list gained round 6's `5`, but it is still **one number short**: seven rounds are 13/19/12/9/6/5/**5**, and round 7's own 5 is missing. `:124` has the same list as prose — "The counts fell 13 → 19 → 12 → 9 → 6 → 5" — six numbers for the same seven rounds, and "fell" is wrong where 13 → 19 rose. Both are notes (finding 3 below). |
| 5 | **Note**: the M0 record still described two outstanding review rounds | **verified fixed where round 7 named it, with a residue** | The four sites round 7 quoted are gone: no "`| pending | pending |`", no "still needs its two rounds", no "both rounds for the schema document", no "last M0 exit criterion that is **unmet**" (grep for `pending`, `two rounds`, `outstanding`, `unmet` finds none). `:19` says "**pass**", `:120` says "**seven further rounds**, every defect fixed", `:230`'s §4a is titled "The M1 hand-proof (porting plan §6 task 3) — **passed**", §5 `:177` says "Seven rounds fixed every defect", §8 `:219` says "every defect from seven review rounds is fixed", and §5 `:175-181` now lists the M1 notes. **Residue:** `:3` still says "**six** adversarial review rounds" against `:19`'s seven, and §4's "**seven further rounds**" after round 1 means eight rounds total — three different totals in one file (finding 2 below). |

### New findings

1. `docs/milestones/M0.md:180` (§5's not-done bullet) — claims: "and **four** ungoverned fields
   (`manifest.bundle_id`, `manifest.title`, `ComicProject.settings`, `Page.page_number`,
   `Panel.generation_status`)". True: the parenthesis names **five**, and the review record names the
   same five (`docs/reviews/M0-schema-document.md:2212`), so the count is one behind the list it
   introduces — round 3's four plus the fifth the record added later. **wording** (the five names are
   right there; a reader governs five, not four).
2. `docs/milestones/M0.md:3` (§3's status line) — claims: "the schema document has had **six**
   adversarial review rounds, all defects fixed". True: the same file says seven at `:19`, "seven
   further rounds" (i.e. eight) at `:120`, "Seven rounds fixed every defect" at `:177` and "seven
   review rounds" at `:219`. One of the four is right and three are not; the record disagrees with
   itself about the round count. **wording** (the lead's status record, rewritten on this verdict).
3. `docs/milestones/M0.md:19` and `:124` — claims: "seven rounds so far (13/19/12/9/6/5 findings)"
   and "The counts fell 13 → 19 → 12 → 9 → 6 → 5". True: the record holds **seven** rounds with
   **13/19/12/9/6/5/5** findings, so both lists are one entry short, and the second is wrong in
   direction as well (13 → 19 rose). This is round 7's note 4, moved rather than closed. **wording**
   (the same file, the same standing reason).
4. `docs/comiccraft-design.md:1510` (§16.1 "Decisions taken in this revision") — claims: this section
   gives the decisions taken in this revision, and §16's intro `:1487` now names ADR-011 … **ADR-017**
   as taken or revised here, with the table's `:1508` marking ADR-017 `Accepted (new)`. True: §16.1's
   prose `:1512-1548` runs **ADR-011 … ADR-016** — six paragraphs, no ADR-017 — so the one decision
   that closed risk R1 has no entry where a reader looks for a decision's reasoning. Its reasoning is
   present at §8.3 `:940-954` and §20 R1 `:1651`, which is why this is **wording** and not a defect:
   nothing in §16.1 states a span or a count, so no arithmetic is wrong and no implementer is sent to
   the wrong section — they are simply sent to §8.3 instead of §16.1. The fix is one paragraph, or one
   clause in §16.1's heading saying the lettering decision is reasoned in §8.3.
5. `docs/comiccraft-project-schema.md:167-171` (§5.3's face table) — claims: a `| Setting | Face |
   Used for |` table naming four settings — `dialogue_font`, `dialogue_font_emphasis`,
   `dialogue_font_aside`, `display_font` — as the ComicCraft defaults. True: no struct in this
   document declares any of those four names. `ProjectSettings` `:157` holds exactly three fields
   (`default_page_size`, `default_template`, `default_lettering_style`), the design's §5.2 `:435`
   lists the same three, §5.4's `StyleProfile` carries `palette`/`line_weight`/`rendering_notes` and
   no font roles, and §5.6's delta list — whose own rule is "a delta that is not here is a defect in
   this document" — does not mention them. So "Setting" is a header the document does not honour:
   either the four names are fields (then §5.6 owes a delta entry, and §1 makes adding one to v1 a
   version bump with a migration and a fixture) or they are the roles the default style fills (then
   the header should say so, and the project's own choice lives in the per-lettering
   `TextLayoutSettings.font`, §6.6 `:355`). I report it as **wording**, not a defect, because the
   second reading is complete and implementable and the document supplies it: §6.6's
   `font: Option<String>` is the documented per-object override, so "a project can choose different
   faces" (`:173`) holds, and §1 `:15-17` tells an implementer not to add a v1 field — no implementer
   is pushed into a wire format the frozen schema rejects. The fix is one clause in the paragraph or a
   "Role" header.

### Count/range sweep

Every count, range and enumeration in the four documents against its contents, plus the forms round
7's generic sweep does not read. `✓` = agrees. **Guarded** means a test fails if the two drift apart.

| Document | Site | States | Contents | |
|---|---|---|---|---|
| schema | §5.3 `:167-171` | the two faces, four roles | Comic Neue ×3, Bangers ×1 — 2 families | ✓ guarded (**new this round**: `the_lettering_faces_the_frozen_schema_names_are_the_ones_the_code_loads`, against `COMIC_FAMILIES` and design §8.3) |
| schema | §5.6 `:241` | "design §9.3" | §9.3 exists and holds the requirement | ✓ guarded (round 7's citation test) |
| schema | §13 `:606-624` | 18 rules | 18 contiguous | ✓ guarded |
| schema | §9.1 `:490`, `:509` | five mitigations; "1–4 stand alone" | 5 items, guards 1–4, 5 upstream | ✓ guarded |
| schema | §14 `:659` | rules 1–18 | 18 | ✓ guarded |
| schema | §3 `:93` | three bases | Linked/Package/LinkInfo.relative | ✓ guarded |
| design | §16 `:1487-1488` | ADR-011 … ADR-017 | table marks 7 new (011–017) | ✓ guarded (round 7's test) |
| design | §16.1 `:1512-1548` | decisions taken in this revision | ADR-011 … ADR-016 prose, table has 7 | **✗ finding 4** — note, no count stated so nothing to pin |
| design | §19 `:1637-1641` | "those three" | 3 items, lettering named as closed | ✓ guarded (round 7's test) |
| design | §0 `:10-13`, §1 `:27`, §5.3.1 `:606-631`, §12.3 `:1318` | section ranges, five principles, five mitigations, five layers | all agree | ✓ guarded (sweep + mitigation tests) |
| design | §20 `:1649-1658` | R1–R10 | 10 rows, one `Resolved` | ✓ |
| plan | §5.2 `:306`, §15 `:833-848`, §17 `:969-972` | five mitigations, nine milestones, three concerns | all agree | ✓ |
| M0.md | §1 `:19`, §4 `:124` | seven rounds; 13/19/12/9/6/5 | seven rounds, 13/19/12/9/6/5/**5** | **✗ finding 3** — note, not pinned |
| M0.md | §3 `:3` vs `:19`/`:120` | six / seven / seven-further rounds | one file, three totals | **✗ finding 2** — note, not pinned |
| M0.md | §5 `:180` | "four ungoverned fields" | five named | **✗ finding 1** — note, not pinned |

**What the generic sweep reads, and what it does not.** Round 7's
`every_count_a_document_states_is_the_length_of_the_list_it_introduces` pairs a spelled number with
an *adjacent* plural noun (`count_noun_pairs` reads `windows(2)`) and then counts the `- `/`* `/
`N. ` items that follow, skipping indented lines and paragraphs with more than one count pair. So it
guards the paragraph-introduced bullet-list form (10 lists across the three documents, `checked >= 5`
asserted) and cannot see: a count separated from its noun by an adjective ("four **ungoverned**
fields"), an inline parenthesised enumeration, or a count on an indented continuation line — which is
exactly why `M0.md:180` escaped a sweep that has been green since round 7. I ran those three forms by
hand over the four documents (a spelled number, up to three adjectives, a plural noun, then a
`(…)` of two or more comma/`and`-separated items, and separately every paragraph with two or more
count pairs that introduces a list): the **only** mismatch is `M0.md:180`; the frozen pair has no
instance of the form at all (the two candidates — schema §5.5 `:200`'s "one section … (`PageSize` …)" and
design §4.2 `:280`'s "one path … (`Session::execute` …)" — are not enumerations of the counted noun, which
is why a naive version of that check would be a false-positive generator and I did not add it). No
paragraph with two or more count pairs introduces a list anywhere, so the sweep's
`pairs.len() != 1 → continue` hole is not hiding anything today.

So: **the class is genuinely guarded for the frozen pair's counted lists, ranges and citations, and
not for the inline-parenthesis form.** The one live instance is in the M0 record, which the lead
rewrites before the commit; I have not pinned it, for rounds 5–7's reason (a red test there would
block a commit on the lead's own status line).

### Security

Re-read, unchanged from round 6 in every operative sentence. §13 rule 9 `:615` resolves against the
**declared** base, canonicalises **before** the confinement check, never resolves an escaping path,
and refuses `.save-tmp/` as a source; §13.1 `:633-641` keeps the hard-link limit and the
canonicalise/open TOCTOU window visible; §14 `:649-664` makes only a parse failure fatal and reports
everything else on a usable project. §8.1 `:428-429` ("**Credentials never appear here.**") is
untouched, and the round-7 fixes were a range, a clause and a citation in prose about *which section
records a consent requirement* — none of them touches a resolution path, a credential, or a file
read. The new test reads two documents and one source file as text; it opens nothing. No finding this
round can be turned into an exploit.

### Defects vs notes

- **No defect.** Nothing this round would cause a wrong implementation, so the verdict is
  **approved**: findings 1–3 are arithmetic and status drift in `docs/milestones/M0.md`, finding 4 is
  a missing subsection whose content exists elsewhere, and finding 5 is a table header the document
  does not honour while supplying the implementable reading in §6.6. Round 7's three defects are
  fixed and pinned by three tests that are now green.
- **Notes, not blockers:** findings 1–5, all five fixable in one edit each, and findings 1–3 in the
  file the lead must rewrite on this verdict anyway. Nothing here needs a ninth round.

Unit tests:    `crates/doc/tests/schema_claims.rs` — `cargo test -p vectorcraft-doc --test schema_claims`
               — **50 passed; 0 failed** (50 total; round 7 was 49 with 3 red). The 46 tests rounds 1–6
               left green are still green, round 7's three red pins are now green on the fixes, and
               round 8's new test is the 50th. `cargo clippy -p vectorcraft-doc --all-targets -- -D
               warnings` — clean (the gate's own clippy invocation). `cargo fmt -p vectorcraft-doc` —
               applied to the new block only.

```
$ export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
$ cargo test -p vectorcraft-doc --test schema_claims
running 50 tests
... 50 ok ...
test the_lettering_faces_the_frozen_schema_names_are_the_ones_the_code_loads ... ok
test the_design_covers_every_adr_its_own_table_marks_new ... ok
test every_question_citation_names_a_question_that_exists ... ok
test the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap ... ok
test every_count_a_document_states_is_the_length_of_the_list_it_introduces ... ok
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The new test is not decorative: mutating `docs/comiccraft-project-schema.md:171`'s family
(`Bangers` → `Bangers Two`) fails it —

```
assertion `left == right` failed: schema §5.3's faces must be the families
`craft_fonts::comic_fonts()` loads — the document names ["Bangers Two", "Comic Neue"] and the
filter carries ["Bangers", "Comic Neue"], so a face the document names and the code does not
load would report a missing font for the product's own default lettering
```

— and mutating design §8.3 `:946` the same way fails its design half
(`["Bangers Display", "Comic Neue"]` vs `["Bangers", "Comic Neue"]`). Both documents were restored
byte-for-byte after each mutation and the md5s re-checked: schema
`5a9106745a8af02a8e7a3edf1097dda6`, design `52a5d3c9066c1055cebd38f2e61fa748` — the revision strings
recorded above, so what was reviewed is what the tests ran on. `bash
.github/skills/comiccraft-verify/scripts/check-reviews.sh "" M0` — exit **0**, "All review records
complete" (the M0 record's §4 quote of that command shows one of the two `OK` lines; the schema line
is there too — part of note 2's file, not a separate finding).

Gate:

`cargo xtask ci` — **not run.** This round changed one test file and this record; it touched no
production code and no crate manifest (`crates/doc/Cargo.toml` is untouched — the new test reads
`crates/text/src/craft_fonts.rs` as text rather than taking a dependency edge). The gate was green on
this revision's content per `docs/ci-3da4080b.md`; the round's own evidence is the two commands above.

M1 notes carried:

- **New this round:** schema §5.3 `:167-171` — the four lettering names presented as "Setting" with
  no declaring struct (finding 5). Worth one clause when the lettering policy lands in M1/M2.
- Unchanged from round 7: §9.1 mitigation 4's mechanism (a skipped artboard discarded re-derives the
  same id; kept, it is an artboard that is not a page against §3's "artboards are pages"); §11.1's
  "1 job" with no stated home in `jobs/` or `provenance/`; §12's undefined term "entity" (`:595`) and
  the seven closed shapes it says carry no `unknown` (the document states no count, so there is
  nothing to mismatch); and the five ungoverned fields `manifest.bundle_id`, `manifest.title`,
  `ComicProject.settings`, `Page.page_number` and `Panel.generation_status` — none of which can make
  a legal bundle invalid, so none is a finding.
- **Closed since round 7:** §5.6 `:241`'s consent row is no longer carried — round 7's finding 3
  fixed the citation that note was about.
- And the M0 record's own drift (findings 1–3), which is the lead's to correct on this verdict rather
  than a note for M1.

Unverified:

- The serving backend for **this** session: `COPILOT_MODEL=deepseek-v4.1-flash` is set and the
  session store records that model for 630 of its 632 rows, but **0** rows name this session, so
  nothing in the environment attests the backend of the model answering this review.
- Whether the owner prefers §16.1 to gain an ADR-017 paragraph or §16's intro to point at §8.3, and
  whether §5.3's fourth column should be "Role" or the four names declared as fields. Both are the
  owner's calls; the new test asserts the invariant that matters (the faces the document names are
  the families the code loads), not a wording.
- The `cargo xtask ci` run on this revision (see Gate), and any run of the doc-claim suite under
  `--release` or on another platform.
- The §5.6 completeness claim ("a delta that is not here is a defect in this document") as a whole —
  I re-audited §5.3 (finding 5) and the field lists of §5.1–§5.4 against design §5.2 by hand, but the
  rest of the §5.5/§5.6 class remains a re-audit rather than a count, and is carried.
- The hard-link and TOCTOU behaviour of rule 9 on a real filesystem: still no fixture and no code
  exercise it; it is verified as text.
- Whether `craft-fonts` itself still ships the two faces the schema names: the test reads
  ComicCraft's own filter, not the companion repository (which is not checked out here, and whose
  public copy M0 §4b records as incomplete for the Arabic faces).
