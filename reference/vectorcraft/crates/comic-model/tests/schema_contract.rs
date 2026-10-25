//! Reviewer tests (`code-reviewer`, M0 round 1).
//!
//! The implementer's inline tests pin the version against a *literal*, which
//! restates the constant rather than checking the contract. The contract the schema freezes
//! is: the version this crate implements must equal the version `docs/comiccraft-project-schema.md`
//! declares, in every place it declares it. These tests read the frozen document, so a bump
//! on either side without the other is caught.

// `clippy.toml` sets `allow-panic-in-tests`, but clippy only applies it to `#[cfg(test)]`
// modules, not to an integration-test crate. The repository's policy (clippy.toml header) is
// that tests may panic, so this is the file-level equivalent for an integration test.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use vectorcraft_comic_model::{ComicId, SCHEMA_VERSION};

const SCHEMA_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/comiccraft-project-schema.md");

fn schema_doc() -> String {
    fs::read_to_string(SCHEMA_DOC).unwrap_or_else(|e| panic!("the frozen schema doc must be readable at {SCHEMA_DOC}: {e}"))
}

/// Every `"schema_version": N` literal in the frozen document's JSON examples.
fn schema_versions_in_examples(doc: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut rest = doc;
    while let Some(i) = rest.find("\"schema_version\"") {
        rest = &rest[i + "\"schema_version\"".len()..];
        let Some(colon) = rest.find(':') else { break };
        rest = &rest[colon + 1..];
        let digits: String = rest.chars().skip_while(|c| c.is_whitespace()).take_while(char::is_ascii_digit).collect();
        assert!(!digits.is_empty(), "a `\"schema_version\"` key with no integer value; the schema's shape changed");
        out.push(digits.parse().expect("schema_version is a u32 per schema §4"));
    }
    out
}

#[test]
fn schema_version_matches_every_example_in_the_frozen_document() {
    let doc = schema_doc();
    let versions = schema_versions_in_examples(&doc);
    assert!(!versions.is_empty(), "no `schema_version` example found — the parse must not silently pass on an empty set");
    for v in &versions {
        assert_eq!(*v, SCHEMA_VERSION, "the crate implements {SCHEMA_VERSION} but the frozen schema shows {v}");
    }
}

#[test]
fn schema_version_matches_the_document_that_declares_it_frozen() {
    // schema §1: "It is **frozen at version 2** for M2, and was frozen at version 1 for M1." The
    // phrase is built from the constant rather than repeated, so the two sides still cannot move
    // one without the other — which is the whole point of reading the document here.
    let doc = schema_doc();
    let declared = format!("frozen at version {SCHEMA_VERSION}");
    assert!(doc.contains(&declared), "the schema document no longer declares itself `{declared}`; SCHEMA_VERSION must be revisited");
}

#[test]
fn the_document_places_the_v2_field_on_the_panel_and_gives_it_no_rule() {
    // schema §6.2 documents v2's one field, and §13 gains no rule for it. A rule row for `locked`
    // would be a referential rule where none can exist — locking relates nothing to anything — so
    // the table shape (`| `locked` |`) is what this refuses to find under §13, prose apart.
    let doc = schema_doc();
    assert!(doc.contains("| `locked` | `bool` | yes |"), "§6.2 must document v2's field with its type");
    let rules = doc.split("## 13. Referential rules").nth(1).expect("§13 is in the document");
    let rules = rules.split("## 14.").next().expect("§13 ends at §14");
    assert!(!rules.contains("`locked` |"), "§13 must gain no rule for `locked`");
}

#[test]
fn comic_id_accepts_only_the_bare_integer_shape() {
    // Schema §9: "Every comic id is a #[serde(transparent)] newtype over u64, so a JSON id
    // is a bare number." Anything else must fail loudly rather than coerce silently.
    assert!(serde_json::from_str::<ComicId>("7").is_ok());
    assert!(serde_json::from_str::<ComicId>("\"7\"").is_err(), "a quoted id is not the frozen shape");
    assert!(serde_json::from_str::<ComicId>("1.5").is_err(), "an id is a u64, not a float");
    assert!(serde_json::from_str::<ComicId>("-1").is_err(), "an id is unsigned");
    assert!(serde_json::from_str::<ComicId>("{}").is_err(), "an id is not an object");
    assert!(serde_json::from_str::<ComicId>("[7]").is_err(), "an id is not an array");
    assert!(serde_json::from_str::<ComicId>("null").is_err(), "an id is never null");
}

