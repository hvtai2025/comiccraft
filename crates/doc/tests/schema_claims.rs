//! Reviewer tests (`code-reviewer`, M0 schema round 1).
//!
//! `docs/comiccraft-project-schema.md` makes several claims about *existing* VectorCraft
//! behaviour rather than about future comic code: the artboard-id reuse hazard (§9, §9.1), the
//! artboard rect being the page size in points (§5.3), the FNV-1a link hash (§7.2), the optional
//! document-folder-relative link path (§3, §7.1), and the `Document::unknown` / `Document::extra`
//! round-trip maps (§1, §12). Each test below pins exactly one of those claims against the code
//! the document cites, so a change here makes the document false loudly instead of silently.
//!
//! The repository's policy (`clippy.toml`) allows panics in tests; clippy only applies that to
//! `#[cfg(test)]` modules, so this is the file-level equivalent for an integration test.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use vectorcraft_doc::links::hash_bytes;
use vectorcraft_doc::{Artboard, Document, LinkInfo, NodeId};
use vectorcraft_geom::Rect;

fn artboard(id: u32) -> Artboard {
    Artboard { id, name: format!("Artboard {id}"), rect: Rect::new(0.0, 0.0, 100.0, 100.0), show_center_mark: false, show_cross_hairs: false }
}

// -------------------------------------------------------------------------------------------------
// §9.1 "The artboard-id hazard" — `Artboard.id` is allocated `max(id) + 1` and may be reused.
// -------------------------------------------------------------------------------------------------

#[test]
fn artboard_ids_are_reused_after_the_highest_artboard_is_deleted() {
    // Schema §9: "the document's `max(id) + 1`", and §9.1: "deleting the highest-id artboard lets
    // the next one take that id". `crates/doc/src/lib.rs:1028-1030` (`next_artboard_id`).
    let mut d = Document::new(595.0, 842.0);
    assert_eq!(d.artboards[0].id, 1, "`Document::new` starts at artboard 1");
    d.artboards.push(artboard(2));
    assert_eq!(d.next_artboard_id(), 3);

    d.artboards.retain(|a| a.id != 2);
    assert_eq!(
        d.next_artboard_id(),
        2,
        "a retired artboard id is handed out again — this is why `manifest.retired_artboard_ids` \
         (§4) and mitigation 2 of §9.1 exist; a `Page.artboard_id` of 2 would silently resolve to \
         the new artboard 2"
    );
}

#[test]
fn artboard_id_is_u32_and_the_rect_is_document_points() {
    // Schema §9 table ("Page artboard | `u32` (`Artboard.id`)") and §5.3 ("`PageSize` … in
    // document points, mirroring `Artboard.rect`").
    let d = Document::new(595.0, 842.0);
    let a = &d.artboards[0];
    let id: u32 = a.id;
    assert_eq!(id, 1);
    let r: Rect = a.rect;
    let (x0, y0, x1, y1): (f64, f64, f64, f64) = (r.x0, r.y0, r.x1, r.y1);
    assert_eq!((x0, y0, x1, y1), (0.0, 0.0, 595.0, 842.0), "A4 at 72dpi is 595×842 points, not pixels");
}

// -------------------------------------------------------------------------------------------------
// §7.2 "Two hashes, two jobs" — the link hash is FNV-1a 64-bit, not an integrity hash.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_link_hash_is_fnv1a_64_bit_not_a_sha256_integrity_hash() {
    // Schema §7.2: "`LinkInfo.hash` is FNV-1a 64-bit … it is *not* collision-resistant", and
    // `Asset.content_hash` is the SHA-256 that integrity and provenance rely on.
    // `crates/doc/src/links.rs:76-79` (`hash_bytes`).
    assert_eq!(hash_bytes(b""), "imgcbf29ce484222325", "FNV-1a 64 of nothing is the offset basis, tagged `img`");
    let h = hash_bytes(b"ComicCraft");
    assert!(h.starts_with("img"), "the link hash carries its `img` tag: {h}");
    assert_eq!(h.len() - 3, 16, "a 64-bit hash is 16 hex digits: {h}");
    assert!(h[3..].chars().all(|c| c.is_ascii_hexdigit()), "lowercase hex per §7.2's shape for a content hash: {h}");
    assert_eq!(hash_bytes(b"ComicCraft"), h, "the hash is deterministic");
    assert_ne!(hash_bytes(b"comiccraft"), h, "…and case-sensitive");
}

// -------------------------------------------------------------------------------------------------
// §3 / §7.1 — the linked path is optional, and its base is the document's own folder.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_link_relative_path_is_optional_and_skipped_when_unset() {
    // Schema §7.1 makes `StorageRef::Linked { relative_path }` required, and §4 requires `files`
    // to cover every file — but `LinkInfo.relative` itself is an `Option` that is only written on
    // save (`crates/doc/src/links.rs:25-28`), so a reader must not demand it.
    let l = LinkInfo::new("/work/art/photo.jpg");
    assert!(l.relative.is_none(), "a fresh link knows its absolute path only");
    let json = serde_json::to_value(&l).unwrap();
    assert!(json.get("relative").is_none(), "`relative` is skipped when unset: {json}");
    assert!(json.get("path").is_some(), "`path` is always written");
}

#[test]
fn the_link_relative_path_is_relative_to_the_documents_folder_not_the_bundle() {
    // Round 1: schema §3 said "All paths are bundle-relative with `/` separators, matching
    // `LinkInfo.relative`", and §7.1 said a `StorageRef::Linked.relative_path` "Matches
    // `LinkInfo.relative` so the document's own link and this record agree".
    //
    // The post-fix §3 (schema:90-97) now states the opposite — the two bases differ — but §7.1
    // still claims they match; `the_schema_does_not_assert_both_path_bases_at_once` below pins
    // that contradiction. This test is unchanged and still pins the code, which sides with §3.
    //
    // The code says otherwise: `LinkInfo.relative` is "The path relative to the document's folder"
    // (`crates/doc/src/links.rs:25-28`), it is produced by `relative_path(target, base = the
    // document's directory)` which emits `..` components (`crates/engine/src/cmd/links.rs:627-645`),
    // and it is resolved by joining it onto the document's folder
    // (`crates/engine/src/cmd/links.rs:237-239`). For the bundle layout of §3 — the document at
    // `document/comic.vectorcraft`, the asset at `assets/panels/p1.png` — the value is
    // `../assets/panels/p1.png`, which is *not* the bundle-relative `assets/panels/p1.png`.
    // This test pins that a leading-`..` value is representable and round-trips, so a rule that
    // rejects any `..` outright would reject the document's own links.
    let mut l = LinkInfo::new("/work/bundle/document/../assets/panels/p1.png");
    l.relative = Some("../assets/panels/p1.png".into());
    let back: LinkInfo = serde_json::from_value(serde_json::to_value(&l).unwrap()).unwrap();
    assert_eq!(back.relative.as_deref(), Some("../assets/panels/p1.png"));
    assert_ne!(back.relative.as_deref(), Some("assets/panels/p1.png"), "the two bases are not interchangeable strings");
}

// -------------------------------------------------------------------------------------------------
// §1 / §12 — the visual document's foreign-data maps exist, and the schema must never use them.
// -------------------------------------------------------------------------------------------------

#[test]
fn document_unknown_and_extra_are_present_and_empty_by_default() {
    // Schema §1: "never `Document::unknown` / `Document::extra` (design ADR-013)"; §12 explains
    // they exist to round-trip *foreign* data inside the visual document.
    let d = Document::new(10.0, 10.0);
    assert!(d.unknown.is_empty(), "a new document carries no foreign data");
    assert!(d.extra.is_empty(), "…and no unknown sections");
}

// -------------------------------------------------------------------------------------------------
// §9 — visual object ids come from the document's own counter and are never reused.
// -------------------------------------------------------------------------------------------------

#[test]
fn node_ids_save_as_bare_numbers_and_allocate_upward() {
    // Schema §9: "`NodeId(u64)` (from `crates/doc`) | the document's own `next_id`", and "a JSON
    // id is a bare number" for comic ids; `NodeId` is the same transparent shape
    // (`crates/doc/src/node.rs:16-19`).
    assert_eq!(serde_json::to_string(&NodeId(7)).unwrap(), "7");
    assert_eq!(serde_json::to_string(&NodeId(u64::MAX)).unwrap(), u64::MAX.to_string());

    let mut d = Document::new(10.0, 10.0);
    let before = d.peek_next_id();
    let a = d.alloc_id();
    let b = d.alloc_id();
    assert!(a < b, "allocation moves upward: {a} then {b}");
    assert!(d.peek_next_id() > b.0, "the counter is past every allocated id");
    assert!(before <= a.0, "the counter never goes backwards");
}

// =================================================================================================
// Round 2 — rules, failure paths, edges (post-fix revision).
//
// Round 1 pinned the document's claims about *existing* VectorCraft code. Round 2 adds the claims
// the fixes introduced, all of them testable without any comic code existing yet: the referential
// rule set's own consistency (§13/§14), the one path-base statement the fix left behind (§7.1),
// and the byte-identity the frozen fixture promises (§11.1). These read the frozen document, so
// the document — not a paraphrase of it — is what fails.
// =================================================================================================

const SCHEMA_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-project-schema.md");

fn schema_doc() -> String {
    std::fs::read_to_string(SCHEMA_DOC).unwrap_or_else(|e| panic!("the frozen schema doc must be readable at {SCHEMA_DOC}: {e}"))
}

/// The rule numbers in the §13 referential-rule table, in table order.
fn referential_rule_numbers(doc: &str) -> Vec<u32> {
    let table = doc
        .split_once("## 13. Referential rules")
        .expect("§13 exists")
        .1
        .split_once("\n\nRule 9 must")
        .expect("§13's table ends before its trailing note")
        .0;
    let mut out = Vec::new();
    for line in table.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("| ") else { continue };
        let Some((n, _)) = rest.split_once(" |") else { continue };
        if let Ok(n) = n.trim().parse::<u32>() {
            out.push(n);
        }
    }
    out
}

#[test]
fn every_referential_rule_is_numbered_once_and_contiguously() {
    // A rule set the validator implements is 1..=N. A gap means a rule was dropped; a duplicate
    // means two rules share a number and `comic.project.validate` cannot cite them.
    let doc = schema_doc();
    let mut numbers = referential_rule_numbers(&doc);
    assert!(!numbers.is_empty(), "the §13 table must parse; a silent empty set would pass every check below");
    numbers.sort_unstable();
    let expected: Vec<u32> = (1..=numbers.len() as u32).collect();
    assert_eq!(numbers, expected, "§13's rules must be 1..=N with no gap and no duplicate");
}

