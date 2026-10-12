//! M2 W1 review — round 1 (`code-reviewer`).
//!
//! The implementer's own tests (`fixture_v1.rs`, `fixture_v2.rs`) pin the migration's *report*, the
//! default and the byte identity. These tests take the boundaries those do not cover:
//!
//! - the write half of §2's "the writer writes the version it implements", stated **on disk** rather
//!   than through the identity test (which compares against a fixture the same writer generated);
//! - that a **failed** save leaves the model at the version it *read*, not the one it would have
//!   written — the property `save`'s own doc comment claims and §10 depends on;
//! - the hand-edited v1 `locked` surviving a **save**, not only a load;
//! - "locking adds no referential rule" as a **behavioural** claim: the same bundle validates
//!   identically locked and unlocked, and a locked panel whose frame node is gone is reported by the
//!   rule that already governs a missing frame node;
//! - §2's asymmetry in the direction a *wrong writer* would produce: a v2 manifest over a v1 page
//!   file is rule 3's finding, not a refusal.
//!
//! `clippy.toml` allows panics in tests; an integration-test crate needs the file-level equivalent.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use vectorcraft_comic_model::{
    ArtboardFacts, DocumentFacts, Finding, NodeId, PageId, SCHEMA_VERSION, SaveOptions, ValidationInput, load, save, validate,
};
use vectorcraft_geom::Rect;

const FIXTURE_V1: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");
const FIXTURE_V2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v2/minimal.comiccraft");
const NOW: &str = "2026-10-11T06:00:00Z";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m2w1-review-{}-{name}", std::process::id()));
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

/// The document facts the fixture's own tests state: one artboard and nodes 2–13.
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn v1_copy(name: &str) -> PathBuf {
    let root = scratch(name);
    copy_tree(Path::new(FIXTURE_V1), &root);
    root
}

fn v2_copy(name: &str) -> PathBuf {
    let root = scratch(name);
    copy_tree(Path::new(FIXTURE_V2), &root);
    root
}

fn options() -> SaveOptions {
    SaveOptions { now: NOW.to_string(), document: None }
}

fn findings_of(root: &Path, bundle: &vectorcraft_comic_model::ComicBundle, document: &DocumentFacts) -> Vec<Finding> {
    validate(bundle, &ValidationInput { root, document, archive_root: None })
}

