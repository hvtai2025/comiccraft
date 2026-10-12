//! The v1 fixture: the migration anchor for the whole M1 surface (schema §11.1).
//!
//! `tests/fixtures/v1/minimal.comiccraft/` is a frozen bundle — 1 chapter, 1 scene, 1 page,
//! 3 panels, 2 speech balloons, 1 caption, 1 linked asset and 1 job — and it is **byte-frozen**:
//! the v2 bump (schema §6.2, `Panel.locked`) did not touch one byte of it. §11.1 requires a fixture
//! for every previous version, and a schema bump that rewrote history would leave no way to test
//! the migration at all.
//!
//! What this file checks about it under **v2 code**:
//!
//! - it loads, its ids are unchanged, its unknown fields survive, and it validates clean;
//! - it is migrated forward: every panel takes `locked: false`, and the load **reports** that
//!   migration rather than doing it in the dark (§11);
//! - a **load** writes nothing, so the frozen bytes on disk are exactly what M1 froze;
//! - saving it produces `tests/fixtures/v2/minimal.comiccraft/` **byte for byte**, which is what
//!   makes the v2 fixture a migrated v1 rather than a second hand-written copy;
//! - a newer `schema_version` is refused, naming the version it needs (§2).
//!
//! The byte-identical round trip §11.1 requires now belongs to the fixture at the version this
//! crate writes, and that is `tests/fixture_v2.rs`.
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
/// The same fixture read forward: the canonical form this build writes (§5.7, §11.1).
const FIXTURE_V2: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v2/minimal.comiccraft");

/// The fixture's `manifest.updated`, which a save writes back, so the manifest's bytes do not move.
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";

fn fixture_root() -> PathBuf {
    PathBuf::from(FIXTURE)
}

fn fixture_v2_root() -> PathBuf {
    PathBuf::from(FIXTURE_V2)
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

    // Read forward, not rewritten: the model is v2's shape, the bytes on disk are still v1's.
    assert_eq!(loaded.bundle.manifest.schema_version, 1, "a load does not rewrite the version it read");
    assert_eq!(loaded.migrations.len(), 1, "the migration is reported rather than done in the dark (§11)");
    assert_eq!((loaded.migrations[0].from, loaded.migrations[0].to), (1, 2));
    assert!(page.panels.iter().all(|panel| !panel.locked), "every v1 panel takes v2's default (§6.2)");
}

#[test]
fn the_fixture_is_migrated_by_the_load_and_the_report_says_what_it_did() {
    // §11: "Report incompatible data as a visible, actionable error" and, for a migration that
    // succeeds, report what it did rather than acting silently. The report is a value on
    // `LoadedBundle`, so a caller that never reads it has still been told.
    let loaded = load(&fixture_root()).expect("the v1 fixture loads");
    let migration = &loaded.migrations[0];
    assert_eq!(migration.from, 1);
    assert_eq!(migration.to, vectorcraft_comic_model::SCHEMA_VERSION);
    assert!(
        migration.notes.iter().any(|note| note.contains("Panel.locked") && note.contains("3 panels")),
        "the report names the field v2 added and how many panels took it: {:?}",
        migration.notes
    );
    // The `Display` form is what a caller logs; it must carry the whole report, not just the pair.
    let rendered = migration.to_string();
    assert!(rendered.contains("1") && rendered.contains("2") && rendered.contains("Panel.locked"), "{rendered}");

    // The same project the v2 fixture holds: a migration preserves every stable id (§11), so the
    // two fixtures must agree on all of them, and `retired_artboard_ids` is not an exception.
    let read_forward = load(&fixture_v2_root()).expect("the v2 fixture loads");
    assert_eq!(read_forward.bundle.highest_comic_id(), loaded.bundle.highest_comic_id());
    assert_eq!(read_forward.bundle.next_comic_id(), loaded.bundle.next_comic_id());
    assert_eq!(read_forward.bundle.manifest.bundle_id, loaded.bundle.manifest.bundle_id);
    assert_eq!(read_forward.bundle.manifest.retired_artboard_ids, loaded.bundle.manifest.retired_artboard_ids);
    assert_eq!(read_forward.bundle.pages.keys().collect::<Vec<_>>(), loaded.bundle.pages.keys().collect::<Vec<_>>(), "the page ids survive the bump");
}