#[test]
fn every_rule_reference_in_the_schema_names_a_rule_that_exists() {
    // §12 (schema:516) ends "This is the list rule 15 checks." §13 defines 14 rules. A reference
    // to a rule that does not exist is a rule the validator will never implement.
    let doc = schema_doc();
    let numbers = referential_rule_numbers(&doc);
    let mut referenced = Vec::new();
    let mut rest = doc.as_str();
    while let Some(i) = rest.find("rule ") {
        rest = &rest[i + "rule ".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            referenced.push(digits.parse::<u32>().expect("a rule number is a u32"));
        }
    }
    assert!(!referenced.is_empty(), "the document cites rules; the scan must not silently find none");
    for n in &referenced {
        assert!(
            numbers.contains(n),
            "the schema cites rule {n}, which §13 does not define (it defines {numbers:?}); \
             a dangling rule citation is a check no one can implement"
        );
    }
}

#[test]
fn the_load_sequence_runs_every_referential_rule() {
    // §14 step 5 (schema:556) says "Referential rules 1–12 run and their findings are returned".
    // The post-fix table has 14 rules: 13 (page file name + `pages/` reachability) and 14 (the
    // artboard-rect warning) were added and are never run by a load.
    let doc = schema_doc();
    let numbers = referential_rule_numbers(&doc);
    let step5 = doc.split_once("5. Referential rules ").expect("§14 has a step 5 naming the referential rules").1;
    let cited: String = step5.chars().take_while(|c| c.is_ascii_digit() || *c == '–' || *c == '-').collect();
    let cited_max: u32 = cited
        .split(['–', '-'])
        .next_back()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| panic!("§14 step 5 must cite a rule number or range, got {cited:?}"));
    let table_max = numbers.iter().copied().max().expect("the table is non-empty");
    assert_eq!(
        cited_max,
        table_max,
        "§14 step 5 runs rules up to {cited_max} but §13 defines up to {table_max}; \
         rules {:?} never run on load",
        (cited_max + 1)..=table_max
    );
}

#[test]
fn the_schema_does_not_assert_both_path_bases_at_once() {
    // The round-1 finding: `LinkInfo.relative` is document-folder-relative (`crates/doc/src/links.rs:25-28`)
    // while a comic `StorageRef` path is bundle-relative. The fix corrected §3 (schema:90-97) to
    // say the two differ, but §7.1 (schema:359) still says a `StorageRef::Linked` path
    // "Matches `LinkInfo.relative` so the document's own link and this record agree". Both cannot
    // hold, and the same file is `../assets/panels/p1.png` from `document/` and
    // `assets/panels/p1.png` from the bundle root.
    let doc = schema_doc();
    let asserts_the_bases_differ = doc.contains("This is not the convention `LinkInfo.relative` uses");
    let asserts_the_bases_agree = doc.contains("Matches `LinkInfo.relative`");
    assert!(
        !(asserts_the_bases_differ && asserts_the_bases_agree),
        "§3 says the bundle-relative base is *not* `LinkInfo.relative`'s base, while §7.1 still \
         says a `StorageRef` path matches it — one of the two is wrong"
    );
}

#[test]
fn the_round_trip_the_fixture_promises_needs_a_canonical_json_form() {
    // §11.1 (schema:494) requires every fixture to "load, validate and round-trip byte-identically
    // after a save", and §5.5/§12 put free-form JSON objects (`request: Value`) and `unknown` maps
    // in the format. The workspace's `serde_json` (Cargo.toml:49) enables `float_roundtrip` but
    // not `preserve_order`, so `serde_json::Map` is a `BTreeMap`: object keys are written in
    // alphabetical order. A typed `f64` field is worse — `595` loads and is written back as
    // `595.0`. A hand-authored fixture therefore does not round-trip byte-identically unless the
    // document fixes key order and number form, which it does not say anywhere.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct PointSize {
        width: f64,
        height: f64,
    }

    let authored = r#"{"width":595,"height":842}"#;
    let size: PointSize = serde_json::from_str(authored).expect("a hand-authored points size parses");
    let written = serde_json::to_string(&size).expect("it serializes");
    assert_eq!(
        written, r#"{"width":595.0,"height":842.0}"#,
        "a typed `f64` (schema §5.3 `PageSize`, §5.5 `Progress`, `rect`) writes `595.0`, not the authored `595`"
    );
    assert_ne!(written, authored, "…so the authored bytes are not the saved bytes");

    let authored_map = r#"{"z_note":"later","a_note":"first"}"#;
    let value: serde_json::Value = serde_json::from_str(authored_map).expect("the fixture shape parses");
    let written_map = serde_json::to_string(&value).expect("it serializes");
    assert_eq!(
        written_map, r#"{"a_note":"first","z_note":"later"}"#,
        "`serde_json::Map` is a BTreeMap here (no `preserve_order`): keys are sorted on write"
    );
    assert_ne!(written_map, authored_map, "a byte-identical round trip needs a canonical JSON form the schema does not currently fix");
}

// =================================================================================================
// Round 3 — the current text, plus the design it defers to.
//
// Round 2's three red tests are green (their findings are fixed). Each test below is the
// reproduction of one claim the round-3 review found unbacked, and each goes green the moment the
// document is corrected. They read the document rather than restating it: the sentence that fails
// is quoted in the message.
// =================================================================================================

const DESIGN_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-design.md");

/// §13's rules as `(number, text)`.
fn referential_rules(doc: &str) -> Vec<(u32, String)> {
    let table = doc
        .split_once("## 13. Referential rules")
        .expect("§13 exists")
        .1
        .split_once("\n\nRule 9 must")
        .expect("§13's table ends before its trailing note")
        .0;
    let mut out = Vec::new();
    for line in table.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("| ") else { continue };
        let Some((n, text)) = rest.split_once(" |") else { continue };
        if let Ok(n) = n.trim().parse::<u32>() {
            out.push((n, text.trim().trim_end_matches('|').trim().to_string()));
        }
    }
    out
}

