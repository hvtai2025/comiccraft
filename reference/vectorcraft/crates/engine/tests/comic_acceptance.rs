//! M1 acceptance tests — the twelve in `docs/milestones/M1.md` §1, written against the exit
//! criteria rather than against the implementation.
//!
//! Every test drives the public command surface (`Session::execute`) and then asks the *artefacts*
//! — the bundle on disk, the visual document, `comic.project.validate`, the exported PNG and PDF —
//! whether the criterion holds. Where a criterion names a user action ("the user moves and resizes
//! a panel on the canvas") the test drives both the comic path and the plain document path, because
//! those are two different users of the same record.
//!
//! Test 11 is special: §5.3.1's artboard-id hazard needs a **failing-then-passing** test, so
//! `acceptance_11_...` is written to fail against a plausible-but-wrong implementation. The wrong
//! implementation and the red output are recorded in `docs/reviews/M1-acceptance.md`.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_comic_model::{
    ApprovalStatus, Asset, AssetId, AssetType, ComicBundle, ContentHash, LetteringKind, MediaMetadata, PageId, StorageRef, load,
};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind, Selection};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;
use vectorcraft_geom::{PathData, Rect};

// ---------- fixtures ----------

/// A scratch directory inside the workspace's `target/`: never a system temporary directory and
/// never a tracked one, so the `assets` and `brands` gates never see a test bundle.
fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-acceptance").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn exec(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, &params).unwrap_or_else(|e| panic!("{id} rejected {params}: {e}"))
}

fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    exec(&mut s, "comic.project.create", json!({"path": root.to_string_lossy(), "title": "Acceptance"}));
    (s, root)
}

fn new_page(s: &mut Session, params: Value) -> u64 {
    exec(s, "comic.page.create", params)["page"].as_u64().expect("a page id")
}

fn add_panel(s: &mut Session, page: u64, rect: [f64; 4]) -> u64 {
    exec(s, "comic.panel.add", json!({"page": page, "rect": rect}))["panel"].as_u64().expect("a panel id")
}

fn open_comic(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn open_comic_mut(s: &mut Session) -> &mut OpenComic {
    Arc::make_mut(s.doc_mut().expect("a document").comic.as_mut().expect("an open project"))
}

fn doc(s: &Session) -> Arc<Document> {
    s.doc().expect("a document").doc.clone()
}

fn validate(s: &mut Session) -> Value {
    exec(s, "comic.project.validate", json!({}))
}

fn errors_of(v: &Value) -> u64 {
    v["errors"].as_u64().expect("errors")
}

fn pages_of(s: &Session) -> Vec<(PageId, u32)> {
    open_comic(s).bundle.pages.iter().map(|(id, file)| (*id, file.page.artboard_id)).collect()
}

fn page_file(s: &Session, page: u64) -> &vectorcraft_comic_model::PageFile {
    open_comic(s).bundle.pages.get(&PageId(page)).expect("the page file")
}

/// The ids of the artboards the document holds, in document order.
fn artboard_ids(s: &Session) -> Vec<u32> {
    doc(s).artboards.iter().map(|a| a.id).collect()
}

/// The ids the manifest has retired (§5.3.1 mitigation 2): "never again" numbers.
fn retired_artboards(s: &Session) -> Vec<u32> {
    open_comic(s).bundle.manifest.retired_artboard_ids.clone()
}

/// The artboard id each page resolves to — an id a page *names*, whether or not the artboard behind
/// it still exists.
fn named_artboards(s: &Session) -> Vec<u32> {
    open_comic(s).bundle.pages.values().map(|f| f.page.artboard_id).collect()
}

fn panel_record(s: &Session, id: u64) -> vectorcraft_comic_model::Panel {
    open_comic(s).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == id).cloned().expect("the panel record")
}

fn lettering_record(s: &Session, id: u64) -> vectorcraft_comic_model::Lettering {
    open_comic(s).bundle.pages.values().flat_map(|f| f.lettering.iter()).find(|l| l.id.0 == id).cloned().expect("the lettering record")
}

fn dialogue_record(s: &Session, id: u64) -> vectorcraft_comic_model::Dialogue {
    open_comic(s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id.0 == id).cloned().expect("the dialogue record")
}

fn node(s: &Session, id: u64) -> Node {
    doc(s).node(NodeId(id)).unwrap_or_else(|| panic!("NodeId({id}) is not in the document")).clone()
}

fn path_of(s: &Session, id: u64) -> PathData {
    node(s, id).path_data().cloned().unwrap_or_else(|| panic!("NodeId({id}) is not a path"))
}

fn bounds(s: &Session, id: u64) -> [f64; 4] {
    let b = node(s, id).geometric_bounds().expect("the node has geometry");
    [b.x0, b.y0, b.x1, b.y1]
}

fn node_ids(s: &Session) -> BTreeSet<u64> {
    let mut out = BTreeSet::new();
    doc(s).walk(|n| {
        out.insert(n.id.0);
    });
    out
}

fn shown_text(s: &Session, id: u64) -> String {
    match &node(s, id).kind {
        NodeKind::Text(t) => t.plain_text(),
        other => panic!("NodeId({id}) is not a type object: {other:?}"),
    }
}

fn artboard_rect(s: &Session, artboard_id: u32) -> [f64; 4] {
    let d = doc(s);
    let a = d.artboards.iter().find(|a| a.id == artboard_id).expect("the artboard");
    [a.rect.x0, a.rect.y0, a.rect.x1, a.rect.y1]
}

fn dialogue_ids(s: &Session, panel: u64) -> Vec<u64> {
    panel_record(s, panel).dialogue_ids.iter().map(|d| d.0).collect()
}