#[test]
fn comic_id_round_trips_at_the_u64_boundary() {
    // `u64` is the declared width; the boundary is where a narrowing conversion would hide.
    for raw in [0u64, 1, u64::from(u32::MAX), u64::MAX] {
        let id = ComicId(raw);
        let json = serde_json::to_string(&id).expect("serializes");
        assert_eq!(json, raw.to_string(), "an id must serialize as a bare number");
        assert_eq!(serde_json::from_str::<ComicId>(&json).expect("deserializes"), id);
    }
}

#[test]
fn comic_id_display_matches_the_wire_form() {
    // `Display` is what provenance and log lines will use; it must not add decoration that
    // would make an id unparseable back.
    assert_eq!(ComicId(7).to_string(), "7");
    assert_eq!(ComicId(u64::MAX).to_string(), u64::MAX.to_string());
    let rendered = ComicId(42).to_string();
    assert_eq!(serde_json::from_str::<ComicId>(&rendered).expect("Display output re-parses"), ComicId(42));
}

// -------------------------------------------------------------------------------------------------
// Round 2 — failure paths and edges. Round 1 pinned the *shape* of a `ComicId`; round 2 pins the
// edges of the `u64` itself and the empty/degenerate inputs, which are what a corrupted or
// hand-edited bundle actually contains. Schema §9: ids are opaque and never reused; a silently
// wrapped or truncated id would break every cross-reference that points at it.
// -------------------------------------------------------------------------------------------------

#[test]
fn an_id_past_the_u64_boundary_is_refused_rather_than_wrapped() {
    // `u64::MAX + 1` and `1e23` both overflow. Wrapping them would alias a new id onto an
    // existing one — exactly the reuse schema §9 forbids — so both must be refused.
    assert_eq!(serde_json::from_str::<ComicId>(&u64::MAX.to_string()).expect("u64::MAX is a legal id"), ComicId(u64::MAX));
    for too_big in ["18446744073709551616", "99999999999999999999999", "184467440737095516150", "1e23", "1e3", "1.5", "7.0"] {
        assert!(serde_json::from_str::<ComicId>(too_big).is_err(), "{too_big} is past the u64 boundary and must not wrap");
    }
    for negative in ["-0", "-1", "-9223372036854775809"] {
        assert!(serde_json::from_str::<ComicId>(negative).is_err(), "{negative} is not a u64");
    }
}

#[test]
fn an_empty_or_degenerate_id_input_is_refused() {
    // Empty input, whitespace only, and the number-like spellings JSON does not allow. Each must
    // fail loudly: an id that reads as zero would point every reference at the first entity.
    for bad in ["", " ", "\t", "null", "true", "false", "[7]", "[0]", "{}", r#"{"id":7}"#, "+7", "07", "0x7", "seven"] {
        assert!(serde_json::from_str::<ComicId>(bad).is_err(), "{bad:?} must not deserialize to a ComicId");
    }
    // Leading whitespace *around* a number is legal JSON and must still load — the refusal above
    // is about the value, not about leniency the format already sanctions.
    assert_eq!(serde_json::from_str::<ComicId>(" 7").expect("JSON permits surrounding whitespace"), ComicId(7));
    // Zero is legal: it is the first id the bundle ever allocates.
    assert_eq!(serde_json::from_str::<ComicId>("0").expect("0 is a legal id"), ComicId(0));
}

#[test]
fn ids_compare_and_sort_the_way_the_schema_declares_them() {
    // Schema §9: ids are opaque — "never derive meaning from magnitude or order". The derived
    // `Ord` therefore exists only to sort deterministically; this pins that behaviour so a
    // change to the newtype's derives is noticed.
    let mut ids = vec![ComicId(9), ComicId(0), ComicId(u64::MAX), ComicId(1)];
    ids.sort();
    assert_eq!(ids, vec![ComicId(0), ComicId(1), ComicId(9), ComicId(u64::MAX)]);
    let set: std::collections::BTreeSet<ComicId> = ids.iter().copied().collect();
    assert_eq!(set.len(), ids.len(), "distinct ids must not collide in a set");
    assert_ne!(ComicId(0), ComicId(1));
    assert_eq!(ComicId(7), ComicId(7));
}
