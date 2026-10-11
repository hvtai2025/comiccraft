//! M1 fix round, D1: **⌘S and the quit prompt's Save must not discard a model-only comic edit.**
//!
//! W4's F5 made `is_dirty()` count the comic model, so the asterisk, Save's enablement and the
//! quit prompt are honest about a model-only edit (`comic.panel.set_shot`, `comic.reading_order.set`).
//! The save they were wired to was not: ⌘S (`menus.rs` → the `file.save` UI command → `io::save`)
//! ran `document.save`, which writes `document/comic.vectorcraft` and not the bundle, so the model
//! stayed unsaved — and at the quit prompt the document then closed and the edit was gone.
//!
//! The fix has two halves, and this file drives both through the real UI path:
//!
//! 1. `io::save` routes a plain Save to `comic.project.save` when the active document has an open
//!    project, so the save the user asked for writes the whole bundle.
//! 2. `unsaved::confirm` re-checks `is_dirty()` after the save on **both** paths, not only when the
//!    save went to the background, so a save that did not clear it stops the close instead of
//!    discarding the work.
//!
//! `document.save` itself is deliberately unchanged (design §10.1), and the last two tests pin
//! that: the command still writes the document alone, and a session with no project still gets the
//! document save.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_ui_egui::{Services, VectorcraftApp};

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-acceptance-ui").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn app() -> VectorcraftApp {
    // The desktop host's file writer: any document save routes through it.
    let services =
        Services { write: Some(Box::new(|path: &str, bytes: &[u8]| std::fs::write(path, bytes).map_err(|e| e.to_string()))), ..Default::default() };
    VectorcraftApp::new(Session::new(), services)
}

fn run(app: &mut VectorcraftApp, id: &str, params: Value) -> Value {
    app.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The shot description the *bundle on disk* holds for a page's first panel.
fn saved_shot(root: &Path, page: u64) -> String {
    let file = root.join("pages").join(format!("page-{page}.json"));
    let v: Value = serde_json::from_slice(&std::fs::read(&file).expect("the page file")).expect("JSON");
    v["panels"][0]["shot_description"].as_str().expect("the shot").to_string()
}

/// A project with one page, one panel (brief "First brief.") and one balloon, saved to disk.
fn project(root: &Path) -> (VectorcraftApp, u64, u64) {
    let mut app = app();
    run(&mut app, "comic.project.create", json!({"path": root.to_string_lossy(), "title": "Save", "pageSize": [612.0, 792.0]}));
    let page = run(&mut app, "comic.page.create", json!({}))["page"].as_u64().expect("a page");
    let panel = run(&mut app, "comic.panel.add", json!({"page": page, "rect": [40.0, 40.0, 240.0, 200.0], "shot": "First brief."}))["panel"]
        .as_u64()
        .expect("a panel");
    run(&mut app, "comic.lettering.add", json!({"page": page, "panel": panel, "kind": "speech", "text": "Before."}));
    run(&mut app, "comic.project.save", json!({}));
    assert!(!app.session.documents()[0].is_dirty(), "a saved project is clean");
    (app, page, panel)
}

/// ⌘S: the `file.save` UI command, exactly what the menu entry and its `Cmd+S` shortcut run.
fn cmd_s(app: &mut VectorcraftApp) -> Value {
    run(app, "file.save", Value::Null)
}

/// Quit and answer the prompt with Save, as `unsaved::close_all` + `unsaved::confirm` do.
fn quit_saving(app: &mut VectorcraftApp) -> Value {
    let asked = vectorcraft_ui_egui::unsaved::close_all(app, "quit").expect("Quit asks");
    vectorcraft_ui_egui::unsaved::confirm(app).expect("Save answers the prompt");
    asked
}

/// ⌘S after a model-only edit writes the **whole bundle**: the model reaches `pages/*.json`, the
/// dirty flag clears (both halves), and the manifest no longer describes the document's previous
/// generation — reopening the bundle reports no §14 stale-hash finding.
#[test]
fn cmd_s_saves_the_whole_project_and_clears_the_dirty_flag() {
    let root = scratch("cmd-s-saves").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    // The user edits the comic model and nothing else (a shot brief — `comic.panel.set_shot`).
    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    assert!(app.session.documents()[0].is_dirty(), "F5: a model-only edit is unsaved work, so the asterisk is honest");

    let saved = cmd_s(&mut app);
    println!("file.save (⌘S) → {saved}");
    assert_eq!(saved["root"].as_str(), Some(root.to_string_lossy().as_ref()), "⌘S saved the project, not only its document");
    assert!(!app.session.documents()[0].is_dirty(), "and both halves are clean afterwards: the asterisk goes off");
    assert_eq!(saved_shot(&root, page), "Second brief.", "the model is on disk");

    // The manifest was rebuilt with the document as written: its recorded size is the file's.
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("the manifest")).expect("JSON");
    let entry = &manifest["files"]["document/comic.vectorcraft"];
    let bytes = std::fs::metadata(root.join("document/comic.vectorcraft")).expect("the document").len();
    assert_eq!(entry["bytes"].as_u64(), Some(bytes), "the manifest describes this generation: {entry}");

    // And a reopen says so: no `previous generation` warning for the document (§14 step 1).
    run(&mut app, "file.close", json!({"index": 0}));
    let reopened = run(&mut app, "comic.project.open", json!({"path": root.to_string_lossy()}));
    let findings = reopened["findings"].as_array().cloned().unwrap_or_default();
    assert!(
        !findings.iter().any(|f| f["message"].as_str().is_some_and(|m| m.contains("previous generation"))),
        "a reopen after ⌘S names no stale file: {findings:?}"
    );
    assert_eq!(saved_shot(&root, page), "Second brief.", "and the edit is still the one on disk");
}

