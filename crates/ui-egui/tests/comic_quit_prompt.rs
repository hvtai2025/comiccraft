//! W5 acceptance: what ⌘S and the quit prompt do when a ComicCraft project is open.
//!
//! Not one of the twelve acceptance tests — it is the D1 invariant's acceptance. W4's F5 made
//! `is_dirty()` count a model-only comic edit, so the asterisk and the quit prompt are honest; D1 was
//! that the answer the prompt took could not be honoured: ⌘S was `document.save`
//! (`crates/ui-egui/src/menus.rs:69`), which writes the **document**, and the quit prompt's Save then
//! closed a document that was still dirty, taking the edit with it.
//!
//! These tests drive the real UI path — `unsaved::close_all` / `unsaved::confirm`, which is what
//! File → Quit, File → Close and the window's close button call — and then read the bundle on disk to
//! see what survived. The invariant they hold is the one D1's fix states: **after the prompt's Save,
//! either `is_dirty()` is false and the document closes, or the close is refused and nothing is
//! discarded — never both.** The first test drives the ⌘S binding and the prompt's Save with a
//! project open (both must write the model); the second drives a Save that cannot complete and
//! requires the close to be refused rather than to discard the work.
//!
//! R1-1: this file used to assert the *loss* — the old
//! `evidence_the_quit_prompts_save_discards_a_model_only_comic_edit` was the reproduction, and it was
//! green while the defect existed. That is evidence, not an acceptance test, so it is inverted here:
//! what it pinned is now asserted the other way round.
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
    // The desktop host's file writer: the UI's Save routes through it (`io::write_to`).
    let services =
        Services { write: Some(Box::new(|path: &str, bytes: &[u8]| std::fs::write(path, bytes).map_err(|e| e.to_string()))), ..Default::default() };
    VectorcraftApp::new(Session::new(), services)
}

fn run(app: &mut VectorcraftApp, id: &str, params: Value) -> Value {
    app.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn dirty(app: &VectorcraftApp) -> bool {
    app.session.documents()[0].is_dirty()
}

/// The shot description the *bundle on disk* holds for a page's first panel.
fn saved_shot(root: &Path, page: u64) -> String {
    let file = root.join("pages").join(format!("page-{page}.json"));
    let v: Value = serde_json::from_slice(&std::fs::read(&file).expect("the page file")).expect("JSON");
    v["panels"][0]["shot_description"].as_str().expect("the shot").to_string()
}

/// The shot description the open **model** holds for a panel — the edit a close would discard. Read
/// through the model, not the disk, so it is there even when no save happened.
fn model_shot(app: &VectorcraftApp, panel: u64) -> String {
    let oc = app.session.documents()[0].comic.as_ref().expect("an open project");
    oc.bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == panel).expect("the panel record").shot_description.clone()
}

/// A project with one page, one panel (brief "First brief.") and one balloon, saved to disk.
/// Returns the app, the page id and the panel id.
fn project(root: &Path) -> (VectorcraftApp, u64, u64) {
    let mut app = app();
    run(&mut app, "comic.project.create", json!({"path": root.to_string_lossy(), "title": "Quit", "pageSize": [612.0, 792.0]}));
    let page = run(&mut app, "comic.page.create", json!({}))["page"].as_u64().expect("a page");
    let panel = run(&mut app, "comic.panel.add", json!({"page": page, "rect": [40.0, 40.0, 240.0, 200.0], "shot": "First brief."}))["panel"]
        .as_u64()
        .expect("a panel");
    run(&mut app, "comic.lettering.add", json!({"page": page, "panel": panel, "kind": "speech", "text": "Before."}));
    run(&mut app, "comic.project.save", json!({}));
    assert!(!app.session.documents()[0].is_dirty(), "a saved project is clean");
    assert_eq!(saved_shot(root, page), "First brief.");
    (app, page, panel)
}

/// Quit, and answer the prompt with Save. Returns what the prompt asked.
fn quit_saving(app: &mut VectorcraftApp) -> Value {
    let asked = vectorcraft_ui_egui::unsaved::close_all(app, "quit").expect("Quit asks");
    println!("quit asked first: {asked}");
    vectorcraft_ui_egui::unsaved::confirm(app).expect("Save answers the prompt");
    println!("documents left open: {} · status: {:?}", app.session.documents().len(), app.ui.status);
    asked
}