/// A PNG under the bundle plus the `Linked` `Asset` record naming it. M1 has no `comic.asset.*`
/// command, so the record is added to the open model exactly as W1's fixture does; every other step
/// goes through `Session::execute`.
fn add_linked_png(s: &mut Session, root: &Path, relative: &str, rgb: [u8; 3]) -> u64 {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the assets directory");
    let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    img.save(&path).expect("write the png");
    let bytes = std::fs::read(&path).expect("read the png");
    let hash = vectorcraft_comic_model::sha256_hex(&bytes);
    let oc = open_comic_mut(s);
    let id = oc.bundle.allocate::<AssetId>().expect("an asset id");
    oc.bundle.assets.assets.push(Asset {
        id,
        asset_type: AssetType::Source,
        storage_reference: StorageRef::Linked { relative_path: relative.to_string() },
        content_hash: ContentHash { algorithm: "sha256".into(), value: hash },
        media_metadata: MediaMetadata { mime: "image/png".into(), width: 64, height: 64, color_profile: None, dpi: None },
        provenance: None,
        parent_asset_ids: Vec::new(),
        approval_status: ApprovalStatus::Pending,
        unknown: Default::default(),
    });
    id.0
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read_dir") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Bundle-relative path → bytes, for every file except the named ones (the manifest and project
/// carry `updated` timestamps, which are not the layout these tests are about).
fn file_contents(root: &Path, skip: &[&str]) -> BTreeMap<String, Vec<u8>> {
    files_under(root)
        .into_iter()
        .filter(|p| !skip.contains(&p.file_name().expect("a file name").to_string_lossy().as_ref()))
        .map(|p| (p.strip_prefix(root).expect("under the root").to_string_lossy().replace('\\', "/"), std::fs::read(&p).expect("read")))
        .collect()
}

/// A snapshot of everything an undo step must restore: the visual document, the selection and the
/// comic model (design §11.2: "undo must restore both semantic and visual state").
#[derive(Debug, PartialEq)]
struct State {
    doc: Arc<Document>,
    selection: Selection,
    bundle: ComicBundle,
}

fn state(s: &Session) -> State {
    let st = s.doc().expect("a document");
    State { doc: st.doc.clone(), selection: st.selection.clone(), bundle: st.comic.as_ref().expect("an open project").bundle.clone() }
}

/// Run one command and require it to be exactly one undo step: one action, undone back to the state
/// before it and redone to the state after it, with document, selection and model all in step.
fn one_step(s: &mut Session, id: &str, params: Value) {
    let before = state(s);
    let undo_before = s.doc().expect("a document").history.undo.len();
    exec(s, id, params);
    let undo_after = s.doc().expect("a document").history.undo.len();
    assert_eq!(undo_after, undo_before + 1, "{id} must add exactly one undo step");
    let after = state(s);
    assert_ne!(after, before, "{id} is not one of the criteria if it changes nothing");
    exec(s, "edit.undo", json!({}));
    assert_eq!(state(s), before, "{id}: one undo must restore the document, the selection and the model");
    exec(s, "edit.redo", json!({}));
    assert_eq!(state(s), after, "{id}: one redo must restore the document, the selection and the model");
}

fn bucket(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

// ---------- 1. project and page create ----------

/// `comic.project.create` and `comic.page.create` produce a bundle with one `.vectorcraft`
/// document and one artboard per page (M1 §1 test 1).
#[test]
fn acceptance_1_one_document_and_one_artboard_per_page() {
    let (mut s, root) = project("t1");
    for _ in 0..3 {
        new_page(&mut s, json!({}));
    }

    // One `.vectorcraft` document in the whole bundle, and the manifest names exactly it.
    let docs: Vec<PathBuf> = files_under(&root).into_iter().filter(|p| p.extension().is_some_and(|e| e == "vectorcraft")).collect();
    assert_eq!(docs.len(), 1, "one project is one visual document (design §5.1.1/ADR-011): {docs:?}");
    let relative = docs[0].strip_prefix(&root).expect("under the root").to_string_lossy().replace('\\', "/");
    let loaded = load(&root).expect("the bundle loads");
    assert_eq!(loaded.bundle.manifest.document.relative_path, relative, "the manifest names the one document");

    // One artboard per page, each distinct, each present in the document, each the page's size.
    let pages = pages_of(&s);
    assert_eq!(pages.len(), 3, "three pages");
    let artboards: Vec<u32> = pages.iter().map(|(_, a)| *a).collect();
    assert_eq!(artboards.iter().copied().collect::<BTreeSet<u32>>().len(), 3, "no two pages share an artboard: {artboards:?}");
    assert_eq!(doc(&s).artboards.len(), 3, "one artboard per page and no spare");
    for (page, artboard) in &pages {
        let rect = artboard_rect(&s, *artboard);
        assert_eq!(bucket(rect[2] - rect[0]), 612.0, "page {} artboard {artboard} is the page size", page.0);
        assert_eq!(bucket(rect[3] - rect[1]), 792.0, "page {} artboard {artboard} is the page size", page.0);
        assert_eq!(page_file(&s, page.0).page.artboard_id, *artboard);
    }
    assert_eq!(errors_of(&validate(&mut s)), 0, "and the fresh bundle validates");
}

// ---------- 2. panel add ----------

/// `comic.panel.add` creates three panels with stable `PanelId`s and `frame_node_id`s pointing at
/// real document objects (M1 §1 test 2).
#[test]
fn acceptance_2_three_panels_with_stable_ids_and_real_frames() {
    let (mut s, _root) = project("t2");
    let page = new_page(&mut s, json!({}));
    let rects = [[40.0, 40.0, 240.0, 200.0], [40.0, 280.0, 240.0, 200.0], [300.0, 40.0, 240.0, 440.0]];
    let panels: Vec<u64> = rects.iter().map(|r| add_panel(&mut s, page, *r)).collect();

    assert_eq!(panels.iter().copied().collect::<BTreeSet<u64>>().len(), 3, "three distinct PanelIds: {panels:?}");
    let artboard = page_file(&s, page).page.artboard_id;
    let page_bounds = artboard_rect(&s, artboard);
    for (i, id) in panels.iter().enumerate() {
        let record = panel_record(&s, *id);
        assert_eq!(record.page_id, PageId(page), "the record agrees with its file (rule 11)");
        assert_eq!(record.artboard_id, artboard, "the record agrees with the page (rule 5)");
        let frame = node(&s, record.frame_node_id.0);
        assert!(matches!(frame.kind, NodeKind::Path { .. }), "a panel frame is a document path");
        assert_eq!(frame.id.0, record.frame_node_id.0);
        let b = bounds(&s, record.frame_node_id.0);
        assert!(b[0] >= page_bounds[0] - 1e-6 && b[1] >= page_bounds[1] - 1e-6, "panel {id} is inside the artboard");
        assert_eq!(bucket(b[2] - b[0]), rects[i][2], "the frame is the rectangle asked for");
        assert_eq!(bucket(b[3] - b[1]), rects[i][3], "the frame is the rectangle asked for");
        assert_eq!(record.reading_order_index as usize, i, "the narrative index follows creation order");
    }

    // **Stable**: the ids survive a re-layout that rewrites every frame rectangle.
    let frames_before: Vec<u64> = panels.iter().map(|id| panel_record(&s, *id).frame_node_id.0).collect();
    exec(&mut s, "comic.page.template.apply", json!({"page": page, "template": "three-tier"}));
    let frames_after: Vec<u64> = panels.iter().map(|id| panel_record(&s, *id).frame_node_id.0).collect();
    assert_eq!(frames_before, frames_after, "a re-layout keeps every PanelId and frame_node_id");
    assert_eq!(page_file(&s, page).panels.iter().map(|p| p.id.0).collect::<Vec<u64>>(), panels, "and the page still lists them in order");
    assert_eq!(page_file(&s, page).page.reading_order.iter().map(|p| p.0).collect::<Vec<u64>>(), panels, "and the reading order is unchanged");
}

// ---------- 3. place, clip, crop ----------

/// An image is placed and clipped in a panel, and cropped with `object.cropImage` (M1 §1 test 3).
#[test]
fn acceptance_3_artwork_is_placed_clipped_and_cropped() {
    let (mut s, root) = project("t3");
    let page = new_page(&mut s, json!({}));
    let panel = add_panel(&mut s, page, [50.0, 60.0, 300.0, 220.0]);
    let asset = add_linked_png(&mut s, &root, "assets/plate.png", [200, 40, 40]);

    let placed = exec(&mut s, "comic.panel.replace_art", json!({"panel": panel, "asset": asset}));
    let group = placed["node"].as_u64().expect("the clip group");
    let image = placed["image"].as_u64().expect("the placed image");

    // Clipped: the record names a clip group whose first child is a path copying the panel frame.
    let record = panel_record(&s, panel);
    let frame = record.frame_node_id.0;
    let artwork = record.artwork_refs.first().expect("an artwork reference").clone();
    assert_eq!(artwork.asset_id.0, asset);
    assert_eq!(artwork.node_id.0, group, "the clip group is what the record names");
    let NodeKind::Group { children, clip: true } = &node(&s, group).kind else {
        panic!("{group} is not a clip group");
    };
    let clip_path = children.first().expect("the clipping path").clone();
    assert!(matches!(clip_path.kind, NodeKind::Path { clipping: true, .. }), "the first child clips");
    assert_eq!(path_of(&s, clip_path.id.0), path_of(&s, frame), "the clipping path copies the panel's frame");
    assert!(children.iter().any(|c| c.id.0 == image), "the image is inside the clip group");
    assert!(node_ids(&s).contains(&image), "the placed image is a real document object");
    assert_eq!(errors_of(&validate(&mut s)), 0, "the placed, clipped artwork validates");

    // Cropped with `object.cropImage`, whose rect is in document space (M0 §4a finding H1).
    let b = bounds(&s, image);
    let frame_b = bounds(&s, frame);
    assert!(
        b[0] >= frame_b[0] - 1e-6 && b[1] >= frame_b[1] - 1e-6 && b[2] <= frame_b[2] + 1e-6 && b[3] <= frame_b[3] + 1e-6,
        "the placed image is fitted inside the panel frame: {b:?} in {frame_b:?}"
    );
    assert!(b[2] - b[0] > 1.0 && b[3] - b[1] > 1.0, "and it is a real image, not a degenerate one");
    let cropped = exec(&mut s, "object.cropImage", json!({"rect": [b[0] + 20.0, b[1] + 20.0, 32.0, 32.0]}));
    assert_eq!(cropped["id"].as_u64(), Some(image), "the crop keeps the node id");
    let after = bounds(&s, image);
    let (w, h) = (after[2] - after[0], after[3] - after[1]);
    assert!(w < b[2] - b[0], "the image is smaller than it was: {w} < {}", b[2] - b[0]);
    assert!(w <= 32.0 + 1e-9 && h <= 32.0 + 1e-9, "and it is cropped inside the requested rectangle: {w}x{h}");
    assert!(w > 0.0 && h > 0.0, "and is not degenerate");
    assert_eq!(bucket(w), bucket(h), "a square crop stays square (the crop keeps whole source pixels)");
    // The crop snaps to whole source pixels, so the top-left is within one source pixel of the rect.
    let pixel_pt = (b[2] - b[0]) / 64.0;
    assert!((after[0] - (b[0] + 20.0)).abs() <= pixel_pt + 1e-9, "the crop's place is the rect, to a source pixel");
    assert!((after[1] - (b[1] + 20.0)).abs() <= pixel_pt + 1e-9);
    // The clip is still the panel's frame, so the crop shows through the panel unchanged.
    assert_eq!(path_of(&s, clip_path.id.0), path_of(&s, frame));
    assert_eq!(panel_record(&s, panel).artwork_refs, vec![artwork], "and the record's artwork reference is unchanged");
    assert_eq!(errors_of(&validate(&mut s)), 0, "and the cropped panel still validates");
}

// ---------- 4. move and resize on the canvas ----------

/// The user moves and resizes a panel on the canvas; the panel id and its artwork, dialogue and
/// reading order are unchanged (M1 §1 test 4).
#[test]
fn acceptance_4_move_and_resize_keep_the_panel_identity_art_letters_and_order() {
    let (mut s, root) = project("t4");
    let page = new_page(&mut s, json!({}));
    let a = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    let b = add_panel(&mut s, page, [40.0, 520.0, 240.0, 200.0]);
    let asset = add_linked_png(&mut s, &root, "assets/a.png", [10, 20, 30]);
    let placed = exec(&mut s, "comic.panel.replace_art", json!({"panel": a, "asset": asset}));
    let image = placed["image"].as_u64().expect("the image");
    let balloon = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": a, "kind": "speech", "text": "Hold still."}));
    let lettering = balloon["lettering"].as_u64().expect("the lettering");
    let dialogue = balloon["dialogue"].as_u64().expect("the dialogue");
    let frame = panel_record(&s, a).frame_node_id.0;

    let before = panel_record(&s, a);
    let order_before: Vec<u64> = page_file(&s, page).page.reading_order.iter().map(|p| p.0).collect();
    let image_before = bounds(&s, image);
    let balloon_before = bounds(&s, lettering_record(&s, lettering).frame_node_id.0);

    // `comic.panel.move` translates the frame **and the objects the panel owns**, so the crop the
    // clip makes is preserved (design §8.2).
    exec(&mut s, "comic.panel.move", json!({"panel": a, "dx": 60.0, "dy": 25.0}));
    assert_eq!(panel_record(&s, a), before, "a move changes no field of the record");
    assert_eq!(page_file(&s, page).page.reading_order.iter().map(|p| p.0).collect::<Vec<u64>>(), order_before);
    let frame_moved = bounds(&s, frame);
    assert_eq!([bucket(frame_moved[0] - 40.0), bucket(frame_moved[1] - 40.0)], [60.0, 25.0], "the frame moved by the delta");
    let image_moved = bounds(&s, image);
    assert_eq!([bucket(image_moved[0] - image_before[0]), bucket(image_moved[1] - image_before[1])], [60.0, 25.0], "and the artwork went with it");
    let balloon_moved = bounds(&s, lettering_record(&s, lettering).frame_node_id.0);
    assert_eq!([bucket(balloon_moved[0] - balloon_before[0]), bucket(balloon_moved[1] - balloon_before[1])], [60.0, 25.0], "and so did the balloon");

    // `comic.panel.resize` changes the frame only, which reveals or hides more of the same image.
    exec(&mut s, "comic.panel.resize", json!({"panel": a, "width": 300.0, "height": 260.0}));
    let resized = bounds(&s, frame);
    assert_eq!([bucket(resized[2] - resized[0]), bucket(resized[3] - resized[1])], [300.0, 260.0]);
    assert_eq!(bounds(&s, image), image_moved, "a resize does not move the artwork");
    assert_eq!(panel_record(&s, a), before, "and it changes no field of the record");
    assert_eq!(page_file(&s, page).page.reading_order.iter().map(|p| p.0).collect::<Vec<u64>>(), order_before);
    assert_eq!(dialogue_ids(&s, a), vec![dialogue], "the dialogue is unchanged");
    assert_eq!(panel_record(&s, b).id.0, b, "and the other panel is untouched");

    // The plain canvas path: an ordinary document transform of the frame (no comic command at all)
    // also leaves the record alone, because the record holds ids and meaning, never a rectangle.
    exec(&mut s, "object.transform", json!({"ids": [frame], "matrix": [1.0, 0.0, 0.0, 1.0, -12.0, 7.0]}));
    assert_eq!(panel_record(&s, a), before, "a canvas transform changes no field of the record either");
    assert_eq!(bucket(bounds(&s, frame)[0]), bucket(resized[0] - 12.0), "the document geometry did move");
    let v = validate(&mut s);
    assert_eq!(errors_of(&v), 0, "and the page still validates: {}", v["findings"]);
}

// ---------- 5. dialogue update ----------

/// `comic.dialogue.update` changes dialogue without touching artwork or typography (M1 §1 test 5).
#[test]
fn acceptance_5_dialogue_update_touches_no_artwork_and_no_typography() {
    let (mut s, root) = project("t5");
    let page = new_page(&mut s, json!({}));
    let panel = add_panel(&mut s, page, [40.0, 40.0, 300.0, 220.0]);
    let asset = add_linked_png(&mut s, &root, "assets/p.png", [7, 8, 9]);
    let placed = exec(&mut s, "comic.panel.replace_art", json!({"panel": panel, "asset": asset}));
    let image = placed["image"].as_u64().expect("the image");
    let group = placed["node"].as_u64().expect("the clip group");
    let balloon = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": panel, "kind": "speech", "text": "First words."}));
    let lettering = balloon["lettering"].as_u64().expect("the lettering");
    let dialogue = balloon["dialogue"].as_u64().expect("the dialogue");
    let text_node = balloon["text_node"].as_u64().expect("the text object");

    let l_before = lettering_record(&s, lettering);
    let frame_path = path_of(&s, l_before.frame_node_id.0);
    let tail = l_before.tail_node_id.expect("a speech balloon has a tail");
    let tail_path = path_of(&s, tail.0);
    let text_before = node(&s, text_node).clone();
    let art_before = panel_record(&s, panel).artwork_refs.clone();
    let group_before = node(&s, group).clone();
    let image_before = node(&s, image).clone();

    let out = exec(&mut s, "comic.dialogue.update", json!({"dialogue": dialogue, "text": "Second words.", "approval": "approved"}));

    assert_eq!(dialogue_record(&s, dialogue).text, "Second words.", "the record owns the words");
    assert_eq!(dialogue_record(&s, dialogue).approval_status, ApprovalStatus::Approved);
    assert_eq!(shown_text(&s, text_node), "Second words.", "and the page shows them (rule 8: one source of the words)");

    // The command's report is checked against the **artefacts**, not taken as a literal (R1-4): what
    // it says it did not touch is what the record, the frame and the tail must show is unchanged. The
    // field-by-field comparisons below are the real check; this ties the report to them, so a command
    // that moved the balloon while reporting `geometryTouched: false` reds here too.
    let l_after = lettering_record(&s, lettering);
    let typography_touched = l_after.text_layout_settings != l_before.text_layout_settings || l_after.overflow_policy != l_before.overflow_policy;
    let geometry_touched = path_of(&s, l_after.frame_node_id.0) != frame_path || path_of(&s, tail.0) != tail_path;
    assert_eq!(out["typographyTouched"], json!(typography_touched), "typographyTouched is what the record shows");
    assert_eq!(out["geometryTouched"], json!(geometry_touched), "geometryTouched is what the frame and the tail show");

    // Typography: the record's layout settings and the text object's own style are untouched.
    assert_eq!(l_after.text_layout_settings, l_before.text_layout_settings, "no TextLayoutSettings changed");
    assert_eq!(l_after.overflow_policy, l_before.overflow_policy);
    let mut text_after = node(&s, text_node).clone();
    // The words are expected to differ; everything else about the text object must not. The
    // `cached_bounds`/`cached_baselines` pair is the layout engine's cache *of the words*, which the
    // command refreshes on purpose; it is not typography.
    let mut before_copy = text_before.clone();
    if let (NodeKind::Text(a), NodeKind::Text(b)) = (&mut text_after.kind, &mut before_copy.kind) {
        a.runs = Vec::new();
        b.runs = Vec::new();
        a.cached_bounds = None;
        b.cached_bounds = None;
        a.cached_baselines = Vec::new();
        b.cached_baselines = Vec::new();
    }
    assert_eq!(text_after, before_copy, "the text object differs only in its words");
    // Geometry: frame, tail and the artwork are exactly as they were.
    assert_eq!(path_of(&s, l_after.frame_node_id.0), frame_path, "the balloon frame path is unchanged");
    assert_eq!(bounds(&s, l_after.frame_node_id.0), bounds(&s, l_before.frame_node_id.0));
    assert_eq!(path_of(&s, tail.0), tail_path, "the tail path is unchanged");
    assert_eq!(panel_record(&s, panel).artwork_refs, art_before, "no artwork reference changed");
    assert_eq!(node(&s, group), group_before, "the clip group is unchanged");
    assert_eq!(node(&s, image), image_before, "the placed image is unchanged");
}

// ---------- 6. replace one panel's artwork ----------

/// `comic.panel.replace_art` replaces one panel image and leaves every other object, and the
/// previous asset, intact (M1 §1 test 6).
#[test]
fn acceptance_6_replace_art_leaves_every_other_object_and_the_previous_asset_intact() {
    let (mut s, root) = project("t6");
    let page = new_page(&mut s, json!({}));
    let a = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    let b = add_panel(&mut s, page, [40.0, 280.0, 240.0, 200.0]);
    let asset_a = add_linked_png(&mut s, &root, "assets/a.png", [200, 0, 0]);
    let asset_b = add_linked_png(&mut s, &root, "assets/b.png", [0, 200, 0]);
    let asset_c = add_linked_png(&mut s, &root, "assets/c.png", [0, 0, 200]);
    let first = exec(&mut s, "comic.panel.replace_art", json!({"panel": a, "asset": asset_a}));
    exec(&mut s, "comic.panel.replace_art", json!({"panel": b, "asset": asset_b}));
    let balloon = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": b, "kind": "narration", "text": "Later."}));
    let caption = balloon["lettering"].as_u64().expect("the caption");

    let old_group = first["node"].as_u64().expect("the old clip group");
    let b_ref = panel_record(&s, b).artwork_refs.clone();
    let nodes_before: BTreeSet<u64> = node_ids(&s);
    let assets_before: BTreeSet<u64> = open_comic(&s).bundle.assets.assets.iter().map(|x| x.id.0).collect();

    let second = exec(&mut s, "comic.panel.replace_art", json!({"panel": a, "asset": asset_c}));
    let new_group = second["node"].as_u64().expect("the new clip group");
    assert_eq!(second["previous"].as_u64(), Some(old_group), "it reports what it replaced");
    let assets_after: BTreeSet<u64> = open_comic(&s).bundle.assets.assets.iter().map(|x| x.id.0).collect();
    // `asset_kept` is the command's report; the registry is what it reports, and the replaced asset's
    // record is still in it (design §11.1: lineage outlives the panel). Tying the one to the other
    // is what makes the line able to fail (R1-4) — as a literal `true` it could not.
    assert_eq!(
        second["asset_kept"],
        json!(assets_after.contains(&asset_a)),
        "the report is what the registry shows: the replaced asset's record survives"
    );

    // The replaced subtree is gone; everything else is still there.
    let after: BTreeSet<u64> = node_ids(&s);
    let added: BTreeSet<u64> = after.difference(&nodes_before).copied().collect();
    let removed: BTreeSet<u64> = nodes_before.difference(&after).copied().collect();
    assert!(removed.contains(&old_group), "the old clip group was deleted: {removed:?}");
    assert!(!added.contains(&old_group), "and the id was not reused");
    assert!(added.contains(&new_group), "the new clip group is in the document");
    assert!(after.contains(&b_ref[0].node_id.0), "panel b's clip group survived");
    assert_eq!(panel_record(&s, b).artwork_refs, b_ref, "and so did its artwork reference");
    assert_eq!(panel_record(&s, a).artwork_refs.len(), 1, "panel a has exactly one piece of artwork");
    assert_eq!(panel_record(&s, a).artwork_refs[0].node_id.0, new_group);
    assert!(after.contains(&lettering_record(&s, caption).frame_node_id.0), "the caption survived");

    // **The previous asset is intact**: the record is not deleted (design §11.1: lineage outlives
    // the panel), and the new one is registered too.
    assert_eq!(assets_after, assets_before, "no Asset record was added or removed by replacing art");
    assert!(assets_after.contains(&asset_a), "the replaced asset's record survives");
    assert_eq!(errors_of(&validate(&mut s)), 0, "and the page still validates");
}

// ---------- 7. balloons and a caption ----------

/// Two speech balloons and one caption exist as `Lettering` records whose text is editable and whose
/// frames are editable paths (M1 §1 test 7) — and so does the **sound-effect object** M1 §1 also
/// requires, the one kind whose tail rules are the tightest (comic-layout's `SoundEffect` style is
/// the only non-spoken kind with a `Burst` shape and no tail at all).
#[test]
fn acceptance_7_two_balloons_and_a_caption_are_lettering_records_with_editable_text_and_frames() {
    let (mut s, _root) = project("t7");
    let page = new_page(&mut s, json!({}));
    let a = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    let b = add_panel(&mut s, page, [40.0, 280.0, 240.0, 200.0]);
    let one = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": a, "kind": "speech", "text": "Balloon one."}));
    let two = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": b, "kind": "speech", "text": "Balloon two."}));
    let cap = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": a, "kind": "narration", "text": "Meanwhile\u{2026}"}));
    let sfx = exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": b, "kind": "sound_effect", "text": "KRAK!"}));
    let records: Vec<(u64, LetteringKind, u64, Value, &str)> = vec![
        (one["lettering"].as_u64().unwrap(), LetteringKind::Speech, a, one.clone(), "Balloon one."),
        (two["lettering"].as_u64().unwrap(), LetteringKind::Speech, b, two.clone(), "Balloon two."),
        (cap["lettering"].as_u64().unwrap(), LetteringKind::Narration, a, cap.clone(), "Meanwhile\u{2026}"),
        (sfx["lettering"].as_u64().unwrap(), LetteringKind::SoundEffect, b, sfx.clone(), "KRAK!"),
    ];

    for (id, kind, panel, out, words) in &records {
        let l = lettering_record(&s, *id);
        assert_eq!(l.kind, *kind, "lettering {id}");
        assert_eq!(l.panel_id.0, *panel);
        // The frame is an editable document path (design §8: the plan's geometry is written as
        // ordinary editable objects, never a flat image).
        let frame = node(&s, l.frame_node_id.0);
        let NodeKind::Path { clipping, guide, .. } = frame.kind else {
            panic!("lettering {id}'s frame is not a path: {:?}", frame.kind);
        };
        assert!(!clipping && !guide, "it is an ordinary path, not a clipping or guide path");
        let path = path_of(&s, l.frame_node_id.0);
        assert!(path.anchor_count() >= 3, "a balloon outline has an outline: {}", path.anchor_count());
        assert!(path.subpaths.iter().any(|sp| sp.closed), "and it is closed");
        // Rule 8: the words have exactly one home.
        match kind {
            LetteringKind::Speech => {
                assert!(l.dialogue_id.is_some(), "a spoken kind's words are the linked Dialogue's");
                assert!(l.text.is_none());
            }
            _ => {
                assert!(l.dialogue_id.is_none(), "{kind:?} carries its own words");
                assert_eq!(l.text.as_deref(), Some(*words), "and they are this record's, not a dialogue's");
            }
        }
        assert!(out["frame"].as_u64().is_some() && out["text_node"].as_u64().is_some());
    }
    assert_eq!(page_file(&s, page).lettering.len(), 4, "four Lettering records");
    assert_eq!(page_file(&s, page).dialogue.len(), 2, "and a Dialogue only for the spoken kinds (rule 8)");

    // The sound effect is the kind whose tail rules are the tightest: a tail is what a *spoken* kind
    // points at, so this kind has none and the command reports none (comic-layout's SoundEffect style:
    // `tail: None`). Nothing else in the twelve rows exercises it.
    let sfx_record = lettering_record(&s, records[3].0);
    assert_eq!(sfx_record.kind, LetteringKind::SoundEffect, "M1 §1's basic sound-effect object");
    assert!(sfx_record.tail_node_id.is_none(), "a sound effect has no tail");
    assert!(records[3].3["tail"].is_null(), "and the command reports no tail for it: {}", records[3].3);
    assert_eq!(sfx_record.panel_id.0, b, "it belongs to its panel");
    assert_eq!(shown_text(&s, records[3].3["text_node"].as_u64().unwrap()), "KRAK!", "and its words are on the page");

    // Editable text: a spoken balloon's words change through the dialogue, the caption's and the
    // sound effect's through the lettering record itself — where rule 8 puts them.
    let dialogue = lettering_record(&s, records[0].0).dialogue_id.expect("the dialogue");
    let text_node = records[0].3["text_node"].as_u64().unwrap();
    exec(&mut s, "comic.dialogue.update", json!({"dialogue": dialogue.0, "text": "Balloon one, edited."}));
    assert_eq!(shown_text(&s, text_node), "Balloon one, edited.");
    let cap_text_node = cap["text_node"].as_u64().unwrap();
    exec(&mut s, "comic.lettering.set_text", json!({"lettering": records[2].0, "text": "Edited caption."}));
    assert_eq!(lettering_record(&s, records[2].0).text.as_deref(), Some("Edited caption."));
    assert_eq!(shown_text(&s, cap_text_node), "Edited caption.");
    let sfx_text_node = records[3].3["text_node"].as_u64().unwrap();
    exec(&mut s, "comic.lettering.set_text", json!({"lettering": records[3].0, "text": "KRAKOOM!"}));
    assert_eq!(lettering_record(&s, records[3].0).text.as_deref(), Some("KRAKOOM!"), "the sound effect's words are editable");
    assert_eq!(shown_text(&s, sfx_text_node), "KRAKOOM!", "and the page shows the new ones");

    // Editable frame: the outline is a document path, so an ordinary transform edits it and moves
    // nothing semantic.
    let frame_id = lettering_record(&s, records[1].0).frame_node_id.0;
    let before = bounds(&s, frame_id);
    let text_before = shown_text(&s, records[1].3["text_node"].as_u64().unwrap());
    exec(&mut s, "object.transform", json!({"ids": [frame_id], "matrix": [1.0, 0.0, 0.0, 1.0, 9.0, -4.0]}));
    assert_eq!(bucket(bounds(&s, frame_id)[0]), bucket(before[0] + 9.0), "the outline moved");
    assert_eq!(lettering_record(&s, records[1].0).frame_node_id.0, frame_id, "the id is stable");
    assert_eq!(shown_text(&s, records[1].3["text_node"].as_u64().unwrap()), text_before);
}

// ---------- 8. undo and redo ----------

/// Undo and redo each of the above as a single step (M1 §1 test 8).
#[test]
fn acceptance_8_every_operation_is_exactly_one_undo_and_redo_step() {
    let (mut s, root) = project("t8");
    let asset = add_linked_png(&mut s, &root, "assets/x.png", [1, 2, 3]);

    let page = new_page(&mut s, json!({}));
    let a = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    one_step(&mut s, "comic.page.create", json!({})); // a second page, so reorder has something to do
    one_step(&mut s, "comic.panel.add", json!({"page": page, "rect": [300.0, 40.0, 240.0, 440.0]}));
    one_step(&mut s, "comic.panel.set_shot", json!({"panel": a, "shot": "Wide establishing shot, dusk."}));
    one_step(&mut s, "comic.panel.move", json!({"panel": a, "dx": 12.0, "dy": -6.0}));
    one_step(&mut s, "comic.panel.resize", json!({"panel": a, "width": 250.0, "height": 210.0}));
    one_step(&mut s, "comic.panel.replace_art", json!({"panel": a, "asset": asset}));
    one_step(&mut s, "comic.lettering.add", json!({"page": page, "panel": a, "kind": "speech", "text": "Hello."}));
    one_step(&mut s, "comic.lettering.add", json!({"page": page, "panel": a, "kind": "narration", "text": "Later."}));
    let lettering = page_file(&s, page).lettering[0].id.0;
    let dialogue = page_file(&s, page).lettering[0].dialogue_id.expect("a speech balloon").0;
    one_step(&mut s, "comic.lettering.set_text", json!({"lettering": lettering, "text": "Hello, again."}));
    one_step(&mut s, "comic.lettering.set_style", json!({"lettering": lettering, "size": 14.0, "align": "center"}));
    one_step(&mut s, "comic.dialogue.update", json!({"dialogue": dialogue, "text": "Hello, once more."}));
    let reversed: Vec<u64> = page_file(&s, page).panels.iter().map(|p| p.id.0).rev().collect();
    one_step(&mut s, "comic.reading_order.set", json!({"page": page, "order": reversed}));
    let page2 = pages_of(&s).iter().map(|(id, _)| id.0).find(|id| *id != page).expect("the second page");
    one_step(&mut s, "comic.page.template.apply", json!({"page": page2, "template": "two-tier"}));
    one_step(&mut s, "comic.page.reorder", json!({"page": page2, "to": 0}));
    one_step(&mut s, "comic.panel.delete", json!({"panel": a, "reuse": [dialogue]}));
    one_step(&mut s, "comic.page.delete", json!({"page": page}));
    one_step(&mut s, "comic.page.create", json!({}));
}

// ---------- 9. save, close, reopen, validate ----------

/// The bundle is saved, closed, reopened and validated, and the page matches the saved layout, with
/// every id preserved (M1 §1 test 9).
#[test]
fn acceptance_9_save_close_reopen_keeps_every_id_and_the_saved_layout() {
    let (mut s, root) = project("t9");
    let page = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [300.0, 400.0]}));
    let a = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    let b = add_panel(&mut s, page, [300.0, 40.0, 240.0, 440.0]);
    let asset = add_linked_png(&mut s, &root, "assets/art.png", [90, 120, 30]);
    exec(&mut s, "comic.panel.replace_art", json!({"panel": a, "asset": asset}));
    exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": b, "kind": "speech", "text": "Saved."}));
    exec(&mut s, "comic.lettering.add", json!({"page": page, "panel": b, "kind": "narration", "text": "A caption."}));
    exec(&mut s, "comic.page.template.apply", json!({"page": second, "template": "four-tier"}));
    exec(&mut s, "comic.page.reorder", json!({"page": second, "to": 0}));

    let ids_before: BTreeSet<u64> = node_ids(&s);
    let bundle_before = open_comic(&s).bundle.clone();
    let frames_before: BTreeMap<u64, [f64; 4]> =
        bundle_before.pages.values().flat_map(|f| f.panels.iter()).map(|p| (p.id.0, bounds(&s, p.frame_node_id.0))).collect();
    let order_before: Vec<u64> = bundle_before.pages[&PageId(page)].page.reading_order.iter().map(|p| p.0).collect();
    let ids = |v: &BTreeMap<PageId, vectorcraft_comic_model::PageFile>, pick: fn(&vectorcraft_comic_model::PageFile) -> Vec<u64>| {
        v.values().flat_map(&pick).collect::<Vec<u64>>()
    };
    let bundle_ids = |b: &ComicBundle| {
        (
            ids(&b.pages, |f| f.panels.iter().map(|p| p.id.0).collect()),
            ids(&b.pages, |f| f.lettering.iter().map(|l| l.id.0).collect()),
            ids(&b.pages, |f| f.dialogue.iter().map(|d| d.id.0).collect()),
        )
    };
    let ids_before_records = bundle_ids(&bundle_before);

    let saved = exec(&mut s, "comic.project.save", json!({}));
    let written: Vec<String> = saved["files"].as_array().expect("files").iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(written.iter().any(|f| f.ends_with(".vectorcraft")), "the save wrote the document: {written:?}");
    let content_before = file_contents(&root, &["manifest.json", "project.json"]);

    // Closed, then reopened from disk.
    assert!(s.close_document(0), "the document closes");
    assert!(s.doc().is_err(), "and there is no open document");
    exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));

    // Every id preserved (design §5.3.1 mitigation 3: ids are the comic's identity).
    let bundle_after = open_comic(&s).bundle.clone();
    assert_eq!(bundle_after.manifest.bundle_id, bundle_before.manifest.bundle_id);
    assert_eq!(bundle_after.project.id, bundle_before.project.id);
    assert_eq!(bundle_after.pages.keys().map(|k| k.0).collect::<Vec<u64>>(), bundle_before.pages.keys().map(|k| k.0).collect::<Vec<u64>>());
    assert_eq!(bundle_ids(&bundle_after), ids_before_records, "every PanelId, LetteringId and DialogueId survives the round trip");
    assert_eq!(pages_of(&s), bundle_before.pages.iter().map(|(id, f)| (*id, f.page.artboard_id)).collect::<Vec<_>>());

    // The page matches the saved layout: every document node, every frame rectangle, the order.
    assert_eq!(node_ids(&s), ids_before, "the document that came back is the document that was saved");
    let frames_after: BTreeMap<u64, [f64; 4]> =
        bundle_after.pages.values().flat_map(|f| f.panels.iter()).map(|p| (p.id.0, bounds(&s, p.frame_node_id.0))).collect();
    assert_eq!(frames_after, frames_before, "every panel frame is where it was saved");
    assert_eq!(page_file(&s, page).page.reading_order.iter().map(|p| p.0).collect::<Vec<u64>>(), order_before);

    // Validated: a clean bundle.
    assert_eq!(errors_of(&validate(&mut s)), 0, "the reopened bundle validates");

    // And a reopen does not rewrite the bundle on disk.
    assert_eq!(file_contents(&root, &["manifest.json", "project.json"]), content_before, "a reopen does not touch the bundle on disk");
}