fn flattened(doc: &str) -> String {
    doc.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The top-level stores in §3's tree, e.g. `story/`, `jobs/`.
fn bundle_stores(doc: &str) -> Vec<String> {
    let tree = doc.split_once("```text").expect("§3 has a tree block").1.split_once("```").expect("the tree block closes").0;
    tree.lines()
        .filter(|l| l.starts_with("├── ") || l.starts_with("└── "))
        .filter_map(|l| {
            let entry = l.trim_start_matches("├── ").trim_start_matches("└── ");
            let name = entry.split_whitespace().next().unwrap_or("");
            name.ends_with('/').then(|| name.to_string())
        })
        .collect()
}

#[test]
fn every_store_the_bundle_layout_declares_has_a_file_the_schema_names() {
    // §3 declares `jobs/`, §4 requires `files` to cover "every file except the manifest and except
    // `exports/` and `.save-tmp/`", and §14 step 3 parses "the comic files". A store with no named
    // file and no section that fixes its shape cannot be written, hashed or parsed by
    // `crates/comic-model`, and §11.1's fixture ("1 job") does not say which home that job lives in.
    let doc = schema_doc();
    let stores = bundle_stores(&doc);
    assert!(stores.len() >= 5, "§3's tree must parse; found {stores:?}");
    let mut undefined = Vec::new();
    for store in &stores {
        if store == "exports/" {
            continue; // §3: derived output only, deletable, never read as input
        }
        let named = doc.match_indices(store).any(|(i, s)| {
            let rest = &doc[i + s.len()..];
            rest.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.')
        });
        if !named {
            undefined.push(store.clone());
        }
    }
    assert!(
        undefined.is_empty(),
        "§3 declares {undefined:?} in the bundle layout, but no file inside them is named anywhere in \
         the schema, so `manifest.files`, the v1 fixture and the loader have nothing to work with"
    );
}

#[test]
fn s1_closes_the_page_size_item_with_the_rule_that_states_it() {
    // §15 S1 is "**Closed** by rule 14: … an artboard rect that differs is a warning", but the rect
    // warning is rule 15; rule 14 is the orphan-page warning. A closure that cites the wrong rule
    // cannot be checked by anyone reading rule 14.
    let doc = schema_doc();
    let rules = referential_rules(&doc);
    let rect_rule = rules.iter().find(|(_, text)| text.contains("`rect` differing")).map(|(n, _)| *n).expect("§13 must state the rect warning");
    let s1 = doc.split_once("| S1 |").expect("§15 has an S1 row").1.split_once('\n').expect("the S1 row ends").0;
    let cited: u32 = s1
        .split("rule ")
        .nth(1)
        .and_then(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok())
        .expect("S1 must cite a rule");
    assert_eq!(
        cited, rect_rule,
        "S1 closes the page-size item with rule {cited}, but the rule that says a differing rect is a warning is rule {rect_rule}"
    );
}

#[test]
fn a_reused_artboard_id_does_not_end_up_permanently_broken() {
    // §9's identity table: the page artboard id "**can be reused — see §9.1**", and §9.1's own
    // first paragraph says "there is no never-reuse guarantee"; mitigation 4 (a persisted counter)
    // is only *proposed*. Rule 4 requires every `Page.artboard_id` to be absent from
    // `retired_artboard_ids`, and rule 17 requires that list to be **unique and append-only**. So
    // deleting a page and creating another — a legitimate `comic.page.create` — gives the new page
    // an id that is retired, rule 4 reports it broken, and rule 17 forbids the only repair.
    let doc = schema_doc();
    let reuse_permitted = doc.contains("**can be reused — see §9.1**") && doc.contains("there is no never-reuse guarantee");
    let rule4_rejects_retired = doc.contains("is **not** in `retired_artboard_ids`");
    let rule17_forbids_repair = doc.contains("`retired_artboard_ids` is **unique and append-only**");
    assert!(
        !(reuse_permitted && rule4_rejects_retired && rule17_forbids_repair),
        "§9 permits a reused artboard id, rule 4 rejects any page whose artboard id is retired, and \
         rule 17 forbids removing it from the list: a reused id is a permanent false finding. One of \
         the three must change (allocate from a persisted counter now, accept a retired id that is \
         present in the document, or allow the list to be pruned)"
    );
}

#[test]
fn rule_3_names_every_file_that_declares_a_schema_version() {
    // §5.4 (schema:175-178) says each story file "is `{ "schema_version": 1, … }`" *so that* "rule 3
    // can check the version against the manifest", but rule 3's enumeration names only
    // `project.json`, `pages/page-<id>.json`, `assets/index.json` and `provenance/generations.json`.
    let doc = schema_doc();
    let section54 = doc.split_once("### 5.4 Story files").expect("§5.4 exists").1.split_once("### 5.5").expect("§5.5 follows §5.4").0;
    assert!(section54.contains("\"schema_version\": 1"), "§5.4 must still declare the story files versioned");
    let story_files: Vec<String> = section54
        .lines()
        .filter_map(|l| {
            let (path, _) = l.trim().strip_prefix("| `")?.split_once('`')?;
            Some(path.to_string())
        })
        .collect();
    assert!(!story_files.is_empty(), "§5.4's table must parse");
    let rule3 = referential_rules(&doc).into_iter().find(|(n, _)| *n == 3).map(|(_, text)| text).expect("§13 has rule 3");
    for file in &story_files {
        assert!(
            rule3.contains(file.as_str()),
            "§5.4 declares `{file}` versioned so rule 3 can check it, but rule 3 does not name it: \
             a validator built from the rule's list skips all five story files"
        );
    }
}

#[test]
fn the_schema_does_not_assert_one_storage_base_where_rule_9_splits_them() {
    // Rule 9: "The bases are not one base: `Linked` resolves against the **bundle root**,
    // `Package` against the **archive root**". §3 still says "A `StorageRef` path resolves against
    // the bundle root", and §7.1 says `Package` is "a file inside a packaged archive".
    let doc = flattened(&schema_doc());
    let rule9_splits = doc.contains("`Package` against the **archive root**");
    let s3_flattens = doc.contains("A `StorageRef` path resolves against the bundle root");
    assert!(
        !(rule9_splits && s3_flattens),
        "rule 9 splits the `StorageRef` bases but §3 still resolves every `StorageRef` path against \
         the bundle root: an implementer following §3 unpacks a packaged archive's paths against the \
         wrong root"
    );
}

#[test]
fn the_files_tree_comparison_excludes_what_files_excludes() {
    // §4's `files` row excludes `exports/` and `.save-tmp/` from the file set; §14 step 1 "compares
    // `files` with the tree" without restating the exclusions, so a bundle that has either path
    // reports a spurious inconsistency on every load.
    let doc = schema_doc();
    let files_row = doc.lines().find(|l| l.starts_with("| `files`")).expect("§4 has a `files` row");
    let excluded: Vec<String> = files_row.split("except").skip(1).flat_map(|part| part.split('`').skip(1).step_by(2).map(str::to_string)).collect();
    assert!(!excluded.is_empty(), "§4's `files` row must name the paths it excludes, found none in {files_row}");
    let step1 = doc.split_once("1. `manifest.json` parses").expect("§14 has step 1").1.split_once("\n2. ").expect("§14 step 2 follows step 1").0;
    for path in &excluded {
        assert!(
            step1.contains(path.as_str()),
            "§4 excludes `{path}` from `files`, but §14 step 1 compares `files` with the whole tree, so \
             every bundle containing `{path}` reports an inconsistency that is not one"
        );
    }
}

#[test]
fn the_missing_font_claim_cites_a_section_that_contains_the_check() {
    // §5.3 (schema:172): "the lettering then reports a missing font (§13)". §13 is the referential
    // rules 1–17 and contains no font rule at all; the check is design §12.1's "Missing lettering
    // font". A reader who follows the citation finds nothing.
    let doc = schema_doc();
    let claim = "reports a missing font (§13)";
    let section13 = doc.split_once("## 13. Referential rules").expect("§13 exists").1.split_once("## 14.").expect("§14 follows §13").0;
    assert!(
        !doc.contains(claim) || section13.to_lowercase().contains("font"),
        "§5.3 cites §13 for the missing-font report, but §13's rules mention no font; the check is \
         design §12.1, not a referential rule"
    );
}

#[test]
fn rule_12_says_what_a_null_speaker_means() {
    // Rule 12: "Every `Dialogue.speaker_character_id` resolves to a `Character`". §6.3 makes the
    // field `Option<CharacterId>` with "`null` for narration", so read literally the rule reports
    // every narration line in the book.
    let doc = schema_doc();
    assert!(doc.contains("`speaker_character_id` | `Option<CharacterId>` | yes | `null` for narration"), "§6.3 must still make the speaker optional");
    let (number, text) =
        referential_rules(&doc).into_iter().find(|(_, text)| text.contains("speaker_character_id")).expect("§13 has a rule about the speaker");
    assert!(
        text.contains("present") || text.contains("when set") || text.contains("null") || text.contains("Option"),
        "rule {number} requires every `Dialogue.speaker_character_id` to resolve, but §6.3 makes it \
         `Option<CharacterId>` (`null` for narration): \"{text}\""
    );
}

#[test]
fn the_save_order_does_not_claim_content_no_reader_can_see() {
    // §10 step 4 replaces the content files *before* step 5 writes the manifest, so the new
    // generation's files are on disk at their final paths from step 4 on. "No reader can see the
    // new generation yet" (schema:475) and "only the last step is visible to a reader" (schema:468)
    // are both false of that window, which is the window §14 step 1 exists to tolerate.
    let doc = flattened(&schema_doc());
    let published_before_the_manifest = doc.contains("**Replace the content files**") && doc.contains("**Write `manifest.json` last**");
    let claims_invisible = doc.contains("No reader can see the new generation yet") || doc.contains("only the last step is visible to a reader");
    assert!(
        !(published_before_the_manifest && claims_invisible),
        "§10 publishes the content files in step 4 and the manifest in step 5, yet still claims the new \
         generation is not visible until the last step: a reader between the two steps sees new content \
         under old hashes"
    );
}

#[test]
fn the_schema_and_the_design_it_defers_to_do_not_contradict_each_other() {
    // §1: "where the two could be read differently, `comiccraft-design.md` decides the meaning and
    // this document is corrected." Two seams still differ, and §9.1 cites the first as its source.
    let schema = flattened(&schema_doc());
    let design = flattened(&std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable at its documented path"));
    let mut seams = Vec::new();
    if schema.contains("**Replace the content files**")
        && schema.contains("**Write `manifest.json` last**")
        && design.contains("4. Write `manifest.json` last")
        && design.contains("5. Replace the changed files")
    {
        seams.push("save order: schema §10 replaces the files then writes the manifest; design §7.2 writes the manifest then replaces the files");
    }
    if schema.contains("It checks no size expectation") && design.contains("the page size the project expects") {
        seams.push("rect expectation: schema §9.1 says there is none, design §5.3.1 (which §9.1 cites) requires one");
    }
    assert!(seams.is_empty(), "the schema and the design it defers to disagree on {} seam(s): {}", seams.len(), seams.join(" | "));
}

// ---------------------------------------------------------------------------------------------
// Round 3 (code-reviewer): drift classes.
//
// Round 3 found that most of its findings were not mistakes in *what* the schema says but in
// how the document keeps its own cross-references in step — a rule renumbering left a stale
// citation, an enumeration gained a member somewhere else, a claim of completeness outlived
// the list. Those are mechanical, so they belong in tests rather than in another prose pass.
// ---------------------------------------------------------------------------------------------

/// The bundle layout block's `├── <dir>/` entries.
fn bundle_directories(doc: &str) -> Vec<String> {
    let block = doc.split_once("```text").expect("§3 has a layout block").1;
    let block = block.split("```").next().expect("the layout block is closed");
    block
        .lines()
        .filter_map(|l| {
            let name = l.split("── ").nth(1)?;
            let name = name.split_whitespace().next()?;
            name.ends_with('/').then(|| name.trim_end_matches('/').to_string())
        })
        .collect()
}

#[test]
fn every_directory_in_the_bundle_layout_is_defined_somewhere() {
    // Round 3 finding 1: `jobs/` was declared in the layout and defined nowhere, so a reader
    // could not tell whether it was part of the format.
    let doc = schema_doc();
    let dirs = bundle_directories(&doc);
    assert!(dirs.len() >= 5, "the layout block lists the bundle's directories, found {dirs:?}");
    for dir in dirs {
        // Defined if the prose names the directory itself or any path inside it — `story/` is
        // defined by naming `story/chapters.json`, which is how §5.4 does it.
        let defined = doc.contains(&format!("`{dir}/`")) || doc.contains(&format!("{dir}/"));
        assert!(defined, "§3 declares `{dir}/` but the document never defines it");
    }
}

#[test]
fn rule_three_names_every_file_kind_that_carries_a_schema_version() {
    // Round 3 finding 4: §5.4 gave the story files a `schema_version` and rule 3's list did not
    // gain them, so the rule checked a subset of what it claimed to.
    let doc = schema_doc();
    let rule3 = doc
        .split_once("| 3 | `manifest.comic_id` equals `project.id`")
        .expect("§13 defines rule 3")
        .1
        .split_once('\n')
        .expect("rule 3 is one line")
        .0;
    for kind in ["project.json", "pages/page-", "story/", "assets/index.json", "provenance/generations.json", "jobs/job-"] {
        assert!(rule3.contains(kind), "rule 3 does not name `{kind}`, which §5.4/§8 declare versioned");
    }
}

#[test]
fn the_three_path_bases_are_named_wherever_a_base_is_claimed() {
    // Round 3 finding 5: round 2 fixed §7.1 and rule 9 but left §3 asserting one base for every
    // `StorageRef`. A claim about bases is only true if every place that makes one agrees.
    let doc = schema_doc();
    for (where_, needle) in [("§3", "- **Comic paths are bundle-relative"), ("§7.1", "- `Linked { relative_path: String }`")] {
        let section = doc.split_once(needle).unwrap_or_else(|| panic!("{where_} still makes a path-base claim")).1;
        let window = &section[..section.len().min(900)];
        assert!(window.contains("archive root"), "{where_} names a base but not the archive root");
        assert!(window.contains("document"), "{where_} names a base but not the document folder");
    }
    let rule9 = doc.split_once("| 9 | Every path is resolved").expect("§13 defines rule 9").1;
    let window = &rule9[..rule9.len().min(1200)];
    for base in ["bundle root", "archive root", "document folder"] {
        assert!(window.contains(base), "rule 9 does not name the {base}");
    }
}

#[test]
fn section_5_5_does_not_overclaim_completeness() {
    // Round 3 finding 9: "Every type a table names appears here" was false — `ComicId` and
    // `JobStatus` are named by tables and defined elsewhere. A completeness claim the document
    // cannot keep is worse than none.
    let doc = schema_doc();
    let intro = doc.split_once("### 5.5 Shared types").expect("§5.5 exists").1;
    let intro = &intro[..intro.find("\n\n|").expect("§5.5 has a table after its intro")];
    assert!(
        !intro.contains("Every type a table names appears here"),
        "§5.5 claims to define every type a table names, which is not true (see §9, §8.1)"
    );
    assert!(intro.contains("this document defines"), "§5.5 must scope its claim to what it defines");
}

#[test]
fn no_directory_or_rule_is_defined_twice() {
    // A renumbering or a careless edit can leave two definitions of one thing, which is how the
    // round-3 regressions started.
    let doc = schema_doc();
    let numbers = referential_rule_numbers(&doc);
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, numbers, "a rule number is defined more than once: {numbers:?}");
    assert_eq!(numbers, (1..=numbers.len() as u32).collect::<Vec<_>>(), "rules must be 1..=N with no gaps");
}

// =================================================================================================
// Round 4 — what the round-3 fixes left behind, and what they moved.
//
// Round 3's ten red tests are green (their findings are fixed). Each test below is the
// reproduction of one claim the round-4 review found unbacked, and each goes green the moment the
// document is corrected. They read the two documents rather than restating them: the sentence
// that fails is quoted in the message.
// =================================================================================================

/// A small count spelled as a word, e.g. `Some(4)` for "Four".
fn word_number(word: &str) -> Option<u32> {
    [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ]
    .iter()
    .position(|w| w.eq_ignore_ascii_case(word))
    .map(|i| i as u32)
}

/// Every `#`-heading number in a document, e.g. `"5.5"` for `### 5.5 Shared types`.
fn heading_numbers(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            l.starts_with('#').then_some(l)?;
            let text = l.trim_start_matches('#').trim_start();
            let num: String = text.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            (!num.is_empty()).then(|| num.trim_end_matches('.').to_string())
        })
        .collect()
}

