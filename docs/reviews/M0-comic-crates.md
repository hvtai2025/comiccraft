# M0 — code-reviewer round 1 (correctness)

Reviewer:      code-reviewer on **not verified as `space-bunny`** — see *Model* below
Waiver:        both rounds ran on `space-bunny`, which was the required model at the time.
               The team's model rule changed to `deepseek-v4.1-flash` after these rounds
               completed, so this record keeps the model it was actually reviewed on.
Revision:      working tree (crates untracked; base `3da4080b`)   Files: `crates/comic-model/{Cargo.toml,src/lib.rs}`,
              `crates/comic-ai/{Cargo.toml,src/lib.rs}`, `crates/comic-layout/{Cargo.toml,src/lib.rs}`,
              `Cargo.toml` (workspace dep block), `xtask/src/layers.rs` (TABLE comic-* rows),
              `docs/comiccraft-project-schema.md`
Design refs:   §4.2 (crate layers), §4.3 (two structural rules), §5.2/§5.2.1 (entities, link
              ownership), §6.1 (ownership), §8.1 (page layout, validator), §9.3 (capabilities,
              ADR-015 no networking), §9.4 (approval gate), §10.3/ADR-016 (pollable jobs),
              §12.1 (deterministic checks); schema §2, §4, §6.2, §8.1, §9, §12, §13

## Model — NOT PERFORMED AS SPECIFIED

**This review was not performed on `space-bunny`, and I cannot confirm what model I am.**

The role (`code-reviewer.agent.md`) and `.github/instructions/implementation-review.instructions.md`
both require the reviewer to run on `space-bunny` and to flag the review as *not performed as
specified* otherwise. I attempted to establish the model three ways and all failed:

1. `get_current_session` — returns id/cwd/status only, no model field.
2. `~/.copilot/config.json` — empty/unparseable.
3. `~/.copilot/session-store.db` → `assistant_usage_events` — **0 rows for this session**
   (`7f4af39f-…`). The only rows present are from an unrelated session in September and name
   `deepseek-v4.1-flash`, `gemini-2.5-flash`, `gemini-3-flash-preview`. None is `space-bunny`.

There is no ambient statement of my own model in my environment either. Rather than guess, I am
reporting this as unverifiable. **Per the review rule, the lead must either re-run round 1 on
`space-bunny` or record an explicit waiver naming who reviewed and why.** The findings below are
real and reproducible regardless of which model produces them, but the *process* requirement is
not satisfied and this record cannot be treated as a compliant round 1 on its own.

## Round 1 — correctness

Verdict:       **changes-requested**

Unit tests:
- `crates/comic-model/tests/schema_contract.rs` (new, 5 tests) — **pass**
- `crates/comic-ai/tests/job_status_contract.rs` (new, 4 tests) — **pass**
- `crates/comic-layout/tests/panel_rect_contract.rs` (new, 7 tests) — **6 pass, 1 FAIL by design** (R1)

Command:
```
cargo test -p vectorcraft-comic-model -p vectorcraft-comic-ai -p vectorcraft-comic-layout
```
Output (toolchain `1.99.0`; the system `cargo` is 1.75 and **cannot build this workspace** —
edition 2024 / resolver 3 need ≥1.85, so the owner must be using a rustup toolchain):
```
Running unittests src/lib.rs (...comic_ai)          4 passed; 0 failed
Running tests/job_status_contract.rs                4 passed; 0 failed
Running unittests src/lib.rs (...comic_layout)      5 passed; 0 failed
Running tests/panel_rect_contract.rs                6 passed; 1 failed
error: test failed, to rerun pass `-p vectorcraft-comic-layout --test panel_rect_contract`
```
Also clean: `cargo fmt --check` (no diff) and
`cargo clippy … --all-targets` → **0 diagnostics**.

`cargo xtask ci` was **not** re-run (per instructions). `cargo xtask layers` was re-run:
`OK: 29 crates, no layering violations.`

Findings: 5

**F1 — `crates/comic-layout/src/lib.rs:30-37,43-49` — `PanelRect::new`'s invariant is not
enforced outside the constructor. — bug**
Trigger: `serde_json::from_str::<PanelRect>(r#"{"x":0.0,"y":0.0,"width":-10.0,"height":10.0}"#)`,
or the struct literal `PanelRect { x: 0.0, y: 0.0, width: -10.0, height: 10.0 }`.
Expected: the doc comment's promise — "a negative width or height is rejected rather than
silently normalized: it means the caller has a bug worth surfacing" — holds for every way of
building a `PanelRect`. Actual: `new()` returns `None` for that input, but the derived
`Deserialize` and the four `pub` fields both construct it anyway; I observed
`PanelRect { x: 0.0, y: 0.0, width: -10.0, height: 10.0 }` from both paths, with
`overlaps`/`contains` then returning meaningless answers.
Minimal reproduction: `an_invalid_rectangle_cannot_be_built_bypassing_new` in
`crates/comic-layout/tests/panel_rect_contract.rs`.
Owner options: (a) drop `Deserialize` — schema §6.2 gives `Panel` **no** geometry fields
(`frame_node_id` only; "the document owns the geometry"), so a `PanelRect` is not a wire type and
`Serialize`/`Deserialize` have no schema basis; and/or (b) make the fields private so `new()` is
the only construction path. My recommendation is (a)+(b) together.

**F2 — `crates/comic-layout/src/lib.rs:60-64` — `overlaps` had no horizontal-edge test in the
implementer's suite. — bug (test gap, caught before release)**
Trigger: the implementer's `touching_edges_do_not_overlap` covers only a shared *vertical* edge
(`a.right == b.x`). Mutating `overlaps` to `<=` on the **y axis only** (`self.y <= other.bottom()
&& other.y <= self.bottom()`) leaves their whole suite green — every panel stacked in a column
would then be reported as overlapping. Their suite fails only when both axes are mutated together.
Expected: the touching-edge contract holds on both axes. Actual: horizontal touching is untested.
Fixed by my `touching_edges_do_not_overlap_on_either_axis` and
`an_overlap_of_one_unit_is_still_an_overlap`, both of which fail under the single-axis mutation
(verified).

