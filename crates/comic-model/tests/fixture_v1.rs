//! The v1 fixture: the regression anchor for the whole M1 surface (schema §11.1).
//!
//! `tests/fixtures/v1/minimal.comiccraft/` is a frozen bundle — 1 chapter, 1 scene, 1 page,
//! 3 panels, 2 speech balloons, 1 caption, 1 linked asset and 1 job — and §11.1 requires it to
//! **load, validate and round-trip byte-identically after a save, with ids unchanged**.
//!
//! The round trip is the strongest statement this crate can make about §5.7's canonical form: a
//! save is compared byte for byte against the committed fixture, so a change to field order, to
//! number form, to the sorted-ness of a map, or to the trailing newline fails here rather than in
//! a creator's bundle.
//!
//! `clippy.toml` allows panics in tests; an integration-test crate needs the file-level
//! equivalent, because clippy only applies the `allow` to `#[cfg(test)]` modules.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vectorcraft_comic_model::{
    ArtboardFacts, AssetType, DocumentFacts, Finding, JobId, JobStatus, LetteringKind, NodeId, PageId, Severity, StorageRef, ValidationInput, load,
    save, validate,
};
use vectorcraft_geom::Rect;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");

/// The fixture's `manifest.updated`, which a save writes back, so the manifest's bytes do not move.
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";

fn fixture_root() -> PathBuf {
    PathBuf::from(FIXTURE)
}

/// A scratch directory under the target directory, so a test never writes outside the checkout.
///
/// Unique per process as well as per test: two runs can be in flight at once, and a shared path
/// lets one delete the directory the other is using, which reads as a spurious failure.
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

/// What rules 4, 7 and 15 need, read from the fixture's real `.vectorcraft` document.
///
/// The ids are the document's own: `document/comic.vectorcraft` holds one artboard (id 1,
/// 612×792 pt) and nodes 2–13 — three panel frames, two balloon outlines, two tails, three area
/// texts and one placed image. `crates/format` is L3 and this crate is L2, so the fixture's test
/// states them rather than parsing the document; a mismatch would be caught by rule 4 or 7.
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn errors(findings: &[Finding]) -> Vec<&Finding> {
    findings.iter().filter(|f| f.is_error()).collect()
}

#[test]
fn the_fixture_is_the_bundle_the_schema_describes() {
    let loaded = load(&fixture_root()).expect("the v1 fixture loads");
    let bundle = &loaded.bundle;

    assert_eq!(bundle.chapters.chapters.len(), 1);
    assert_eq!(bundle.scenes.scenes.len(), 1);
    assert_eq!(bundle.pages.len(), 1, "one page");
    let page = bundle.pages.get(&PageId(8)).expect("pages/page-8.json holds page 8");
    assert_eq!(page.page.id, PageId(8));
    assert_eq!(page.panels.len(), 3, "three panels");
    assert_eq!(page.dialogue.len(), 2, "two dialogue entries");
    assert_eq!(page.lettering.len(), 3, "two balloons and one caption");

    let balloons = page.lettering.iter().filter(|l| l.kind == LetteringKind::Speech).count();
    let captions = page.lettering.iter().filter(|l| l.kind == LetteringKind::Narration).count();
    assert_eq!((balloons, captions), (2, 1), "2 speech balloons, 1 caption");

    assert_eq!(bundle.assets.assets.len(), 1, "one linked asset");
    assert!(matches!(bundle.assets.assets[0].storage_reference, StorageRef::Linked { .. }), "linked, not embedded");
    assert_eq!(bundle.assets.assets[0].asset_type, AssetType::Source);

    assert_eq!(bundle.provenance.jobs.len(), 1, "one job, recorded in `provenance/`");
    assert_eq!(bundle.jobs.len(), 1, "and its live copy in `jobs/`");
    assert_eq!(bundle.provenance.decisions.len(), 1, "one review decision");

    assert_eq!(loaded.findings, Vec::new(), "a frozen fixture has nothing to report");
    assert!(loaded.bundle.manifest.next_comic_id > loaded.bundle.highest_comic_id(), "the counter is ahead of every id");
}

#[test]
fn the_fixture_validates_with_no_findings() {
    let loaded = load(&fixture_root()).expect("loads");
    let document = document_facts();
    let findings = validate(&loaded.bundle, &ValidationInput { root: &fixture_root(), document: &document, archive_root: None });
    assert_eq!(errors(&findings), Vec::<&Finding>::new(), "{findings:?}");
    assert_eq!(findings, Vec::new(), "the frozen fixture must be clean, warnings included: {findings:?}");
}