/// Write a second `pages/` file that encodes the same page id (`page-07.json` beside `page-7.json`).
/// Schema §3 makes the padding cosmetic and rule 13 parses the number, so the two are one page and
/// `comic.project.save` refuses to write the bundle at all — a *documented* refusal, and the smallest
/// way to make the prompt's Save not happen.
fn make_the_next_save_refuse(root: &Path) -> String {
    let dir = root.join("pages");
    let name = std::fs::read_dir(&dir)
        .expect("the pages directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with("page-") && n.ends_with(".json"))
        .expect("a page file");
    let number = name.trim_start_matches("page-").trim_end_matches(".json").to_string();
    let padded = format!("page-{:0>width$}.json", number, width = number.len() + 1);
    std::fs::write(dir.join(&padded), b"{}").expect("the duplicate page file");
    padded
}

/// **The D1 invariant, first arm: the Save the user answers writes the model, and the document closes
/// only because nothing is unsaved.**
///
/// Two Save paths carry a model-only comic edit to the disk, and both are asserted here: **⌘S**
/// (`file.save`, the app-level binding) and the **quit prompt's Save**. On the revision W5 measured,
/// both wrote `document/comic.vectorcraft` alone, `is_dirty()` stayed true, and the prompt then closed
/// the document and discarded the brief (`docs/reviews/M1-acceptance.md` §5 D1). This is that
/// reproduction inverted, so it fails if either path regresses to a document-only write.
#[test]
fn the_quit_prompts_save_writes_a_model_only_comic_edit_and_closes_the_document() {
    let root = scratch("quit-writes").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    // The user edits the comic model and nothing else (a shot brief — `comic.panel.set_shot`).
    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    assert!(dirty(&app), "F5: a model-only edit is unsaved work, so the asterisk is honest");

    // **⌘S**: with a project open it is `comic.project.save`, so the model reaches the bundle and
    // nothing is left unsaved. `root` is what tells the two apart: `document.save` reports `path`.
    let saved = run(&mut app, "file.save", json!({}));
    println!("⌘S → {saved}");
    assert!(saved.get("root").is_some(), "⌘S with a project open is comic.project.save, not document.save: {saved}");
    assert_eq!(saved_shot(&root, page), "Second brief.", "⌘S wrote the model to the bundle");
    assert!(!dirty(&app), "and ⌘S cleared the model half of the dirty flag");

    // A second model-only edit, and now the quit prompt.
    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Third brief."}));
    assert!(dirty(&app), "a model-only edit is unsaved work");
    let asked = quit_saving(&mut app);
    assert_eq!(asked["pending"], json!("saveChanges"), "the prompt appears, because the model is unsaved work");
    assert_eq!(saved_shot(&root, page), "Third brief.", "answering Save wrote the model: the edit the prompt asked about is on disk");
    assert!(app.session.documents().is_empty(), "the document closed — which `unsaved::confirm` only does once `is_dirty()` is false");
    assert_eq!(app.ui.status, "quit", "and the quit went through");
}

/// **The D1 invariant, second arm: a Save that does not happen refuses the close and discards
/// nothing.** The two are never both.
///
/// The prompt's Save is made to fail through the bundle's own §13 rule-13 refusal (two `pages/` files
/// encoding one id), so the document stays open, still dirty, with the edit intact in the model and
/// the pre-edit brief still on disk. An implementation that closed the document after a failed save
/// would red the first assertion below.
#[test]
fn a_save_that_cannot_complete_refuses_the_close_and_discards_nothing() {
    let root = scratch("quit-refused").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    assert!(dirty(&app), "a model-only edit is unsaved work");
    let duplicate = make_the_next_save_refuse(&root);
    println!("wrote pages/{duplicate}, so the next save must refuse");

    let asked = vectorcraft_ui_egui::unsaved::close_all(&mut app, "quit").expect("Quit asks");
    println!("quit asked first: {asked}");
    assert_eq!(asked["pending"], json!("saveChanges"), "the prompt appears");
    let refused = vectorcraft_ui_egui::unsaved::confirm(&mut app).expect_err("the save cannot complete, so the close must be refused");
    println!("the close was refused with: {refused}");

    assert!(refused.contains("two files encode one id"), "the refusal is the bundle's own, not a silent close: {refused}");
    assert_eq!(app.session.documents().len(), 1, "the document is still open");
    assert!(dirty(&app), "and it is still unsaved work, so the prompt was not answered by discarding it");
    assert_eq!(model_shot(&app, panel), "Second brief.", "and the edit is not discarded");
    assert_eq!(saved_shot(&root, page), "First brief.", "nothing was written, so the bundle is untouched");
    assert_ne!(app.ui.status, "quit", "and the quit did not go through");
}

/// The control: the same quit, the same answer, after the one save that does write the model. This is
/// what a fix had to produce — and it is reachable by asking the comic layer to save.
#[test]
fn evidence_the_same_quit_keeps_the_edit_after_comic_project_save() {
    let root = scratch("quit-keeps").join("book.comiccraft");
    let (mut app, page, panel) = project(&root);

    run(&mut app, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    run(&mut app, "comic.project.save", json!({}));
    assert!(!dirty(&app), "the project save clears both halves");
    assert_eq!(saved_shot(&root, page), "Second brief.", "and it is the model that reaches the disk");

    let asked = vectorcraft_ui_egui::unsaved::close_all(&mut app, "quit").expect("Quit");
    println!("quit asked first: {asked}");
    assert_eq!(asked, Value::Null, "nothing is unsaved, so no prompt is raised");
    assert_eq!(app.ui.status, "quit", "the host is told to close the window");
    assert_eq!(saved_shot(&root, page), "Second brief.", "and the edit is still there");
}