/// Every `prefix`-then-number citation in a document, as `(line, number)`.
fn number_citations(doc: &str, prefix: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(i) = doc[offset..].find(prefix) {
        let start = offset + i + prefix.len();
        let num: String = doc[start..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let num = num.trim_end_matches('.').to_string();
        if !num.is_empty() {
            out.push((doc[..start].matches('\n').count() + 1, num));
        }
        offset = start;
    }
    out
}

#[test]
fn the_mitigation_list_matches_its_count_and_its_fallback() {
    // §9.1 (schema:468) opens "Four mitigations, all mandatory (design §5.3.1):" and then lists
    // five, the fifth being the upstream proposal. Its closing sentence (schema:487), "Until it
    // lands, 1–3 stand alone", is the design's sentence for a four-item list whose fourth item
    // *is* the upstream fix (design §5.3.1); here it excludes mitigation 4, which the schema calls
    // mandatory and which is the one that keeps rules 4 and 17 from turning §9.1's reuse into a
    // permanent finding.
    let doc = schema_doc();
    let section = doc.split_once("### 9.1 The artboard-id hazard").expect("§9.1 exists").1;
    let section = section.split_once("\n## 10.").expect("§10 follows §9.1").0;
    let stated = section
        .split_once(" mitigations")
        .and_then(|(before, _)| before.rsplit(char::is_whitespace).next())
        .and_then(word_number)
        .expect("§9.1 must state how many mitigations are mandatory");
    let items: Vec<&str> =
        section.lines().map(str::trim_start).filter(|l| l.starts_with(|c: char| c.is_ascii_digit()) && l.contains(". **")).collect();
    assert_eq!(
        stated,
        items.len() as u32,
        "§9.1 says {stated} mitigations are mandatory but lists {}: {}",
        items.len(),
        items.iter().map(|l| l.split(". **").nth(1).unwrap_or("?").split("**").next().unwrap_or("?")).collect::<Vec<_>>().join(" | ")
    );
    let fallback = section
        .lines()
        .find(|l| l.contains("stand alone") || l.contains("stand on their own"))
        .map(|l| l.split(" stand").next().unwrap_or(""))
        .map(|before| {
            before.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse::<u32>().ok()).collect::<Vec<u32>>()
        })
        .unwrap_or_default();
    let upstream = items.iter().position(|l| l.contains("audit §13 U1"));
    let without_upstream = (items.len() - usize::from(upstream.is_some())) as u32;
    if let (Some(&start), Some(&end)) = (fallback.get(fallback.len().saturating_sub(2)), fallback.last()) {
        assert!(
            start == 1 && end >= without_upstream,
            "§9.1 says mitigations 1–{end} stand alone while it lists {} of them and calls all of them \
             mandatory; the fallback sentence has to cover every mitigation the upstream fix (item \
             {:?}) does not replace, i.e. 1–{without_upstream}",
            items.len(),
            upstream.map(|i| i + 1)
        );
    }
}

#[test]
fn the_stale_jobs_file_is_reported_by_a_rule_that_governs_it() {
    // §8.3 (schema:442): "a stale file there (a terminal status) is an inconsistency rule 1
    // reports." Rule 1 walks references and has nothing to say about a file in `jobs/`, so a
    // validator built from rule 1 never reports the inconsistency §8.3 promises — while §3
    // declares the directory and §4/§14 require `manifest.files` to cover every file in it.
    let doc = schema_doc();
    let sentence = doc
        .split_once("### 8.3 `jobs/`")
        .expect("§8.3 exists")
        .1
        .lines()
        .find(|l| l.contains("inconsistency rule"))
        .expect("§8.3 must still make the claim this test pins");
    let cited: u32 = sentence
        .split("rule ")
        .nth(1)
        .and_then(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok())
        .expect("§8.3 must cite the rule that reports a stale `jobs/` file");
    let (_, text) = referential_rules(&doc)
        .into_iter()
        .find(|(n, _)| *n == cited)
        .unwrap_or_else(|| panic!("§8.3 cites rule {cited}, which §13 does not define"));
    assert!(
        text.contains("jobs/") || text.contains("terminal"),
        "§8.3 says a stale `jobs/` file \"{sentence}\" — but rule {cited} is \"{text}\", which walks \
         references and cannot report a file's status. The directory §3 declares and §4 requires \
         `manifest.files` to cover has no rule that governs it, so the finding §8.3 promises is never \
         produced"
    );
}

#[test]
fn the_rule_9_note_does_not_flatten_the_bases_rule_9_splits() {
    // Rule 9 (schema:592) names three bases; the note under it (schema:610) still says "Rule 9 must
    // confine paths to the bundle root" — §3's pre-fix sentence, wrong for `Package`, whose base
    // rule 9 says is the archive root.
    let doc = schema_doc();
    let note = doc.split_once("\n\nRule 9 must").expect("§13 has its trailing note").1;
    let note = note.split_once("\n## 14.").expect("§14 follows §13").0;
    assert!(
        !note.contains("confine paths to the bundle root") || note.contains("archive root"),
        "the note under rule 9 confines every path to the bundle root while rule 9 splits the bases \
         (`Package` resolves against the archive root): an implementer following the note rejects \
         valid packaged paths. Note text: {note:?}"
    );
}

#[test]
fn section_13s_rule_table_is_one_table() {
    // Round 3's rule-9 fix embedded two paragraphs in rule 9's cell with blank lines around them.
    // A markdown table cannot span a blank line, so the table ends after rule 9, rules 10–17 are
    // rendered as prose, and rule 9's own last sentence ("A `Linked` file must exist and its sha256
    // must match `content_hash`") is left dangling after a stray `|`. The rule set is normative:
    // it has to render as one table.
    let doc = schema_doc();
    let table = doc.split_once("| # | Rule |").expect("§13 has a rule table").1;
    let block = &table[..table.find("\n\n").expect("the table block ends somewhere")];
    let rows: Vec<u32> = block
        .lines()
        .filter_map(|l| {
            let (n, _) = l.trim_start().strip_prefix("| ")?.split_once(" |")?;
            n.trim().parse::<u32>().ok()
        })
        .collect();
    let numbers = referential_rule_numbers(&doc);
    assert_eq!(
        rows,
        numbers,
        "§13's table is split by a blank line after rule {:?}, so rules beyond it render as prose \
         rather than as table rows",
        rows.last()
    );
}

#[test]
fn section_5_5_defines_the_types_it_claims_to() {
    // §5.5's intro (schema:195-198) claims "Every type **this document defines** appears here" and
    // then "A type that is in neither place is a defect in this document". Neither holds: these
    // types are defined in their own sections and none is a row of §5.5's table, so the document's
    // own criterion convicts it.
    let doc = schema_doc();
    let section = doc.split_once("### 5.5 Shared types").expect("§5.5 exists").1;
    let (intro, rest) = section.split_once("\n\n|").expect("§5.5 has a table after its intro");
    if !intro.contains("Every type **this document defines** appears here") {
        return; // the claim was scoped or removed, which is the fix this test asks for
    }
    let table = rest.split_once("### 5.6").expect("§5.6 follows §5.5").0;
    let mut missing = Vec::new();
    for (ty, defined_in) in [
        ("ProjectMetadata", "§5.2"),
        ("ProjectSettings", "§5.3"),
        ("PageSize", "§5.3"),
        ("ArtworkRef", "§6.2"),
        ("ContinuityConstraints", "§6.5"),
        ("TextLayoutSettings", "§6.6"),
        ("StorageRef", "§7.1"),
        ("ContentHash", "§7.2"),
        ("MediaMetadata", "§7"),
    ] {
        if !table.contains(&format!("| `{ty}` |")) {
            missing.push(format!("{ty} ({defined_in})"));
        }
    }
    assert!(
        missing.is_empty(),
        "§5.5 claims every type this document defines appears in its table, but {} do not, so the \
         sentence \"A type that is in neither place is a defect in this document\" convicts the \
         document: add the rows, or scope the claim to the types these tables reference",
        missing.join(", ")
    );
}

#[test]
fn rule_1_carves_out_nullable_keys_like_rule_12() {
    // Round 3's finding 8 was "rule 12 requires a value for a field §6.3 makes optional", and the
    // fix carved the null case out of rule 12 only (schema:603). Rule 1 (schema:584) walks "every
    // singular foreign key" and names three nullable ones, so a validator built from rule 1 reports
    // every narration line, every job with no target panel and every non-superseding decision.
    let doc = schema_doc();
    let (number, text) = referential_rules(&doc).into_iter().find(|(n, _)| *n == 1).expect("§13 has rule 1");
    let mut nullable = Vec::new();
    for key in text.split('`').filter(|s| s.contains('.') && !s.contains(' ')) {
        let Some((_, field)) = key.split_once('.') else { continue };
        if doc.contains(&format!("`{field}` | `Option<"))
            || doc.contains(&format!("`{field}: Option<"))
            || doc.contains(&format!("`{field}`: `Option<"))
        {
            nullable.push(key.to_string());
        }
    }
    assert!(!nullable.is_empty(), "rule 1 must still name the singular foreign keys this test pins");
    assert!(
        text.contains("null") || text.contains("when set") || text.contains("present"),
        "rule {number} requires every singular foreign key to resolve, but the document declares {} \
         as `Option<_>`, where `null` is legal and means \"no reference\". Rule 12 was corrected for \
         exactly this and rule 1 was not",
        nullable.join(", ")
    );
}