#[test]
fn the_fixture_round_trips_byte_identically() {
    let root = scratch("fixture-round-trip");
    copy_tree(&fixture_root(), &root);

    let mut loaded = load(&root).expect("the copy loads");
    let before = loaded.bundle.clone();
    let ids_before = (before.highest_comic_id(), before.next_comic_id());
    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    save(&mut loaded.bundle, &root, &options).expect("saves");

    assert_eq!((loaded.bundle.highest_comic_id(), loaded.bundle.next_comic_id()), ids_before, "a save preserves every id and the counter");

    let expected = tree(&fixture_root());
    let actual = tree(&root);
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "a save writes exactly the fixture's files — no extra, no missing"
    );
    for (key, want) in &expected {
        let got = actual.get(key).expect("present");
        assert_eq!(String::from_utf8_lossy(got), String::from_utf8_lossy(want), "`{key}` must round-trip byte-identically (§5.7, §11.1)");
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_second_save_of_a_reloaded_bundle_is_a_fixed_point() {
    // The strongest form of §11.1: save, reload, save again, and the bytes do not move. A writer
    // whose canonical form was not idempotent would drift here even if it matched once.
    let root = scratch("fixture-fixed-point");
    copy_tree(&fixture_root(), &root);
    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };

    let mut loaded = load(&root).expect("loads");
    save(&mut loaded.bundle, &root, &options).expect("first save");
    let once = tree(&root);

    let mut again = load(&root).expect("reloads");
    save(&mut again.bundle, &root, &options).expect("second save");
    assert_eq!(tree(&root), once, "a save of a loaded bundle changes nothing");
    assert_eq!(tree(&root), tree(&fixture_root()), "and matches the frozen fixture");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_fixture_preserves_the_unknown_field_a_newer_build_wrote() {
    // §12: the fixture's chapter carries two keys no v1 struct declares, written sorted. They must
    // survive the load and the save, or a newer ComicCraft could not read a project an older one
    // saved.
    let root = scratch("fixture-unknown");
    copy_tree(&fixture_root(), &root);
    let mut loaded = load(&root).expect("loads");
    let chapter = &loaded.bundle.chapters.chapters[0];
    assert_eq!(chapter.unknown.get("pov_character").and_then(serde_json::Value::as_u64), Some(5));
    assert_eq!(chapter.unknown.get("revision_note").and_then(serde_json::Value::as_str), Some("the fixture's one unknown field"));

    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    save(&mut loaded.bundle, &root, &options).expect("saves");
    let written = fs::read_to_string(root.join("story/chapters.json")).expect("readable");
    assert!(written.contains("\"pov_character\": 5"), "{written}");
    assert!(written.contains("\"revision_note\""), "{written}");
    assert!(
        written.find("\"pov_character\"").unwrap_or(usize::MAX) < written.find("\"revision_note\"").unwrap_or(0),
        "map keys are written sorted (§5.7)"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_fixture_refuses_a_newer_schema_version() {
    // §2: refuse, naming the required version, and never partially load.
    let root = scratch("fixture-newer-version");
    copy_tree(&fixture_root(), &root);
    let manifest = fs::read_to_string(root.join("manifest.json")).expect("readable");
    fs::write(root.join("manifest.json"), manifest.replace("\"schema_version\": 1", "\"schema_version\": 2")).expect("write");
    match load(&root) {
        Err(vectorcraft_comic_model::Error::SchemaTooNew { found, supported }) => assert_eq!((found, supported), (2, 1)),
        other => panic!("a newer bundle must be refused with the version it needs, got {other:?}"),
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_fixtures_job_is_not_terminal_so_its_file_belongs_in_jobs() {
    // Rule 18 and §8.3: `awaiting_review` is deliberately not terminal, so `jobs/` is not empty in
    // a quiescent project.
    let loaded = load(&fixture_root()).expect("loads");
    let file = loaded.bundle.jobs.get(&JobId(18)).expect("jobs/job-18.json");
    assert_eq!(file.job.status, JobStatus::AwaitingReview);
    assert!(!file.job.status.is_terminal(), "its file legitimately stays");
    assert_eq!(file.job.status, loaded.bundle.provenance.jobs[0].status, "the live copy and the record agree");
}

#[test]
fn the_fixture_is_a_bundle_and_not_a_document() {
    // ADR-013: the visual document stays a plain `.vectorcraft` file with no comic section. This
    // reads the fixture's document as text, because the crate that may parse it is `crates/format`.
    let document = fs::read_to_string(fixture_root().join("document/comic.vectorcraft")).expect("readable");
    assert!(document.starts_with('{'), "it is JSON");
    assert!(document.contains("\"format\": \"vectorcraft\""), "it is a vectorcraft file");
    assert!(document.contains("\"version\": 4"), "at the format version the manifest records");
    for comic_field in ["\"panels\"", "\"dialogue\"", "\"lettering\"", "\"shot_description\"", "\"reading_order\""] {
        assert!(!document.contains(comic_field), "`{comic_field}` must never appear in the visual document (ADR-013, design §6.1)");
    }
}

#[test]
fn every_artifact_reference_in_the_fixture_resolves_to_a_real_file() {
    // The fixture's document and its one linked asset must be there, or the bundle is a sketch
    // rather than an anchor.
    assert!(fixture_root().join("document/comic.vectorcraft").is_file());
    assert!(fixture_root().join("assets/panels/panel-1.png").is_file());
    let index = fs::read_to_string(fixture_root().join("assets/index.json")).expect("readable");
    let hash = vectorcraft_comic_model::sha256_hex(&fs::read(fixture_root().join("assets/panels/panel-1.png")).expect("readable"));
    assert!(index.contains(&hash), "the registry records the file's real sha256");
}

#[test]
fn the_fixtures_severity_vocabulary_is_the_one_the_caller_reports() {
    // A smoke check that `Severity` is usable from outside the crate, which is how the command
    // layer decides whether a validation result blocks a save.
    assert!(Severity::Error > Severity::Warning);
    let _: fn(&[Finding]) -> bool = |findings| findings.iter().any(Finding::is_error);
}
