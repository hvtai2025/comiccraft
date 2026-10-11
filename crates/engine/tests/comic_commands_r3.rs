//! Round-3 review probes for the M1 W4 `comic.*` command surface, by `code-reviewer`.
//!
//! Round 2's F9 was "the prune's predicate is the *declared* set, not the *staged* one", and the
//! fix made `prune_stale` read `ComicBundle::files_of_record`. The round-3 brief asks whether the
//! *next* step exists: the manifest a save writes comes from `scan_tree` (§4: `files` is every file
//! in the bundle), so the file the fix preserved is *declared* by the save that kept it, and a
//! **reload** seeds `files_of_record` from the manifest it reads — so the loss might be deferred
//! across a reload instead of a save.
//!
//! These tests run that trace on the real code, with real files, at both levels: the command
//! surface (`comic.project.save`, which is what M1 ships) and the crate (`comic-model`'s own
//! `load`/`save`, which is what F9 was about).
// Integration tests: unwrapping is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vectorcraft_comic_model::SaveOptions;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;

// ---------- fixtures (the same shape as `comic_commands_r1.rs` / `_r2.rs`) ----------

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-review-r3").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    s.execute("comic.project.create", &json!({"path": root.to_string_lossy(), "title": "R3"})).expect("comic.project.create");
    (s, root)
}

fn page_of(s: &mut Session) -> u64 {
    s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id")
}

fn open_comic(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn record(s: &Session) -> Vec<String> {
    open_comic(s).bundle.files_of_record.iter().cloned().collect()
}

fn read_manifest(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("read manifest.json")).expect("manifest json")
}

fn declares(root: &Path, key: &str) -> bool {
    read_manifest(root)["files"].get(key).is_some()
}

fn reopen(root: &Path) -> Session {
    let mut s = Session::new();
    s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open");
    s
}

/// The `updated` field of the manifest on disk — a canonical timestamp, so a crate-level `save`
/// (which refuses a non-canonical `SaveOptions::now`) can be driven from it.
fn stamp(root: &Path) -> String {
    read_manifest(root)["updated"].as_str().expect("manifest.updated").to_string()
}

// ---------- (1) the reload boundary, at the command surface ----------

/// The round-3 brief's trace, run verbatim: create → save 1 → copy `pages/page-1.json` to
/// `pages/page-99.json` → save 2 → **reopen** (`comic.project.open`, i.e. `comic-model::load`
/// seeding `files_of_record` from the manifest it reads) → save 3 → save 4.
///
/// The claim under test: `page-99` becomes "of record" across the reload without any save staging
/// it, so save 3 prunes it. If instead the *load* adopts it as page 99, it is staged from then on
/// and can never be pruned while the bundle holds it.
#[test]
fn r3_the_reload_boundary_trace() {
    let (mut s, root) = project("reload-boundary");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");
    let stray = root.join("pages/page-99.json");
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &stray).expect("a page file written since the load");

    s.execute("comic.project.save", &json!({})).expect("save 2");
    println!("PROBE save2: stray exists                     = {}", stray.is_file());
    println!("PROBE save2: manifest declares it             = {}", declares(&root, "pages/page-99.json"));
    println!("PROBE save2: files_of_record contains it      = {}", record(&s).contains(&"pages/page-99.json".to_string()));

    let mut reopened = reopen(&root);
    let keys: Vec<u64> = open_comic(&reopened).bundle.pages.keys().map(|k| k.0).collect();
    let rec = record(&reopened);
    println!("PROBE reload: model page keys                 = {keys:?}");
    println!("PROBE reload: files_of_record contains page-99= {}", rec.contains(&"pages/page-99.json".to_string()));
    println!("PROBE reload: manifest declares it            = {}", declares(&root, "pages/page-99.json"));
    println!(
        "PROBE reload: load findings                   = {:?}",
        open_comic(&reopened).findings.iter().map(|f| f.message.clone()).collect::<Vec<_>>()
    );
    let v = reopened.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    let rules: Vec<Value> = v["findings"].as_array().expect("findings").iter().map(|f| json!([f["rule"], f["message"]])).collect();
    println!("PROBE reload: validate errors = {} rules = {}", v["errors"], serde_json::to_string(&rules).unwrap());

    let out = reopened.execute("comic.project.save", &json!({})).expect("save 3");
    let files: Vec<&str> = out["files"].as_array().expect("files").iter().filter_map(Value::as_str).filter(|k| k.starts_with("pages/")).collect();
    println!("PROBE save3: pages/ keys in the result        = {files:?}");
    println!("PROBE save3: stray exists                     = {}", stray.is_file());
    println!("PROBE save3: manifest declares it             = {}", declares(&root, "pages/page-99.json"));

    reopened.execute("comic.project.save", &json!({})).expect("save 4");
    println!("PROBE save4: stray exists                     = {}", stray.is_file());

    let mut again = reopen(&root);
    println!(
        "PROBE reload 2: pages = {}",
        again.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).map(|o| o["pages"].to_string()).unwrap_or_default()
    );
    let keys2: Vec<u64> = open_comic(&again).bundle.pages.keys().map(|k| k.0).collect();
    println!("PROBE reload 2: model page keys               = {keys2:?}");

    // Is the reloaded bundle's page 99 *owned* (so that removing it is legitimate) or merely
    // declared? Removing it is the only thing that can delete the file now.
    assert!(stray.is_file(), "the reload + two saves destroyed the file the round-2 fix preserved");

    let removed = again.execute("comic.page.delete", &json!({"page": 99}));
    println!("PROBE repair: page.delete(99) is Ok          = {}", removed.is_ok());
    println!("PROBE repair: the stray survived the delete   = {}", stray.is_file());
    let saved = again.execute("comic.project.save", &json!({})).expect("save 5");
    println!(
        "PROBE repair: pages/ in the result            = {:?}",
        saved["files"].as_array().map(|a| a.iter().filter_map(Value::as_str).filter(|k| k.starts_with("pages/")).collect::<Vec<_>>())
    );
    println!("PROBE repair: the stray after delete + save   = {}", stray.is_file());
    let v = again.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    println!("PROBE repair: validate errors = {} findings = {}", v["errors"], serde_json::to_string(&v["findings"]).unwrap());
    assert!(
        !stray.is_file(),
        "the file goes only when the model drops the page it was adopted as: the removal is the bundle's own page.delete, not a deferred loss"
    );
}