#[test]
fn every_section_cited_across_the_two_documents_exists() {
    // §1 makes the design authoritative and this document the corrected one, so a citation between
    // them has to resolve. Round 3's fix added one that does not: design §5.3.1 cites
    // "(schema §5.1.1, rule 15)" and the schema has no §5.1.1 — its page/artboard ownership section
    // is §6.1; §5.1.1 is the design's own number.
    let schema = schema_doc();
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable at its documented path");
    let schema_sections = heading_numbers(&schema);
    let design_sections = heading_numbers(&design);
    let rules = referential_rule_numbers(&schema);
    let mut dangling = Vec::new();
    for (line, n) in number_citations(&schema, "design §") {
        if !design_sections.contains(&n) {
            dangling.push(format!("schema:{line} cites design §{n}"));
        }
    }
    for (line, n) in number_citations(&design, "schema §") {
        if !schema_sections.contains(&n) {
            dangling.push(format!("design:{line} cites schema §{n}"));
        }
    }
    for (line, n) in number_citations(&design, "schema rule ") {
        if !n.parse::<u32>().map(|r| rules.contains(&r)).unwrap_or(false) {
            dangling.push(format!("design:{line} cites schema rule {n}"));
        }
    }
    assert!(
        dangling.is_empty(),
        "a cross-document citation that does not resolve: {} — §1 makes the design decide the \
         meaning, so a reader who follows a dangling one has no meaning to find",
        dangling.join(", ")
    );
}

#[test]
fn the_page_dimension_check_names_a_baseline_the_schema_defines() {
    // §1 makes the design decide meaning. Design §12.1 still lists "Unsupported page dimensions |
    // `comic-layout` | the artboard `rect` against the project's print presets", while the schema
    // closes that item (schema §15 S1) with rule 15, whose baseline is
    // `ProjectSettings.default_page_size`. No print-preset field exists in either document.
    let schema = schema_doc();
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable at its documented path");
    assert!(
        !design.contains("against the project's print presets") || schema.contains("print preset"),
        "the design's §12.1 compares the artboard `rect` against \"the project's print presets\", but \
         neither document defines a print preset: the schema's rule 15 compares against \
         `ProjectSettings.default_page_size` and §15 S1 declares the item closed. Name one baseline \
         in both, or say the two are the same check"
    );
}

#[test]
fn section_5_7_covers_map_valued_fields() {
    // §12 flattens `unknown: Map<String, Value>` into every top-level struct and §8 puts
    // `request: Value` in a job, so a saved bundle carries free-form JSON objects whose key order
    // §5.7 does not fix: it says only "Object keys in the order the struct declares them (serde's
    // default), not sorted", while `serde_json::Map` is a `BTreeMap` in this workspace (see
    // `the_round_trip_the_fixture_promises_needs_a_canonical_json_form`) and writes those keys
    // sorted. §11.1 requires a byte-identical round trip, so a fixture author needs the rule.
    let doc = flattened(&schema_doc());
    let section = doc.split_once("### 5.7 Canonical form").expect("§5.7 exists").1;
    let section = section.split_once("## 6. Per-page files").expect("§6 follows §5.7").0;
    assert!(doc.contains("request: Value"), "§8 puts a free-form object in a job");
    assert!(doc.contains("flatten"), "§12 flattens a map into every top-level struct");
    assert!(
        section.split(|c: char| !c.is_alphanumeric()).any(|w| w.eq_ignore_ascii_case("map") || w.eq_ignore_ascii_case("maps")),
        "§5.7 fixes struct-field order but never mentions the map-valued fields the format carries \
         (`unknown`, `request: Value`), whose keys `serde_json` writes sorted — so §11.1's \
         byte-identical round trip is unachievable for them without the fixture author discovering \
         the rule. Section text: {section:?}"
    );
}

// =================================================================================================
// Round 5 — the `jobs/` seam rule 18 opened, and what the round-4 fix left undecidable.
//
// Round 4's nine red tests are green (their findings are fixed). Rule 18 is new in that fix, and
// it governs the one directory whose file the schema never shapes: §8.3 says the file holds "one
// `GenerationJob` with the same shape as §8.1" while rule 3 enumerates it among the files that
// declare a `schema_version`, and rule 18's predicate is the bare word "terminal", which neither
// document defines — while the workspace's own `JobStatus::is_terminal` (crates/comic-model/src/entities.rs,
// which its doc comment freezes against schema §8.1) is `Failed | Cancelled`. Each test reads the
// document, so the sentence that fails is quoted in the message.
// =================================================================================================