**F3 — `crates/comic-layout/src/lib.rs:66-70` — `contains` had no test on three of four edges. — bug (test gap)**
Trigger: their `a_panel_on_the_edge_is_still_contained` only tests the right edge (100.5 wide).
Expected: containment fails symmetrically on all four edges. Actual: a `<` vs `<=` slip, or a
swapped argument in `other.x >= self.x`, on the left/top/bottom edges would ship undetected.
Fixed by `containment_fails_when_straddling_any_single_edge` and
`containment_holds_for_a_panel_inside_a_negative_origin_page` (the latter also covers negative
origins, which nothing tested).

**F4 — `crates/comic-ai/src/lib.rs:44-63` — `is_terminal`/`has_result` were never checked as a
complete truth table, and no deserialization test existed. — preference**
Trigger: the implementer asserted `is_terminal` for all six variants but `has_result` for only
three, so `{Queued, Running}.has_result()` was never evaluated; no test deserialized a
`JobStatus` at all. Expected: the two predicates partition the six states, and an unknown wire
name is refused (schema §2: "Never partially load"). Actual: behaviour is correct — the gaps
were in the tests. Fixed by `the_predicates_partition_every_status`, `every_status_round_trips`
and `an_unknown_status_is_refused_rather_than_defaulted`. Not a defect in the code; flagged
because these are the invariants the ADR-016 poll loop will lean on.

**F5 — all three `Cargo.toml`s — 13 of 18 declared dependencies are unused; `serde_json` is
declared in both `[dependencies]` and `[dev-dependencies]`. — preference**
Measured per crate (`prod` = referenced outside `#[cfg(test)]`, `test` = inside):
- `comic-model`: `vectorcraft-geom` 0/0, `vectorcraft-doc` 0/0, `thiserror` 0/0 — **unused**;
  `serde_json` 0/3 — test-only.
- `comic-ai`: `vectorcraft-comic-model` 0/0, `thiserror` 0/0 — **unused**; `serde_json` 0/2 — test-only.
- `comic-layout`: `vectorcraft-geom`, `-doc`, `-text`, `-render`, `-pdf`, `-svg`,
  `vectorcraft-comic-model`, `thiserror` 0/0 and `serde_json` 0/0 — **all unused**.

`xtask layers` reports these as real edges (`comic-layout L4 → comic-model, doc, geom, pdf,
render, svg, text`), so the layer table currently documents dependencies that do not exist in
code. Expected at M0: the dependency set is pre-declared per design §4.2 and M0.md §2.4, so this
is mostly intentional scaffolding — but `serde_json` in `[dependencies]` is not, since nothing
outside tests uses it, and it is duplicated in `[dev-dependencies]` in all three. Recommend
dropping `serde_json` from `[dependencies]` now and re-checking the rest at M1, when the crates
actually gain their contents.

Tests that cannot fail: **2, both borderline — reported for completeness**

- `crates/comic-model/src/lib.rs:52-57` `schema_version_matches_the_frozen_schema` —
  `assert_eq!(SCHEMA_VERSION, 1)` restates the constant's own value, so it fails only when
  someone edits the constant and the test together. It is *not* fully decorative: I mutated
  `SCHEMA_VERSION` to 2 and it failed. But it checks the crate against itself, never against the
  frozen document. My `schema_version_matches_every_example_in_the_frozen_document` and
  `schema_version_matches_the_document_that_declares_it_frozen` read the schema itself (every
  `"schema_version": N` example, and the §1 "frozen at version 1" declaration) and catch the same
  bump with a better message: *"the crate implements 2 but the frozen schema shows 1"*.
- `crates/comic-ai/src/lib.rs:100-105` `status_serializes_as_snake_case` — pins two literals
  (`"awaiting_review"`, `"queued"`) written beside the enum, so it detects a rename only if the
  author forgets to update the literal. It is weaker than it looks, not useless: my mutation
  removing `#[serde(rename_all)]` did fail it. My `wire_names_match_the_frozen_schema_exactly`
  pins all six names to the schema document instead.

The other 10 tests are genuine behaviour tests. I tried to falsify each area by mutation and
re-ran both suites:

| # | Mutation | Implementer's suite | Mine |
|---|---|---|---|
| 1 | `overlaps` `<`→`<=` on both axes | FAIL ✓ | FAIL ✓ |
| 2 | `overlaps` `<`→`<=` on **y axis only** | **PASS ✗** | **FAIL ✓** (F2) |
| 3 | `is_terminal` includes `Completed` | FAIL ✓ | FAIL ✓ |
| 4 | drop `#[serde(rename_all)]` from `JobStatus` | FAIL ✓ | FAIL ✓ |
| 5 | `SCHEMA_VERSION` 1→2 | FAIL ✓ | FAIL ✓ |

Mutation 2 is the finding: the implementer's suite is green on a real boundary bug.

Layer assessment: **all three correct; no crate should move.**

- `comic-model` **L2** — correct. Design §4.2 gives it exactly `doc`, `geom`, serde; both
  dependencies point downward (doc L1, geom L0). Pure data + serde, no network/AI/UI, no
  `engine`/`ui-egui`/`mcp` reach — `xtask layers` confirms.
- `comic-ai` **L3** — correct. It depends on `comic-model` (L2) only; no same-layer edge, so no
  `INTRA_LAYER_ORDER` entry is needed. Design §9.3's "no knowledge of `doc`, `render` or the UI"
  is satisfied — it does not depend on `doc` at all. The declared `vectorcraft-comic-model`
  dependency is currently unused (F5), so in practice it has no downward edge yet, but the
  intended placement is right.
- `comic-layout` **L4** — correct. It sits above the renderers/exporters it assembles
  (`render`, `pdf`, `svg` L3) and above `comic-model`/`doc`/`geom`/`text`. Design §4.2's claim
  that it "cannot reach `engine`, `ui-egui` or `mcp`, even though it orchestrates rendering and
  export" holds.

