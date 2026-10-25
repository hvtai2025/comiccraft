//! Round-2 review tests for M2 W3 — the command layer for panel locking — by `code-reviewer`.
//!
//! Round 1 approved W3's revision and left six notes, three of which were **read from the code and
//! not driven**. Round 2 drives the two that can be driven on real files:
//!
//! * **F5** — an undo after a save revives the migration report the save retired, for a bundle whose
//!   manifest is no longer migrating. The shape is M1's `saved_comic` defect one milestone later: an
//!   undo rewinding a record the save path maintains.
//! * **F6** — `comic.project.settings.set` stores a face name verbatim while every reader trims it.
//!
//! It also answers the question F5 implies but does not ask: whether the same rewind reaches
//! `OpenComic::findings`. It does — by the same whole-`OpenComic` snapshot — and the reason that is
//! *not* a second bug is that nothing reads the field, which is itself worth recording: the load's
//! findings are not retired by a save the way `migrations` is, and the field's own doc comment says
//! `comic.project.validate` refreshes it while `validate` in fact recomputes locally and never writes
//! back.
//!
//! Each test prints the sequence it drives, so the record can quote real output rather than assert
//! alone: `cargo test -p vectorcraft-engine --test comic_locking_r2 -- --nocapture`.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;

// ---------- fixtures ----------

/// A scratch directory inside the workspace's `target/` (never a system temporary directory, never a
/// tracked one): the `assets` and `brands` gates must not see a test bundle.
fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-review-m2-w3-r2").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// W1's v1 fixture, which the whole migration path is anchored on.
fn v1_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../comic-model/tests/fixtures/v1/minimal.comiccraft")
}

/// Copy a directory tree: both the fixture and this test's bundles stay read-only to the fixture.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create the destination");
    for entry in std::fs::read_dir(from).expect("read the source").flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy a file");
        }
    }
}

fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    s.execute("comic.project.create", &json!({"path": root.to_string_lossy(), "title": "R2"})).expect("comic.project.create");
    (s, root)
}

fn page_of(s: &mut Session) -> u64 {
    s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id")
}

fn panel_on(s: &mut Session, page: u64, rect: [f64; 4]) -> u64 {
    s.execute("comic.panel.add", &json!({"page": page, "rect": rect})).expect("comic.panel.add")["panel"].as_u64().expect("panel id")
}