/// The quit prompt's Save, with no ⌘S first: the prompt appears because the model is unsaved work,
/// and answering **Save** keeps the edit. This is W5's D1 reproducer, inverted.
#[test]
fn the_quit_prompts_save_keeps_a_model_only_comic_edit() {
    let root = scratch("quit-keeps-edit").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    assert!(app.session.documents()[0].is_dirty(), "the prompt appears, because the model is unsaved work");

    let asked = quit_saving(&mut app);
    assert_eq!(asked["pending"], json!("saveChanges"), "the prompt was raised");
    assert!(app.session.documents().is_empty(), "the document closed");
    assert_eq!(app.ui.status, "quit", "and the quit went through");
    assert_eq!(
        saved_shot(&root, page),
        "Second brief.",
        "the user answered Save and the comic edit is on disk: the save the prompt performs writes the model too"
    );
}

/// The whole sequence the defect was reported as: a model-only edit, ⌘S, then quit. After ⌘S
/// nothing is unsaved, so the quit asks nothing, closes clean, and both the edit and the quit are
/// kept — no prompt whose Save could discard it.
#[test]
fn the_full_sequence_model_edit_cmd_s_quit_keeps_the_edit() {
    let root = scratch("edit-save-quit").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    cmd_s(&mut app);
    assert!(!app.session.documents()[0].is_dirty(), "⌘S finished the save: no asterisk, so nothing to ask about");

    let asked = vectorcraft_ui_egui::unsaved::close_all(&mut app, "quit").expect("Quit");
    assert_eq!(asked, Value::Null, "nothing is unsaved, so no prompt is raised");
    assert_eq!(app.ui.status, "quit", "the host is told to close the window");
    assert!(!app.session.documents()[0].is_dirty(), "the document is still open, and clean (the host closes the window)");
    assert_eq!(saved_shot(&root, page), "Second brief.", "and the edit is the one on disk");
}

/// §10.1: `document.save` is **not** changed. Named directly (an agent, or a menu item that means
/// the document), it still writes the visual document alone and leaves the model unsaved — which is
/// what keeps a document-only write reachable on a bundle that needs hand repair (§14).
#[test]
fn document_save_still_writes_only_the_document() {
    let root = scratch("document-save").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    let saved = run(&mut app, "document.save", json!({}));
    println!("document.save → {saved}");
    assert!(saved["path"].as_str().is_some_and(|p| p.ends_with("document/comic.vectorcraft")), "{saved}");
    assert!(saved.get("root").is_none(), "a document save reports a document, not a bundle: {saved}");
    assert!(app.session.documents()[0].is_dirty(), "the model half is still unsaved work");
    assert_eq!(saved_shot(&root, page), "First brief.", "the model did not reach the disk: that is what the project save is for");
}

/// With no comic project, `file.save` is the document save and nothing else — the shape a caller
/// has always got, with no `root`/`files` from the comic route.
#[test]
fn without_a_project_plain_save_is_the_document_save() {
    let dir = scratch("no-project");
    let out = dir.join("plain.vectorcraft");
    let mut app = app();
    let picked = out.clone();
    app.services.pick_save = Some(Box::new(move |_: &vectorcraft_ui_egui::FilePick| Some(picked.to_string_lossy().into_owned())));
    run(&mut app, "file.new", json!({"width": 100, "height": 100}));
    run(&mut app, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10}));
    assert!(app.session.documents()[0].is_dirty());

    let saved = cmd_s(&mut app);
    println!("file.save with no project → {saved}");
    assert_eq!(saved["path"].as_str(), Some(out.to_string_lossy().as_ref()), "the picked document path: {saved}");
    assert!(saved.get("root").is_none() && saved.get("files").is_none(), "no project was written: {saved}");
    assert!(out.is_file(), "the document is on disk");
    assert!(!app.session.documents()[0].is_dirty(), "and the document is saved");
}