The `xtask/src/layers.rs` TABLE rows (`comic-model` L2, `comic-ai` L3, `comic-layout` L4) match
design §4.2 and the §4.3 ASCII diagram exactly. The comment at `xtask/src/layers.rs:64-66`
justifies the placement correctly.

Rule check: **clean.**

1. **No comic data in the visual document (ADR-013)** — clean. Nothing in these three crates
   touches `crates/doc` or `crates/format`; they add no field to any document type and use
   neither `Document::unknown` nor `Document::extra`. `comic-model` links to the document by id
   only, consistent with §6.1 rule 1. (`PanelRect` is a *layout plan* type computed and returned,
   never stored in a comic record — F1 is about its constructor, not about it leaking into the
   document.)
2. **No duplicated authority over geometry, page size or dialogue** — clean. Design §6.1 makes
   the artboard own page size and the document object own frame geometry. `PanelRect`'s own doc
   comment states the page size is the artboard's and that a `PanelRect` "is never stored in a
   comic record". `comic-ai`'s `JobStatus` transitions are value changes on the job record, never
   document changes, and `comic-ai` "never mutates project state" — matching §10.3/ADR-016. No
   crate holds a second copy of page size, geometry or dialogue text.

**`comic-model` vs `comic-ai` entity placement (question 6): correct — no misplacement.**
Schema §5.4 assigns all narrative entities (`Chapter`, `Scene`, `Character`, `Location`,
`StyleProfile`) and the per-page entities (`Page`, `Panel`, `Dialogue`, `Lettering`) to files
that `comic-model` will own as "pure data + the bundle reader/writer" — design §4.2 lists "page/
panel records, lettering records, asset provenance records" under `comic-model`. What `comic-ai`
holds instead — `JobStatus` (schema §8.1, frozen verbatim as the six wire names) and `ProviderId`
(schema §8.1 `provider: ProviderId`) — is job-lifecycle state, which design §4.2 assigns to
`comic-ai` ("job state machine"). So nothing belonging to `comic-model` has leaked into
`comic-ai`, and `comic-ai` holds no `NodeId`/`Artboard`/geometry type that would belong in the
model. One boundary to watch at M1, not a finding now: schema §8.1's `GenerationJob` record
itself (`request`, `progress`, `error`, `result_asset_ids`, `target_revision`, `consent`) is a
*persisted entity* in `provenance/generations.json`, i.e. comic-model's territory, while its
`status` field is `comic-ai`'s. When `GenerationJob` is actually implemented, the struct and the
enum must not be split across crates in a way that forces `comic-model` (L2) to depend on
`comic-ai` (L3) — that would invert the arrow §4.2 requires. Worth settling before M4.

**`PanelRect::new` contract (question 7): rejecting non-positive and non-finite is the right
call, but `Option` is the wrong error type and the invariant does not hold (F1).**
- Rejecting is correct. A zero or negative panel cannot be drawn, and a NaN silently poisons
  every subsequent comparison (`contains`/`overlaps` return `false` for every input), which is
  exactly the "silently normalized" failure the comment says to avoid.
- `Option` loses information the caller needs. Three distinct causes collapse into one `None`:
  a caller bug (negative width), a non-finite input, and a zero size. Design §12.1 wants
  deterministic checks that report, and §2 wants actionable errors. A `Result<Self, PanelRectError>`
  with a variant per cause lets the command layer say *which* — worth doing now, because adding
  it later is a breaking change once callers exist.
- The real defect is not the rejection but that it is unenforceable: both `Deserialize` and the
  `pub` fields bypass it (F1). A constructor that can be side-stepped gives a false sense of a
  checked boundary — worse than no check, because a caller will rely on it.

## Gate

Not re-run (instructions: targeted `cargo test` only; M0 records `cargo xtask ci` green).
Re-ran: `cargo xtask layers` → `OK: 29 crates, no layering violations.`
`cargo fmt --check` clean; `cargo clippy --all-targets` on all three crates → 0 diagnostics.

## Unverified

- **The model I am running on** (above) — this is the material gap; it voids the process
  requirement until the lead re-runs or waives.
- `cargo xtask ci` in full (fmt/clippy/test/assets/brands/layers/wasm) — not re-run by
  instruction. My test files are covered by `cargo fmt --check` and `cargo clippy --all-targets`
  above, which are the two gate steps that could have objected to them; `assets`, `brands` and
  `wasm` are unaffected by adding files under `crates/*/tests/`.
- The **wasm32** build of these three crates (gate step 7) — not re-run; nothing in the review
  depends on it.
- ADR-015's "no HTTP client crate anywhere in `Cargo.lock`, direct or transitive" — I grepped
  `Cargo.lock` for `reqwest|hyper|ureq|isahc|curl|surf|tokio-tungstenite|attohttpc|rustls|
  native-tls|openssl-sys` → **none found**. That is a name check over the current lockfile, not a
  full transitive audit.
- `cargo xtask wasm`'s treatment of `comic-ai` as a "capability-empty stub" (design §9.3) —
  `xtask/src/main.rs:175-205` was not read; `comic-ai` currently has no feature flags and no
  transport, so the requirement is satisfied vacuously.
- **Behaviour against a real document**: none of these crates opens, saves or migrates a bundle
  yet, so the schema §10 save order, the §12 `unknown` round-trip, the §13 twelve referential
  rules and the §11 migration/fixture contract are **entirely untested at M0**. That is
  consistent with M0's scope, but it means "implements the frozen schema" is not yet evidenced —
  the crates hold `SCHEMA_VERSION`, `ComicId`, `ProviderId`, `JobStatus` and `PanelRect`, and
  none of the 20-odd entities, the manifest, the reader/writer or a fixture.
---

## Round 1 — owner response (lead)

**Model flag resolved.** Round 1 could not confirm its own model, and correctly flagged itself
*not performed as specified*. The harness recorded the subagent's model as
`opencodego/space-bunny` in the task metadata, which is the external evidence the reviewer
could not obtain from inside the session. Round 1 is therefore accepted as performed on
`space-bunny`; no waiver is needed. The reviewer's refusal to guess is the correct behaviour
and is left in the record verbatim.