// -------------------------------------------------------------------------------------------------
// The write half: §2, on disk.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_migrated_save_declares_one_version_in_every_versioned_file_on_disk() {
    // §2: "the writer writes the version it implements", on **every** file that declares one (rule
    // 3). Stated against the bytes the save wrote rather than against the v2 fixture, which the same
    // writer generated: with the stamping removed the save writes a v2 manifest over v1 page, story
    // and job files — rule 3's own contradiction, in the bytes it just wrote — and every JSON file
    // on disk is the observable that says so.
    let root = v1_copy("stamp-on-disk");
    let mut loaded = load(&root).expect("the v1 fixture loads");
    save(&mut loaded.bundle, &root, &options()).expect("saves, which migrates v1 → v2");

    let manifest: Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("JSON");
    let declared = manifest["schema_version"].as_u64().expect("the manifest declares a version");
    assert_eq!(declared, u64::from(SCHEMA_VERSION), "the writer writes the version it implements");

    let mut checked = 0usize;
    for (key, bytes) in tree(&root) {
        if !key.ends_with(".json") || key == "manifest.json" {
            continue;
        }
        let file: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("`{key}` is not JSON: {e}"));
        assert_eq!(
            file["schema_version"].as_u64(),
            Some(declared),
            "`{key}` declares a version the manifest does not: the save wrote a contradiction (§2, rule 3)"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 10,
        "every `.json` the bundle owns, the manifest apart, was checked: `project.json`, the five story files, `assets/index.json`, \
         `provenance/generations.json`, the page file and the job file"
    );

    let document = document_facts();
    assert_eq!(findings_of(&root, &loaded.bundle, &document), Vec::new(), "and the bundle it wrote is self-consistent by rule 3");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_failed_write_leaves_the_model_at_the_version_it_read_not_the_one_it_would_have_written() {
    // §10 and `save`'s own doc comment: "the stamping happens on derived copies, and the model is
    // touched only once the write has succeeded". The refusal cases the round-2 test covers all
    // return before the write; this one fails **after** staging, which is the only case that can
    // tell "stamp the model up front" from "stamp the copies and adopt on success" — a plausible
    // implementation that every other test in the suite accepts.
    let root = v1_copy("failed-write-model");
    let mut loaded = load(&root).expect("the v1 fixture loads");
    assert_eq!(loaded.bundle.manifest.schema_version, 1, "read as v1");
    let model_before = loaded.bundle.clone();

    // A regular file where `pages/` belongs: step 4 cannot rename the staged page file into place.
    fs::remove_dir_all(root.join("pages")).expect("remove the pages directory");
    fs::write(root.join("pages"), b"a file where the pages directory belongs").expect("block step 4");
    let result = save(&mut loaded.bundle, &root, &options());
    assert!(result.is_err(), "step 4 cannot replace a directory with a file: {result:?}");

    assert_eq!(
        loaded.bundle, model_before,
        "a save that failed must leave the model exactly as the load left it — versions included, or a caller that retries \
         validates a model that does not describe the bytes on disk"
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// The migration's own effect: the hand-edit, through a save.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_v1_bundle_that_declares_locked_keeps_it_through_a_migrated_save() {
    // The hand-edit branch is tested at the **load**; the value has to survive the save as well, or
    // a lock a foreign tool wrote would be reported and then silently dropped by the first save.
    let root = v1_copy("hand-edit-lock-save");
    let page_path = root.join("pages/page-8.json");
    let page = fs::read_to_string(&page_path).expect("readable");
    let edited = page.replacen("\"generation_status\": \"none\"", "\"generation_status\": \"none\",\n      \"locked\": true", 1);
    assert_ne!(page, edited, "the edit must land");
    fs::write(&page_path, edited).expect("write");

    let mut loaded = load(&root).expect("a hand-edited but parseable page file loads (§14)");
    assert_eq!(loaded.bundle.pages[&PageId(8)].panels.iter().filter(|p| p.locked).count(), 1, "the declared value is preserved");
    save(&mut loaded.bundle, &root, &options()).expect("saves");

    let written = fs::read_to_string(&page_path).expect("readable");
    assert_eq!(written.matches("\"locked\": true").count(), 1, "the declared lock is written back:\n{written}");
    assert_eq!(written.matches("\"locked\": false").count(), 2, "and the two it did not apply to stay at the default");
    let file: Value = serde_json::from_str(&written).expect("JSON");
    assert_eq!(file["schema_version"].as_u64(), Some(u64::from(SCHEMA_VERSION)), "and the file is at the version the writer writes");

    let reloaded = load(&root).expect("reloads");
    assert!(reloaded.migrations.is_empty(), "the save wrote a current bundle: {:?}", reloaded.migrations);
    assert_eq!(reloaded.bundle.pages[&PageId(8)].panels.iter().filter(|p| p.locked).count(), 1, "and the lock is still set");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_migrations_count_is_the_panels_that_took_the_default_and_nothing_else() {
    // The note's arithmetic, both ways. With every panel declaring a lock, **no** panel took the
    // default, and the count says so — a report that counted panels rather than *decisions* would
    // read `3 panels take the default` on a bundle where none did.
    let root = v1_copy("all-locked-count");
    let page_path = root.join("pages/page-8.json");
    let page = fs::read_to_string(&page_path).expect("readable");
    let edited = page.replace("\"generation_status\": \"none\"", "\"generation_status\": \"none\",\n      \"locked\": true");
    assert_ne!(page, edited, "the edit must land");
    fs::write(&page_path, &edited).expect("write");
    // The fixture has two `none` panels and one `completed`; lock the third as well.
    let edited = edited.replace("\"generation_status\": \"completed\"", "\"generation_status\": \"completed\",\n      \"locked\": true");
    fs::write(&page_path, edited).expect("write");

    let loaded = load(&root).expect("loads");
    assert_eq!(loaded.bundle.pages[&PageId(8)].panels.iter().filter(|p| p.locked).count(), 3, "all three declare a lock");
    assert_eq!(
        loaded.migrations[0].notes,
        vec![
            "v2 adds `Panel.locked` (default `false`); 0 panels take the default".to_string(),
            "`pages/page-8.json`: Panel(9) declares `locked: true`, which no v1 writer could have written".to_string(),
            "`pages/page-8.json`: Panel(10) declares `locked: true`, which no v1 writer could have written".to_string(),
            "`pages/page-8.json`: Panel(11) declares `locked: true`, which no v1 writer could have written".to_string(),
        ],
        "the count and the per-panel notes are both exact: {:?}",
        loaded.migrations[0].notes
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// "No new referential rule" — tested, not argued.
// -------------------------------------------------------------------------------------------------

#[test]
fn locking_changes_no_rule_verdict_and_a_broken_panel_is_broken_locked_or_not() {
    // §13's eighteen rules are the deterministic consequences of a reference, an id, a declared
    // agreement, a path or a lifecycle (§6.2). Locking is none of those, so the *bundle* verdict must
    // be identical with every panel locked and with none — and a locked panel whose frame node is
    // missing must be reported exactly as an unlocked one is, by the rule that already governs it.
    let root = v2_copy("lock-verdict");
    let mut loaded = load(&root).expect("the v2 fixture loads");
    let clean = document_facts();
    assert_eq!(findings_of(&root, &loaded.bundle, &clean), Vec::new(), "the canonical fixture is clean");

    for panel in &mut loaded.bundle.pages.get_mut(&PageId(8)).expect("page 8").panels {
        panel.locked = true;
    }
    assert_eq!(
        findings_of(&root, &loaded.bundle, &clean),
        Vec::new(),
        "a bundle with every panel locked is as valid as one with none — no rule was added for `locked`"
    );

    // A frame node that is not in the document: nodes 2–13 today, so drop panel 9's frame node (2).
    let mut broken = document_facts();
    broken.nodes.retain(|id| *id != NodeId(2));
    let locked_findings = findings_of(&root, &loaded.bundle, &broken);
    assert!(
        locked_findings.iter().any(|f| f.subject == "Panel(9).frame_node_id"),
        "a missing frame node is still rule 7's, whatever the lock says: {locked_findings:?}"
    );

    for panel in &mut loaded.bundle.pages.get_mut(&PageId(8)).expect("page 8").panels {
        panel.locked = false;
    }
    assert_eq!(
        findings_of(&root, &loaded.bundle, &broken),
        locked_findings,
        "and the verdict is byte-for-byte the same unlocked: the lock is not a rule input"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_v2_manifest_over_a_v1_page_file_is_rule_3s_finding_not_a_refusal() {
    // The direction a *wrong writer* produces, and the converse of the "a v1 bundle declares v2's
    // key" case: the manifest is current, the page file is not. §2 refuses a **greater** version and
    // reads a lesser one forward, so this loads — and rule 3 names the file rather than letting the
    // disagreement pass. Nothing here is a defect; it is the boundary the asymmetry has to be pinned
    // at, because it is what a save that forgot to stamp would produce.
    let root = v2_copy("v2-manifest-v1-page");
    let page_path = root.join("pages/page-8.json");
    let page = fs::read_to_string(&page_path).expect("readable");
    fs::write(&page_path, page.replace("\"schema_version\": 2", "\"schema_version\": 1")).expect("write");

    let loaded = load(&root).expect("a lesser file version is read forward, not refused (§2)");
    assert!(loaded.migrations.is_empty(), "the manifest is current, so there is no migration to report: {:?}", loaded.migrations);

    let findings = findings_of(&root, &loaded.bundle, &document_facts());
    let rule_3: Vec<&Finding> = findings.iter().filter(|f| f.rule == Some(3)).collect();
    assert_eq!(rule_3.len(), 1, "rule 3 names the one file that disagrees: {findings:?}");
    assert_eq!(rule_3[0].subject, "pages/page-8.json");
    assert!(rule_3[0].is_error(), "the disagreement is an error, not a warning");
    fs::remove_dir_all(&root).ok();
}
