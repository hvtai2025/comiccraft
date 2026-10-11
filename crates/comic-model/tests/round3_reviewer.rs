//! Reviewer tests (`code-reviewer`, M1 **round 3**) for `crates/comic-model`.
//!
//! Round 2 reported 5 findings; the owner fixed all five (R2-F1 a stray `pages/` file must not be a
//! lockout, R2-F2 a save must not delete a declared file it does not own, R2-F3 the one-way timestamp
//! refusal documented and exposed, R2-F4 the fix's own branches pinned, R2-F5 the layer ban made to
//! bite) and closed four mutation gaps round 2 listed beyond the brief (M2b, M5b, M6, M7).
//!
//! This file is the round-3 attack on what is **new in the fix**, not a third copy of the first two:
//!
//! - The **role split** the lead asked for: identity files (`manifest.json`, `project.json`) stay
//!   fatal when absent; collection files load as an empty collection plus a finding. Is every file
//!   classified right, is a *corrupt* collection file still fatal (the other half of the split), and
//!   is every consequence of an absent collection governed elsewhere?
//! - The **open question** the implementer flagged rather than guessed: an absent *declared*
//!   collection file now yields two findings on one subject. These tests pin the answer — two, and
//!   why one is not enough — so a future merge of them cannot happen silently.
//! - The paths the round-2 fixes introduced (`jobs/` strays, the empty collection's severity and
//!   version) that their own tests did not walk.
//!
//! A test whose name starts with `finding_` encodes behaviour the frozen schema requires and is the
//! reproduction for a finding. Everything else is a positive test of a claim the fix makes.

// `clippy.toml` allows panics in tests, but clippy only applies that to `#[cfg(test)]` modules,
// not to an integration-test crate. This is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use vectorcraft_comic_model::{
    ArtboardFacts, ComicBundle, DocumentFacts, Error, Finding, JobId, NodeId, SaveOptions, Severity, ValidationInput, load, save, sha256_hex,
    validate,
};
use vectorcraft_geom::Rect;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";

/// The seven files §14's role split calls **collections**: absent, they become an empty collection.
/// They are also exactly the seven files this build stages from the model on every save.
const COLLECTION_FILES: [&str; 7] = [
    "story/chapters.json",
    "story/scenes.json",
    "story/characters.json",
    "story/locations.json",
    "story/styles.json",
    "assets/index.json",
    "provenance/generations.json",
];

fn scratch(name: &str) -> PathBuf {
    // The pid keeps two *concurrent* runs of this package (this workspace has several agents in it)
    // from clobbering each other's `target/tmp/<name>`; the suite's fixed scratch names made a
    // simultaneous run of one test binary fail spuriously (see the round-3 report).
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
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

/// The fixture's own copy, in a scratch directory, unloaded — so a test can damage it first.
fn fixture_copy(name: &str) -> PathBuf {
    let root = scratch(name);
    copy_tree(Path::new(FIXTURE), &root);
    root
}

/// What rules 4, 7 and 15 need. The fixture's document holds artboard 1 (612×792 pt) and nodes 2–13.
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn options() -> SaveOptions {
    SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None }
}

fn validate_at(root: &Path, bundle: &ComicBundle) -> Vec<Finding> {
    validate(bundle, &ValidationInput { root, document: &document_facts(), archive_root: None })
}

fn subjects(findings: &[Finding], rule: u8) -> Vec<String> {
    findings.iter().filter(|f| f.rule == Some(rule)).map(|f| f.subject.clone()).collect()
}

/// Every finding whose subject is exactly `key`.
fn on<'a>(findings: &'a [Finding], key: &str) -> Vec<&'a Finding> {
    findings.iter().filter(|f| f.subject == key).collect()
}