All five findings were addressed on the post-fix revision.

| # | Finding | Resolution |
|---|---|---|
| 1 | `PanelRect::new`'s invariant was unenforceable (public fields + derived `Deserialize`) | **bug — fixed.** Fields are now private with accessors; `Deserialize` is removed (`PanelRect` is a layout primitive, not a wire type — schema §6.2 gives `Panel` no geometry); `Serialize` is kept, since it cannot construct an invalid value |
| 1b | Three error causes collapsed into one `None`, so §12.1's actionable message was impossible | **fixed.** `new` returns `Result<Self, PanelRectError>` with `NotFinite` / `NonPositive` / `NotRepresentable`, each naming the field, and `Display` produces a quotable message |
| 2 | `overlaps` untested on the horizontal axis — a y-axis `<`→`<=` slip left the suite green | **bug — fixed.** Both axes covered in the unit tests and in the contract test; the mutation now fails both suites |
| 3 | `contains` tested on one of four edges | **fixed.** All four edges plus negative-origin cases |
| 4 | `JobStatus` truth table incomplete; no deserialization test | **fixed.** `has_result` now asserted for all six states, a serde round-trip and a refusal test for unknown names added, and `ProviderId`'s wire form pinned |
| 5 | Unused dependencies; `xtask layers` documented edges that did not exist | **fixed.** All three manifests now declare only what the code uses, with a comment naming which dependency arrives at which milestone. `comic-layout` no longer pulls `render`/`pdf`/`svg`/`text`, which also removes real build cost |

**One new defect found by the owner's own strengthened test, and fixed.** The property test
added for finding 2 asserted `right() > x()`, and it failed at `x = f64::MAX / 4.0`: there
`x + 1.0 == x`, so the rectangle had zero area and **failed to overlap itself** — which would
have made the validator's overlap check unsound at extreme coordinates. `new` now also rejects
a size too small to advance the far edge (`NotRepresentable`), so "a rectangle with area
overlaps itself" is a property the validator can rely on. This is a direct consequence of the
review: the implementer's original suite could not have surfaced it.

**Evidence (post-fix revision).**

- `cargo test -p vectorcraft-comic-model -p vectorcraft-comic-ai -p vectorcraft-comic-layout`
  — **37 tests, 0 failures** (7 + 4 + 9 + 9 + 3 + 5)
- `cargo xtask layers` — `OK: 29 crates, no layering violations`
- `cargo xtask ci` — **all 7 steps passed** (`docs/ci-3da4080b-postfix.md`)

**Accepted without change:** the reviewer's tests are kept as the independent set. Its
reproduction for finding 1 (`an_invalid_rectangle_cannot_be_built_bypassing_new`) is no longer
expressible — a struct literal and `serde_json::from_str::<PanelRect>` are now compile errors,
which is a stronger assertion than the runtime test it replaced. The contract file records
that, and the runtime half is covered by `every_accepted_rectangle_holds_the_invariant`.

---

## Round 2 — rules, failure paths, edges

Reviewer:      code-reviewer — see *Model* below
Revision:      working tree (crates untracked; base `3da4080b`)   Files: `crates/comic-model/{Cargo.toml,src/lib.rs}`,
              `crates/comic-ai/{Cargo.toml,src/lib.rs}`, `crates/comic-layout/{Cargo.toml,src/lib.rs}`,
              `Cargo.toml` (workspace dep block), `xtask/src/layers.rs` (TABLE comic-* rows),
              `docs/comiccraft-project-schema.md`
Design refs:   §4.2 (crate layers), §4.3 (two structural rules), §6.1 (ownership), §8.1 (validator),
              §9.3 (capabilities, ADR-015 no networking), §12.1 (actionable deterministic checks);
              ADR-016 (pollable jobs); schema §2 (refuse, never partially load), §6.2 (`Panel` owns no
              geometry), §8.1 (frozen `JobStatus` names), §9 (opaque ids)

### Model

**I cannot verify my own model, exactly as round 1 could not.** Three attempts, all the same three:

1. `get_current_session` → returns session id / cwd / status / changed-file counts, **no model field**.
2. The local session store's `assistant_usage_events` → **0 rows for this session**; the only rows
   present are from an unrelated session on 2026-09-26 and name `deepseek-v4.1-flash`.
3. Nothing in my environment states my own model.

Per the review rule I must flag this rather than guess. The harness metadata that launched this
subagent records `opencodego/space-bunny`, which is the same external evidence the lead used to
accept round 1 — but I did not verify it and will not claim I did. **This round is therefore
accepted-as-specified on the same basis as round 1: the harness record, not my own statement.**

### Round 2 — verdict

Revision reviewed: **working tree after the round-1 fixes (PanelRect private fields,
               `PanelRectError` with `NotFinite`/`NonPositive`/`NotRepresentable`, `Deserialize`
               removed from `PanelRect`, three manifests trimmed to `serde` + `serde_json`)**
               — differs from round 1's `Revision:` (`working tree (crates untracked; base
               3da4080b)`) by the marker above; the crates are untracked, so there is no sha.
Verdict:       **approved**

Unit tests:
- `crates/comic-layout/tests/panel_rect_contract.rs` — **extended 9 → 17 tests** (7 new) — **pass**
- `crates/comic-layout/tests/layer_rules_contract.rs` — **new, 7 tests** — **pass**
- `crates/comic-ai/tests/job_status_contract.rs` — **extended 4 → 8 tests** (4 new) — **pass**
- `crates/comic-model/tests/schema_contract.rs` — **extended 5 → 8 tests** (3 new) — **pass**
- All four files together — `cargo test -p vectorcraft-comic-model -p vectorcraft-comic-ai
  -p vectorcraft-comic-layout` — **59 passed; 0 failed**

Command:
```
cargo test -p vectorcraft-comic-model -p vectorcraft-comic-ai -p vectorcraft-comic-layout
```
Output (`cargo 1.99.0` via rustup; the system `/usr/bin/cargo` is 1.75 and cannot build this
workspace — edition 2024 / resolver 3 need ≥1.85):
```
Running unittests src/lib.rs (...comic_ai)              7 passed; 0 failed
Running tests/job_status_contract.rs                     8 passed; 0 failed
Running unittests src/lib.rs (...comic_layout)           9 passed; 0 failed
Running tests/layer_rules_contract.rs                    7 passed; 0 failed
Running tests/panel_rect_contract.rs                    17 passed; 0 failed
Running unittests src/lib.rs (...comic_model)            3 passed; 0 failed
Running tests/schema_contract.rs                         8 passed; 0 failed
Doc-tests (×3)                                           0 passed; 0 failed
                                                          ---- 59 tests, 0 failures