// ---------- (2) the reload boundary, at the crate the predicate lives in ----------

/// The same class, one level down: a `pages/` file whose *name parses to an id another file also
/// encodes* — the one flat, prunable name a load does **not** adopt (it reports the pair and keeps
/// the first in sort order). `comic-model::save` has no duplicate check of its own (that refusal is
/// the engine's), so this is the one sequence where a save declares a file the load skipped.
///
/// It is here to measure what the deferred prune actually destroys: the file is removed, but the
/// content the load adopted from it is what the save re-writes under the model's own name, so no
/// bytes are lost by the *prune*; the bytes `step 4` overwrites are why `comic.project.save`
/// refuses the pair in the first place.
#[test]
fn r3_the_crate_level_duplicate_loser_across_a_reload() {
    let (mut s, root) = project("crate-duplicate");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");
    drop(s);

    let loser = root.join("pages/page-001.json");
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &loser).expect("a second file encoding page 1");

    // save 2 at the crate level: the loser is not of record, so it is kept, and the manifest the
    // scan produces declares both files (§4).
    let mut bundle = vectorcraft_comic_model::load(&root).expect("load").bundle;
    vectorcraft_comic_model::save(&mut bundle, &root, &SaveOptions { now: stamp(&root), document: None }).expect("save 2");
    println!("PROBE crate save2: loser exists                = {}", loser.is_file());
    println!(
        "PROBE crate save2: manifest declares both      = {} / {}",
        declares(&root, "pages/page-1.json"),
        declares(&root, "pages/page-001.json")
    );

    // reload: the pair is reported, the load keeps the first in sort order (`page-001.json`), and
    // the record is seeded from the manifest that declares both.
    let loaded = vectorcraft_comic_model::load(&root).expect("load");
    let keys: Vec<u64> = loaded.bundle.pages.keys().map(|k| k.0).collect();
    let rules: Vec<Option<u8>> = loaded.findings.iter().map(|f| f.rule).collect();
    println!("PROBE crate reload: model page keys            = {keys:?}");
    println!("PROBE crate reload: findings rules             = {rules:?}");
    println!("PROBE crate reload: record declares the loser  = {}", loaded.bundle.files_of_record.contains("pages/page-001.json"));

    let mut bundle = loaded.bundle;
    vectorcraft_comic_model::save(&mut bundle, &root, &SaveOptions { now: stamp(&root), document: None }).expect("save 3");
    println!("PROBE crate save3: loser exists                = {}", loser.is_file());
    println!("PROBE crate save3: winner exists               = {}", root.join("pages/page-1.json").is_file());
    println!("PROBE crate save3: manifest declares the loser = {}", declares(&root, "pages/page-001.json"));
}

// ---------- (3) the other half: declared, not adopted, and *not prunable* ----------

/// A `pages/` file whose name parses to no page id — a rule-13 stray. The load reports it and
/// leaves it alone, the manifest's scan declares it (§4), and a reload seeds the record with it, so
/// the round-3 trace's precondition holds exactly: it is of record and never staged. It survives,
/// because `is_written_by_this_version` stops the prune before the record is consulted — the §12
/// promise.
#[test]
fn r3_a_declared_page_file_this_version_does_not_own_survives_the_reload_and_a_save() {
    let (mut s, root) = project("declared-not-owned");
    page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");
    let stray = root.join("pages/notes.json");
    std::fs::write(&stray, b"{\"not\":\"a page file name\"}").expect("a file this version does not own");
    s.execute("comic.project.save", &json!({})).expect("save 2");
    println!("PROBE stray save2: exists = {}, declared = {}", stray.is_file(), declares(&root, "pages/notes.json"));

    let mut reopened = reopen(&root);
    println!("PROBE stray reload: findings = {:?}", open_comic(&reopened).findings.iter().map(|f| f.message.clone()).collect::<Vec<_>>());
    println!("PROBE stray reload: record has it = {}", record(&reopened).contains(&"pages/notes.json".to_string()));
    reopened.execute("comic.project.save", &json!({})).expect("save 3");
    println!("PROBE stray save3: exists = {}", stray.is_file());
    assert!(stray.is_file(), "a declared file this version does not own must survive every save (§12)");
}