// ---------- 10. render and PDF export ----------

/// Page render produces the expected page dimensions, and PDF export produces one page per artboard
/// with correct reading order (M1 §1 test 10).
#[test]
fn acceptance_10_render_dimensions_and_one_pdf_page_per_artboard_in_reading_order() {
    let (mut s, root) = project("t10");
    let one = new_page(&mut s, json!({}));
    let two = new_page(&mut s, json!({"size": [300.0, 400.0], "template": "two-tier"}));
    let three = new_page(&mut s, json!({"size": [450.0, 250.0]}));
    let panel = add_panel(&mut s, one, [40.0, 40.0, 240.0, 200.0]);
    exec(&mut s, "comic.lettering.add", json!({"page": one, "panel": panel, "kind": "narration", "text": "Page one."}));

    // The scene reads in creation order; move page 1 to position 1 so the comic's order is
    // [two, one, three] and the three page sizes are all different.
    let reordered = exec(&mut s, "comic.page.reorder", json!({"page": one, "to": 1}));
    assert_eq!(reordered["scene_order"], json!([two, one, three]), "the scene reads two, one, three");

    let book: Vec<(u64, [f64; 2])> = vec![(two, [300.0, 400.0]), (one, [612.0, 792.0]), (three, [450.0, 250.0])];
    let artboards: Vec<u32> = book.iter().map(|(p, _)| page_file(&s, *p).page.artboard_id).collect();
    // The **document** is what a render or a PDF export walks, so the order is read from it, not from
    // the command's own report (R1-5): a `reorder` that reported the right order and wrote the same
    // artboards in creation order passes the second line but reds the first.
    assert_eq!(artboard_ids(&s), artboards, "comic.page.reorder put the document's artboards in book order");
    assert_eq!(reordered["artboards"], json!(artboard_ids(&s)), "and the order it reports is the order the document holds");

    // **Render**: one PNG per artboard, at 72 ppi the artboard's size in points. `artboards` is
    // 0-based (encode.rs `ARTBOARD_PARAMS`) and indexes the document's list, which `comic.page.reorder`
    // has just put in book order.
    for (i, (page, size)) in book.iter().enumerate() {
        let png = root.join(format!("page-{page}.png"));
        exec(&mut s, "document.export", json!({"format": "png", "path": png.to_string_lossy(), "artboards": [i]}));
        let img = image::open(&png).expect("the rendered png opens").to_rgba8();
        assert_eq!([f64::from(img.width()), f64::from(img.height())], *size, "page {page}'s render is the page's dimensions");
    }

    // **Export**: one PDF page per artboard, in the comic's reading order, each the page's size.
    let pdf_path = root.join("book.pdf");
    exec(&mut s, "document.exportPdf", json!({"path": pdf_path.to_string_lossy()}));
    let bytes = std::fs::read(&pdf_path).expect("the pdf is written");
    let info = vectorcraft_pdf::info(&bytes, None).expect("the pdf parses");
    assert_eq!(info.pages.len(), 3, "one page per artboard");
    for (i, (page, size)) in book.iter().enumerate() {
        let got = [bucket(info.pages[i].width), bucket(info.pages[i].height)];
        assert_eq!(got, *size, "PDF page {} is page {page} (artboard {}), in the comic's reading order", i + 1, artboards[i]);
    }
}