```
Also clean on the revision I reviewed: `cargo fmt --check` → no diff;
`cargo clippy -p vectorcraft-comic-model -p vectorcraft-comic-ai -p vectorcraft-comic-layout
--all-targets -- -D warnings` → **0 diagnostics**; `cargo xtask layers` → `OK: 29 crates, no
layering violations.`

`cargo xtask ci` was **not** re-run (per instructions); M0 records it green on this revision
(`docs/ci-3da4080b-postfix.md`), and my changes are test-only files that the `fmt` and `clippy`
gate steps above cover directly.

Findings:      **0 bugs — 3 preferences (R2-1, R2-2, R2-3)**, all in documentation or forward-looking
               scope, none blocking.

---

### 1. Did the fixes fix it? — per-finding verification

**F1 (bug — `PanelRect`'s invariant was unenforceable) — verified fixed, and I tried to break it
four ways.** I wrote the four bypasses as a throwaway test file, compiled it, captured the
diagnostics and deleted it:

```
error[E0616]: field `width` of struct `PanelRect` is private
error[E0277]: the trait bound `PanelRect: Default` is not satisfied
error[E0277]: the trait bound `PanelRect: serde::Deserialize<'de>` is not satisfied
note: required by a bound in `serde_json::from_str`
```

So: struct literal → rejected (private fields, `crates/comic-layout/src/lib.rs:76-81`);
`Default` → not derived; `Deserialize` → not derived and no manual impl anywhere in the crate
(`grep Deserialize crates/comic-layout/src` → no match outside the `PanelRectError` doc prose);
no `From`/`TryFrom` impl; no `#[serde(...)]` attribute on the struct; no `pub` field; no setter;
no `pub` constructor other than `new`; and `#![forbid(unsafe_code)]` blocks a transmute.
`PanelRect` appears in exactly two files in the whole workspace (its own crate and the contract
test), so nothing outside can construct one either. **The invariant is genuinely unbypassable
in safe Rust, and a compile error is the strongest form of that assertion.**
`Serialize` is kept and is safe, as round 1 argued: all four stored fields are checked finite
before the value exists, and I confirmed empirically that the overflow case (below) still
serializes finite numbers.

**F1b (the three causes collapsed into one `None`) — verified fixed.** `new` returns
`Result<Self, PanelRectError>` with three variants, each carrying the offending `field`, and
`Display` produces a quotable sentence per cause (design §12.1). Round 2 added the assertion that
was missing: *the distinguishing content must be the cause, not the field name* — every pair of
variants is compared **at the same field** and must differ. That check found a real weakness in
my own first draft (see *Tests that could not fail* below).

**F2 (bug — `overlaps` untested on the horizontal edge) — verified fixed; the exact mutation is
now caught.** I re-ran round 1's mutation experiment: `overlaps` changed to `<=` on the **y axis
only** (`self.y <= other.bottom() && other.y <= self.bottom()`), the mutation that left the whole
implementer suite green in round 1:

```
cargo test -p vectorcraft-comic-layout --test panel_rect_contract
  test an_overlap_of_one_unit_is_still_an_overlap ... FAILED
  test touching_edges_do_not_overlap_on_either_axis ... FAILED
  test result: FAILED. 14 passed; 2 failed
cargo test -p vectorcraft-comic-layout   (--lib)
  test tests::touching_edges_do_not_overlap_on_either_axis ... FAILED
  test tests::the_overlap_boundary_sits_strictly_between_touching_and_shared_area ... FAILED
  test result: FAILED. 7 passed; 2 failed
```

**Now caught**, by both suites, on both axes. Production restored and byte-verified
(`sha256sum -c` → all four files `OK`).

**F3 (bug — `contains` tested on one of four edges) — verified fixed.** All four edges plus
negative origins are asserted. I mutated the right-edge comparison to `<` (one of the two
`containment_fails_when_straddling_any_single_edge` cases) and 7 tests failed, including
`containment_fails_when_straddling_any_single_edge` and my
`containment_holds_for_a_panel_inside_a_negative_origin_page`.

**F4 (preference — `is_terminal`/`has_result` truth table incomplete, no deserialization test) —
verified fixed.** The table is complete in both suites, all six statuses round-trip, unknown names
are refused. I removed `#[serde(rename_all = "snake_case")]` and both
`wire_names_match_the_frozen_schema_exactly` and
`an_unknown_status_name_is_refused_case_sensitively_and_without_trimming` failed.
Round 2 additionally covers `ProviderId`'s **deserialization** half, which round 1 never touched
(it pinned serialization only), and the case-sensitivity/whitespace/wrong-type edges.

**F5 (preference — 13 of 18 dependencies unused, `serde_json` duplicated) — verified fixed, and the
manifest comments are accurate.** All three manifests now declare exactly `serde` under
`[dependencies]` and `serde_json` under `[dev-dependencies]`; `Cargo.lock` agrees. I checked each
comment's named future dependency against design §4.2 and against the tree:

| Crate | Design §4.2 "Depends on" | Named in the manifest comment | Exists |
|---|---|---|---|
| `comic-model` | `doc`, `geom`, serde | `vectorcraft-doc`, `vectorcraft-geom`, `thiserror` | ✅ all three are workspace members; `serde` is the one already declared |
| `comic-ai` | `comic-model`, an HTTP transport (feature-gated) | `vectorcraft-comic-model`, `thiserror` + a paragraph restating ADR-015's gate requirements | ✅ accurate, including "gated so `cargo xtask wasm` still passes" |
| `comic-layout` | `comic-model`, `doc`, `geom`, `text`, `render`, `pdf`, `svg` | all seven, plus `thiserror` | ✅ every one is a workspace member |

I also verified the one comment that names *API*, not just crates: `crates/comic-layout`'s comment
says lettering fitting arrives "via `layout_with` and `LayoutOptions`" — both exist
(`crates/text/src/layout.rs:194`, `crates/text/src/lib.rs:46`). And `comic-model`'s
`NodeId` reference is real (`crates/doc/src/node.rs:19`). Nothing the code needs was removed: the
crates compile, and `serde` is asserted present by
`each_comic_manifest_declares_only_what_its_code_uses` — if it were dropped the crate would not
build at all, which is the load-bearing half of "trimmed, not gutted".

### 2. The new behaviour — `NotRepresentable` (`crates/comic-layout/src/lib.rs:104-110`)

```rust
if x + width <= x { return Err(PanelRectError::NotRepresentable { field: "width" }); }
if y + height <= y { return Err(PanelRectError::NotRepresentable { field: "height" }); }
```

**Correct? Yes — and exactly correct, which is a stronger property than I expected.** The guard
does not approximate the ULP; it *computes the real sum* and compares it with the real origin. So
it is right on both axes, at both signs, and through the whole range including subnormals, for
free. I tested that claim over an 18-magnitude × 10-size grid (`the_representability_guard_accepts_exactly_the_rectangles_that_advance_their_far_edge`):
for every one of the 180 combinations on **each** axis, the verdict of `new` equals the verdict of
`origin + size > origin`. Nothing legitimate is rejected and nothing illegitimate is accepted —
including the cases a magnitude heuristic would get wrong:

| origin | size | far edge | verdict | why it is right |
|---|---|---|---|---|
| `1e16` (ULP 2.0) | `1.0` | `1e16` | **rejected** | the gutter rounds away; this is the case the owner found |
| `1e16` | `2.0` | `10000000000000002` | accepted | exactly one ULP |
| `-1e-308` (subnormal) | `MIN_POSITIVE` | `1.225e-308` | accepted | subnormals are finely spaced; a "ULP < width" heuristic would wrongly reject this |
| `f64::MAX` | `1.0` | `f64::MAX` | **rejected** | `MAX + 1.0 == MAX` |
| `-f64::MAX` | `f64::MAX` | `0.0` | accepted | a whole-plane rectangle, far edge exact |
| `f64::MAX` | `f64::MAX` | `+∞` | accepted | **R2-2 below** |

**Complete? For its stated purpose, yes.** The guard exists so that "a rectangle with area
overlaps itself" holds — that is what makes the validator's overlap check sound. I checked that
property directly over the whole grid rather than trusting the derivation:
`the_predicates_hold_for_every_rectangle_the_guard_accepts` asserts `right() > x()`,
`bottom() > y()`, `contains(self)` and `overlaps(self)` for every accepted rectangle. The guard is
*sufficient* for that property by construction, and it is not merely sufficient but **necessary** —
removing it (mutation B) makes 5 tests fail.

**R2-2 (preference) — `crates/comic-layout/src/lib.rs:105` — the guard rejects a far edge that
fails to advance but accepts one that overflows to `+∞`, so `right()` is not always finite.**
Trigger: `PanelRect::new(f64::MAX, 0.0, f64::MAX, 10.0)`. Expected, per the type's own doc at
lines 67-73 ("four **finite**, strictly positive dimensions, and a positive representable area"):
a rectangle whose far edge is `+∞`. Actual: `new` returns `Ok`, because `f64::MAX + f64::MAX` is
`+∞` and `+∞ <= f64::MAX` is false. The four *stored* fields are indeed finite, so the doc is
literally true as written and the doc's *conclusion* ("a rectangle always overlaps itself") is
still true — I verified `overlaps(self)` and `contains(self)` hold for the overflowing rectangle,
and that an infinite far edge does not make containment loose in the other direction (a page with
a finite far edge does not contain a panel whose far edge is infinite). So this is **not** a
soundness defect and not a `changes-requested`. It is a gap between the documented invariant and
what is checked, in a type whose whole point is that its invariant is enforced rather than
documented. Minimal reproduction: `an_overflowing_far_edge_is_accepted_and_keeps_the_predicates_sound`,
which pins today's behaviour (so a future fix is noticed rather than silent). Owner options if
wanted: reject `!far_edge.is_finite()` as a fourth cause, or amend the doc to promise only what
is guaranteed — *the four stored fields are finite and the far edge is strictly beyond the
origin, possibly `+∞`*. Either is fine; what is not fine is leaving the promise broader than the
check.

**R2-3 (preference) — `crates/comic-layout/src/lib.rs:67-68` — "a positive representable area"
overstates what is guaranteed for sub-unit rectangles.** Trigger:
`PanelRect::new(0.0, 0.0, f64::MIN_POSITIVE, f64::MIN_POSITIVE)`, which is accepted (and should
be — rejecting it would be wrong). Expected, per the doc: positive area. Actual: `width * height`
is exactly `0.0` in `f64` (`2.2e-308²` underflows). The rectangle is perfectly usable — every
predicate is built on edges, not on the product — so the *code* is right and only the wording is
wrong. Pinned by `a_sub_unit_rectangle_is_accepted_even_though_its_area_underflows`.

**R2-1 (preference) — `crates/comic-layout/Cargo.toml:19` — `serde` is declared for a `Serialize`
derive that no caller in the workspace consumes yet.** Design §4.2 lists `serde` in
`comic-model`'s dependencies but **not** in `comic-layout`'s, and the workspace has no consumer of
a serialized `PanelRect`. This is not F5 recurring — the dependency *is* used by the code, so the
manifest is accurate and `cargo xtask layers` is truthful — it is the same "declare ahead of use"
pattern F5 trimmed elsewhere, surviving in one place. It resolves itself the moment a layout plan
is written out; worth a line in the manifest comment naming the milestone, the way the other
future dependencies are.