#[test]
fn a_v1_bundle_that_declares_locked_is_reported_rather_than_honoured_in_silence() {
    // The one input v2's field default would silently *accept*. A v1 writer had no `locked` key, so
    // a v1 bundle carrying one is a hand-edit (or a foreign tool); §11 says validate the input and
    // refuse rather than guess, and §14 says refuse nothing short of an unparseable file — so the
    // value is **preserved** and the migration says so. Silence here would be a lock nobody set
    // appearing in a project, or a lock the creator set disappearing.
    let root = scratch("fixture-v1-hand-edited-lock");
    copy_tree(&fixture_root(), &root);
    let page_path = root.join("pages/page-8.json");
    let page = fs::read_to_string(&page_path).expect("readable");
    let edited = page.replacen("\"generation_status\": \"none\"", "\"generation_status\": \"none\",\n      \"locked\": true", 1);
    assert_ne!(page, edited, "the edit must land");
    fs::write(&page_path, edited).expect("write");

    let loaded = load(&root).expect("a hand-edited but parseable page file loads (§14)");
    let panels = &loaded.bundle.pages.get(&PageId(8)).expect("page 8").panels;
    assert!(panels.iter().any(|panel| panel.locked), "the declared value is preserved, not dropped");
    assert_eq!(panels.iter().filter(|panel| panel.locked).count(), 1);

    let migration = &loaded.migrations[0];
    assert!(
        migration.notes.iter().any(|note| note.contains("Panel(9)") && note.contains("locked")),
        "the migration reports the key a v1 writer could not have written: {:?}",
        migration.notes
    );
    assert!(migration.notes.iter().any(|note| note.contains("2 panels take")), "and counts the rest: {:?}", migration.notes);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_save_leaves_the_in_memory_model_at_the_version_it_wrote() {
    // §2: "the writer writes the version it implements", on every file that declares one (rule 3).
    // The bytes are checked by the byte-for-byte test above; this is the other half, and the one a
    // command layer feels — `comic.project.save` followed by `comic.project.validate` runs on the
    // model it just saved, so a v2 manifest left over v1 page files in memory would report rule 3 on
    // a project the same call had just written correctly.
    let root = scratch("fixture-save-adopts-version");
    copy_tree(&fixture_root(), &root);
    let mut loaded = load(&root).expect("loads");
    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    save(&mut loaded.bundle, &root, &options).expect("saves");

    for (file, version) in loaded.bundle.declared_versions() {
        assert_eq!(version, vectorcraft_comic_model::SCHEMA_VERSION, "`{file}` still declares what it was read at");
    }
    let document = document_facts();
    let findings = validate(&loaded.bundle, &ValidationInput { root: &root, document: &document, archive_root: None });
    assert_eq!(findings, Vec::new(), "the model it just wrote must be consistent with itself: {findings:?}");
    fs::remove_dir_all(&root).ok();
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
fn a_load_of_the_v1_fixture_writes_nothing() {
    // The fixture is the one artifact that must not move: M1 froze it, and a schema bump that
    // rewrote it would leave nothing to test the migration against (§11.1). A load reports; it
    // does not save. This compares the source tree with itself around a load in a copy of it.
    let before = tree(&fixture_root());
    let root = scratch("fixture-load-writes-nothing");
    copy_tree(&fixture_root(), &root);
    let _loaded = load(&root).expect("the copy loads");
    assert_eq!(tree(&root), before, "a load leaves the bundle's bytes exactly as it found them");
    assert_eq!(tree(&fixture_root()), before, "and the checked-in fixture is never written to");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn saving_the_v1_fixture_produces_the_v2_fixture_byte_for_byte() {
    // The migration, proved by bytes rather than by assertion: the v2 fixture **is** this v1
    // fixture read forward, so the two fixtures cannot drift apart, and the v1 → v2 step is
    // exercised by every run of this test. §11.1's byte-identical round trip belongs to the
    // fixture at the version this crate writes, which is `tests/fixture_v2.rs`.
    let root = scratch("fixture-migrate-to-v2");
    copy_tree(&fixture_root(), &root);

    let mut loaded = load(&root).expect("the copy loads");
    let ids_before = (loaded.bundle.highest_comic_id(), loaded.bundle.next_comic_id());
    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };
    save(&mut loaded.bundle, &root, &options).expect("saves");

    assert_eq!((loaded.bundle.highest_comic_id(), loaded.bundle.next_comic_id()), ids_before, "a save preserves every id and the counter");

    let expected = tree(&fixture_v2_root());
    let actual = tree(&root);
    assert_eq!(actual.keys().collect::<Vec<_>>(), expected.keys().collect::<Vec<_>>(), "a save writes exactly the v2 fixture's files");
    for (key, want) in &expected {
        let got = actual.get(key).expect("present");
        assert_eq!(String::from_utf8_lossy(got), String::from_utf8_lossy(want), "`{key}` must match the v2 fixture byte for byte (§5.7)");
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_second_save_of_a_migrated_bundle_is_a_fixed_point() {
    // The strongest form of §11.1: save, reload, save again, and the bytes do not move. A writer
    // whose canonical form was not idempotent would drift here even if it matched once — and a
    // migration that was re-applied on a v2 bundle would show up as exactly this drift.
    let root = scratch("fixture-fixed-point");
    copy_tree(&fixture_root(), &root);
    let options = vectorcraft_comic_model::SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None };

    let mut loaded = load(&root).expect("loads");
    save(&mut loaded.bundle, &root, &options).expect("first save, which migrates to v2");
    let once = tree(&root);
    assert_eq!(once, tree(&fixture_v2_root()), "the first save is the v2 fixture");

    let mut again = load(&root).expect("reloads");
    assert_eq!(again.migrations, Vec::new(), "a v2 bundle is not migrated again");
    save(&mut again.bundle, &root, &options).expect("second save");
    assert_eq!(tree(&root), once, "a save of a loaded bundle changes nothing");
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
    // §2: refuse, naming the required version, and never partially load. The bump to v2 is what
    // makes this testable at all — `SCHEMA_VERSION + 1` is a version no build implements.
    let root = scratch("fixture-newer-version");
    copy_tree(&fixture_root(), &root);
    let newer = vectorcraft_comic_model::SCHEMA_VERSION + 1;
    let manifest = fs::read_to_string(root.join("manifest.json")).expect("readable");
    fs::write(root.join("manifest.json"), manifest.replace("\"schema_version\": 1", &format!("\"schema_version\": {newer}"))).expect("write");
    match load(&root) {
        Err(vectorcraft_comic_model::Error::SchemaTooNew { found, supported }) => {
            assert_eq!((found, supported), (newer, vectorcraft_comic_model::SCHEMA_VERSION));
        }
        other => panic!("a newer bundle must be refused with the version it needs, got {other:?}"),
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_newer_version_declared_by_a_file_is_refused_rather_than_read_partially() {
    // §2 again, one file down: "never partially load" is a claim about the bundle, and a page file
    // that declares a version this build does not implement may mean something else by its keys.
    // Rule 3 governs a file that declares an *older* version (it is read forward); a *newer* one is
    // this refusal.
    let root = scratch("fixture-newer-file-version");
    copy_tree(&fixture_v2_root(), &root);
    let page_path = root.join("pages/page-8.json");
    let page = fs::read_to_string(&page_path).expect("readable");
    fs::write(&page_path, page.replace("\"schema_version\": 2", "\"schema_version\": 3")).expect("write");
    match load(&root) {
        Err(vectorcraft_comic_model::Error::FileSchemaTooNew { path, found, supported }) => {
            assert_eq!(path, page_path, "the refusal names the file that declares it");
            assert_eq!((found, supported), (3, vectorcraft_comic_model::SCHEMA_VERSION));
        }
        other => panic!("a file declaring a newer version must be refused, got {other:?}"),
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