fn open_comic(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn validate(s: &mut Session) -> Value {
    s.execute("comic.project.validate", &json!({})).expect("comic.project.validate")
}

/// The number of entries in a report's `migrations` (absent/null counts as none).
fn migrations(v: &Value) -> usize {
    v["migrations"].as_array().map_or(0, Vec::len)
}

/// The load's or the validate's findings, as `(subject, message)` pairs, for a readable paste.
fn findings(v: &Value) -> Vec<(String, String)> {
    v["findings"]
        .as_array()
        .map(|a| a.iter().map(|f| (f["subject"].as_str().unwrap_or("?").to_string(), f["message"].as_str().unwrap_or("?").to_string())).collect())
        .unwrap_or_default()
}

fn manifest_version(root: &Path) -> u64 {
    let bytes = std::fs::read(root.join("manifest.json")).expect("read the manifest");
    serde_json::from_slice::<Value>(&bytes).expect("parse the manifest")["schema_version"].as_u64().expect("a schema_version")
}

fn manifest_sha(root: &Path) -> String {
    vectorcraft_comic_model::sha256_hex(&std::fs::read(root.join("manifest.json")).expect("read the manifest"))
}

/// The family the text object of a lettering carries in the document — the one the *reader* used.
fn text_object_family(s: &Session, text_node: u64) -> String {
    let doc = s.doc().expect("a document").doc.clone();
    match &doc.node(vectorcraft_engine::doc::NodeId(text_node)).expect("the text object").kind {
        vectorcraft_engine::doc::NodeKind::Text(t) => t.runs.first().expect("a run").style.font_family.clone(),
        other => panic!("not a type object: {other:?}"),
    }
}

// ---------- F5: the undo after a save revives the migration report ----------

/// **F5, driven.** Open a v1 bundle (one `1 → 2` migration), make one edit, save (the manifest is v2
/// and the save retires the record), then undo.
///
/// The undo restores a **whole `OpenComic` snapshot** taken before the edit, so `migrations` comes
/// back for a bundle that is not migrating: `comic.project.validate` reports `1 → 2` with the note
/// that ends *"Saving writes schema version 2"* — after the save that already wrote it — while the
/// manifest on disk is byte-for-byte the one the save wrote. A fresh `comic.project.open` of the same
/// directory, in the same session, reports nothing to migrate, so the two reports of one file
/// disagree and the stale one is the one a creator is most likely to ask for.
#[test]
fn r2_an_undo_after_a_save_revives_the_migration_report_the_save_retired() {
    let root = scratch("undo-migration").join("v1-copy.comiccraft");
    copy_tree(&v1_fixture(), &root);
    let mut s = Session::new();

    let opened = s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("open the v1 copy");
    println!("1. open      : manifest v{}, migrations={}, note={}", manifest_version(&root), migrations(&opened), opened["migrationsNote"]);
    assert_eq!(manifest_version(&root), 1, "the copy on disk is v1");
    assert_eq!(migrations(&opened), 1, "the load read it forward");

    let page = page_of(&mut s);
    println!("2. edit      : comic.page.create → page {page}");
    assert_eq!(migrations(&validate(&mut s)), 1, "an edit does not spend the load's migration");

    s.execute("comic.project.save", &json!({})).expect("comic.project.save");
    let saved_sha = manifest_sha(&root);
    let after_save = validate(&mut s);
    println!("3. save      : manifest v{} (sha256 {saved_sha})", manifest_version(&root));
    println!("   validate  : migrations={}, note={}", migrations(&after_save), after_save["migrationsNote"]);
    assert_eq!(manifest_version(&root), 2, "the save wrote this build's version");
    assert_eq!(migrations(&after_save), 0, "and retired the record it made stale");

    s.execute("edit.undo", &json!({})).expect("edit.undo");
    let after_undo = validate(&mut s);
    println!("4. undo      : manifest v{} (sha256 {})", manifest_version(&root), manifest_sha(&root));
    println!("   validate  : migrations={}, note={}", migrations(&after_undo), after_undo["migrationsNote"]);
    assert_eq!(manifest_sha(&root), saved_sha, "the undo does not touch the file");
    assert_eq!(manifest_version(&root), 2, "the file the report is about is v2");
    assert_eq!(migrations(&after_undo), 1, "F5: the migration the save retired is back");
    let note = after_undo["migrationsNote"].as_str().expect("a note");
    assert!(note.contains("was read forward"), "the revived note still claims a read-forward: {note}");
    assert!(note.contains("Saving writes schema version 2"), "and its last sentence describes a save that has already happened: {note}");

    let reopened = s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("reopen the same directory");
    println!("5. reopen    : migrations={}, note={}", migrations(&reopened), reopened["migrationsNote"]);
    assert_eq!(migrations(&reopened), 0, "a fresh load of the same directory reports a current bundle");
    assert!(reopened["migrationsNote"].is_null(), "{reopened}");
}

/// The window F5 opens is **self-healing**: any later save retires the revived record again and the
/// file never moves backwards. This is the severity half of the finding — the report is wrong until
/// the next save, and the next save is the one thing a creator who just undid may never make.
#[test]
fn r2_a_second_save_retires_the_revived_migration_again_and_the_file_never_goes_backwards() {
    let root = scratch("undo-migration-heal").join("v1-copy.comiccraft");
    copy_tree(&v1_fixture(), &root);
    let mut s = Session::new();
    s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("open the v1 copy");
    let page = page_of(&mut s);
    s.execute("comic.panel.add", &json!({"page": page, "rect": [40.0, 40.0, 200.0, 150.0]})).expect("comic.panel.add");
    s.execute("comic.project.save", &json!({})).expect("the first save");
    s.execute("edit.undo", &json!({})).expect("edit.undo");
    assert_eq!(migrations(&validate(&mut s)), 1, "the revived report, as in the test above");
    assert!(s.doc().expect("a document").is_dirty(), "and the state it describes is unsaved work, which is honest: the model did move");

    s.execute("comic.project.save", &json!({})).expect("the second save");
    let healed = validate(&mut s);
    println!(
        "after the second save: manifest v{}, migrations={}, dirty={}",
        manifest_version(&root),
        migrations(&healed),
        s.doc().expect("a document").is_dirty()
    );
    assert_eq!(migrations(&healed), 0, "the second save retires it again");
    assert_eq!(manifest_version(&root), 2, "and the file never moves backwards");
    assert!(!s.doc().expect("a document").is_dirty(), "memory and disk agree again");
}

// ---------- the question F5 implies: does the rewind reach `findings`? ----------

/// **The rewind reaches `findings` too, by the same whole-`OpenComic` snapshot — but there is
/// nothing for it to revive, and that is the interesting half.**
///
/// The sequence: a bundle whose load reports something (§14 lets a damaged bundle open so it can be
/// repaired), then an edit and a save, then the undo.
///
/// `comic.project.save` clears `migrations` and leaves `findings` alone — so the load's findings are
/// *not* a record the save maintains, and the undo has nothing to rewind them to. They are also
/// already stale in a way the save cannot fix: the save writes every collection file, so a finding
/// saying `story/styles.json` is absent becomes false about the bundle on disk while the copy on the
/// session still says it. Nothing surfaces that copy — `comic.project.open` returns the *load's*
/// list and `comic.project.validate` recomputes its own — which is why the identical mechanism is a
/// live false report for `migrations` and a latent one for `findings`. If a later milestone wires
/// `OpenComic::findings` into a report, it inherits F5 with the same fix needed.
#[test]
fn r2_the_rewind_reaches_the_loads_findings_and_nothing_reads_them_so_nothing_is_revived() {
    let (mut s, root) = project("undo-findings");
    let page = page_of(&mut s);
    panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.project.save", &json!({})).expect("save a sound bundle");

    // Damage it the way §14 allows: a *collection* file is absent, so the project still opens and the
    // absence is reported rather than fatal.
    std::fs::remove_file(root.join("story/styles.json")).expect("remove a collection file");
    let opened = s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("a damaged bundle opens");
    let loaded = open_comic(&s).findings.clone();
    println!("load         : {} finding(s): {:?}", findings(&opened).len(), findings(&opened));
    println!("OpenComic     : {} finding(s) held", loaded.len());
    assert!(!loaded.is_empty(), "the load reported the absent file: {opened}");
    assert_eq!(loaded.len(), findings(&opened).len(), "and `OpenComic` holds exactly the load's list");
    assert!(loaded.iter().any(|f| f.subject.contains("story/styles.json")), "the absent file is named: {loaded:?}");

    // One edit, then a save: the save writes *every* collection file, so the statement above is now
    // false about the bundle on disk.
    let page2 = page_of(&mut s);
    println!("edit         : comic.page.create → page {page2}");
    s.execute("comic.project.save", &json!({})).expect("save the repaired bundle");
    assert!(root.join("story/styles.json").is_file(), "the save wrote the file the load said was absent");
    let report = validate(&mut s);
    println!("validate     : {} finding(s): {:?}", findings(&report).len(), findings(&report));
    assert!(
        !findings(&report).iter().any(|(subject, _)| subject.contains("story/styles.json")),
        "the recomputed report does not repeat it: {report}"
    );
    assert_eq!(open_comic(&s).findings, loaded, "and nothing retires the load's copy — the save clears `migrations` and leaves `findings`");

    // The same rewind as F5's, over the same kind of snapshot: the findings come back with it (there
    // was nothing to clear, so this is the mechanism showing, not a second resurrection).
    s.execute("edit.undo", &json!({})).expect("edit.undo");
    println!("after undo   : OpenComic holds {} finding(s), the load's are {}", open_comic(&s).findings.len(), loaded.len());
    assert_eq!(open_comic(&s).findings, loaded, "the rewind restores the same snapshot verbatim");
    // And the report a caller can actually read is recomputed, so it is never the stale copy.
    assert!(
        !findings(&validate(&mut s)).iter().any(|(subject, _)| subject.contains("story/styles.json")),
        "the only surface that reports findings is the one that recomputes them"
    );
}

// ---------- F6: the writer stores, the reader trims ----------

/// **F6, driven at both ends.** `comic.project.settings.set` accepts any non-blank string and stores
/// it verbatim (`project.rs:546`, the `s.clone()` arm of `optional_name`), while the reader trims
/// (`lettering.rs:166`, `project_face`). So one setting has two spellings in one project: the one
/// `settings.get` answers with and the one every applied record carries.
#[test]
fn r2_settings_set_stores_a_face_name_verbatim_while_every_reader_trims_it() {
    let (mut s, _root) = project("settings-trim");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": "  Comic Neue  "})).expect("set a padded face name");
    let got = s.execute("comic.project.settings.get", &json!({})).expect("comic.project.settings.get");
    println!("settings.get : {}", got["default_lettering_style"]);
    assert_eq!(got["default_lettering_style"].as_str(), Some("  Comic Neue  "), "the writer kept the padding");

    let added = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "narration", "text": "caption"}))
        .expect("comic.lettering.add");
    let text_node = added["text_node"].as_u64().expect("a text node");
    let applied = text_object_family(&s, text_node);
    println!("applied      : the text object's family is {applied:?}");
    println!("plan         : text_layout={}", added["text_layout"]);
    assert_eq!(applied, "Comic Neue", "the reader trimmed the name it applied");
    let record_font = open_comic(&s)
        .bundle
        .pages
        .values()
        .flat_map(|f| f.lettering.iter())
        .find(|l| l.text_node_id.0 == text_node)
        .and_then(|l| l.text_layout_settings.font.clone())
        .expect("the record carries the family it was set in");
    println!("record       : Lettering.text_layout_settings.font = {record_font:?}");
    assert_eq!(record_font, "Comic Neue", "and the record agrees with the plan and the document");

    // The bite: a caller that checks "was the setting applied?" by comparing what `settings.get`
    // returned with what the record carries is told it was not.
    assert_ne!(got["default_lettering_style"].as_str(), Some(record_font.as_str()), "the two spellings of one setting are unequal as strings");
    // The setting is stored once and read twice; the trim is one line at the writer away.
    assert!(applied == got["default_lettering_style"].as_str().unwrap().trim(), "only the padding separates them");
}