// ---------- 10b. what `comic.page.reorder` does to the document's artboards ----------

/// The second half of test 10's "correct reading order": `comic.page.reorder` is what makes it
/// meaningful, and what it must do is put the document's **artboards** in book order — the order a
/// render or a PDF export walks — while moving nothing on the canvas. Design §5.1.1: "Pages are
/// ordered by an explicit comic `reading_order`, never by artboard position on the canvas. Artboard
/// order in the document is presentation, not narrative." The reorder is the one place the two are
/// made to agree.
#[test]
fn acceptance_10b_page_reorder_orders_the_artboards_and_moves_nothing_on_the_canvas() {
    let (mut s, _root) = project("t10b");
    let sizes = [[612.0, 792.0], [300.0, 400.0], [450.0, 250.0], [200.0, 500.0]];
    let pages: Vec<u64> = sizes.iter().map(|sz| new_page(&mut s, json!({"size": sz}))).collect();
    let artboard_of = |s: &Session, page: u64| page_file(s, page).page.artboard_id;
    let ids: Vec<u32> = pages.iter().map(|p| artboard_of(&s, *p)).collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);

    // A spare artboard no page names, as `artboard.new` leaves one: the reorder must not disturb it
    // either, and it must stay after the pages rather than being interleaved.
    exec(&mut s, "artboard.new", json!({"x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0}));
    let spare = doc(&s).artboards.last().expect("the new artboard").id;
    let rects_before: BTreeMap<u32, [f64; 4]> = doc(&s).artboards.iter().map(|a| (a.id, [a.rect.x0, a.rect.y0, a.rect.x1, a.rect.y1])).collect();
    assert_eq!(doc(&s).artboards.iter().map(|a| a.id).collect::<Vec<u32>>(), vec![1, 2, 3, 4, spare]);

    // Move page 1 to position 2: the scene reads 2, 3, 1, 4.
    let moved = exec(&mut s, "comic.page.reorder", json!({"page": pages[0], "to": 2}));
    assert_eq!(moved["scene_order"], json!([pages[1], pages[2], pages[0], pages[3]]));
    let wanted: Vec<u32> = vec![ids[1], ids[2], ids[0], ids[3]];
    assert_eq!(moved["artboards"], json!(wanted), "the command reports the artboard order it wrote");
    assert_eq!(
        doc(&s).artboards.iter().map(|a| a.id).collect::<Vec<u32>>(),
        wanted.iter().copied().chain([spare]).collect::<Vec<u32>>(),
        "and the document's artboards really are in book order, with the spare left after them"
    );

    // Nothing moved on the canvas: the artboard rectangles are exactly as they were.
    let rects_after: BTreeMap<u32, [f64; 4]> = doc(&s).artboards.iter().map(|a| (a.id, [a.rect.x0, a.rect.y0, a.rect.x1, a.rect.y1])).collect();
    assert_eq!(rects_after, rects_before, "a reorder is presentation order, not geometry");

    // Every page still resolves to its own artboard, and the sizes are the ones each page asked for.
    for (page, size) in pages.iter().zip(&sizes) {
        let rect = artboard_rect(&s, artboard_of(&s, *page));
        assert_eq!([bucket(rect[2] - rect[0]), bucket(rect[3] - rect[1])], *size, "page {page} kept its size");
    }
    assert_eq!(errors_of(&validate(&mut s)), 0, "and the reordered project validates");

    // Repeating the same move changes nothing, and an out-of-range `to` is refused.
    let again = exec(&mut s, "comic.page.reorder", json!({"page": pages[0], "to": 2}));
    assert_eq!(again["artboards"], moved["artboards"], "the same move twice is the same order");
    assert!(s.execute("comic.page.reorder", &json!({"page": pages[0], "to": 4})).is_err(), "`to` is inside the scene");
    assert_eq!(doc(&s).artboards.iter().map(|a| a.id).collect::<Vec<u32>>(), wanted.iter().copied().chain([spare]).collect::<Vec<u32>>());
}

// ---------- 11. the artboard-id hazard ----------

/// Design §5.3.1's artboard-id hazard, as a **failing-then-passing** test — and mitigation 4 has two
/// allocation paths plus a named-id rule, so this asserts all three.
///
/// `Artboard.id` is `max(id) + 1` with no `fix_next_id()` equivalent, so deleting the highest artboard
/// frees its number. `comic.page.delete` retires the id it removed (mitigation 2), and
/// `comic.page.create` must never hand a page a retired id, "because otherwise deleting a page would
/// permanently break the next page to reuse that number, and rules 4 and 17 would leave it
/// unfixable" (mitigation 4). `claim_artboard` (`crates/engine/src/cmd/comic/page.rs`) reaches an
/// artboard by two paths — the **fast path** (`free_artboards`: an artboard is in the document and no
/// page names it, which is what `artboard.new` can make of a retired number again) and the **loop**
/// (`next_artboard_id` is the only candidate, when nothing is free) — and each has to skip both a
/// retired id and one another page still names.
///
/// Three plausible-but-wrong implementations, applied alone; their raw reds are in
/// `docs/reviews/M1-acceptance.md` §11:
/// 1. keep the loop's check, and let `free_artboards` offer any artboard no page currently names. It
///    reads correctly — "the artboard was removed, so a free one cannot be retired" — and it is wrong,
///    because the ordinary still-registered `artboard.new` gives the retired number back to the
///    document (part 1 below);
/// 2. keep `free_artboards` honest and drop the retirement check from the loop, so nothing steps over
///    the picket when no artboard is free (part 2);
/// 3. keep the retirement check and drop the **named** check from the loop, so a page takes a number
///    another page still resolves to — two pages on one artboard, *and* the rule-4 report that was
///    showing the first page's breakage is deleted (part 3).
///
/// Part 4 is the consequence part 1's failure message asserts and nothing checked: a page that does
/// take a retired id is reported by rule 4, and stays reported across a save and a reopen.
#[test]
fn acceptance_11_a_page_never_takes_a_retired_or_named_artboard_id_from_any_allocation_path() {
    // ---- 1. the fast path: `artboard.new` resurrects the retired number ----
    let (mut s, root) = project("t11");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    let retired_artboard = page_file(&s, second).page.artboard_id;
    assert_eq!(retired_artboard, 2, "the second page takes the highest artboard");

    // Delete the second page: its artboard is removed and its id is retired (mitigation 2).
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert!(retired_artboards(&s).contains(&retired_artboard), "comic.page.delete retires the id it removed (§5.3.1 mitigation 2)");
    assert!(!artboard_ids(&s).contains(&retired_artboard), "and the artboard is gone");
    assert!(!named_artboards(&s).contains(&retired_artboard), "and no page names it");

    // The ordinary command gives the number straight back. It is registered, it is reachable while a
    // project is open, and nothing in M1 retires on its behalf.
    exec(&mut s, "artboard.new", json!({"x": 0.0, "y": 0.0, "width": 200.0, "height": 200.0}));
    assert!(artboard_ids(&s).contains(&retired_artboard), "artboard.new hands the retired number back — this is the hazard, not the fix");
    assert!(!named_artboards(&s).contains(&retired_artboard), "and for the moment no page names it: it is 'free'");

    // Now create a page. It must not take the retired id, whatever the document offers.
    let fresh = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    let fresh_artboard = page_file(&s, fresh).page.artboard_id;

    assert!(
        !retired_artboards(&s).contains(&fresh_artboard),
        "FAIL (§5.3.1 mitigation 4, the fast path): comic.page.create gave page {fresh} the retired artboard \
         {fresh_artboard}, the id the deleted page {second} used. A retired number means \"never again\": rules 4 and 17 \
         make a page that takes one permanently reported and permanently unfixable"
    );
    assert_eq!(fresh_artboard, 3, "so the fast path skips the retired 2 and takes the free 3, not allocating past it");
    assert!(
        !named_artboards(&s).contains(&retired_artboard),
        "and the number stays retired: the spare artboard `artboard.new` made is no page's page"
    );
    assert_eq!(named_artboards(&s).iter().copied().collect::<BTreeSet<u32>>(), BTreeSet::from([1, 3]), "two pages, two artboards");
    assert_eq!(errors_of(&validate(&mut s)), 0, "and nothing is reported, because nothing drifted");
    assert_eq!(page_file(&s, first).page.artboard_id, 1, "the first page is untouched");

    // The same holds after a save and a reopen: the retirement is durable, so the guard must be too.
    exec(&mut s, "comic.project.save", json!({}));
    assert!(s.close_document(0));
    exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));
    assert!(retired_artboards(&s).contains(&retired_artboard), "the retirement survived the round trip");
    let again = new_page(&mut s, json!({"size": [260.0, 260.0]}));
    assert!(
        !retired_artboards(&s).contains(&page_file(&s, again).page.artboard_id),
        "and the reopened bundle's allocation still refuses a retired number"
    );

    // ---- 2. the loop: nothing is free, so the retired number is the next candidate ----
    // The fast path is not the only one: with no free artboard at all, `next_artboard_id` is the only
    // candidate and the loop's own check is the only thing that steps over the retired number.
    let (mut s, _root) = project("t11-loop");
    let first = new_page(&mut s, json!({}));
    assert_eq!(page_file(&s, first).page.artboard_id, 1, "the first page claims the artboard the document already had");
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!(page_file(&s, second).page.artboard_id, 2, "the second page takes the highest artboard");
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert_eq!(artboard_ids(&s), vec![1], "the deleted page's artboard is gone, and 2 is retired");
    assert_eq!(retired_artboards(&s), vec![2]);
    // No `artboard.new` here: 1 is named by the first page, so nothing is free and the only candidate
    // `next_artboard_id` offers is the retired 2.
    let third = exec(&mut s, "comic.page.create", json!({"size": [250.0, 250.0]}));
    let taken = u32::try_from(third["artboard_id"].as_u64().expect("an artboard id")).expect("in range");
    assert!(
        !retired_artboards(&s).contains(&taken),
        "FAIL (§5.3.1 mitigation 4, the loop): the allocation loop handed page {} the retired artboard {taken}, which no \
         page may ever resolve to (rule 4)",
        third["page"]
    );
    assert_eq!(taken, 3, "so the loop steps over the picket and allocates past it");
    assert_eq!(third["skipped_retired_artboards"].as_u64(), Some(1), "and it says what it stepped over");
    assert_eq!(named_artboards(&s), vec![1, 3], "two pages, two artboards, neither of them retired");
    assert_eq!(errors_of(&validate(&mut s)), 0, "and nothing drifted");

    // ---- 3. a number a page still names: `artboard.delete` removes it without retiring it ----
    // Mitigation 4 retires the ids `comic.page.delete` removed, but `artboard.delete` (Window →
    // Artboards, `crates/engine/src/cmd/layer.rs`) removes an artboard *without* retiring it and stays
    // reachable while a project is open. A page whose artboard that command removed still names the
    // number, so it must be skipped even though `retired_artboard_ids` does not mention it.
    let (mut s, _root) = project("t11-named");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!([page_file(&s, first).page.artboard_id, page_file(&s, second).page.artboard_id], [1, 2]);
    assert_eq!(retired_artboards(&s), Vec::<u32>::new(), "nothing is retired yet");
    exec(&mut s, "artboard.delete", json!({"index": 1}));
    assert_eq!(artboard_ids(&s), vec![1], "artboard 2 is gone from the document");
    assert_eq!(named_artboards(&s), vec![1, 2], "but the second page still names it, and it is not retired");
    let broken = validate(&mut s);
    assert!(errors_of(&broken) >= 1, "and the dangling page is already reported: {}", broken["findings"]);

    let third = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    let taken = page_file(&s, third).page.artboard_id;
    assert_ne!(
        taken, 2,
        "FAIL (§5.3.1 mitigation 4, a named id): a page took 2, which the second page still resolves to — one artboard, \
         two pages, and the rule-4 report of the first page's breakage is deleted"
    );
    let named = named_artboards(&s);
    assert_eq!(named.iter().copied().collect::<BTreeSet<u32>>().len(), named.len(), "one artboard per page (design §5.1.1): {named:?}");
    let after = validate(&mut s);
    assert_eq!(errors_of(&after), errors_of(&broken), "and the new page adds no report of its own: {}", after["findings"]);

    // ---- 4. the consequence: a page that does take a retired id is reported, and stays reported ----
    // Part 1's failure message claims the state is "permanently reported and permanently
    // unfixable" (rules 4 and 17). No M1 command can produce it — that is the whole point — so the
    // violation is written into the model by hand, and then the bundle is asked to survive a save and
    // a reopen with it.
    let (mut s, root) = project("t11-consequence");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!(page_file(&s, second).page.artboard_id, 2);
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert_eq!(retired_artboards(&s), vec![2], "2 means never again");
    assert_eq!(errors_of(&validate(&mut s)), 0, "and the project is clean");
    {
        let page = open_comic_mut(&mut s).bundle.pages.get_mut(&PageId(first)).expect("the page file");
        page.page.artboard_id = 2;
    }
    let reported = validate(&mut s);
    let rule_four: Vec<&Value> =
        reported["findings"].as_array().expect("findings").iter().filter(|f| f["rule"] == json!(4) && f["severity"] == json!("error")).collect();
    assert!(!rule_four.is_empty(), "rule 4 reports a retired artboard, and as an error: {}", reported["findings"]);
    assert!(
        rule_four.iter().any(|f| f["message"].as_str().unwrap_or("").contains("retired_artboard_ids")),
        "and it names the list that makes the number retired: {rule_four:?}"
    );

    // It is not transient: the report is written, and a reopen finds it again.
    exec(&mut s, "comic.project.save", json!({}));
    assert!(s.close_document(0), "the document closes");
    exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));
    assert!(
        validate(&mut s)["findings"].as_array().expect("findings").iter().any(|f| f["rule"] == json!(4)),
        "the same rule-4 report comes back with the bundle"
    );

    // And creating a page does not launder it: the retired number is still refused, so the breakage
    // cannot spread to a second page.
    let third = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    assert_ne!(page_file(&s, third).page.artboard_id, 2, "no command can hand out a retired id either");
    assert!(
        validate(&mut s)["findings"].as_array().expect("findings").iter().any(|f| f["rule"] == json!(4)),
        "the page that took it is still reported after another page is created"
    );
}