/// The body of the schema section numbered `num`, from its heading to the next heading at the
/// same or a higher level (so `7` includes its `7.1` and `7.2` subsections).
fn schema_section(doc: &str, num: &str) -> String {
    let mut out = String::new();
    let mut level = 0usize;
    let mut inside = false;
    for line in doc.lines() {
        if let Some(rest) = line.strip_prefix('#') {
            let width = 1 + rest.chars().take_while(|c| *c == '#').count();
            let heading = rest.trim_start_matches('#').trim_start();
            if inside {
                if width <= level {
                    break;
                }
            } else {
                let n: String = heading.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if n.trim_end_matches('.') == num {
                    inside = true;
                    level = width;
                }
            }
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn the_jobs_file_declares_the_schema_version_rule_three_checks() {
    // rule 3 (schema:593) enumerates "every `jobs/job-<id>.json`" among the files that "declares
    // `schema_version`", and §5.4, §7 and §8 each show their file's version key. §8.3
    // (schema:442-443) gives the job file the shape of "one `GenerationJob` with the same shape as
    // §8.1", whose field list has no `schema_version` and no wrapper. A writer cannot tell whether
    // the file is a bare job, a job plus a version key, or a `{ schema_version, job }` wrapper, so
    // the bytes and rule 3's check cannot both be right.
    let doc = schema_doc();
    let (_, rule3) = referential_rules(&doc).into_iter().find(|(n, _)| *n == 3).expect("§13 has rule 3");
    assert!(rule3.contains("jobs/job-"), "rule 3 must still enumerate the `jobs/` files this test pins");
    let jobs = doc.split_once("### 8.3 `jobs/`").expect("§8.3 exists").1;
    let jobs = jobs.split_once("\n## 9.").expect("§9 follows §8.3").0;
    assert!(
        jobs.contains("schema_version"),
        "rule 3 lists every `jobs/job-<id>.json` among the files that declare `schema_version`, but \
         §8.3 gives the file the shape of a bare `GenerationJob` and never gives it a version, where \
         §5.4, §7 and §8 each show their own file's `schema_version`. The file's bytes are \
         undetermined: §8.3 text: {jobs:?}"
    );
}

#[test]
fn rule_18_names_the_statuses_that_leave_the_jobs_directory_empty() {
    // rule 18 (schema:608) is "Every file in `jobs/` holds a job whose `status` is **not
    // terminal**", and §8.3 (schema:446) removes the file "when a job reaches a terminal state",
    // "so `jobs/` is empty in a quiescent project". Neither document says which of the six
    // `JobStatus` values is terminal. The workspace's own definition — `crates/comic-model/src/entities.rs`
    // `is_terminal() = matches!(self, Failed | Cancelled)` — makes `completed` and `awaiting_review`
    // files legal, so `jobs/` is *not* empty after a successful generation and rule 18 never
    // reports the case §8.3 promises it does. Whatever "terminal" means, one of the two sentences
    // is wrong until the schema says which statuses it means.
    let doc = schema_doc();
    let (n, rule18) = referential_rules(&doc).into_iter().find(|(n, _)| *n == 18).expect("§13 has rule 18");
    let jobs = doc.split_once("### 8.3 `jobs/`").expect("§8.3 exists").1;
    let jobs = jobs.split_once("\n## 9.").expect("§9 follows §8.3").0;
    let statuses = ["queued", "running", "completed", "failed", "awaiting_review", "cancelled"];
    let named = statuses.iter().filter(|s| rule18.contains(*s) || jobs.contains(*s)).count();
    assert!(
        named >= 2,
        "rule {n} says \"{rule18}\" and §8.3 removes the file at a terminal state, but neither names \
         which of the six `JobStatus` values is terminal ({statuses:?}). `completed` and \
         `awaiting_review` are the two `crates/comic-model`'s `is_terminal` calls non-terminal, and \
         under them `jobs/` is never empty in a quiescent project — exactly the case §8.3 says rule \
         18 reports. Name the statuses the rule means"
    );
}

#[test]
fn the_jobs_lifecycle_says_what_a_job_file_does_to_the_manifest() {
    // §8.3 (schema:441) makes the file durable "so an interrupted session can report what was
    // running" — written when the job starts, outside §10's save. §4 (schema:118) has
    // `manifest.files` cover every file except the manifest, `exports/` and `.save-tmp/`, and §14
    // step 1 (schema:638) explains a `files` mismatch as "interrupted mid-save (§10) or
    // hand-edited". A job file created or removed since the last save is a third cause no sentence
    // names, and nothing says whether such a file joins `files` immediately or at the next save.
    let doc = schema_doc();
    let jobs = doc.split_once("### 8.3 `jobs/`").expect("§8.3 exists").1;
    let jobs = jobs.split_once("\n## 9.").expect("§9 follows §8.3").0;
    let step1 = doc.split_once("1. `manifest.json` parses and its `schema_version` is supported.").expect("§14 has step 1").1;
    let step1 = step1.split_once("\n2. ").expect("§14 step 2 follows step 1").0;
    assert!(
        jobs.contains("manifest") || step1.contains("job"),
        "§8.3 gives `jobs/` a lifecycle but never says what a job file created between saves does \
         to `manifest.files`, and §14 step 1 attributes a `files` mismatch only to an interrupted \
         save or hand-editing — a stale `files` map after a job starts is a third cause the \
         document never names. §8.3: {jobs:?} §14 step 1: {step1:?}"
    );
}

#[test]
fn the_shared_types_section_pairs_each_type_with_its_own_section() {
    // §5.5's intro (schema:200-203) pairs each single-section type with the section that defines
    // it, in the document's own convention: "`A` and `B` §x/§y". `StorageRef` is §7.1 and
    // `ContentHash` is §7.2, but the intro gives the pair one number (§7.1), and §7.1's body never
    // names `ContentHash` — so the one place a reader is sent for that type does not hold it.
    let doc = schema_doc();
    let intro = doc.split_once("### 5.5 Shared types").expect("§5.5 exists").1;
    let intro = &intro[..intro.find("\n\n|").expect("§5.5 has a table after its intro")];
    let Some(rest) = intro.split_once("that section's own table (").map(|(_, rest)| rest) else {
        return; // the sentence was rewritten; there is no pairing claim left to pin
    };
    let Some(paren) = rest.split_once(')').map(|(paren, _)| paren) else {
        return;
    };
    let mut misplaced = Vec::new();
    for part in paren.split(',') {
        let part = part.trim().replace('\n', " ");
        let types: Vec<&str> = part.split('`').skip(1).step_by(2).collect();
        let sections: Vec<String> = part.split('§').skip(1).map(|s| s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect()).collect();
        if types.is_empty() || sections.is_empty() {
            continue;
        }
        // "`A` and `B` §x/§y" pairs in order; "`A` and `B` §x" claims one section for both.
        let paired: Vec<(&str, &String)> = if types.len() == sections.len() {
            types.iter().copied().zip(sections.iter()).collect()
        } else if sections.len() == 1 {
            types.iter().copied().map(|ty| (ty, &sections[0])).collect()
        } else {
            types.iter().copied().flat_map(|ty| sections.iter().map(move |number| (ty, number))).collect()
        };
        for (ty, number) in paired {
            if !schema_section(&doc, number).contains(ty) {
                misplaced.push(format!("`{ty}` (sent to §{number}, in \"{part}\")"));
            }
        }
    }
    assert!(misplaced.is_empty(), "§5.5 sends the reader to a section that does not hold the type it names there: {}", misplaced.join(", "));
}

const MILESTONE_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/milestones/M0.md");

#[test]
fn the_milestone_record_states_the_counts_the_frozen_document_has() {
    // M0.md §2.3 describes the revision this round approves: "the four artboard-id mitigations …
    // the twelve referential rules" (M0.md:54-55), and §3's status table still lists "the schema
    // document's two rounds are outstanding" (M0.md:19, §5 M0.md:175). §13 defines 18 rules and
    // §9.1 lists five mitigations, so the record that will be committed beside the schema
    // describes a document two revisions old.
    let doc = schema_doc();
    let m0 = std::fs::read_to_string(MILESTONE_DOC).expect("the M0 record must be readable at its documented path");
    let rules = referential_rule_numbers(&doc).len();
    let mitigations = doc
        .split_once("### 9.1 The artboard-id hazard")
        .expect("§9.1 exists")
        .1
        .split_once("\n## 10.")
        .expect("§10 follows §9.1")
        .0
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()) && l.contains(". **"))
        .count();
    let mut wrong = Vec::new();
    for (needle, expected) in [("referential rules", rules), ("artboard-id mitigations", mitigations)] {
        let mut rest = m0.as_str();
        while let Some(i) = rest.find(needle) {
            let word = rest[..i].trim_end().rsplit(char::is_whitespace).next().unwrap_or("").to_string();
            rest = &rest[i + needle.len()..];
            let Some(n) = word_number(&word) else { continue };
            if n as usize != expected {
                wrong.push(format!("M0.md says `{word} {needle}` but the schema has {expected}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the M0 record that will be committed beside the schema describes the frozen document \
         incorrectly: {}",
        wrong.join("; ")
    );
}

// =================================================================================================
// Round 6 — closing verification of the round-5 fixes.
//
// Round 5's six findings are fixed in the text these tests read: §8.3 shapes the job file as
// `{ schema_version, job, unknown }` so rule 3 has a version to check; rule 18 names
// `failed`/`cancelled` as terminal and cites `JobStatus::is_terminal`; §8.3 states the third,
// benign cause of a `files` mismatch; §5.5 pairs `ContentHash` with §7.2; the design's §5.3.1
// lists five mitigations like the schema; and the M0 record states the same counts. What round 6
// has to check is the residue the renumbering left behind: the design's list went from four items
// (item 2 folded "remembered" and "never reused") to five, so every *other* reference to the list
// by number still has to cover the guards the two documents call mandatory.
// =================================================================================================

/// The comic job statuses as §8.3's field list spells them (`GenerationJob.status`).
const JOB_STATUSES: [&str; 6] = ["queued", "running", "completed", "failed", "awaiting_review", "cancelled"];

const COMIC_MODEL_ENTITIES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/comic-model/src/entities.rs");

/// `AwaitingReview` -> `awaiting_review`, so a Rust variant compares with a wire name.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// The variants `JobStatus::is_terminal` matches, read from the code that defines the predicate.
fn code_terminal_statuses() -> Vec<String> {
    let code = std::fs::read_to_string(COMIC_MODEL_ENTITIES).expect("crates/comic-model/src/entities.rs must be readable");
    let body = code.split_once("pub fn is_terminal").expect("`JobStatus::is_terminal` exists in crates/comic-model").1;
    let body = body.split_once("\n    }").expect("`is_terminal`'s body closes").0;
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("Self::") {
        rest = &rest[at + "Self::".len()..];
        let name: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
        if !name.is_empty() {
            out.push(snake_case(&name));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The numbered items of a mitigation list (`N. **Title.** …`) as `(number, body)`. An item runs
/// to the next numbered item, so a wrapped sentence whose reference (`U1`, a rule number) sits on
/// a continuation line still belongs to its item.
fn mitigation_items(section: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    for line in section.lines().map(str::trim_start) {
        let item = line
            .split_once(". ")
            .filter(|(n, rest)| n.trim().parse::<u32>().is_ok() && rest.starts_with("**"))
            .map(|(n, rest)| (n.trim().parse::<u32>().expect("digits"), rest.to_string()));
        match item {
            Some((n, body)) => out.push((n, body)),
            None => {
                if let Some((_, body)) = out.last_mut() {
                    body.push(' ');
                    body.push_str(line);
                }
            }
        }
    }
    out
}

/// Every `mitigations A–B` range in a document as `(line number, line, start, end)`.
fn mitigation_ranges(doc: &str) -> Vec<(usize, String, u32, u32)> {
    let mut out = Vec::new();
    for (i, line) in doc.lines().enumerate() {
        let lower = line.to_lowercase();
        let mut rest = lower.as_str();
        while let Some(at) = rest.find("mitigations ") {
            rest = &rest[at + "mitigations ".len()..];
            let start: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if start.is_empty() {
                continue;
            }
            let dash: String = rest.chars().skip(start.len()).take(1).collect();
            if !matches!(dash.as_str(), "–" | "-" | "—") {
                continue;
            }
            let end: String = rest.chars().skip(start.len() + 1).take_while(|c| c.is_ascii_digit()).collect();
            if end.is_empty() {
                continue;
            }
            out.push((i + 1, line.trim().to_string(), start.parse().expect("digits"), end.parse().expect("digits")));
        }
    }
    out
}

#[test]
fn rule_18s_terminal_statuses_are_the_ones_the_code_calls_terminal() {
    // Rule 18's predicate has to be the same one the workspace implements, or the schema and the
    // code disagree about which job files are legal. The code is the authority the schema cites:
    // `crates/comic-model/src/entities.rs`, `is_terminal() = matches!(self, Failed | Cancelled)`,
    // whose doc comment freezes it against schema §8.1. Reading both sides is what makes the two
    // claims one. (The predicate lived in `crates/comic-ai` until M1; that crate now re-exports it
    // from `comic-model`, where the frozen schema puts it, so this file must not be pointed back at
    // `comic-ai` — it no longer defines the predicate that fails this test.)
    let schema = schema_doc();
    let (n, rule18) = referential_rules(&schema).into_iter().find(|(n, _)| *n == 18).expect("§13 has rule 18");
    let code = code_terminal_statuses();
    assert_eq!(
        code,
        vec!["cancelled".to_string(), "failed".to_string()],
        "crates/comic-model's `is_terminal` must keep matching exactly the two terminal statuses; the \
         schema's rule 18 is checked against this set"
    );
    let clause = rule18.split_once("Terminal is exactly").unwrap_or_else(|| {
        panic!(
            "rule {n} says \"{rule18}\" and §8.3 removes a job file at a terminal state, but neither \
             names which of the six `JobStatus` values is terminal ({JOB_STATUSES:?}); the rule is \
             undecidable until it does"
        )
    });
    let clause = clause.1.split(';').next().unwrap_or(clause.1);
    let mut doc: Vec<String> = clause.split('`').skip(1).step_by(2).filter(|w| JOB_STATUSES.contains(w)).map(str::to_string).collect();
    doc.sort();
    doc.dedup();
    assert_eq!(
        doc, code,
        "rule {n} says \"{rule18}\" but `crates/comic-model`'s `is_terminal` matches {code:?}: a job \
         file with a status the code calls non-terminal is then illegal in `jobs/`, or vice versa"
    );
}

#[test]
fn the_jobs_file_wrapper_carries_the_schema_version_rule_three_checks() {
    // §8.3 must fix the file's bytes: rule 3 enumerates every `jobs/job-<id>.json` among the files
    // that declare `schema_version`, and §8.1's `GenerationJob` has no such field, so the version
    // can only live on a wrapper — the shape §5.4, §6, §7 and §8 each use for their own file.
    let doc = schema_doc();
    let jobs = doc.split_once("### 8.3 `jobs/`").expect("§8.3 exists").1;
    let jobs = jobs.split_once("\n## 9.").expect("§9 follows §8.3").0;
    let flat = flattened(jobs);
    assert!(
        flat.contains(r#"{ "schema_version": 1, "job": { … }, "unknown": {} }"#),
        "rule 3 lists every `jobs/job-<id>.json` among the files that declare `schema_version`, so \
         §8.3 has to show the file's wrapper (version, entity, unknown) — a bare `GenerationJob` \
         leaves the bytes undetermined. §8.3 text: {jobs:?}"
    );
    let entity = doc.split_once("### 8.1 `GenerationJob`").expect("§8.1 exists").1;
    let entity = entity.split_once("### 8.2").expect("§8.2 follows §8.1").0;
    assert!(
        !entity.contains("schema_version"),
        "§8.1's `GenerationJob` grew a `schema_version` of its own, which is what the wrapper in \
         §8.3 exists to avoid (and what §5.7's canonical field order is written against)"
    );
}

#[test]
fn the_design_and_the_schema_state_the_same_mandatory_mitigations() {
    // Round 5's finding 5 pinned the two documents to the same guard set; the fix made the design
    // list five items with the schema's numbering. The count a section states, the items it lists,
    // the item each guard sits at and the closing range all have to agree across the two — this is
    // the check that fails if either list is renumbered without the others.
    let schema = schema_doc();
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable");
    let s_section = schema.split_once("### 9.1 The artboard-id hazard").expect("§9.1 exists").1;
    let s_section = s_section.split_once("\n## 10.").expect("§10 follows §9.1").0;
    let d_section = design.split_once("### 5.3.1 The artboard-id hazard").expect("design §5.3.1 exists").1;
    let d_section = d_section.split_once("\n---").expect("design §5.3.1 ends at a rule").0;

    let s_items = mitigation_items(s_section);
    let d_items = mitigation_items(d_section);
    assert_eq!(s_items.len(), 5, "schema §9.1 must list five mitigations, found {s_items:?}");
    assert_eq!(d_items.len(), s_items.len(), "design §5.3.1 lists {} mitigations and schema §9.1 lists {}", d_items.len(), s_items.len());

    for (name, section, items) in [("schema §9.1", s_section, &s_items), ("design §5.3.1", d_section, &d_items)] {
        let stated = section
            .split_once(" mitigations")
            .and_then(|(before, _)| before.rsplit(char::is_whitespace).next())
            .and_then(word_number)
            .unwrap_or_else(|| panic!("{name} must state how many mitigations are mandatory"));
        assert_eq!(stated as usize, items.len(), "{name} says {stated} mitigations are mandatory but lists {}", items.len());
    }

    let guards = s_items.len() as u32 - 1;
    for (name, items) in [("schema §9.1", &s_items), ("design §5.3.1", &d_items)] {
        let at = |needle: &str| items.iter().find(|(_, t)| t.contains(needle)).map(|(n, _)| *n);
        assert_eq!(
            at("Retired ids are remembered"),
            Some(2),
            "{name}: the retired-id memory must be item 2, so the never-reuse guard can be its own item 4"
        );
        assert_eq!(
            at("never reuses a retired id"),
            Some(4),
            "{name}: the never-reuse guard must be item 4 — it is the one §9.1 says keeps rules 4 and 17 from turning the hazard into a permanent finding"
        );
        assert_eq!(at("U1"), Some(s_items.len() as u32), "{name}: the upstream proposal must be the last item, so the closing range can exclude it");
    }

    for (name, section) in [("schema §9.1", s_section), ("design §5.3.1", d_section)] {
        let closing = section
            .lines()
            .find(|l| l.contains("stand alone") || l.contains("stand on their own"))
            .unwrap_or_else(|| panic!("{name} must say which mitigations stand without the upstream fix"));
        let range: Vec<u32> = closing
            .split(" stand")
            .next()
            .unwrap_or("")
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        assert_eq!(
            range,
            vec![1, guards],
            "{name} says \"{closing}\" — with {} items and one upstream proposal, the guards that stand alone are 1–{guards}",
            s_items.len()
        );
    }
}

#[test]
fn every_mitigation_range_covers_the_guards_the_documents_call_mandatory() {
    // Round 5's fix split the design's folded item 2 into item 2 and item 4 and renumbered the
    // list from four to five. Every *other* sentence that refers to the list by number has to
    // follow: design §20's risk table (docs/comiccraft-design.md:1650) still reads "Mitigations
    // 1–3 are mandatory in M1", which after the split excludes item 4 — the guard that stops a
    // deleted page's artboard id from silently re-pointing at a new page, and which §9.1 and the
    // design's own §5.3.1 call mandatory. A range that stops at 3 tells M1 to ship without it.
    let schema = schema_doc();
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable");
    let guards = mitigation_items(
        schema.split_once("### 9.1 The artboard-id hazard").expect("§9.1 exists").1.split_once("\n## 10.").expect("§10 follows §9.1").0,
    )
    .len() as u32
        - 1;

    let mut wrong = Vec::new();
    for (name, doc) in [("docs/comiccraft-design.md", &design), ("docs/comiccraft-project-schema.md", &schema)] {
        let ranges = mitigation_ranges(doc);
        assert!(!ranges.is_empty(), "{name} must still refer to the mitigation list by number");
        for (line, text, start, end) in ranges {
            if start != 1 || end < guards {
                wrong.push(format!("{name}:{line} says \"{text}\" — every reference has to cover the guards 1–{guards}"));
            }
        }
    }
    assert!(wrong.is_empty(), "the mitigation list was renumbered but a reference to it was not: {}", wrong.join("; "));
}

// =================================================================================================
// Round 7 (code-reviewer): the count/range sweep, test-guarded.
//
// Rounds 4, 5 and 6 each shipped a count that had drifted from the list it described — a mitigation
// list that gained an item while a reference to it by number stayed behind, a design section that
// said "Four" while listing five, a risk table that excluded a mandatory guard. Every one of them
// was a *mechanical* drift between an enumeration and its contents, and every one was found by
// reading rather than by the suite. This block is the sweep as a test: the count a sentence states
// against the list the sentence introduces, a stated range of ADRs against the statuses the same
// table gives its rows, and a capability the design calls a *gap* against the same document's own
// "Resolved" records.
// =================================================================================================

const PLAN_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-porting-plan.md");

/// Every `<count> <plural noun>` pair in a paragraph, e.g. `[(5, "mitigations")]` for
/// "Five mitigations are mandatory, because …". The count must be spelled: a bare digit in these
/// documents is a section, rule or step number, so "rule 17 makes" and "§14 reports" are not
/// counts, and a sweep that read them as counts would report noise rather than defects.
fn count_noun_pairs(paragraph: &str) -> Vec<(u32, String)> {
    let tokens: Vec<String> = paragraph.split_whitespace().map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_string()).collect();
    let mut out = Vec::new();
    for pair in tokens.windows(2) {
        let (word, noun) = (&pair[0], &pair[1]);
        let Some(n) = word_number(word) else { continue };
        if n == 0 || !noun.ends_with('s') || noun.len() < 4 {
            continue;
        }
        out.push((n, noun.clone()));
    }
    out
}

/// Whether a line starts a new block of the document other than a prose paragraph: a heading, a
/// table row, a list item, a block quote or a fence. A wrapped line inside a paragraph is prose.
fn starts_a_block(line: &str) -> bool {
    let line = line.trim();
    let numbered = line.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    line.starts_with('#')
        || line.starts_with('|')
        || line.starts_with('>')
        || line.starts_with("```")
        || line.starts_with("- ")
        || line.starts_with("* ")
        || numbered
}

/// The list items directly under a sentence: `- `/`* ` bullets or `N. ` items, up to the end of the
/// block. A wrapped continuation line belongs to the item above it and is not an item of its own.
fn introduced_items(lines: &[&str]) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for line in lines {
        let text = line.trim();
        let bullet = ["- ", "* "].iter().any(|m| text.starts_with(m));
        let numbered = text.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if bullet || numbered {
            items.push(text.to_string());
        } else if text.is_empty() {
            if !items.is_empty() {
                break;
            }
        } else if items.is_empty() {
            break;
        }
    }
    items
}

#[test]
fn every_count_a_document_states_is_the_length_of_the_list_it_introduces() {
    // The sweep rounds 4–6 needed and never had. A paragraph that introduces a list by counting it
    // ("Five mitigations, all mandatory:", "Three rules make this enforceable:", "Two limits are
    // real and are recorded rather than hidden:", and the wrapped "Five mitigations are mandatory,
    // because … :") is checkable, so it is checked — in all three documents, not only the one a
    // finding named. A wrapped sentence is read as its paragraph, so a count is not missed because
    // the sentence reflowed. The count has to equal the list it introduces, or a reader who trusts
    // the count builds the wrong set.
    let documents = [
        ("docs/comiccraft-project-schema.md", schema_doc()),
        ("docs/comiccraft-design.md", std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable")),
        ("docs/comiccraft-porting-plan.md", std::fs::read_to_string(PLAN_DOC).expect("the porting plan must be readable")),
    ];
    let mut wrong = Vec::new();
    let mut checked = 0usize;
    for (name, text) in documents {
        let lines: Vec<&str> = text.lines().collect();
        let mut fenced = false;
        let mut i = 0usize;
        while i < lines.len() {
            let raw = lines[i];
            let line = raw.trim();
            if line.starts_with("```") {
                fenced = !fenced;
                i += 1;
                continue;
            }
            // A paragraph starts and wraps at column 0; an indented line is the continuation of a
            // list item or a nested block, and reading it as prose would count its numbers.
            let indented = raw.starts_with(|c: char| c.is_whitespace());
            if fenced || line.is_empty() || indented || starts_a_block(line) {
                i += 1;
                continue;
            }
            let paragraph_start = i + 1;
            let mut paragraph = String::new();
            while i < lines.len() {
                let raw = lines[i];
                if raw.trim().is_empty() || raw.starts_with(|c: char| c.is_whitespace()) || starts_a_block(raw) {
                    break;
                }
                paragraph.push_str(raw.trim());
                paragraph.push(' ');
                i += 1;
            }
            let pairs = count_noun_pairs(&paragraph.replace(['*', '`'], ""));
            if pairs.len() != 1 {
                continue;
            }
            let (stated, noun) = pairs[0].clone();
            let items = introduced_items(&lines[i..]);
            if items.is_empty() {
                continue;
            }
            checked += 1;
            if stated as usize != items.len() {
                wrong.push(format!("{name}:{paragraph_start} says \"{}\" — {stated} {noun}, but the list has {}", paragraph.trim(), items.len()));
            }
        }
    }
    assert!(checked >= 5, "the sweep is only evidence if it finds the documents' counted lists; it found {checked}");
    assert!(wrong.is_empty(), "a count disagrees with the list it introduces: {}", wrong.join("; "));
}

#[test]
fn the_design_covers_every_adr_its_own_table_marks_new() {
    // §16 opens "ADR-011 … ADR-016 were taken or revised in this document" and then tables every ADR
    // with its status. The range names the decisions this document took; the table is the list. If
    // the table marks an ADR "(new)" that the range stops short of, the two statements disagree about
    // which decisions a reader has to carry out of §16 — the shape of rounds 4–6's drifted counts.
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable");
    let section = design.split_once("# 16. Architecture decisions").expect("§16 exists").1;
    let (intro, table) = section.split_once("| ID | Decision | Status |").expect("§16 tables every ADR");
    let mut new: Vec<u32> = Vec::new();
    for line in table.lines() {
        if !line.contains("| Accepted (new)") {
            continue;
        }
        let Some(rest) = line.split("ADR-").nth(1) else { continue };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<u32>() {
            new.push(n);
        }
    }
    let ends: Vec<u32> = intro
        .split("ADR-")
        .skip(1)
        .filter_map(|rest| rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u32>().ok())
        .collect();
    assert!(ends.len() >= 2, "§16's intro must name the span of ADRs taken here, found {ends:?}");
    let (first, last) =
        (ends.iter().copied().min().expect("at least two ADR numbers"), ends.iter().copied().max().expect("at least two ADR numbers"));
    let range: Vec<u32> = (first..=last).collect();
    let missing: Vec<String> = new.iter().filter(|n| !range.contains(n)).map(|n| format!("ADR-{n:03}")).collect();
    assert!(
        missing.is_empty(),
        "§16's intro says \"{}\", but its own table marks {} ADRs \"Accepted (new)\" ({}) — {} not \
         named by the range, so a reader building the list of decisions this document took drops it",
        flattened(intro),
        new.len(),
        new.iter().map(|n| format!("ADR-{n:03}")).collect::<Vec<_>>().join(", "),
        missing.join(", ")
    );
}

#[test]
fn the_design_does_not_still_call_a_capability_it_records_as_resolved_a_gap() {
    // §19 ends by enumerating what still has to be built and counting it: "There is still no …, no …,
    // no comic lettering font, and no … — those four are the real gaps." The same document records
    // the lettering faces as **Resolved (ADR-017)** twice (§8.3 "The two faces are now in
    // `craft-fonts`", §20 R1 "tests green"), so one of the two statements is stale. A capability
    // recorded as resolved is not still a gap: the sentence either drops it — and the count with it —
    // or says where the faces come from. Either fix passes; silence does not.
    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable");
    let flat = flattened(&design);
    let start = flat.find("There is still no").expect("§19 states what is still missing");
    let tail = &flat[start..];
    let sentence = &tail[..tail.find("the real gaps").expect("the sentence counts its list") + "the real gaps".len()];

    let listed: Vec<String> = sentence
        .trim_start_matches("There is still no ")
        .split(", no ")
        .flat_map(|part| part.split(" and no "))
        .map(|part| part.split(" —").next().unwrap_or(part).trim().to_string())
        .collect();
    let stated = sentence.split("those ").nth(1).and_then(|rest| word_number(rest.split_whitespace().next().unwrap_or("")));
    assert_eq!(stated, Some(listed.len() as u32), "§19 states a count that disagrees with its own list ({} items): \"{sentence}\"", listed.len());

    let records_resolved = design.contains("**Resolved (ADR-017).**")
        && design.contains("Comic Neue")
        && design.lines().any(|l| l.contains("| R1 |") && l.contains("Resolved (ADR-017)"));
    assert!(records_resolved, "the design must record ADR-017's lettering faces as resolved for this check to mean anything");
    let as_gaps: Vec<&str> = listed.iter().filter(|item| item.contains("lettering") || item.contains("font")).map(String::as_str).collect();
    let acknowledges = sentence.contains("craft-fonts") || sentence.contains("ADR-017") || sentence.contains("resolved");
    assert!(
        as_gaps.is_empty() || acknowledges,
        "§19 lists \"{}\" among the real gaps, but the same document records the lettering faces as \
         **Resolved (ADR-017)** — §8.3 \"The two faces are now in `craft-fonts`\" and §20 R1 \"tests \
         green\". A capability recorded as resolved is not still a gap: drop it and correct the count, \
         or name where the faces come from",
        as_gaps.join("\", \"")
    );
}

const AUDIT_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-porting-audit.md");

/// Every `Q<number>` a document names.
fn question_citations(doc: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in doc.lines().enumerate() {
        let bytes = line.as_bytes();
        for (at, b) in bytes.iter().enumerate() {
            if *b != b'Q' || at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
                continue;
            }
            let digits: String = line[at + 1..].chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() && digits.len() <= 2 {
                out.push((i + 1, format!("Q{digits}")));
            }
        }
    }
    out
}

/// Every `Q<number>` a document *defines*: a list or table row, or a heading.
fn question_items(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|line| {
            let text = line.trim();
            let rest = text.strip_prefix('|').unwrap_or(text).trim();
            let rest = rest.strip_prefix("- ").unwrap_or(rest);
            let rest = rest.trim_start_matches('#').trim_start();
            let digits: String = rest.strip_prefix('Q')?.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then(|| format!("Q{digits}"))
        })
        .collect()
}

#[test]
fn every_question_citation_names_a_question_that_exists() {
    // §5.6 justifies placing `GenerationJob.consent` and `Provenance.consent` by pointing at
    // "§9.3/Q13". The section exists; the question does not — no document numbers a question 13
    // (the design's open items are the §20 risk table, `R1`–`R10`, and the audit's are `U1`–`U8`).
    // A citation has to land on an item that exists: §5.5 `:224` cites the same requirement as
    // "design §9.3", and design §9.3 `:1032` is where it is recorded. The row still has to point at
    // that section afterwards, so the check cannot be disarmed by deleting the citation.
    let schema = schema_doc();
    let row =
        schema.lines().find(|l| l.contains("`GenerationJob.consent` and `Provenance.consent`")).expect("§5.6 must still say why `consent` is placed");
    assert!(
        row.contains("§9.3"),
        "§5.6's consent row must still cite the design section that records the requirement; it now reads \"{}\"",
        row.trim()
    );

    let mut defined = Vec::new();
    for path in [DESIGN_DOC, AUDIT_DOC] {
        defined.extend(question_items(&std::fs::read_to_string(path).expect("the cited document must be readable")));
    }
    let citations = question_citations(&schema);
    let dangling: Vec<String> = citations
        .iter()
        .filter(|(_, q)| !defined.contains(q))
        .map(|(line, q)| format!("docs/comiccraft-project-schema.md:{line} cites `{q}`"))
        .collect();
    assert!(
        dangling.is_empty(),
        "a citation that names an item no document defines: {} — the design numbers its open items \
         `R1`–`R10` (§20) and the audit numbers its `U1`–`U8`, so a reader who follows `Q13` has \
         nothing to find. Cite the item that holds the requirement (design §9.3, §5.5's own form) \
         or drop the number",
        dangling.join(", ")
    );
}

// =================================================================================================
// Round 8 (code-reviewer): the lettering faces the frozen schema names are the ones the code loads.
//
// Schema §5.3 gives the ComicCraft lettering roles and the face each one uses — `dialogue_font`,
// `dialogue_font_emphasis` and `dialogue_font_aside` in **Comic Neue**, `display_font` in
// **Bangers** — and the paragraph above the table says the faces are *names*, not files, so a
// build without `CRAFT_FONTS_DIR` still loads. The code that has to make that true is
// `craft_fonts::comic_fonts()`, which filters the craft-fonts manifest down to the families it
// names (`COMIC_FAMILIES`, `crates/text/src/craft_fonts.rs`). A family the document names and the
// filter does not carry is a face a project can select and the font database will not have: the
// product's own default lettering would report a missing font. Design §8.3 names the same two
// faces, so all three — the frozen schema, the design and the code — must agree.
// =================================================================================================

const CRAFT_FONTS_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/text/src/craft_fonts.rs");

/// The cells of column `col` (0-based, counting the empty cell before the leading `|`) in the table
/// a header line introduces, as written. Stops at the first line that is not a table row.
fn table_column(section: &str, header: &str, col: usize) -> Vec<String> {
    let rest = section.split_once(header).unwrap_or_else(|| panic!("the table introduced by \"{header}\" is gone")).1;
    let mut out = Vec::new();
    for line in rest.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if out.is_empty() {
                continue;
            }
            break;
        }
        let cells: Vec<&str> = line.split('|').collect();
        let Some(cell) = cells.get(col).map(|c| c.trim()) else { continue };
        if cell.is_empty() || cell.chars().all(|c| c == '-' || c == ':') {
            continue;
        }
        out.push(cell.to_string());
    }
    out
}

/// The font family a table cell names: its backticked run if it has one, else the whole cell with
/// markdown emphasis removed. The style (`Regular`, `Bold`, …) that follows is not part of it.
fn face_family(cell: &str) -> String {
    let cell = cell.replace("**", "");
    if let Some(start) = cell.find('`')
        && let Some(end) = cell[start + 1..].find('`')
    {
        return cell[start + 1..start + 1 + end].trim().to_string();
    }
    cell.trim().to_string()
}

#[test]
fn the_lettering_faces_the_frozen_schema_names_are_the_ones_the_code_loads() {
    let schema = schema_doc();
    let section = schema.split_once("### 5.3 `ProjectSettings`").expect("§5.3 exists").1;
    let section = section.split_once("\n### 5.4").expect("§5.4 follows §5.3").0;
    let schema_faces: Vec<String> = table_column(section, "| Setting | Face | Used for |", 2).iter().map(|c| face_family(c)).collect();

    let design = std::fs::read_to_string(DESIGN_DOC).expect("the design must be readable");
    let section = design.split_once("## 8.3 Lettering").expect("§8.3 exists").1;
    let section = section.split_once("\n## 9.").expect("§9 follows §8.3").0;
    let design_faces: Vec<String> = table_column(section, "| Face | Styles | For |", 1).iter().map(|c| face_family(c)).collect();

    let src = std::fs::read_to_string(CRAFT_FONTS_SRC)
        .unwrap_or_else(|e| panic!("the code that loads the comic faces must be readable at {CRAFT_FONTS_SRC}: {e}"));
    let families = src.split_once("COMIC_FAMILIES").expect("the code still names the families `comic_fonts()` loads").1;
    let families = families.split_once(';').expect("the `COMIC_FAMILIES` declaration ends").0;
    let code_faces: Vec<String> = families.split('"').skip(1).step_by(2).map(str::to_string).collect();

    assert!(
        code_faces.len() >= 2 && schema_faces.len() >= 2 && design_faces.len() >= 2,
        "the check is only evidence if all three name the faces; code {code_faces:?}, schema {schema_faces:?}, design {design_faces:?}"
    );
    for face in &schema_faces {
        assert!(
            !face.contains('.') && !face.contains('/'),
            "schema §5.3 names `{face}` as a face — the faces are family names resolved through the font database, never a bundled file (ADR-017)"
        );
    }

    let mut unique: Vec<&String> = schema_faces.iter().collect();
    unique.sort();
    unique.dedup();
    let mut code_sorted = code_faces.clone();
    code_sorted.sort();
    let mut design_sorted = design_faces.clone();
    design_sorted.sort();
    design_sorted.dedup();
    let schema_sorted: Vec<String> = unique.into_iter().cloned().collect();
    assert_eq!(
        schema_sorted, code_sorted,
        "schema §5.3's faces must be the families `craft_fonts::comic_fonts()` loads — the document names \
         {schema_sorted:?} and the filter carries {code_sorted:?}, so a face the document names and the \
         code does not load would report a missing font for the product's own default lettering"
    );
    assert_eq!(
        design_sorted, code_sorted,
        "design §8.3's faces must be the families `craft_fonts::comic_fonts()` loads — the design names \
         {design_sorted:?} and the filter carries {code_sorted:?}"
    );
}