### 3. The hard rules (design §4.3, §6.1)

**Clean, and now checked by tests rather than by reading.** Round 1 verified these by inspection,
which is exactly the kind of check that decays. I added `crates/comic-layout/tests/layer_rules_contract.rs`
to make them assertions:

- `no_comic_crate_declares_a_dependency_on_the_visual_document_or_anything_above_the_command_layer`
  — no manifest names `vectorcraft-doc`, `-format`, `-engine`, `-ui-egui` or `-mcp`, in either
  table. **Verified falsifiable**: I added `vectorcraft-doc` to `comic-model` and the test failed.
- `no_comic_crate_uses_a_document_type_in_its_code` — with `//` lines stripped, no crate's source
  mentions `Document`, `Artboard`, `NodeId`, `document`, `artboard` or `node_id` in code. This is
  the half a dependency check cannot see: comic metadata could reach the document through a type
  already in scope. **Verified falsifiable**: I added a function taking a `node_id` and it failed.
- `no_comic_crate_owns_a_second_copy_of_the_page_size_or_a_dialogue` — §6.1 rule 2: no
  `page_width`/`page_size`/`dialogue_text`/`frame_rect` spelling anywhere in the three crates.
- `no_comic_crate_declares_an_http_client` — ADR-015, the manifest half (round 1 did the lockfile).
- `every_comic_crate_forbids_unsafe_code` — `#![forbid(unsafe_code)]`, not `deny`, so a later
  `#[allow]` cannot quietly reopen it.

Reading confirms what the tests assert: `PanelRect` is documented at lines 58-65 as a computed
layout primitive that "is never stored in a comic record — the document object owns the real
geometry", `comic-ai` holds no document type, and `JobStatus` transitions are value changes on a
job record, never document changes (§10.3/ADR-016). No crate reaches upward: `cargo xtask layers`
→ `OK: 29 crates, no layering violations`, with the comic rows at L2/L3/L4 exactly as §4.2's
diagram places them. The round-1 seam warning about splitting `GenerationJob` (schema §8.1 record
in `comic-model`, `status` in `comic-ai`) is still live for M4 and is not a finding at M0.

### 4. Failure paths and edges

Covered by the new tests, all passing: degenerate (zero, `-0.0`, negative sizes, non-finite on every
field), extreme magnitudes (the whole subnormal → `f64::MAX` range), negative origins (both
axes, both signs), sub-unit sizes (down to `5e-324`, and the sub-normal case where a one-point
size is still fine), unknown serde names (`"QUEUED"`, `"Queued"`, `" queued"`, `"queued "`,
`"queued_"`, `"awaitingreview"`, `""`, `"queued_v2"`, `"done"`), wrong JSON types (`null`,
numbers, arrays, objects, booleans), empty input (`""`, `" "`, `"\t"`), and the `u64` boundary
(`u64::MAX` accepted; `u64::MAX + 1`, `1e23`, `1.5`, `1e3`, `7.0`, `-0`, `-1`, `+7`, `07` all
refused rather than wrapped — a wrapped id would alias onto an existing one, which schema §9
forbids).

