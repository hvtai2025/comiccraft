# M2 W1 — schema v2: panel locking, by `schema-architect`

**Status:** complete and ready for review. Gate: see §9 (blocked at `fmt` by a file this work does
not own — stated there, with the run I did make).
**Milestone:** M2 (porting plan §8, Phase 2) · work item **W1** — `docs/milestones/M2.md` §4.2
**Owner:** `schema-architect`
**Specification:** M2 §4.2 verbatim: *"one field, on `Panel` alone; a **v1 → v2 migration** that is
genuinely exercised (a v1 fixture loads and gains the field's default); a **v2 fixture**, and the
rule set updated only where a new rule is actually needed; **no other field rides along**"* ·
*"the migration must have a failing-then-passing test in the sense §5.3.1's exit criterion uses — a
v1 bundle read by v2 code, with the v1 fixture checked in and byte-stable"*
**Design refs:** porting plan §8 ("Panel locking and selection"; *"Panel shapes, borders, image
transforms, cropping, clipping and **locking** are engine features already"*) · design §6.1 (one
authority per fact), §12.1 (the deterministic-check criteria a rule must meet), §8.2 (a frame is a
document object); schema §2 (versioning and refusal), §5.4/§5.6/§6.2/§6.3, §5.7 (canonical form),
§10 (save protocol), §11/§11.1 (migrations and fixtures), §13 (the eighteen rules), §14 (what a load
tolerates)
**Revision measured:** the working tree (base `e26f030d`), not a commit — every `sha256` is in §10

---

## 1. What was built

One field, its migration, two fixtures, and the tests that make the migration falsifiable.

| File | What changed |
|---|---|
| `crates/comic-model/src/entities.rs` | `Panel.locked: bool`, `#[serde(default)]` — §6.2's v2 field |
| `crates/comic-model/src/migrate.rs` | **new.** `Migration` and `migrate()`: the v1 → v2 step and §11's report |
| `crates/comic-model/src/bundle.rs` | `LoadedBundle.migrations`; the load reports the migration; §2's refusal for a **file**; the writer stamps the version it implements on every file |
| `crates/comic-model/src/lib.rs` | `SCHEMA_VERSION = 2`, the module and its exports, one inline test |
| `crates/comic-model/src/error.rs` | `Error::FileSchemaTooNew` (§2, one file down) |
| `crates/comic-model/src/validate.rs` | rule 3 reads `ComicBundle::declared_versions()` (one authority, shared with the refusal); three test literals gain the field |
| `crates/comic-model/tests/fixture_v1.rs` | the migration tests; the byte-identical round trip moves to the fixture at the current version |
| `crates/comic-model/tests/fixture_v2.rs` | **new.** The v2 fixture's round trip, the default, the field's round trip, the refusals, and the ignored generator |
| `crates/comic-model/tests/fixtures/v2/minimal.comiccraft/**` | **new.** 13 files, derived from v1 — **no file of `fixtures/v1/` is touched** |
| `crates/comic-model/tests/schema_contract.rs` | the two doc-reading contracts follow the version (§11) |
| `crates/doc/tests/schema_claims.rs` | **2 assertions + 1 helper** — the same class of doc contract, in another crate; see §6.7 |
| `crates/mcp/tests/comic_headless.rs` | **1 fixture** — a hand-written `assets/index.json` that declared the old version; see §6.7 |
| `docs/comiccraft-project-schema.md` | §1, §2, §5.4, §5.6, §6, §6.2, §7, §8.3, §11, §11.1, §13, §15 |
| `ASSETS.md` | two rows for the v2 fixture's two asset files (the `assets` gate fails on any unattributed file) |
| `crates/engine/src/cmd/comic/panel.rs` | **2 lines**, `locked: false` in the two `Panel` literals — see §6 |
| `crates/comic-layout/src/reading_order/mod.rs` | **1 line**, the same, in its `#[cfg(test)]` fixture — see §6 |

---

## 2. The field

| | |
|---|---|
| **Wire name** | `locked` |
| **Type** | `bool` |
| **Default** | `false` — and **always written**, so canonical v2 carries it on every panel |
| **Where** | `Panel` only (§6.2's table, last declared field before `unknown`); the §5.6 delta table says why the design's prose list had no field for it |
| **Not referential** | no §13 rule reads it, and none was added — §4 below |

A v1 file has no such key. `#[serde(default)]` is what makes the reader accept it (§5.7: *"A reader
must accept any valid JSON"*), and the migration is what **reports** the transition (§3).

**Why not the document's own lock flag.** `crates/doc/src/node.rs:502` already has a `locked` on
every node, and `crates/doc/src/hit.rs` uses it for clicks and Select All. That is **not** the same
fact and it is not a second authority this document creates: it is the *document editor's* session
behaviour for a node, and this crate is L2 and never parses the document — it cannot read it at all.
The panel's field is the durable, bundle-level record, and it survives a template re-apply, an
artboard rebuild, or a document written by another tool. §6.2 writes the relationship down instead
of leaving a reader to guess it. Whether W3 also mirrors it onto the frame node is a command-layer
decision, and it is **not** part of this wire format — flagged in §8 as something this document
cannot verify.

---

## 3. The migration

**What v1 → v2 does.** `migrate::migrate(&mut bundle, 1)` runs after every file has parsed and the
id counter has been repaired, and returns `Some(Migration { from: 1, to: 2, notes })`.

The field's *value* is not the migration's to write. It arrives from `#[serde(default)]` at parse
time, and a migration cannot be the thing that supplies it: after parsing, a `false` from the key and
a `false` from the key's absence are the same value. What the step does is the part a default cannot:

1. it names the field v2 added and its default, in one place, next to the version pair — the
   neighbouring step for v3 is now a shape to imitate;
2. it **counts** the panels that take the default (`"v2 adds `Panel.locked` (default `false`); 3
   panels take the default"`);
3. it refuses to be silent about the one input a default would silently *accept*: a **v1 bundle that
   declares `locked`**. No v1 writer could have written that key, so it is a hand-edit or a foreign
   tool; the value is **preserved** and reported. Stated limit: after parsing, `locked: false` is
   indistinguishable from the key's absence, so only a truthy value is reportable — written in the
   module doc rather than papered over.

**How it is reported.** §11's bullets are a contract on a migration (refuse rather than guess,
preserve ids, preserve unknown fields, report, ship fixtures), and the third of them —
*"Report incompatible data as a visible, actionable error"* — is about data the migration cannot
handle. A **successful** migration still moved the project, and the caller must be told, so the
report is a **returned value**: `LoadedBundle.migrations: Vec<Migration>`, each with the version pair,
one actionable note per effect, and a `Display` that renders the whole thing for a log.

It is deliberately **not** a §14 `Finding`. Findings are the *inconsistencies* of a load, and a
bundle read forward is not inconsistent, only older; filing one there would also put noise on every
`comic.project.open` of every v1 project. The consequence is stated in §7: nothing in the command
layer surfaces `migrations` yet, and that is one line in a tree this work item does not own.

**The test that exercises it** (`crates/comic-model/tests/fixture_v1.rs`):

| Test | What it pins |
|---|---|
| `the_fixture_is_the_bundle_the_schema_describes` | the v1 fixture loads under v2 code, `manifest.schema_version` is still `1` (a load rewrites nothing), **every panel is `locked: false`**, and exactly one migration is reported |
| `the_fixture_is_migrated_by_the_load_and_the_report_says_what_it_did` | the report names the field and the count; `Display` carries it; the v1 and v2 loads agree on **every id**, `retired_artboard_ids` included |
| `a_v1_bundle_that_declares_locked_is_reported_rather_than_honoured_in_silence` | the hand-edit branch: the value is preserved **and** reported, and the rest are counted |
| `saving_the_v1_fixture_produces_the_v2_fixture_byte_for_byte` | the migration's output **is** the v2 fixture — the strongest available statement that this is a real read-forward and not two hand-written files |
| `a_load_of_the_v1_fixture_writes_nothing` | the frozen bytes do not move (checked against the source fixture too) |
| `a_save_leaves_the_in_memory_model_at_the_version_it_wrote` | the writer's half: after a save, every file declares v2 *in memory* and `validate` is clean |
| `a_version_with_no_migration_step_refuses_rather_than_guesses` (bundle.rs, inline) | `migrate(&mut b, 0)` is `Error::Migration`, not `Ok` |
| `a_migration_is_reported_and_a_current_bundle_is_not` (bundle.rs, inline) | a v2 bundle reports nothing; the v1 report's exact notes |

---

## 4. No new referential rule — stated, not implied

§13 stays at **eighteen rules**, and the document now says so explicitly (§13's head, §6.2's note):

> `locked` is not a reference (rule 1), an id (2, 4, 17), a declared agreement (3, 5, 6, 8, 11, 13),
> a path (9) or a lifecycle (10, 14, 15, 16, 18). It relates nothing to anything: a bundle full of
> locked panels is exactly as valid as one with none, and a locked panel makes no other fact illegal.

§12.1's own test for a rule is that it is a **deterministic check on the bundle's own data** whose
failure is an inconsistency a creator can repair. "This panel is locked" is true or false and
inconsistent with nothing, so there is nothing to check and nothing to report. What locking
constrains is **commands** — which panel commands move, resize or delete — and a constraint on a
command belongs to that command (W3's tree), not to `comic.project.validate`.

A test pins the absence rather than the prose:
`schema_contract.rs::the_document_places_the_v2_field_on_the_panel_and_gives_it_no_rule` reads the
frozen document and fails if §13 gains a `| \`locked\` |` row.

---

## 5. The fixtures

**v1 — unchanged, and proved so.** `git diff HEAD -- crates/comic-model/tests/fixtures/v1` is empty:
every byte is what M1 froze under `b68150ba`/`79f40dd3`/`e26f030d`. A schema bump that rewrote
history would leave nothing to test the migration against, so the load reads v1 **in memory only**
and the schema doc's §11.1 now says why the round-trip rule belongs to the current version's fixture.

**v2 — canonical, byte-stable, and derived.**

| | |
|---|---|
| `manifest.json` | `6f9f3e55f9b564ea66ea77c6978949feb99eacda169a026290373999f14dc22d` |
| `pages/page-8.json` | `efebecd4bbb33ec86a4e4758e9d4ff4d5a364385a57b3672b24d7ec283c4053e` |
| whole tree (13 files, sorted, hashed) | `4be4c745bca688ac271ce3eb048583689df75b7cf9080a9503523996f2297c5f` |
| the v1 tree, for contrast | `8a39a1e3728e7abfd225cecb64af08223b4e8f07593a696c265a898e19d1f234` |

What pins its canonical form:

- `fixture_v2.rs::the_v2_fixture_round_trips_byte_identically_and_a_second_save_moves_nothing` — a
  save is compared **byte for byte** against the committed fixture, and then saved again: §5.7's
  field order, number form, map sort and trailing newline, plus idempotence;
- `fixture_v1.rs::saving_the_v1_fixture_produces_the_v2_fixture_byte_for_byte` — the derivation is
  checked on every run, so the fixtures cannot drift apart;
- a **generator** rather than a hand-edit: `cargo test -p vectorcraft-comic-model --test fixture_v2
  -- --ignored regenerate_the_v2_fixture --nocapture`, named in the file's header and in `ASSETS.md`'s
  rows. `#[ignore]`d, so no ordinary run writes the checkout.

The two files that differ *between* the fixtures are the page file (3 × `"locked": false`) and the
manifest (whose `files` hashes describe the new bytes). `document/comic.vectorcraft` and
`assets/panels/panel-1.png` are byte-identical to v1 — `a_saved_lock_does_not_reach_the_visual_document_or_another_file`
also asserts that setting a lock changes **exactly** `pages/page-8.json` and `manifest.json`, and
that no `locked` string appears in the document (ADR-013).

The manifest's `title` still reads `Minimal v1 fixture`, deliberately: a migration preserves authored
data, and a v2 fixture that retitled it would be a migration inventing content.

---

## 6. Deviations, each with its reason

**6.1 Two files outside my tree gain one line each — unavoidable, and reported rather than smuggled.**

Adding a field to a public struct breaks every exhaustive struct literal, and Rust has no default
field values (`default_field_values` is nightly-gated; probed on this toolchain and refused). The
sites are:

- `crates/engine/src/cmd/comic/panel.rs` — `locked: false` in the two `comic.panel.create`-path
  literals (lines ~272 and ~539). **W3's tree.** The line is the *default value only*: nothing reads
  `locked`, no behaviour changes, and W3 is the writer that gives it meaning. Without it the engine
  does not compile, so W3 could not have been dispatched at all;
- `crates/comic-layout/src/reading_order/mod.rs` — `locked: false` in the `#[cfg(test)]` `panel()`
  helper. **W5's tree**, test-only, and its 176 lib tests pass with it.

Both are one line, both are the field at its default, and neither changes any behaviour. I am flagging
them rather than leaving them for their owners because a red workspace is worse than a disclosed,
mechanical edit; if the Lead would rather they were reverted to preserve the writer boundaries, the
field cannot land without them.

**6.2 §2's refusal, one file down (`Error::FileSchemaTooNew`).** §2's reader rule is *"Refuse a
`schema_version` greater than the one they implement … Never partially load"*, and it is stated for
the bundle; §13 rule 3 already requires every file to declare the manifest's version. Today a
`pages/page-<id>.json` declaring version 99 would be **read as v2** — the partial load §2 forbids, and
exactly the "a v2 record the code cannot represent is refused rather than mis-read" case this work
item was asked to test. So the load now walks `ComicBundle::declared_versions()` and refuses the first
file whose declared version is greater. Rule 3 still governs an *older* version (read forward,
reported); only the greater direction is fatal. It is behaviour beyond the one field, it is ~10 lines,
and it is **not** a rule — §13's table is untouched. I would rather it were judged than assumed.

**6.3 The writer stamps the version it implements on every file.** `write_generation` serialised each
file as the model held it, which was invisible at v1 (every file was `1`). At v2 it is not: a v1
bundle read forward has files declaring `1`, and the writer already forced
`manifest.schema_version = SCHEMA_VERSION` — so the save would have written a **v2 manifest over v1
page files**, which is rule 3's own contradiction, in the bytes the save just wrote, and it fails M1's
inline `a_validation_of_a_saved_bundle_matches_the_one_of_the_model`. `stage_versioned` writes each
file at the implemented version and the model adopts the same values on success (§2: *"the writer
writes the version it implements"*). `save`'s doc-comment states precisely what it may still mutate
after a failure — nothing: the stamping happens on derived copies, and the model is touched only once
the write has succeeded.

**6.4 Rule 3 reads one list.** `validate.rs`'s rule 3 built its own file list; the refusal needs the
same list, so it is now `ComicBundle::declared_versions()` — one authority for "every file that
declares a version", used by rule 3 and by §2's refusal. Finding subjects, severities and messages are
unchanged (`round1_reviewer::rule_3_checks_the_version_every_jobs_file_declares` and
`round2_reviewer::rule_3_names_the_file_that_declares_the_wrong_version` still pass); an inline test
pins the list against the fixture's own files.

**6.5 `schema_contract.rs` is a reviewer-authored file.** Two of its contracts read the frozen
document: `schema_version_matches_every_example_in_the_frozen_document` (all `"schema_version": N`
examples must equal the constant) and `..._declares_it_frozen` (which asserted the literal phrase
`"frozen at version 1"`). The second now *derives* the phrase from `SCHEMA_VERSION`, which keeps its
purpose — neither side can move alone — without pinning a version number. One test is added (see §4).

**6.6 `ASSETS.md`, two rows.** The `assets` gate fails on any unattributed asset file, and the v2
fixture carries a copy of the v1 document and PNG. W2 added a row to the same table concurrently;
mine are additive and adjacent to the v1 rows they mirror.

**6.7 Two files outside this crate pin the schema document or its version, and the bump broke them.**
`cargo xtask ci`'s `test` step found both, and both are the same class of ripple as §6.5 — a contract
that names the version number the document used to carry:

- `crates/doc/tests/schema_claims.rs` — M0's reviewer contracts on the frozen document. Two of them
  asserted the *literal* `"schema_version": 1` inside §5.4's and §8.3's examples
  (`rule_3_names_every_file_that_declares_a_schema_version`,
  `the_jobs_file_wrapper_carries_the_schema_version_rule_three_checks`). They now read the version
  **out of the document** (`declared_version`) and compare that, which keeps what they are for — the
  sections that describe a versioned file must still *show* the wrapper, and must agree with each
  other — without hard-coding a number in a crate that cannot depend on `SCHEMA_VERSION`
  (`crates/doc` is below `crates/comic-model`). The number-versus-code half stays where the constant
  is: `crates/comic-model/tests/schema_contract.rs`.
- `crates/mcp/tests/comic_headless.rs` — its acceptance test makes a bundle over MCP and then writes
  one `assets/index.json` **by hand** (M1 has no `comic.asset.*` command), which is the writer's job
  done in a test. The fixture declared `1`; the manifest the engine creates now declares `2`, so rule
  3 reported the registry and the test's `errors == 0` assertion failed — a *true* finding against a
  stale fixture. The fixture now takes the version from the bundle's own `manifest.json`, so it
  cannot go stale again.

---

## 7. Fields and rules I did **not** add — reported, not added

These are decisions for the Lead, not free additions, and none is in the diff:

- **A lock *reason* or *actor*** (`locked_by`, `locked_at`, a comment). No criterion asks for it, the
  design says only "locking", and §8.1's *"panel importance or intended emphasis"* is deferred by
  M2-F0. If it is ever wanted, it is a v3 field with its own migration.
- **`locked` on `Page`, `Lettering` or `Selection`.** The plan's "panel locking **and selection**"
  splits at M2 §2: locking is durable model state (this field), and **selection is session state**
  whose home is M6's UI. Persisting a selection into the bundle is the §5 risk row, and this work
  writes none.
- **A `Node.locked` mirror.** The document node already has one; encoding the mirror in the schema
  would create the second authority §1's structural rule forbids, and this crate cannot read the
  document to check it. Documented as a relationship instead (§2 above, §6.2).
- **A rule for locking** — see §4. §13 stays at eighteen.
- **Custom panel shapes or per-panel geometry** (M2 §2: an extension, not a prerequisite; §8.1's
  fixed geometry is deferred).

---

## 8. Open item this crate cannot close

`LoadedBundle.migrations` is the report, and **nothing surfaces it yet**: `crates/engine/src/cmd/comic/project.rs`
destructures `let LoadedBundle { bundle, findings, .. } = load(...)`, so `comic.project.open` returns
the findings and not the migration. The model's half of §11 is delivered and tested; the creator-facing
half is one line in `crates/engine/src/cmd/comic/**`, which this work item is explicitly forbidden to
touch. Handing it to W3 (or the Lead) rather than reaching into that tree.

---

## 9. Verification

### 9.1 Tests

```text
$ export PATH="$HOME/.cargo/bin:$PATH"          # cargo 1.99.0; the system cargo is 1.75 and cannot build this workspace
$ cargo test -p vectorcraft-comic-model
     Running unittests src/lib.rs                        → ok. 74 passed; 0 failed; 0 ignored
     Running tests/fixture_v1.rs                         → ok. 15 passed; 0 failed; 0 ignored
     Running tests/fixture_v2.rs                         → ok.  7 passed; 0 failed; 1 ignored   (the generator)
     Running tests/round1_reviewer.rs                    → ok. 19 passed; 0 failed; 0 ignored
     Running tests/round2_reviewer.rs                    → ok. 18 passed; 0 failed; 0 ignored
     Running tests/round3_reviewer.rs                    → ok.  8 passed; 0 failed; 0 ignored
     Running tests/schema_contract.rs                    → ok.  9 passed; 0 failed; 0 ignored
   Doc-tests vectorcraft_comic_model                     → ok.  1 passed; 0 failed; 0 ignored
   **151 passed, 0 failed, 1 ignored**  (M1's baseline was 135; all 135 still pass, unmodified except
   where §9.4 lists)

$ cargo test -p vectorcraft-engine
   unittests src/lib.rs → ok. 1605 passed; 0 failed
   tests/comic_acceptance.rs → ok. 15 · tests/comic_acceptance_reviewer.rs → ok. 3
   tests/comic_commands_r1.rs → ok. 14 · _r2 → ok. 5 · _r3 → ok. 6 · tests/comic_golden_render.rs → ok. 6
   **every test binary green**

$ cargo test -p vectorcraft-comic-layout
   unittests src/lib.rs → ok. 176 · layer_rules_contract → 8 · lettering_round1_reviewer → 31
   panel_rect_contract → 17 · round1..round5_* → 27/12/12/4/5/1 · m2_panel_cycle → 1
   m2_reading_order_independence → 4 · w4_* → 5/7/4
   **every test binary green** (the one line this work adds to `reading_order/mod.rs`'s fixture is in
   the 176)

$ cargo test -p vectorcraft-doc --test schema_claims      → ok. 50 passed; 0 failed   (§6.7)
$ cargo test -p vectorcraft-mcp
   unittests src/lib.rs → ok. 66 · tests/automation_roots.rs → 4 · tests/comic_headless.rs → 1   (§6.7)
```

### 9.2 Clippy, layers, brands, assets

```text
$ cargo clippy -p vectorcraft-comic-model --all-targets --no-deps -- -D warnings   → Finished, 0 diagnostics
$ cargo clippy -p vectorcraft-engine   --all-targets --no-deps -- -D warnings      → Finished, 0 diagnostics
$ cargo clippy -p vectorcraft-comic-layout --all-targets --no-deps -- -D warnings  → Finished, 0 diagnostics
$ cargo xtask layers   → OK: 29 crates, no layering violations.
$ cargo xtask brands   → brands: no vendor names in user-visible text
$ cargo xtask assets   → assets: all 323 asset files attributed in ASSETS.md
```

`--no-deps` is used because `cargo clippy -p vectorcraft-engine` also lints its path dependencies, and
while I was working those trees held their owners' in-flight edits; the three crates above are clean
**with** deps too once W4/W5's work settled (`cargo clippy -p vectorcraft-comic-layout --all-targets
-- -D warnings` → clean).

### 9.3 The gate

Two attempts to get a clean run, and then the real thing:

```text
$ cargo xtask ci
  === ci: fmt ===
  Diff in crates/comic-layout/tests/w4_font_contract.rs:300   (W4's in-flight file)
  FAIL  fmt: cargo fmt --all -- --check: exited with exit status: 1
  skip  clippy / test / assets / brands / layers / wasm
```

Retry, minutes later (W4 had formatted): `fmt` **ok**, `clippy` **ok**, then `test` failed with five
errors in a file this work does not own, all of the form
`the fixture copy loads: Read { path: ".../target/tmp/r1-refused-save/manifest.json", source: NotFound }`.
Those are a **concurrent-run collision, not a defect**:

```text
$ cargo test -p vectorcraft-comic-model --test round1_reviewer      # alone, immediately after
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`crates/comic-model/tests/round1_reviewer.rs:36` and `round2_reviewer.rs:52` build their scratch
directory as `CARGO_TARGET_TMPDIR.join(name)` — **no process id** — and then `remove_dir_all` it. With
several agents in this workspace, a second `cargo test` of the same package deletes the directory
under the first run's test, which reads exactly like a broken fixture. `round3_reviewer.rs:51` was
given `format!("{name}-{}", std::process::id())` for precisely this reason, with a comment pointing at
the round-3 report; round 1 and round 2 were left behind, and this work's own fixtures
(`fixture_v1.rs`, `fixture_v2.rs`) carry the pid. **Finding, for the owners of those two files (both
are `*_reviewer.rs`, which this work item may not touch):** the same one-line change round 3 already
carries is what makes the gate's `test` step survive M2's parallelism.

Third attempt found the two stale contracts of §6.7 (`crates/doc`'s document claims and `mcp`'s
hand-written registry fixture), which are now fixed; the fourth is the gate itself:

```text
$ cargo xtask ci
  ok    fmt
  ok    clippy
  ok    test        (cargo test --workspace)
  ok    assets
  ok    brands
  ok    layers
  ok    wasm

CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)
```

So the milestone's own gate is green with this work in the tree, `wasm32-unknown-unknown` included.

### 9.4 M1's tests that had to change, and exactly how

A schema bump cannot leave every M1 assertion literally true, and pretending otherwise would be the
thing M2 §4.2 warns about. Four changes, all in files this work owns, none weakening a property:

| M1 assertion | Now |
|---|---|
| `the_fixture_round_trips_byte_identically` (v1 fixture) | Split: the v1 fixture is frozen and a **load writes nothing**; a **save migrates it to v2** and equals the v2 fixture byte for byte. §11.1's round trip is now the v2 fixture's (`fixture_v2.rs`), which is the only version a writer may produce (§2) |
| `a_second_save_of_a_reloaded_bundle_is_a_fixed_point` | Same property, run across the migration: save → v2 fixture, reload (**no** second migration), save → nothing moves |
| `the_fixture_refuses_a_newer_schema_version` | `"schema_version": 2` is now this build's version, so the test uses `SCHEMA_VERSION + 1`. The property (refuse, naming the version, never partially load) is unchanged |
| `the_fixture_is_the_bundle_the_schema_describes` | `assert_eq!(loaded.findings, Vec::new())` **still holds** — the migration is not a finding (§3). Additions: the single migration, and that every panel is unlocked |

`round1/2/3_reviewer.rs`, `comic_acceptance*` and every `*_reviewer.rs` are **untouched**; the three
`Panel` literals in `validate.rs`'s inline fixtures gained `locked: false` alongside the field.

### 9.5 Could a wrong implementation pass these tests? Four mutations, each restored

The M2 §4.1 lesson (*"ask could a wrong implementation satisfy this test?"*) applied to this work.
Each mutation: applied, run, reverted, and the tree re-verified (`fmt`, then the full crate suite).

| Mutation | Result |
|---|---|
| Remove `#[serde(default)]` from `Panel.locked` | **10 of 15** `fixture_v1` tests fail — the v1 fixture no longer parses. The default is what carries v1 forward, and it is pinned |
| Delete `bundle.normalize_declared_versions()` from `save`'s success path | `a_save_leaves_the_in_memory_model_at_the_version_it_wrote` fails (found by adding that test *after* the mutation showed the other five did not catch it — the byte-level tests pass, because `stage_versioned` writes the disk copy) |
| Make `set_declared_version` a no-op | `saving_the_v1_fixture_produces_the_v2_fixture_byte_for_byte` and `a_second_save_of_a_migrated_bundle_is_a_fixed_point` fail — the v2 fixture really is the writer's output, not an authored file |
| Drop the migration's `v2 adds Panel.locked …` note | `the_fixture_is_migrated_by_the_load_and_the_report_says_what_it_did` and `a_v1_bundle_that_declares_locked_…` fail — the report is pinned, not decorative |

The second row is the useful one: it is the case where the first three tests were *not* enough, and
the fix was a test rather than a weaker claim.

---

## 10. Revisions

`sha256` of every file this work changed (the review document does not carry its own):

```text
crates/comic-model/src/entities.rs        92a777ed28757902441e51bd1d16f48c8675f11269b5d8467c2b4a1c76f4f106
crates/comic-model/src/migrate.rs         f4192bb0a33675026ac007ba2ee9cb4c8c45a81000c025ad02a891b77f11c9ea
crates/comic-model/src/bundle.rs          0dcd929bbbf1b740dbef65f2d7308b59dfb32c62fffb1d789dc9f3bdf1c4ece7
crates/comic-model/src/lib.rs             106052b43cf13ec2aa3286eeca385062718ea160cc9d41b96fe29dad0f7a103a
crates/comic-model/src/error.rs           a607d15319680d1f0cfeb62ca835c3a7ebc4452dc724f673d91e1d93577c79c8
crates/comic-model/src/validate.rs        0cd445038efc8e06c315078f28125b98e42b4c4e5146118fe13200e352c73bf5
crates/comic-model/tests/fixture_v1.rs    e76fc34969cb864e328b68c210c7180b105e147a05796aec6a68355ba49b7084
crates/comic-model/tests/fixture_v2.rs    9a5d6285f897b4e55470280513e9fb82cdd82443e555a001b0919b35a14f4679
crates/comic-model/tests/schema_contract.rs 2db78d11396976fbded18c2bddb5094bfc552c975fc1167add8840e24ef52246
crates/doc/tests/schema_claims.rs        e8b7af71c5c2bd7e3298a4606b517ccce99dd6016d5403e9c7c3ddbab3b83b06
crates/mcp/tests/comic_headless.rs       fc63154d97a8953de90a805f91f945785f069072f335f2c161f469b57af644ec
crates/engine/src/cmd/comic/panel.rs      ecc0445e7b0fa0acd05d29c869801cb0c6cf758c38c94bc79a4fc29b3d0ba444
crates/comic-layout/src/reading_order/mod.rs c599b0b6267b3ae5b5cd6cae64a34f78f88b70a9650b1f47c7171df7d20d802a
docs/comiccraft-project-schema.md         e14c6d2079692b42d91ece4d5c45cbfe01130d0c2c3b6c6c300bef7facce4e58
ASSETS.md                                 d9f753deb85a4ef8f668473b6ad42f8eb344c56c15c9bcf3b0077ec7e0bf2114

tests/fixtures/v2/minimal.comiccraft/manifest.json                 6f9f3e55f9b564ea66ea77c6978949feb99eacda169a026290373999f14dc22d
tests/fixtures/v2/minimal.comiccraft/pages/page-8.json             efebecd4bbb33ec86a4e4758e9d4ff4d5a364385a57b3672b24d7ec283c4053e
tests/fixtures/v2/minimal.comiccraft/project.json                  bdf5ddc225197e130365b8156ca1b9b36bd163eacf604227253896ade781b651
tests/fixtures/v2/minimal.comiccraft/provenance/generations.json   44377affdc2ad504eb3635fde5cf420f211079d005ce545899306f472e263e38
tests/fixtures/v2/minimal.comiccraft/jobs/job-18.json              79daf912f08c3441d8d86c49acd5908539f2a9d38ef534da78b4c70cc77b387c
tests/fixtures/v2/minimal.comiccraft/assets/index.json             3d49783a1eb6fa12093edd6a1e8ad6ff4f5526d436b18d6f5d92365d63cd6d09
tests/fixtures/v2/minimal.comiccraft/assets/panels/panel-1.png     c54af4e9162be74adff0711701a4f1f656dc6a49f8c6acd9f9120aef38476081
tests/fixtures/v2/minimal.comiccraft/document/comic.vectorcraft    1ee906fa3cc5085e03e4bdce71fa577ad53058cfabf55280e40a00256e8b1685
tests/fixtures/v2/minimal.comiccraft/story/chapters.json           9d985a42e259740dbc692089accdf29ac6074eebb9258f3e373b0b3bb0b40ef0
tests/fixtures/v2/minimal.comiccraft/story/scenes.json             7ab80b4a08f8073faca4f1241df1450d07431dc2151cd3a8c01c5247dc838d17
tests/fixtures/v2/minimal.comiccraft/story/characters.json         421b287cd230d8a75adf42602001d175e9442799b0d658c30be70ef68f645873
tests/fixtures/v2/minimal.comiccraft/story/locations.json          66a68a19b5ff82233f94f4414067f4aed9eb4414824cb8163006713c8562457b
tests/fixtures/v2/minimal.comiccraft/story/styles.json             e2d66eeb67e1adc2109f385d3819890a83782cecc25e329ed09755a991094dad

v2 fixture tree (13 files, sorted, hashed)  4be4c745bca688ac271ce3eb048583689df75b7cf9080a9503523996f2297c5f
v1 fixture tree (unchanged from M1)         8a39a1e3728e7abfd225cecb64af08223b4e8f07593a696c265a898e19d1f234
```

**Toolchain.** `cargo 1.99.0` / `rustc 1.99.0`
(`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`). The system `cargo` is 1.75 and cannot
build this workspace (edition 2024, resolver 3), as the M0/M1 records note.

**Model.** `COPILOT_MODEL=deepseek-v4.1-flash`, stated by the environment. There is **no third
corroboration**: `~/.copilot/session-store.db :: assistant_usage_events` has **0 rows** for this
session (`bcb34430-fc9d-43ac-bcf6-98b25038e242`), the same gap M1's records report. Recorded as the
model I ran on, with the verification gap stated rather than papered over — per M1 §5.4e, if it could
not be determined, saying so is the correct answer, and what I have is one in-environment statement
rather than a determination.

**A disclosure, in the interest of an accurate record.** I ran `cargo fmt --all` once, which
reformatted in-flight files I do not own (`crates/comic-layout/src/{validate/mod.rs,lettering/plan.rs,template/mod.rs}`
carry mtimes from that window). Those are rustfmt output only — `cargo xtask ci`'s `fmt` step requires
exactly that formatting — but their revision hashes moved, and a concurrent report quoting a hash for
one of them may have taken it before that happened. W2's report discloses the same operation, so the
two runs' effects are not separable from the mtimes. Every later formatting I did was scoped to this
work's own files.

---

## 11. Unverified

- **`cargo xtask ci` was run three times and the fourth passed all 7 steps** (§9.3). The first two
  failures are recorded there because they are not this work's: an unformatted W4 file, then five
  spurious `round1_reviewer` failures from a concurrent run of the same test binary. The document
  claims and the `mcp` fixture the third run caught **are** this work's ripple (they pin the schema
  document or its version) and are fixed.
- **The engine's creator-facing surface of the migration** (§8): `comic.project.open` does not return
  `LoadedBundle.migrations`, and that file is off-limits here. The model's report is tested; the
  command layer's is not written.
- **Whether W3 will mirror `Panel.locked` onto the frame node**, and how the two are kept consistent
  if it does. §6.2 documents the relationship and states that the mirror is outside this document; no
  check exists for it, and this crate could not run one (§2).
- **Windows.** Untested, as in M1.
- **`CRAFT_FONTS` empty** — untouched by this work; no lettering path is involved. Every command above
  ran in this session's environment, which has no `CRAFT_FONTS_DIR`.
- **The mutation table is four mutations, not a harness.** Each was applied by hand, run, and
  reverted; there is no scripted mutation runner for this crate, so the evidence is the failure
  output quoted in §9.5 rather than a reproducible artefact.

Revision: working tree (base `e26f030d`) — `crates/comic-model/src/{entities,migrate,bundle,lib,error,validate}.rs`,
`crates/comic-model/tests/{fixture_v1,fixture_v2,schema_contract}.rs`,
`crates/comic-model/tests/fixtures/v2/minimal.comiccraft/**` (new; `fixtures/v1/**` **byte-identical to M1**),
`crates/engine/src/cmd/comic/panel.rs` (2 lines), `crates/comic-layout/src/reading_order/mod.rs` (1 line),
`crates/doc/tests/schema_claims.rs` (2 assertions + 1 helper), `crates/mcp/tests/comic_headless.rs` (1 fixture),
`docs/comiccraft-project-schema.md`, `ASSETS.md`; hashes in §10. Gate: `cargo xtask ci` all 7 steps green.