/// Rewrite `manifest.json` in place, as a hand edit or a newer ComicCraft would.
fn rewrite_manifest(root: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = root.join("manifest.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("readable")).expect("the manifest is JSON");
    edit(&mut manifest);
    fs::write(&path, serde_json::to_vec_pretty(&manifest).expect("serializes")).expect("writable");
}

#[test]
fn an_absent_declared_collection_file_is_reported_by_both_halves_and_they_say_different_things() {
    // The open question the implementer flagged. **Two findings, on purpose**, and this test pins
    // both halves so that neither can be dropped silently:
    //
    //   * the §14 step 1 half — `manifest.files` and the tree disagree, which is the same condition
    //     the interrupted-save window produces for *any* declared file, collection or not;
    //   * the step 3 half — what the project actually opened as (an empty collection), which is the
    //     only signal for an absent *undeclared* collection file (see the test below).
    //
    // They must not be one message twice: a caller has to be able to tell "the consistency record is
    // stale" from "this part of the project is empty now and you can repair it".
    let root = fixture_copy("r3-absent-declared-locations");
    fs::remove_file(root.join("story/locations.json")).expect("remove the collection");
    let loaded = load(&root).expect("§14: an absent collection file is a finding, not a refusal");

    let both = on(&loaded.findings, "story/locations.json");
    assert_eq!(both.len(), 2, "the tree half and the model half, one each: {both:?}");
    let tree_half = both.iter().find(|f| f.message.contains("lists a file that is not in the bundle")).expect("§14 step 1's half");
    let model_half = both.iter().find(|f| f.message.contains("empty collection")).expect("the model's half");
    assert_ne!(tree_half.message, model_half.message, "two findings for one file must not be one sentence written twice: {both:?}");
    assert!(model_half.message.contains("opens and can be repaired"), "the model's half names the consequence: {model_half:?}");
    for finding in both {
        assert_eq!(finding.severity, Severity::Warning, "§14 makes the whole condition one non-fatal report: {finding:?}");
        assert_eq!(finding.rule, None, "it is a §14 load observation, not one of the eighteen: {finding:?}");
    }

    // What the model opened as, and where the file was: the vacant collection is empty and named.
    assert!(loaded.bundle.locations.locations.is_empty());
    assert!(!loaded.files.contains_key("story/locations.json"), "`files` describes the tree, and the file is not in it");
    assert!(loaded.bundle.scenes.scenes[0].location_id.is_some(), "the scene still points at the location it had");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_absent_undeclared_collection_file_is_reported_once_and_only_by_the_model_half() {
    // The other side of the open question, and the reason one finding is not enough: when the
    // `manifest.files` entry is gone too — a hand-damaged bundle, or a newer version's manifest —
    // there is **no** tree half, so the model's half is the only thing that keeps the absence from
    // being silent. Merging the two into one finding would have to pick a side, and either choice
    // leaves one of these two bundles saying nothing.
    let root = fixture_copy("r3-absent-undeclared-styles");
    fs::remove_file(root.join("story/styles.json")).expect("remove the collection");
    rewrite_manifest(&root, |manifest| {
        manifest["files"].as_object_mut().expect("`files` is an object").remove("story/styles.json");
    });
    let loaded = load(&root).expect("§14: an absent collection file is a finding, not a refusal");

    let reported = on(&loaded.findings, "story/styles.json");
    assert_eq!(reported.len(), 1, "with no `files` entry there is no tree half to report: {reported:?}");
    assert!(reported[0].message.contains("empty collection"), "the model's half is what keeps it non-silent: {reported:?}");
    assert_eq!(reported[0].severity, Severity::Warning, "{reported:?}");
    assert!(loaded.bundle.styles.styles.is_empty(), "and the model really did open without it");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_corrupt_collection_file_is_fatal_and_is_not_loaded_as_an_empty_one() {
    // The **other half of the split**, and the one a "be tolerant of missing collection files" change
    // can quietly take with it: §14 step 3 makes a comic file that will not parse fatal, and it must
    // stay fatal. Treating a corrupt file as an absent one would be worse than a lockout — the load
    // would report it as *not in the bundle*, and the next save would overwrite the readable parts of
    // the damaged file with an empty collection, so the creator's only copy of recoverable content
    // would be gone. The line the code draws is exactly the right one and this pins it: *absent* is a
    // finding, *there but unreadable* is an error.
    let root = fixture_copy("r3-corrupt-collection");
    let path = root.join("story/locations.json");
    // A file a crash or a half-finished edit leaves: valid prefix, truncated body.
    let corrupt = b"{\n  \"schema_version\": 1,\n  \"locations\": [\n    { \"id\": 6, \"name\": \"Rooftop\"\n".to_vec();
    fs::write(&path, &corrupt).expect("write the damaged file");

    match load(&root) {
        Err(Error::Parse { path: reported, what, .. }) => {
            assert_eq!(reported, path, "the refusal names the file that would not parse");
            assert_eq!(what, "locations file", "and what it was expecting to hold");
        }
        other => panic!("§14 step 3: a comic file that will not parse is fatal, got {other:?}"),
    }
    assert_eq!(fs::read(&path).expect("readable"), corrupt, "a refused load repairs nothing, and must not have rewritten it");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn every_collection_file_absent_at_once_still_opens_and_no_consequence_is_silent() {
    // The extreme edge of the split: all seven collections gone. The bundle must open (or §14's
    // "a creator can open a damaged bundle and repair it" is a promise the *worst* case breaks), the
    // model must be seven empty collections rather than invented content, and every reference that
    // pointed into one of them must still be governed — by rule 1 or rule 12, not by nothing.
    let root = fixture_copy("r3-all-collections-absent");
    for key in COLLECTION_FILES {
        fs::remove_file(root.join(key)).unwrap_or_else(|e| panic!("remove {key}: {e}"));
    }
    let loaded = load(&root).expect("§14: seven absent collection files are findings, not a refusal");

    // Every one is declared, so every one is reported twice; an absent collection is never silent.
    for key in COLLECTION_FILES {
        let reported = on(&loaded.findings, key);
        assert_eq!(reported.len(), 2, "{key} is declared and gone, so both halves report it: {reported:?}");
        assert!(reported.iter().any(|f| f.message.contains("empty collection")), "{key}: {reported:?}");
    }
    assert!(
        loaded.bundle.chapters.chapters.is_empty()
            && loaded.bundle.scenes.scenes.is_empty()
            && loaded.bundle.characters.characters.is_empty()
            && loaded.bundle.locations.locations.is_empty()
            && loaded.bundle.styles.styles.is_empty()
            && loaded.bundle.assets.assets.is_empty()
            && loaded.bundle.provenance.jobs.is_empty()
            && loaded.bundle.provenance.decisions.is_empty(),
        "every absent collection loaded empty"
    );
    // An absent file cannot disagree with the manifest's version, so the empty collection takes it
    // and rule 3 has nothing to say. (At v1 this is not observable — no supported bundle declares a
    // version other than 1 — so it is pinned as "rule 3 stays silent", which is the property that
    // matters; see the round-3 report's note on why the choice is right rather than a hiding place.)
    assert_eq!(loaded.bundle.locations.schema_version, loaded.bundle.manifest.schema_version);
    assert_eq!(loaded.bundle.assets.schema_version, loaded.bundle.manifest.schema_version);
    assert_eq!(loaded.bundle.provenance.schema_version, loaded.bundle.manifest.schema_version);

    // The pages and jobs that remain still load — the split must not take a *present* file with it.
    assert_eq!(loaded.bundle.pages.len(), 1);
    assert!(loaded.bundle.jobs.contains_key(&JobId(18)));

    let findings = validate_at(&root, &loaded.bundle);
    let rule_1 = subjects(&findings, 1);
    for expected in [
        // A reference from `project.json` into a story file that is now empty.
        "ComicProject(1).chapter_ids",
        "ComicProject(1).character_ids",
        "ComicProject(1).location_ids",
        "ComicProject(1).style_profile_ids",
        // From the surviving page file into the emptied story files.
        "Page(8).chapter_id",
        "Page(8).scene_ids",
        "Panel(9).scene_id",
        "Panel(11).scene_id",
        // From the surviving page file and job file into the emptied asset registry.
        "Panel(9).artwork_refs[].asset_id",
        "GenerationJob(18).input_asset_ids/result_asset_ids",
    ] {
        assert!(rule_1.contains(&expected.to_string()), "`{expected}` dangles into an empty collection and must be rule 1's: {rule_1:?}");
    }
    // `Dialogue.speaker_character_id` is rule 12's, not rule 1's, even when the characters file is gone.
    assert!(subjects(&findings, 12).contains(&"Dialogue(12).speaker_character_id".to_string()), "{findings:?}");
    // A page no scene reaches is rule 14's warning — the emptied scenes file does not make it silent.
    assert!(subjects(&findings, 14).contains(&"pages/page-8.json".to_string()), "{findings:?}");
    // And nothing reports the *absence itself* as a rule-3 version disagreement.
    assert!(subjects(&findings, 3).is_empty(), "a file that is not there cannot disagree with the manifest's version: {findings:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_stray_job_file_is_a_finding_not_a_lockout_and_a_save_leaves_it_where_it_is() {
    // R2-F1's fix has two branches — `pages/` and `jobs/` — and the owner's test walks only the
    // `pages/` one. `jobs/` is the branch §8.3 makes live: a job file is written while the app runs,
    // so a person or a half-finished 0.9 could drop a file there. It must be reported without a
    // numbered rule (no §13 rule governs a `jobs/` file's name), the load must continue, and a save
    // must neither delete it nor drop its entry.
    let root = fixture_copy("r3-stray-job-file");
    fs::write(root.join("jobs/notes.json"), b"{}\n").expect("a stray file");
    let mut loaded = load(&root).expect("§14: only a document or a comic file that will not parse is fatal");

    let stray = on(&loaded.findings, "jobs/notes.json");
    assert_eq!(stray.len(), 2, "the not-listed half and the stray half: {stray:?}");
    let stray_half = stray.iter().find(|f| f.message.contains("is not `job-<JobId>.json`")).expect("the stray half");
    assert_eq!(stray_half.rule, None, "no numbered rule governs a `jobs/` file's name: {stray_half:?}");
    assert!(stray_half.message.contains("the file is left where it is"), "and it says what it did about it: {stray_half:?}");
    // The other half is §14 step 1's, and it says something else: the file is not in the consistency
    // record either. Two findings on one subject is the *normal* shape here, not an accident — the
    // same two-question split the absent-collection case has.
    assert!(stray.iter().any(|f| f.message.contains("does not list it")), "{stray:?}");
    assert!(loaded.bundle.jobs.contains_key(&JobId(18)), "the fixture's real job still loads");

    save(&mut loaded.bundle, &root, &options()).expect("the bundle saves");
    assert!(root.join("jobs/notes.json").is_file(), "a save must leave a file this version does not own where it is");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("JSON");
    assert!(manifest["files"].get("jobs/notes.json").is_some(), "and it stays declared, because §4's `files` covers every file");

    // After the save the tree half is satisfied — the file is listed now — so the only finding left
    // for it is the stray half, which is still true: the name still encodes no id.
    let reloaded = load(&root).expect("loads back");
    let after = on(&reloaded.findings, "jobs/notes.json");
    assert!(after.iter().all(|f| !f.message.contains("lists a file that is not in the bundle")), "{after:?}");
    assert!(after.iter().any(|f| f.message.contains("is not `job-<JobId>.json`")), "{after:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_asset_registry_leaves_every_dangling_asset_reference_to_rule_1() {
    // The `load` doc comment claims every consequence of an absent collection is governed elsewhere
    // and names rule 9 for "a missing asset". With the registry *gone* there is no asset left to
    // resolve a path for, so rule 9 has nothing to say — rule 1 is what actually governs it, through
    // every reference that named an asset. Which rule does it is worth pinning, because "rule 9
    // covers it" is the reading a reader would take from the comment and it is the wrong one.
    let root = fixture_copy("r3-missing-asset-registry");
    fs::remove_file(root.join("assets/index.json")).expect("remove the registry");
    let loaded = load(&root).expect("§14: an absent collection file is a finding, not a refusal");
    assert!(loaded.bundle.assets.assets.is_empty());

    let findings = validate_at(&root, &loaded.bundle);
    let rule_1 = subjects(&findings, 1);
    assert!(rule_1.contains(&"Panel(9).artwork_refs[].asset_id".to_string()), "{rule_1:?}");
    assert!(rule_1.contains(&"GenerationJob(18).input_asset_ids/result_asset_ids".to_string()), "{rule_1:?}");
    assert!(subjects(&findings, 9).is_empty(), "nothing is registered, so rule 9 has no declared base to resolve: {findings:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_directory_where_a_collection_file_belongs_is_fatal_and_not_an_empty_collection() {
    // The line the split draws, at exactly the place a sloppy implementation would blur it:
    // `read_collection` answers `None` **only** for `NotFound`. A directory sitting where a
    // collection file belongs is content we cannot read, not content that is not there — and calling
    // it "absent" would be a lie the next save would inherit, since a rename cannot put a file where
    // a directory is. Fatal is the honest answer, and this pins it so a future "be tolerant of
    // missing collection files" change cannot quietly swallow it.
    let root = fixture_copy("r3-collection-is-a-directory");
    let path = root.join("story/locations.json");
    fs::remove_file(&path).expect("remove the file");
    fs::create_dir(&path).expect("a directory where the file was");

    match load(&root) {
        Err(Error::Read { path: reported, .. }) => assert_eq!(reported, path, "the refusal names the thing that cannot be read"),
        other => panic!("a directory is not an absent collection file, so it is not a finding: {other:?}"),
    }
    assert!(path.is_dir(), "and a refused load repairs nothing");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_asset_files_a_save_still_writes_are_unaffected_by_an_absent_registry() {
    // A save after an absent registry must not delete the artwork: `assets/` is content-addressed and
    // is left alone even when nothing references it, which is the one directory the prune never
    // touches. The asset's bytes are the creator's work; losing them is the failure mode a "tolerant
    // load" makes possible if the prune is ever pointed at `assets/`.
    let root = fixture_copy("r3-asset-bytes-survive");
    let asset = root.join("assets/panels/panel-1.png");
    let before = fs::read(&asset).expect("readable");
    assert_eq!(sha256_hex(&before), "c54af4e9162be74adff0711701a4f1f656dc6a49f8c6acd9f9120aef38476081", "the fixture's own hash");

    fs::remove_file(root.join("assets/index.json")).expect("remove the registry");
    let mut loaded = load(&root).expect("the bundle opens with an empty registry");
    save(&mut loaded.bundle, &root, &options()).expect("the bundle saves");

    assert_eq!(fs::read(&asset).expect("readable"), before, "an unreferenced asset is still an asset: its bytes must not change");
    assert!(root.join("assets/index.json").is_file(), "the registry is re-written empty, which is what the model holds");
    fs::remove_dir_all(&root).ok();
}