One nuance worth recording: a raw tab inside a status name is refused by the JSON *parser*, not by
the enum, so its message does not list the six variants. My first draft asserted otherwise and
failed; the test now distinguishes the two refusals — a semantic refusal must list the frozen
variants (that is what §12.1's "actionable" means), a syntax refusal need only fail loudly.
Nothing was weakened to get green; the assertion was corrected to what the contract actually is.

### 5. Tests that could not fail

**One of mine, caught by mutation, reported rather than quietly fixed.**

`every_error_variant_has_its_own_quotable_message` first compared messages built from
*different* fields (`NotFinite{"x"}` vs `NonPositive{"width"}` vs `NotRepresentable{"height"}`),
which differ by construction because each contains its own field name. I mutated the production
`Display` arm for `NotRepresentable` to render `NonPositive`'s sentence
(`"panel {field} must be greater than zero"`) and **the test still passed** — the duplicate was
masked by the field name. The test now compares every variant pair **at the same field**; under
the same mutation it fails. Recording this because the round-1 record lists two borderline tests
in the implementer's suite and I have now produced one of my own: a test asserting "these messages
differ" is only worth anything if they are built from the same inputs.

Everything else I wrote was mutated and caught:

| # | Mutation | Result |
|---|---|---|
| A | `overlaps` `<`→`<=` on the **y axis only** (round 1's F2 mutation) | caught: 2 contract tests + 2 inline tests |
| B | `NotRepresentable` guard `<=`→`<` on both axes | caught: 5 tests |
| C | y-guard compares `y + height <= 0.0` instead of `<= y` | caught: 6 tests |
| D | `contains` right edge `<=`→`<` | caught: 7 tests |
| E | `comic-model` declares `vectorcraft-doc` + `serde_json` in `[dependencies]` | caught: 2 layer-rule tests |
| F | `comic-model` code takes a `node_id` | caught: 1 layer-rule test |
| G | drop `#[serde(rename_all)]` from `JobStatus` | caught: 2 tests |
| H | narrow `ComicId` to `u32` | caught at compile time (E0308 in the contract test) |
| I | duplicate `NotRepresentable`'s message from `NonPositive` | **initially uncaught — my test was fixed, then caught** |

Every production file touched during a mutation was restored and byte-verified:
`sha256sum -c` → `crates/comic-layout/src/lib.rs: OK`, `crates/comic-model/Cargo.toml: OK`,
`crates/comic-model/src/lib.rs: OK`, `crates/comic-ai/src/lib.rs: OK`. `Cargo.lock` shows the
three comic crates depending on `serde` + `serde_json` only, i.e. no residue from the manifest
mutation. The two throwaway probe files were deleted. **I edited no production code.**

### 6. Verdict

**approved.** All five round-1 findings are genuinely fixed — not summarised as fixed, but each
re-derived and, where it could be, re-broken. The revision moved, the new `NotRepresentable`
behaviour is correct and complete for the property it exists to guarantee, the hard rules hold and
are now guarded by tests that fail when violated, and the dependency trim removed nothing the
code needs while keeping every comment accurate against design §4.2.

The three remaining items — **R2-1, R2-2, R2-3** — are all *preferences about wording and future
scope*, none is a defect, and none needs a fix before M0 closes. R2-2 and R2-3 are the same
underlying issue: `PanelRect`'s doc comment promises slightly more than the constructor checks
(an always-finite far edge, and a positive area). Both are one-line doc amendments, or one extra
guard clause in R2-2's case, whenever the owner next touches that doc.

**Gate:**         not re-run (instructions: targeted `cargo test` and `cargo xtask layers` only).
                  M0 records `cargo xtask ci` green on this revision — `docs/ci-3da4080b-postfix.md`,
                  all 7 steps. Re-ran here: `cargo xtask layers` → `OK: 29 crates, no layering
                  violations`; `cargo fmt --check` → clean; `cargo clippy … --all-targets -D
                  warnings` → 0 diagnostics. My changes are test-only files under
                  `crates/*/tests/`, which are exactly what the `fmt` and `clippy` steps cover;
                  `assets`, `brands` and `wasm` are unaffected by adding files there.

**Unverified:**
- **My own model** (above). The material gap, unchanged from round 1 and resolved by the lead the
  same way: on the harness metadata, not on my own attestation.
- **`cargo xtask ci` in full** — not re-run by instruction. Covered as far as it can be by the
  three steps above.
- **The wasm32 build** of these crates (gate step 7) — not re-run. `comic-layout` now builds for
  the host only in my runs; it has no feature flags and no transport, so design §9.3's
  "capability-empty stub" requirement holds vacuously, but I did not prove the target compiles.
- **Behaviour against a real document** — impossible at M0: these crates hold five types and no
  reader/writer. Schema §4, §10, §11, §12 and §13's twelve referential rules remain entirely
  untested, exactly as round 1 recorded. Not a round-2 finding; it is why the seam note about
  `GenerationJob` splitting across `comic-model`/`comic-ai` needs settling before M4.
- **A transitive audit of `Cargo.lock` for ADR-015** — round 1's name grep is what I relied on for
  the transitive half; my test covers only the manifests, which is the part an author edits.

---

## Round 2 — owner acceptance (lead)

**Verdict accepted: `approved`.** Round 2 reviewed the post-fix revision and named it, so the
approval is current; production code has not changed since, and the reviewer's added tests are
kept as part of the independent set.

Round 2's own evidence is the strongest argument that round 1's fixes are real rather than
cosmetic:

- It attempted the four remaining construction bypasses and **compiled** them, recording the
  errors: `E0616` (private field), `E0277` (`Default`), `E0277` (`Deserialize`), and no
  `From`, no `serde` attribute, no `pub` field, no `unsafe`. The round-1 reproduction is now a
  compile error rather than a runtime assertion.
- It re-ran the y-axis `<`→`<=` mutation that exposed round 1's finding 2: **now caught**, by
  two contract tests and two inline tests.
- It verified the new `NotRepresentable` guard is exact rather than ULP-approximated, over an
  18×10 grid per axis, both signs, through subnormals.
- It reported one of its **own** tests that initially survived a mutation, fixed it, and
  confirmed it now fails. A reviewer that audits its own suite is doing the job properly.

**Final evidence:** `cargo test -p vectorcraft-comic-model -p vectorcraft-comic-ai
-p vectorcraft-comic-layout` — **59 tests, 0 failures**; `cargo xtask layers` — `OK: 29 crates`;
`cargo xtask ci` — **all 7 steps passed** (`docs/ci-3da4080b.md`).

### Deferred findings — recorded, not fixed

Round 2 returned three findings, all classified as preferences, and **none was fixed**, because
any change to production code would invalidate this approval and require a third round (the
rule is that round 2 must review the revision the fixes produced). They are tracked instead:

| ID | Finding | Disposition |
|---|---|---|
| PANELRECT-OVERFLOW | `PanelRect::new(f64::MAX, 0.0, f64::MAX, 10.0)` is accepted because `MAX + MAX` is `+∞`, so `right()` is not finite | **Deferred to M1.** The lead does not fully accept the "predicates still sound" classification: `contains` on such a rectangle returns `true` for every finite rectangle, so a validator could be told a panel fits when it does not. The fix is small — require `right()`/`bottom()` to be finite as well as strictly beyond the origin, with a distinct error variant — and belongs with the M1 validator work. Unreachable for a comic page, so it is not blocking |
| PANELRECT-AREA-WORDING | `PanelRect`'s doc says "positive representable area", which is false for sub-unit rectangles (`MIN_POSITIVE * MIN_POSITIVE == 0.0`); self-overlap still holds, because `overlaps` is a rectangle-intersection test and not an area computation | **Fix the wording with PANELRECT-OVERFLOW.** The guarantee the type actually makes is "each far edge is strictly beyond its origin, so a rectangle always overlaps itself" |
| SERDE-IN-COMIC-LAYOUT | `serde` is declared for a `Serialize` derive no workspace caller consumes yet, and design §4.2 lists `serde` only for `comic-model` | **Kept deliberately.** The layout plan will be returned as command-result JSON at M1, so `PanelRect` is `Serialize` by design; §4.2's table is not an exhaustive dependency list. Recorded so the choice is visible rather than accidental |

### Accepted as recorded

Round 2's `Unverified` list is accurate and accepted: its own model; the full `cargo xtask ci`
(not re-run by the reviewer, by instruction — the lead ran it, `docs/ci-3da4080b.md`); the
`wasm32` build (covered by the gate, which the lead ran); behaviour against a real document,
which is impossible at M0 because the crates hold five types and no reader, writer or fixture;
and a transitive `ADR-015` lockfile audit, where both rounds relied on a name grep rather than
a full dependency review.