/// The other half of the ⌘S question: what the bundle says about itself after a document-only save.
///
/// `document.save` writes `document/comic.vectorcraft` and nothing else, so `manifest.json` goes on
/// describing the previous generation of that file — the one §14 step 1 exists to report.
#[test]
fn evidence_cmd_s_leaves_the_manifests_hash_for_the_document_stale_and_a_reopen_says_so() {
    let (mut s, root) = project("cmds-hash");
    let page = new_page(&mut s, json!({}));
    let panel = add_panel(&mut s, page, [40.0, 40.0, 240.0, 200.0]);
    exec(&mut s, "comic.project.save", json!({}));

    let manifest_hash = || -> String {
        let m: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("manifest.json")).expect("JSON");
        m["files"]["document/comic.vectorcraft"]["sha256"].as_str().expect("the document's entry").to_string()
    };
    let file_hash = || vectorcraft_comic_model::sha256_hex(&std::fs::read(root.join("document/comic.vectorcraft")).expect("the document"));
    assert_eq!(manifest_hash(), file_hash(), "the project save wrote the manifest from the tree it wrote");

    // A model-only edit, then ⌘S.
    exec(&mut s, "comic.panel.set_shot", json!({"panel": panel, "shot": "Second brief."}));
    let saved = exec(&mut s, "document.save", json!({}));
    println!("document.save → {}", saved["path"]);
    println!("manifest sha256 {}", manifest_hash());
    println!("file     sha256 {}", file_hash());
    assert_ne!(manifest_hash(), file_hash(), "the manifest now describes the previous generation (§14 step 1)");
    assert!(s.doc().expect("a document").is_dirty(), "and the model is still unsaved work, so ⌘S is not a lie, only incomplete");

    // A reopen reports it — as a warning on a usable project, never as a refusal (§14).
    assert!(s.close_document(0));
    let reopened = exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));
    println!("reopen findings: {}", reopened["findings"]);
    assert_eq!(reopened["pages"].as_u64(), Some(1), "the bundle still opens: §14 refuses only two things");
    let messages: Vec<String> =
        reopened["findings"].as_array().expect("findings").iter().map(|f| f["message"].as_str().unwrap_or("").to_string()).collect();
    assert!(messages.iter().any(|m| m.contains("previous generation")), "§14 step 1 reports the mismatch on open: {messages:?}");

    // And the model edit itself was never written: `comic.project.save` is the only command that does.
    let page_json: Value =
        serde_json::from_slice(&std::fs::read(root.join(format!("pages/page-{page}.json"))).expect("the page file")).expect("JSON");
    assert_eq!(page_json["panels"][0]["shot_description"], json!(""), "⌘S wrote no page file, so the brief on disk is the old one");
    assert_eq!(panel_record(&s, panel).shot_description, "", "and reopening cannot bring back what was never written");
}

