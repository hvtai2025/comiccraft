//! The v2 fixture: the canonical form of the version this crate writes (§5.7, §11.1).
//!
//! `tests/fixtures/v2/minimal.comiccraft/` is `tests/fixtures/v1/minimal.comiccraft/` **read
//! forward**: every file declares `schema_version: 2`, every panel carries `locked: false` (v2's one
//! added field, §6.2, at its default), and the manifest's `files` hashes describe those bytes. Not
//! one id, timestamp or unknown field moved. `tests/fixture_v1.rs` proves that relationship by
//! saving the v1 fixture and comparing the result with this one byte for byte, so the two fixtures
//! cannot drift into two hand-written copies.
//!
//! It is the fixture §11.1's byte-identical round trip now belongs to: the current version's fixture
//! must save and reload to the same bytes, and a fixture for an **older** version must migrate
//! forward instead (§11.1 lists every previous version).
//!
//! The manifest's `title` still reads `Minimal v1 fixture`, deliberately: the v2 fixture is the
//! *same project* read forward, and a migration that retitled it would be inventing authored data
//! (§11 preserves every id and every unknown field). That is also why this fixture is derived rather
//! than hand-written — the only way to be sure the two fixtures describe one project is to produce
//! one from the other.
//!
//! Regenerate it (and only it — it is derived, never hand-edited) with:
//!
//! ```text
//! cargo test -p vectorcraft-comic-model --test fixture_v2 -- --ignored regenerate_the_v2_fixture --nocapture
//! ```
//!
//! `clippy.toml` allows panics in tests; an integration-test crate needs the file-level equivalent.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vectorcraft_comic_model::{
    ArtboardFacts, DocumentFacts, Error, Finding, NodeId, PageId, PanelId, SCHEMA_VERSION, SaveOptions, ValidationInput, load, save, validate,
};
use vectorcraft_geom::Rect;

const FIXTURE_V1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");
const FIXTURE_V2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v2/minimal.comiccraft");

/// The fixture's `manifest.updated`, which a save writes back, so the manifest's bytes do not move.
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";

fn fixture_v1_root() -> PathBuf {
    PathBuf::from(FIXTURE_V1)
}

fn fixture_v2_root() -> PathBuf {
    PathBuf::from(FIXTURE_V2)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{}-{name}", std::process::id()));
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("destination");
    for entry in fs::read_dir(from).expect("source") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// Every file under `root`, keyed by its `/`-separated relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("type").is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).expect("inside").to_string_lossy().replace('\\', "/");
                out.insert(rel, fs::read(&path).expect("readable"));
            }
        }
    }
    out
}

/// A copy of the fixture in a scratch directory, for the tests that must change a file.
fn damaged_copy(name: &str) -> PathBuf {
    let root = scratch(name);
    copy_tree(&fixture_v2_root(), &root);
    root
}

/// A copy of the v2 fixture with one string replaced in one file.
fn with_file_rewritten(name: &str, key: &str, from: &str, to: &str) -> PathBuf {
    let root = damaged_copy(name);
    let path = root.join(key);
    let text = fs::read_to_string(&path).expect("readable");
    assert!(text.contains(from), "`{key}` does not contain {from:?}, so this test would prove nothing");
    fs::write(&path, text.replace(from, to)).expect("write");
    root
}

/// What rules 4, 7 and 15 need, from the fixture's own `.vectorcraft` document (see `fixture_v1.rs`).
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

// -------------------------------------------------------------------------------------------------
// The fixture itself.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_v2_fixture_is_the_version_this_crate_implements_and_is_not_migrated_again() {
    let loaded = load(&fixture_v2_root()).expect("the v2 fixture loads");
    assert_eq!(loaded.bundle.manifest.schema_version, SCHEMA_VERSION);
    assert!(loaded.migrations.is_empty(), "a current bundle is not a migration: {:?}", loaded.migrations);
    assert_eq!(loaded.findings, Vec::new(), "the canonical fixture is clean: {:?}", loaded.findings);
    for (file, version) in loaded.bundle.declared_versions() {
        assert_eq!(version, SCHEMA_VERSION, "`{file}` must declare the current version (rule 3)");
    }

    let page = loaded.bundle.pages.get(&PageId(8)).expect("pages/page-8.json holds page 8");
    assert_eq!(page.panels.len(), 3);
    assert!(page.panels.iter().all(|panel| !panel.locked), "the fixture is the v1 fixture migrated, so every panel takes v2's default");
    let bytes = fs::read_to_string(fixture_v2_root().join("pages/page-8.json")).expect("readable");
    assert_eq!(bytes.matches("\"locked\": false").count(), 3, "the field is written on every panel, not omitted when false:\n{bytes}");
}