/// The same, one directory down: `pages/backup/page-99.json` is in the scan (§4 declares it) and
/// the load skips it (`pages/` subdirectories are not page files), so nothing adopts it — and the
/// prune walks only the *flat* entries of the directories it owns, so it is never removed either.
#[test]
fn r3_a_nested_page_file_survives_the_reload_and_a_save() {
    let (mut s, root) = project("nested");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");
    std::fs::create_dir_all(root.join("pages/backup")).expect("a nested directory");
    let stray = root.join("pages/backup/page-99.json");
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &stray).expect("a nested copy");

    s.execute("comic.project.save", &json!({})).expect("save 2");
    println!("PROBE nested save2: exists = {}, declared = {}", stray.is_file(), declares(&root, "pages/backup/page-99.json"));
    let mut reopened = reopen(&root);
    println!("PROBE nested reload: model page keys = {:?}", open_comic(&reopened).bundle.pages.keys().map(|k| k.0).collect::<Vec<_>>());
    reopened.execute("comic.project.save", &json!({})).expect("save 3");
    println!("PROBE nested save3: exists = {}", stray.is_file());
    assert!(stray.is_file(), "a nested page file is neither a page nor a prunable name");
}

// ---------- (4) the new public field: not on the wire, and no prune-all it cannot justify ----------

/// `ComicBundle::files_of_record` is documented as in-memory and non-wire. Two checks: the
/// `manifest.json` the save writes carries §4's fields and nothing else, and no `.json` anywhere in
/// the bundle mentions the field. (The type carries no `Serialize`/`Deserialize` at all, so this is
/// the empirical half of the same claim.)
#[test]
fn r3_the_in_memory_record_does_not_reach_the_wire() {
    let (mut s, root) = project("no-wire");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("the save");

    let manifest = read_manifest(&root);
    let mut keys: Vec<String> = manifest.as_object().expect("manifest object").keys().cloned().collect();
    keys.sort();
    println!("PROBE wire: manifest keys = {keys:?}");
    assert_eq!(
        keys,
        vec![
            "app",
            "bundle_id",
            "comic_id",
            "created",
            "document",
            "files",
            "next_comic_id",
            "retired_artboard_ids",
            "schema_version",
            "title",
            "updated"
        ],
        "the manifest carries schema §4's fields and nothing else"
    );

    let mut checked = 0usize;
    check_no_record(&root, &mut checked);
    println!("PROBE wire: json files scanned = {checked}");
    assert!(checked >= 8, "the scan saw the bundle's files: {checked}");
    assert!(root.join(format!("pages/page-{page}.json")).is_file());
}

fn check_no_record(dir: &Path, checked: &mut usize) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if entry.file_type().expect("file type").is_dir() {
            check_no_record(&path, checked);
        } else if path.extension().is_some_and(|e| e == "json") {
            let bytes = std::fs::read(&path).expect("read a json file");
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains("files_of_record"), "{} carries the in-memory field on the wire", path.display());
            *checked += 1;
        }
    }
}

/// The "no generation of record" branch, reached with a **non-empty tree**: a manifest whose `files`
/// map is empty seeds an empty record, so `written.is_empty()` is true and the prune is at its most
/// aggressive (round 0's behaviour). The page files survive anyway, because the load *adopts* every
/// flat `pages/page-<n>.json` and the save stages what the model holds — so "prune everything" can
/// only reach a name the model does not have. (This is the state the lead asked to be checked: an
/// empty record must not destroy a page the bundle carries.)
#[test]
fn r3_an_empty_files_map_prunes_nothing_the_model_holds() {
    let (mut s, root) = project("empty-record");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");

    let mut manifest = read_manifest(&root);
    manifest["files"] = json!({});
    std::fs::write(root.join("manifest.json"), serde_json::to_vec(&manifest).expect("manifest bytes")).expect("hand-edit the manifest");

    let mut reopened = reopen(&root);
    println!("PROBE empty record: record len                = {}", record(&reopened).len());
    println!("PROBE empty record: model page keys           = {:?}", open_comic(&reopened).bundle.pages.keys().map(|k| k.0).collect::<Vec<_>>());
    reopened.execute("comic.project.save", &json!({})).expect("save 2");
    let file = root.join(format!("pages/page-{page}.json"));
    println!("PROBE empty record: the page file exists      = {}", file.is_file());
    assert!(!record(&reopened).is_empty(), "the save it just made is the new record");
    assert!(file.is_file(), "the prune-everything branch removed a page the model holds");
}