// ---------- evidence: `default_page_size: None` ----------

/// The W4 round-2 reviewer left `comic.project.create`'s `pageSize` → `default_page_size` seeding
/// **against a bundle whose `default_page_size` is `None`** unverified. The reader must accept such
/// a bundle (§5.7: a reader accepts any valid JSON) and no command may invent a size for a page.
#[test]
fn evidence_a_bundle_with_no_default_page_size_refuses_to_invent_a_size() {
    let (mut s, root) = project("none");
    let first = new_page(&mut s, json!({}));

    // A bundle that names no default, as a newer ComicCraft or a hand edit can leave it.
    open_comic_mut(&mut s).bundle.project.settings.default_page_size = None;
    exec(&mut s, "comic.project.save", json!({}));
    assert!(s.close_document(0));
    exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));
    assert_eq!(open_comic(&s).bundle.project.settings.default_page_size, None, "the None came back as None");

    // A page with no size is refused by name, not given an invented one.
    let err = s.execute("comic.page.create", &json!({})).expect_err("a page needs a size");
    let message = err.to_string();
    assert!(message.contains("size"), "the refusal names what is missing: {message}");
    assert_eq!(pages_of(&s).len(), 1, "and nothing was created");

    // A page that states its own size is fine, and the size is the artboard's.
    let second = new_page(&mut s, json!({"size": [240.0, 320.0]}));
    let rect = artboard_rect(&s, page_file(&s, second).page.artboard_id);
    assert_eq!([bucket(rect[2] - rect[0]), bucket(rect[3] - rect[1])], [240.0, 320.0]);

    // With no default, rule 15 is silent by design — and `validate` says so in `notChecked` rather
    // than reporting a difference against a size the project never named.
    let v = validate(&mut s);
    assert_eq!(v["errors"], json!(0));
    let not_checked =
        v["notChecked"].as_array().expect("notChecked").iter().map(|e| e["what"].as_str().unwrap().to_string()).collect::<Vec<String>>();
    assert!(not_checked.iter().any(|w| w.contains("page size against a default")), "the silence is named: {not_checked:?}");

    // Reachability, recorded: no M1 command can make an artboard *unusable*, so the warning's `None`
    // branch cannot be reached through the command surface at all — `artboard.setProps` clamps a
    // side at 1 pt (`rect_from`, `crates/engine/src/cmd/layer.rs:729`), and JSON carries no
    // infinity. The test below checks that branch on a document that carries such an artboard.
    exec(&mut s, "artboard.setProps", json!({"index": 0, "width": 0.0, "height": 0.0}));
    let clamped = artboard_rect(&s, page_file(&s, first).page.artboard_id);
    assert_eq!(
        [bucket(clamped[2] - clamped[0]), bucket(clamped[3] - clamped[1])],
        [1.0, 1.0],
        "a commanded artboard side is clamped at 1 pt, so a degenerate artboard is not command-reachable"
    );
}