#[test]
fn the_v2_fixture_validates_with_no_findings() {
    let loaded = load(&fixture_v2_root()).expect("loads");
    let document = document_facts();
    let findings = validate(&loaded.bundle, &ValidationInput { root: &fixture_v2_root(), document: &document, archive_root: None });
    assert_eq!(findings, Vec::new(), "the canonical fixture must be clean, warnings included: {findings:?}");
}

#[test]
fn the_v2_fixture_round_trips_byte_identically_and_a_second_save_moves_nothing() {
    // §11.1's strongest statement about §5.7's canonical form, at the version this crate writes: a
    // save is compared byte for byte against the committed fixture, so a change to field order, to
    // number form, to the sorted-ness of a map, or to the trailing newline fails here rather than in
    // a creator's bundle. Saving twice says the form is a fixed point, not merely correct once.
    let root = scratch("v2-round-trip");
    copy_tree(&fixture_v2_root(), &root);
    let options = SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };

    let mut loaded = load(&root).expect("the copy loads");
    let ids_before = (loaded.bundle.highest_comic_id(), loaded.bundle.next_comic_id());
    save(&mut loaded.bundle, &root, &options).expect("saves");
    assert_eq!((loaded.bundle.highest_comic_id(), loaded.bundle.next_comic_id()), ids_before, "a save preserves every id and the counter");

    let expected = tree(&fixture_v2_root());
    let actual = tree(&root);
    assert_eq!(actual.keys().collect::<Vec<_>>(), expected.keys().collect::<Vec<_>>(), "a save writes exactly the fixture's files");
    for (key, want) in &expected {
        let got = actual.get(key).expect("present");
        assert_eq!(String::from_utf8_lossy(got), String::from_utf8_lossy(want), "`{key}` must round-trip byte-identically (§5.7, §11.1)");
    }

    let mut again = load(&root).expect("reloads");
    save(&mut again.bundle, &root, &options).expect("second save");
    assert_eq!(tree(&root), expected, "a save of a loaded bundle changes nothing");
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// The field: default, round trip, and what the reader does with a shape it cannot represent.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_locked_panel_round_trips_as_locked_and_unlocking_round_trips_back() {
    // The default is half the field's contract; the other half is that a *set* value survives a save
    // and a reload, and that unlocking is a change the writer records rather than ignores.
    let root = damaged_copy("v2-locked-round-trip");
    let options = SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };

    let mut loaded = load(&root).expect("loads");
    let page = loaded.bundle.pages.get_mut(&PageId(8)).expect("page 8");
    page.panels.iter_mut().find(|panel| panel.id == PanelId(10)).expect("panel 10").locked = true;
    save(&mut loaded.bundle, &root, &options).expect("saves");

    let written = fs::read_to_string(root.join("pages/page-8.json")).expect("readable");
    assert!(written.contains("\"locked\": true"), "a locked panel is written as `true`:\n{written}");
    assert_eq!(written.matches("\"locked\": false").count(), 2, "and the others stay `false`:\n{written}");
    assert_eq!(written.matches("\"locked\": true").count(), 1);

    let reloaded = load(&root).expect("reloads");
    let panels = &reloaded.bundle.pages[&PageId(8)].panels;
    assert_eq!(panels.iter().filter(|panel| panel.locked).map(|panel| panel.id.0).collect::<Vec<_>>(), vec![10]);

    let mut unlocked = reloaded.bundle;
    unlocked.pages.get_mut(&PageId(8)).expect("page 8").panels.iter_mut().find(|panel| panel.id == PanelId(10)).expect("panel 10").locked = false;
    save(&mut unlocked, &root, &options).expect("saves");
    let written = fs::read_to_string(root.join("pages/page-8.json")).expect("readable");
    assert!(!written.contains("\"locked\": true"), "unlocking is recorded:\n{written}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_saved_lock_does_not_reach_the_visual_document_or_another_file() {
    // ADR-013 and design §6.1: the lock is a comic record, and the document owns geometry and art.
    // A panel's lock must change exactly one file's bytes — the page file that holds it.
    let root = damaged_copy("v2-lock-scope");
    let options = SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    let before = tree(&root);

    let mut loaded = load(&root).expect("loads");
    loaded.bundle.pages.get_mut(&PageId(8)).expect("page 8").panels[0].locked = true;
    save(&mut loaded.bundle, &root, &options).expect("saves");

    let after = tree(&root);
    let changed: Vec<&String> = after.keys().filter(|key| after.get(*key) != before.get(*key)).collect();
    assert_eq!(
        changed.iter().map(|key| key.as_str()).collect::<Vec<_>>(),
        vec!["manifest.json", "pages/page-8.json"],
        "only the page file (and the manifest that hashes it) may change: {changed:?}"
    );
    let document = fs::read_to_string(root.join("document/comic.vectorcraft")).expect("readable");
    assert!(!document.contains("locked"), "no comic field may appear in the visual document (ADR-013)");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_v2_panel_without_the_key_loads_as_unlocked_and_reports_the_stale_manifest() {
    // §5.7: "A reader must accept any valid JSON". The key's absence is a value — v2's default — so
    // it loads; the bytes no longer match the manifest's `files`, and §14 makes that a *finding* on
    // a usable project rather than a refusal, exactly as for any other hand-edit.
    // Removing the key leaves the previous field without its trailing comma, because `locked` is the
    // last declared field a panel writes.
    let root = with_file_rewritten(
        "v2-key-absent",
        "pages/page-8.json",
        "\"generation_status\": \"none\",\n      \"locked\": false\n",
        "\"generation_status\": \"none\"\n",
    );
    let loaded = load(&root).expect("§5.7: a valid file loads");
    let panels = &loaded.bundle.pages[&PageId(8)].panels;
    assert_eq!(panels.len(), 3);
    assert!(panels.iter().all(|panel| !panel.locked), "the absent key is v2's default");
    let mismatch: Vec<&Finding> = loaded.findings.iter().filter(|finding| finding.subject == "pages/page-8.json").collect();
    assert_eq!(mismatch.len(), 1, "the hand-edit is reported: {:?}", loaded.findings);
    assert!(!mismatch[0].is_error(), "§14: a `files` mismatch does not lock the creator out: {mismatch:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_locked_value_the_code_cannot_represent_is_refused_rather_than_mis_read() {
    // §2/§14 step 3: a comic file that will not parse is fatal. `locked` is a bool, so anything else
    // — a truthy string, a number, `null` — is a file this build cannot represent, and guessing
    // (`"yes"` → `true`, `0` → `false`) is exactly what §11's "refuse rather than guess" forbids.
    for (tag, bogus) in [("string", "\"locked\": \"yes\""), ("number", "\"locked\": 1"), ("null", "\"locked\": null")] {
        let root = with_file_rewritten(&format!("v2-bogus-locked-{tag}"), "pages/page-8.json", "\"locked\": false", bogus);
        match load(&root) {
            Err(Error::Parse { path, what, .. }) => {
                assert_eq!(path, root.join("pages/page-8.json"), "the refusal names the file: {tag}");
                assert_eq!(what, "page file");
            }
            other => panic!("a `locked` this build cannot represent must be refused, got {other:?}"),
        }
        fs::remove_dir_all(&root).ok();
    }
}

// -------------------------------------------------------------------------------------------------
// The fixture is derived, not authored.
// -------------------------------------------------------------------------------------------------

/// Rewrite the checked-in v2 fixture from the v1 fixture. Not a test: a generator, ignored by
/// default so no run ever writes the checkout, and named in this file's header so the derivation is
/// reproducible. The tests above are what fails when the fixture and the code disagree.
#[test]
#[ignore = "writes tests/fixtures/v2/; run with --ignored when the v2 canonical form changes"]
fn regenerate_the_v2_fixture() {
    let dest = fixture_v2_root();
    fs::remove_dir_all(&dest).ok();
    copy_tree(&fixture_v1_root(), &dest);
    let mut loaded = load(&dest).expect("the migrated copy loads");
    let options = SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    save(&mut loaded.bundle, &dest, &options).expect("saves");
    println!("regenerated {} ({} files)", dest.display(), tree(&dest).len());
}