/// The uncovered case W4's round-2 review named: with `default_page_size: None`, *nothing* reported a
/// page whose artboard makes the geometry check meaningless, because rule 15 returns early and
/// `comic-layout`'s `validate::page` skips the page. `comic.project.validate` now does, and this is
/// the branch that only exists for the `None` case.
#[test]
fn evidence_a_degenerate_artboard_is_reported_when_the_project_names_no_default_size() {
    let (mut s, _root) = project("degenerate");
    let first = new_page(&mut s, json!({}));
    open_comic_mut(&mut s).bundle.project.settings.default_page_size = None;

    // A zero-area artboard: the reader accepts it (§5.7), and no command can produce one (`rect_from`
    // clamps at 1 pt), so it is written into the document directly — the state a hand edit or a
    // future tool leaves, not a state M1 can reach.
    let index = doc(&s).artboards.iter().position(|a| a.id == page_file(&s, first).page.artboard_id).expect("the artboard");
    {
        let st = s.doc_mut().expect("a document");
        Arc::make_mut(&mut st.doc).artboards[index].rect = Rect::new(0.0, 0.0, 0.0, 0.0);
    }

    let v = validate(&mut s);
    let warnings: Vec<String> = v["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["severity"] == json!("warning"))
        .map(|f| f["message"].as_str().unwrap().to_string())
        .collect();
    assert!(
        warnings.iter().any(|m| m.contains("no default_page_size")),
        "the geometry-check warning says rule 15 cannot report it either: {warnings:?}"
    );
    assert!(
        v["notChecked"].as_array().expect("notChecked").iter().any(|e| e["why"].as_str().unwrap_or("").contains("not a usable page")),
        "and 'we could not check this' is data the caller sees next to it"
    );
}
