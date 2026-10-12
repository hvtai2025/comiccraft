//! Tests for the `comic.*` command surface (M1 W4).
//!
//! One test per command, each covering the happy path **and** at least one malformed call, plus the
//! design §11.1 related-entity relationships, the duplicate-page-file refusal, and the artboard-id
//! hazard of design §5.3.1.
//!
//! Two fixtures are built by hand because M1 has no command that could: an `Asset` in the registry
//! (`comic.asset.*` is M4) and a `GenerationJob` in the provenance (`comic.job.*` is M5). They are
//! added to the open bundle's model directly, which is exactly what a W1 fixture does, and every
//! other step goes through `Session::execute`.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::EngineError;
use vectorcraft_comic_model::{AssetId, StorageRef};
use vectorcraft_doc::NodeKind;

// ---------- fixtures ----------

/// A scratch directory inside the workspace's `target/`, never a system temporary directory and
/// never a directory the repository tracks: `target/` is git-ignored, so the `assets` and `brands`
/// gates do not see a test's bundle (a bundle holds a PNG and a document, and an entry in
/// `ASSETS.md` for a test fixture would be noise).
fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-tests").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

/// A session with an open project, created through the command.
fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    s.execute("comic.project.create", &json!({"path": root.to_string_lossy(), "title": "Test Book"})).expect("comic.project.create");
    (s, root)
}

fn page_with_template(tag: &str) -> (Session, PathBuf, u64) {
    let (mut s, root) = project(tag);
    let page = s.execute("comic.page.create", &json!({"template": "three-tier"})).expect("comic.page.create")["page"].as_u64().expect("page id");
    (s, root, page)
}

fn panel_on(s: &mut Session, page: u64, rect: [f64; 4]) -> u64 {
    s.execute("comic.panel.add", &json!({"page": page, "rect": rect})).expect("comic.panel.add")["panel"].as_u64().expect("panel id")
}

fn open_comic(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn open_comic_mut(s: &mut Session) -> &mut OpenComic {
    Arc::make_mut(s.doc_mut().expect("a document").comic.as_mut().expect("an open project"))
}

/// A PNG on disk plus the `Asset` record that names it, allocated through the bundle's counter.
fn add_asset(s: &mut Session, root: &std::path::Path) -> AssetId {
    let relative = "assets/panels/art.png".to_string();
    let path = root.join(&relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("assets dir");
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 40, 40, 255]));
    img.save(&path).expect("write the png");
    let bytes = std::fs::read(&path).expect("read the png");
    let hash = vectorcraft_comic_model::sha256_hex(&bytes);
    let oc = open_comic_mut(s);
    let id = oc.bundle.allocate::<AssetId>().expect("an asset id");
    oc.bundle.assets.assets.push(vectorcraft_comic_model::Asset {
        id,
        asset_type: vectorcraft_comic_model::AssetType::Source,
        storage_reference: StorageRef::Linked { relative_path: relative },
        content_hash: vectorcraft_comic_model::ContentHash { algorithm: "sha256".into(), value: hash },
        media_metadata: vectorcraft_comic_model::MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: None },
        provenance: None,
        parent_asset_ids: Vec::new(),
        approval_status: vectorcraft_comic_model::ApprovalStatus::Pending,
        unknown: Default::default(),
    });
    id
}

/// A job still targeting `panel`, as an interrupted session would leave behind (design §10.3).
fn add_job(s: &mut Session, panel: PanelId) -> vectorcraft_comic_model::JobId {
    let oc = open_comic_mut(s);
    let id = oc.bundle.allocate::<vectorcraft_comic_model::JobId>().expect("a job id");
    let stamp = now();
    oc.bundle.provenance.jobs.push(vectorcraft_comic_model::GenerationJob {
        id,
        operation: vectorcraft_comic_model::JobOperation::TextToImage,
        target_panel_id: Some(panel),
        input_asset_ids: Vec::new(),
        provider: vectorcraft_comic_model::ProviderId("test".into()),
        model: "test".into(),
        request: Value::Null,
        status: vectorcraft_comic_model::JobStatus::Running,
        progress: None,
        error: None,
        result_asset_ids: Vec::new(),
        created: stamp.clone(),
        updated: stamp,
        target_revision: 1,
        consent: None,
        unknown: Default::default(),
    });
    id
}

fn lettering_of(s: &Session, id: u64) -> vectorcraft_comic_model::Lettering {
    open_comic(s).bundle.pages.values().flat_map(|f| f.lettering.iter()).find(|l| l.id.0 == id).cloned().expect("the lettering record")
}

fn panel_record(s: &Session, id: u64) -> vectorcraft_comic_model::Panel {
    open_comic(s).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == id).cloned().expect("the panel record")
}

/// The number of the `Dialogue` a lettering points at.
fn dialogue_of_lettering(s: &Session, lettering: u64) -> u64 {
    lettering_of(s, lettering).dialogue_id.expect("a spoken lettering names a dialogue").0
}

fn dialogue_of(s: &Session, id: u64) -> vectorcraft_comic_model::Dialogue {
    open_comic(s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id.0 == id).cloned().expect("the dialogue record")
}

fn text_of(s: &Session, node: u64) -> String {
    match &s.doc().expect("a document").doc.node(NodeId(node)).expect("the text object").kind {
        NodeKind::Text(t) => t.plain_text(),
        other => panic!("not a type object: {other:?}"),
    }
}

fn bad_params(err: &EngineError) -> bool {
    matches!(err, EngineError::BadParams { .. })
}

// ---------- comic.project.* ----------

#[test]
fn project_create_makes_a_bundle_that_a_page_can_join() {
    let (mut s, root) = project("create");
    assert!(root.join("manifest.json").is_file(), "the manifest is written last");
    assert!(root.join("project.json").is_file());
    assert!(root.join("document/comic.vectorcraft").is_file());
    // M1 acceptance test 1: one document, and one artboard per page.
    assert_eq!(s.doc().expect("a document").doc.artboards.len(), 1, "the document starts with one artboard");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create");
    assert_eq!(page["artboard_id"].as_u64(), Some(1), "the first page claims the document's artboard");
    assert_eq!(s.doc().expect("a document").doc.artboards.len(), 1, "one artboard per page, not two");
    // Malformed: no path, and a path that exists.
    assert!(bad_params(&s.execute("comic.project.create", &json!({})).expect_err("no path")));
    assert!(s.execute("comic.project.create", &json!({"path": root.to_string_lossy()})).is_err(), "an existing path is refused");
}

#[test]
fn project_open_reads_back_what_save_wrote() {
    let (mut s, root, _page) = page_with_template("open");
    let first = s.execute("comic.project.save", &json!({})).expect("comic.project.save");
    assert!(first["files"].as_array().is_some_and(|f| !f.is_empty()), "the save lists what it wrote");

    let mut reopened = Session::new();
    let out = reopened.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open");
    assert_eq!(out["pages"].as_u64(), Some(1));
    assert_eq!(reopened.doc().expect("a document").doc.artboards.len(), 1);
    assert!(bad_params(&reopened.execute("comic.project.open", &json!({})).expect_err("no path")));
    assert!(reopened.execute("comic.project.open", &json!({"path": "no-such-bundle"})).is_err());
}

#[test]
fn project_save_refuses_while_two_files_encode_one_page_id() {
    // The one write the save refuses: `page-7.json` and `page-007.json` encode one id and §3 makes
    // the padding cosmetic, so the two are indistinguishable and which one the next load reads is
    // decided by sort order. The bytes are not at risk — the prune leaves a file the generation of
    // record never wrote — the project is (M1 W1's carried note).
    let (mut s, root) = project("dup");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    s.execute("comic.project.save", &json!({})).expect("the first save");
    let padded = root.join(format!("pages/page-{page:03}.json"));
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &padded).expect("a second file for the same page");
    let err = s.execute("comic.project.save", &json!({})).expect_err("a duplicate page file refuses the save");
    let message = err.to_string();
    assert!(message.contains("both encode id"), "{message}");
    assert!(padded.is_file(), "the refused save left the loser's bytes alone");
    std::fs::remove_file(&padded).expect("clean up");
    s.execute("comic.project.save", &json!({})).expect("and saving works once it is resolved");
}

#[test]
fn project_validate_reports_findings_geometry_and_what_it_cannot_check() {
    let (mut s, root, _page) = page_with_template("validate");
    let asset = add_asset(&mut s, &root);
    let page = open_comic(&s).bundle.pages.keys().next().copied().expect("a page").0;
    let panel = open_comic(&s).bundle.pages.values().next().and_then(|f| f.panels.first()).map(|p| p.id.0).expect("a panel");
    s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": asset.0})).expect("artwork");

    let out = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    assert_eq!(out["errors"].as_u64(), Some(0), "{out}");
    assert_eq!(out["severityErrorBlocksSave"].as_bool(), Some(false), "an Error is data, not a gate");
    assert_eq!(out["geometry"][0]["page"].as_u64(), Some(page), "the artboard rect behind every page is information");
    assert!(out["geometry"][0]["rect"][2].as_f64().is_some_and(|w| w > 0.0));
    assert!(out["notChecked"].as_array().is_some_and(|a| a.len() >= 3), "it says what it cannot check: {out}");
    assert!(
        out["notChecked"].as_array().is_some_and(|a| a.iter().any(|e| e["what"] == "the visual document's own parse")),
        "the format half is named"
    );
    // A query tolerates extra keys (schema §12): it is not a parameter error.
    assert!(s.execute("comic.project.validate", &json!({"findings": 1})).is_ok());
}

// ---------- comic.page.* ----------

#[test]
fn page_create_refuses_without_a_size_and_never_reuses_a_retired_artboard() {
    let (mut s, _root) = project("hazard");
    let first = s.execute("comic.page.create", &json!({})).expect("the first page uses the project's default_page_size");
    assert_eq!(first["artboard_id"].as_u64(), Some(1), "and claims the artboard the document already had");
    assert!(bad_params(&s.execute("comic.page.create", &json!({"size": {"width": -1.0, "height": 10.0}})).expect_err("a bad size")));

    // The artboard-id hazard (design §5.3.1, M1 acceptance test 11): deleting the **highest**
    // artboard frees its number, and the comic layer must never hand it to a page.
    let second = s.execute("comic.page.create", &json!({"size": [200.0, 200.0]})).expect("a second page")["page"].as_u64().expect("page id");
    assert_eq!(open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(second)].page.artboard_id, 2, "the highest artboard");
    let deleted = s.execute("comic.page.delete", &json!({"page": second})).expect("comic.page.delete");
    assert_eq!(deleted["artboard_id"].as_u64(), Some(2), "the deleted page's artboard");
    assert!(open_comic(&s).bundle.manifest.retired_artboard_ids.contains(&2), "the id is remembered (rule 17)");

    let fresh = s.execute("comic.page.create", &json!({"size": [200.0, 200.0]})).expect("a new page");
    assert_eq!(fresh["artboard_id"].as_u64(), Some(3), "the retired number is stepped over, never handed out again");
    assert_eq!(fresh["skipped_retired_artboards"].as_u64(), Some(1));
    // And it survives a save/reopen with the retirement intact.
    s.execute("comic.project.save", &json!({})).expect("save");
    let mut reopened = Session::new();
    reopened.execute("comic.project.open", &json!({"path": open_comic(&s).root.to_string_lossy()})).expect("reopen");
    assert!(open_comic(&reopened).bundle.manifest.retired_artboard_ids.contains(&2));
}

#[test]
fn page_reorder_moves_within_the_scene_and_puts_the_artboards_in_book_order() {
    let (mut s, _root) = project("reorder");
    let a = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page a");
    let b = s.execute("comic.page.create", &json!({"size": [200.0, 200.0]})).expect("a command result")["page"].as_u64().expect("page b");
    let c = s.execute("comic.page.create", &json!({"size": [300.0, 300.0]})).expect("a command result")["page"].as_u64().expect("page c");
    let out = s.execute("comic.page.reorder", &json!({"page": c, "to": 0})).expect("comic.page.reorder");
    assert_eq!(out["scene_order"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()), Some(vec![c, a, b]));
    let artboards: Vec<u32> = s.doc().expect("a document").doc.artboards.iter().map(|a2| a2.id).collect();
    assert_eq!(artboards, vec![3, 1, 2], "the document walks in reading order (M1 acceptance test 10)");
    assert!(bad_params(&s.execute("comic.page.reorder", &json!({"page": a})).expect_err("no `to`")));
    assert!(bad_params(&s.execute("comic.page.reorder", &json!({"page": a, "to": 99})).expect_err("out of range")));
}

#[test]
fn template_apply_creates_then_relays_panels_without_changing_their_ids() {
    let (mut s, _root) = project("template");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let first = s.execute("comic.page.template.apply", &json!({"page": page, "template": "three-tier"})).expect("apply");
    let ids: Vec<u64> = first["panels"].as_array().expect("panels").iter().filter_map(|p| p["panel"].as_u64()).collect();
    assert_eq!(ids.len(), 3, "three-tier places three panels");
    assert_eq!(first["reading_order"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()), Some(ids.clone()));

    // Re-applying another three-panel template keeps the ids and moves the frames.
    let before: Vec<u64> = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.iter().map(|p| p.frame_node_id.0).collect();
    let second = s.execute("comic.page.template.apply", &json!({"page": page, "template": "wide-top"})).expect("wide-top needs three panels");
    let after: Vec<u64> = second["panels"].as_array().expect("panels").iter().filter_map(|p| p["panel"].as_u64()).collect();
    let frames: Vec<u64> = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.iter().map(|p| p.frame_node_id.0).collect();
    assert_eq!(after, ids, "the panels keep their ids");
    assert_eq!(frames, before, "and their frame objects");
    // A different count is refused unless `replace` is asked for.
    assert!(bad_params(
        &s.execute("comic.page.template.apply", &json!({"page": page, "template": "two-tier"})).expect_err("two slots, three panels")
    ));
    s.execute("comic.page.template.apply", &json!({"page": page, "template": "two-tier", "replace": true})).expect("replace");
    assert_eq!(open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.len(), 2);
    assert!(bad_params(&s.execute("comic.page.template.apply", &json!({"page": page})).expect_err("no template")));
    assert!(s.execute("comic.page.template.apply", &json!({"page": page, "template": "nope"})).is_err());
}

// ---------- comic.panel.* ----------

#[test]
fn panel_add_move_and_resize_keep_every_id() {
    let (mut s, _root) = project("panelgeom");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    // M1 acceptance test 2/4: stable ids, a frame that is a real document object.
    let record = panel_record(&s, panel);
    assert!(s.doc().expect("a document").doc.node(record.frame_node_id).is_some(), "the frame is a real object");
    assert_eq!(record.artboard_id, 1);

    let moved = s.execute("comic.panel.move", &json!({"panel": panel, "dx": 10.0, "dy": 20.0})).expect("move");
    assert_eq!(moved["frame"].as_u64(), Some(record.frame_node_id.0));
    let after = panel_record(&s, panel);
    assert_eq!(after.frame_node_id, record.frame_node_id, "the frame id is unchanged");
    assert_eq!(after.artwork_refs.len(), record.artwork_refs.len());
    let bounds = s.doc().expect("a document").doc.node(record.frame_node_id).and_then(|n| n.geometric_bounds()).expect("bounds");
    assert!((bounds.x0 - 50.0).abs() < 1e-9 && (bounds.y0 - 60.0).abs() < 1e-9, "{bounds:?}");

    s.execute("comic.panel.resize", &json!({"panel": panel, "width": 100.0, "height": 100.0})).expect("resize");
    let bounds = s.doc().expect("a document").doc.node(record.frame_node_id).and_then(|n| n.geometric_bounds()).expect("bounds");
    assert!((bounds.width() - 100.0).abs() < 1e-9 && (bounds.height() - 100.0).abs() < 1e-9, "{bounds:?}");
    assert_eq!(panel_record(&s, panel).frame_node_id, record.frame_node_id, "still the same object");

    assert!(bad_params(&s.execute("comic.panel.move", &json!({"panel": panel, "x": 1.0})).expect_err("x without y")));
    assert!(bad_params(&s.execute("comic.panel.resize", &json!({"panel": panel, "rect": [0.0, 0.0, -1.0, 5.0]})).expect_err("negative width")));
    assert!(bad_params(&s.execute("comic.panel.add", &json!({"page": page})).expect_err("no rect")));
    assert!(bad_params(&s.execute("comic.panel.move", &json!({})).expect_err("no panel")));
}

#[test]
fn panel_geometry_is_reported_not_refused() {
    let (mut s, _root) = project("geomreport");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    // A panel far off the artboard and smaller than the minimum: reported, and created.
    let out = s.execute("comic.panel.add", &json!({"page": page, "rect": [2000.0, 2000.0, 4.0, 4.0]})).expect("comic.panel.add");
    let findings = out["findings"].as_array().expect("findings");
    assert!(!findings.is_empty(), "the geometry is checked and reported: {out}");
}

#[test]
fn panel_replace_art_keeps_the_previous_asset_and_deletes_only_its_artwork() {
    let (mut s, root) = project("art");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let first = add_asset(&mut s, &root);
    let out = s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": first.0})).expect("replace_art");
    let node = out["node"].as_u64().expect("the clip group");
    assert_eq!(out["asset_kept"].as_bool(), Some(true));
    assert_eq!(panel_record(&s, panel).artwork_refs.len(), 1);
    assert_eq!(panel_record(&s, panel).artwork_refs[0].asset_id, first);
    assert!(s.doc().expect("a document").doc.node(NodeId(node)).is_some());
    // The image is selected, so `object.cropImage` acts on it (M1 acceptance test 3).
    assert!(s.doc().expect("a document").selection.objects.contains(&(out["image"].as_u64().map(NodeId).expect("image"))));

    // Replace it with a second asset: one artwork, the first node gone, both Assets intact.
    let second = add_asset_of(&mut s, &root, "assets/panels/art2.png");
    let again = s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": second.0})).expect("replace again");
    assert_eq!(again["previous"].as_u64(), Some(node), "the replaced artwork is deleted");
    assert!(s.doc().expect("a document").doc.node(NodeId(node)).is_none());
    assert_eq!(panel_record(&s, panel).artwork_refs.len(), 1);
    assert_eq!(open_comic(&s).bundle.assets.assets.len(), 2, "both Asset records are kept (design §11.1)");
    assert!(bad_params(&s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": 9999.0})).expect_err("no such asset")));
}

/// A second PNG and its `Asset`, so a replacement has somewhere to point.
fn add_asset_of(s: &mut Session, root: &std::path::Path, relative: &str) -> AssetId {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("assets dir");
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([40, 40, 200, 255]));
    img.save(&path).expect("write the png");
    let bytes = std::fs::read(&path).expect("read the png");
    let hash = vectorcraft_comic_model::sha256_hex(&bytes);
    let oc = open_comic_mut(s);
    let id = oc.bundle.allocate::<AssetId>().expect("an asset id");
    oc.bundle.assets.assets.push(vectorcraft_comic_model::Asset {
        id,
        asset_type: vectorcraft_comic_model::AssetType::Source,
        storage_reference: StorageRef::Linked { relative_path: relative.to_string() },
        content_hash: vectorcraft_comic_model::ContentHash { algorithm: "sha256".into(), value: hash },
        media_metadata: vectorcraft_comic_model::MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: None },
        provenance: None,
        parent_asset_ids: Vec::new(),
        approval_status: vectorcraft_comic_model::ApprovalStatus::Pending,
        unknown: Default::default(),
    });
    id
}

#[test]
fn panel_set_shot_changes_only_the_brief() {
    let (mut s, _root) = project("shot");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let before = panel_record(&s, panel);
    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "a rooftop at dusk"})).expect("set_shot");
    let after = panel_record(&s, panel);
    assert_eq!(after.shot_description, "a rooftop at dusk");
    assert_eq!(after.frame_node_id, before.frame_node_id);
    assert_eq!(after.artwork_refs, before.artwork_refs);
    assert!(bad_params(&s.execute("comic.panel.set_shot", &json!({"panel": panel})).expect_err("no shot")));
    assert!(s.execute("comic.panel.set_shot", &json!({})).is_err());
}

#[test]
fn panel_delete_applies_the_related_entity_policy_of_design_11_1() {
    let (mut s, root) = project("delete");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let keep = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let doomed = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let asset = add_asset(&mut s, &root);
    let job = add_job(&mut s, vectorcraft_comic_model::PanelId(doomed));
    s.execute("comic.panel.replace_art", &json!({"panel": doomed, "asset": asset.0})).expect("artwork");

    // A speech balloon in the doomed panel, plus a caption whose Dialogue is flagged reusable.
    let spoken = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": doomed, "kind": "speech", "text": "Look out!", "at": [360.0, 100.0]}))
        .expect("lettering.add");
    let reused =
        s.execute("comic.lettering.add", &json!({"page": page, "panel": doomed, "kind": "narration", "text": "Meanwhile…"})).expect("caption");
    // A second speech balloon whose Dialogue is flagged reusable in the record's own `unknown` map.
    let flagged = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": doomed, "kind": "speech", "text": "Keep me", "at": [300.0, 200.0]}))
        .expect("lettering.add");
    let flagged_dialogue = lettering_of(&s, flagged["lettering"].as_u64().expect("id")).dialogue_id.expect("a dialogue").0;
    {
        let oc = open_comic_mut(&mut s);
        for file in oc.bundle.pages.values_mut() {
            if let Some(d) = file.dialogue.iter_mut().find(|d| d.id.0 == flagged_dialogue) {
                d.unknown.insert("reusable".into(), Value::Bool(true));
            }
        }
    }
    let spoken_dialogue = dialogue_of_lettering(&s, spoken["lettering"].as_u64().expect("id"));
    let reused_lettering = reused["lettering"].as_u64().expect("id");
    let frame = lettering_of(&s, reused_lettering).frame_node_id;
    let text_node = lettering_of(&s, reused_lettering).text_node_id;
    let art_node = panel_record(&s, doomed).artwork_refs[0].node_id;

    let out = s.execute("comic.panel.delete", &json!({"panel": doomed})).expect("comic.panel.delete");
    let deletion = &out["deletion"];
    // Lettering frame, tail and text objects — and their records — go with the panel.
    assert_eq!(deletion["letteringDeleted"].as_array().map(Vec::len), Some(3), "the speech balloon, the caption and the flagged balloon: {out}");
    assert!(s.doc().expect("a document").doc.node(frame).is_none(), "the caption's frame is gone");
    assert!(s.doc().expect("a document").doc.node(text_node).is_none(), "its text object too");
    // Artwork objects go with the panel; the Asset record does not.
    assert!(s.doc().expect("a document").doc.node(art_node).is_none(), "the artwork object is deleted");
    assert_eq!(deletion["assetsKept"].as_array().map(Vec::len), Some(1));
    assert_eq!(open_comic(&s).bundle.assets.assets.len(), 1, "Asset records are never deleted (design §11.1)");
    // Dialogue: deleted unless flagged reusable, in which case detached **and reported**.
    assert_eq!(deletion["dialogueDeleted"], json!([spoken_dialogue]), "the balloon that was not flagged loses its dialogue: {out}");
    assert_eq!(deletion["dialogueDetached"], json!([flagged_dialogue]), "the flagged one is detached: {out}");
    assert_eq!(dialogue_of(&s, flagged_dialogue).text, "Keep me", "detached, so the words are moved rather than retyped");
    // A job still targeting the panel is left to finish and reported.
    assert_eq!(deletion["jobsStillTargeting"], json!([job.0]));
    assert!(open_comic(&s).bundle.provenance.jobs.iter().any(|j| j.id == job), "the job is still there");
    // The reading order is renumbered in the same operation, not left with a gap.
    assert_eq!(out["readingOrder"], json!([keep]));
    assert_eq!(panel_record(&s, keep).reading_order_index, 0);
    assert!(bad_params(&s.execute("comic.panel.delete", &json!({})).expect_err("no panel")));
    assert!(s.execute("comic.panel.delete", &json!({"panel": doomed})).is_err(), "already deleted");
}

#[test]
fn deleting_a_panel_detaches_reusable_dialogue_rather_than_retyping_it() {
    let (mut s, _root) = project("detach");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "Moved later", "at": [120.0, 100.0]}))
        .expect("lettering.add");
    let dialogue_id = lettering_of(&s, balloon["lettering"].as_u64().expect("id")).dialogue_id.expect("a dialogue").0;

    let out = s.execute("comic.panel.delete", &json!({"panel": panel, "reuse": [dialogue_id]})).expect("comic.panel.delete");
    let deletion = &out["deletion"];
    assert!(deletion["dialogueDeleted"].as_array().is_some_and(|a| a.is_empty()), "not deleted: {out}");
    assert_eq!(deletion["dialogueDetached"], json!([dialogue_id]));
    assert!(deletion["dialogueDetachedNote"].as_str().is_some_and(|s| s.contains("not retyped")), "{out}");
    // The words survive, unchanged — that is the whole point of detaching rather than deleting.
    assert_eq!(dialogue_of(&s, dialogue_id).text, "Moved later");
    // And the detachment is visible to validation rather than silently repaired (schema rule 1).
    let report = s.execute("comic.project.validate", &json!({})).expect("validate");
    assert!(
        report["findings"]
            .as_array()
            .is_some_and(|a| a.iter().any(|f| f["severity"] == "error" && f["subject"] == format!("Dialogue({dialogue_id}).panel_id"))),
        "the detached dialogue is reported, not fixed: {report}"
    );
}

// ---------- comic.reading_order.set ----------

#[test]
fn reading_order_set_writes_both_halves_and_refuses_a_non_permutation() {
    let (mut s, _root) = project("order");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let out = s.execute("comic.reading_order.set", &json!({"page": page, "order": [b, a]})).expect("comic.reading_order.set");
    assert_eq!(out["reading_order"], json!([b, a]));
    assert_eq!(panel_record(&s, b).reading_order_index, 0);
    assert_eq!(panel_record(&s, a).reading_order_index, 1);
    // An order that is not a permutation is refused, never repaired (design §11.1).
    assert!(bad_params(&s.execute("comic.reading_order.set", &json!({"page": page, "order": [a]})).expect_err("a gap")));
    assert!(bad_params(&s.execute("comic.reading_order.set", &json!({"page": page, "order": [a, a]})).expect_err("a repeat")));
    assert!(bad_params(&s.execute("comic.reading_order.set", &json!({"page": page})).expect_err("no order")));
    assert_eq!(panel_record(&s, b).reading_order_index, 0, "the refused call changed nothing");
}

// ---------- comic.lettering.* and comic.dialogue.update ----------

#[test]
fn lettering_add_writes_an_editable_balloon_a_tail_and_a_text_object() {
    let (mut s, _root) = project("lettering");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 300.0, 250.0]);
    // M1 acceptance test 7: two speech balloons and one caption, each an editable path and text.
    let one = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "Hi there", "at": [150.0, 130.0]}))
        .expect("balloon one");
    let two = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "Hello", "at": [200.0, 180.0]}))
        .expect("balloon two");
    let cap = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "narration", "text": "Three hours earlier"}))
        .expect("caption");

    for out in [&one, &two, &cap] {
        let l = lettering_of(&s, out["lettering"].as_u64().expect("id"));
        let frame = s.doc().expect("a document").doc.node(l.frame_node_id).expect("the frame");
        assert!(frame.path_data().is_some_and(|p| p.is_closed() && !p.subpaths.is_empty()), "the frame is an editable path");
        assert!(s.doc().expect("a document").doc.node(l.text_node_id).is_some(), "the text object exists");
    }
    // Rule 8: a speech balloon takes its words from the Dialogue; a caption carries its own.
    let speech = lettering_of(&s, one["lettering"].as_u64().expect("id"));
    assert!(speech.dialogue_id.is_some() && speech.text.is_none());
    assert_eq!(text_of(&s, speech.text_node_id.0), "Hi there");
    assert!(speech.tail_node_id.is_some(), "a speech balloon points at someone");
    let caption = lettering_of(&s, cap["lettering"].as_u64().expect("id"));
    assert!(caption.dialogue_id.is_none() && caption.text.as_deref() == Some("Three hours earlier"));
    assert!(caption.tail_node_id.is_none(), "a caption points at nobody");
    assert!(s.doc().expect("a document").doc.node(caption.tail_node_id.unwrap_or(NodeId(0))).is_none());
    assert_eq!(panel_record(&s, panel).lettering_ids.len(), 3);

    assert!(bad_params(&s.execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech"})).expect_err("nothing said")));
    assert!(bad_params(&s.execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "narration"})).expect_err("no text")));
    assert!(bad_params(
        &s.execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "nope", "text": "x"})).expect_err("no such kind")
    ));
    assert!(s.execute("comic.lettering.add", &json!({})).is_err());
}

#[test]
fn lettering_set_text_refits_and_set_style_changes_the_typography() {
    let (mut s, _root) = project("letterstyle");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 300.0, 250.0]);
    let cap = s.execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "narration", "text": "Short"})).expect("caption");
    let id = cap["lettering"].as_u64().expect("id");

    let out = s
        .execute(
            "comic.lettering.set_text",
            &json!({"lettering": id, "text": "A much longer caption than the box was planned for, sentence after sentence"}),
        )
        .expect("set_text");
    assert_eq!(lettering_of(&s, id).text.as_deref(), Some("A much longer caption than the box was planned for, sentence after sentence"));
    assert_eq!(text_of(&s, lettering_of(&s, id).text_node_id.0), lettering_of(&s, id).text.as_deref().expect("text"));
    assert!(out["outcome"].as_str().is_some(), "the plan reports how it resolved: {out}");

    s.execute("comic.lettering.set_style", &json!({"lettering": id, "size": 9.0, "align": "left", "font": "Comic Neue"})).expect("set_style");
    let l = lettering_of(&s, id);
    assert_eq!(l.text_layout_settings.size, Some(9.0));
    assert_eq!(l.text_layout_settings.align, Some(vectorcraft_comic_model::TextAlign::Left));
    assert_eq!(l.text_layout_settings.font.as_deref(), Some("Comic Neue"));
    assert!(bad_params(&s.execute("comic.lettering.set_text", &json!({"lettering": id})).expect_err("no text")));
    assert!(bad_params(&s.execute("comic.lettering.set_style", &json!({"lettering": id, "size": -3.0})).expect_err("a bad size")));
    assert!(s.execute("comic.lettering.set_style", &json!({"lettering": 9999})).is_err());
}

#[test]
fn dialogue_update_changes_the_words_without_touching_typography_or_geometry() {
    let (mut s, _root) = project("dialogue");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 300.0, 250.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "First draft", "at": [150.0, 130.0]}))
        .expect("lettering.add");
    let lettering_id = balloon["lettering"].as_u64().expect("id");
    let dialogue_id = lettering_of(&s, lettering_id).dialogue_id.expect("a dialogue").0;
    let before = lettering_of(&s, lettering_id);
    let frame_before = s.doc().expect("a document").doc.node(before.frame_node_id).and_then(|n| n.path_data().cloned()).expect("a path");

    let out =
        s.execute("comic.dialogue.update", &json!({"dialogue": dialogue_id, "text": "Final line", "approval": "approved"})).expect("dialogue.update");
    assert_eq!(out["typographyTouched"].as_bool(), Some(false));
    assert_eq!(out["geometryTouched"].as_bool(), Some(false));
    // M1 acceptance test 5: the words changed, nothing else did.
    let after = lettering_of(&s, lettering_id);
    assert_eq!(dialogue_of(&s, dialogue_id).text, "Final line");
    assert_eq!(text_of(&s, after.text_node_id.0), "Final line", "the page shows the record's words");
    assert_eq!(after.text_layout_settings, before.text_layout_settings, "typography is untouched");
    let frame_after = s.doc().expect("a document").doc.node(after.frame_node_id).and_then(|n| n.path_data().cloned()).expect("a path");
    assert_eq!(format!("{frame_before:?}"), format!("{frame_after:?}"), "the frame is untouched");
    assert_eq!(after.tail_node_id, before.tail_node_id);
    // The other fields can be set too, and a bad value is refused.
    s.execute("comic.dialogue.update", &json!({"dialogue": dialogue_id, "speaker": 7, "language": "fr", "readingOrderIndex": 2}))
        .expect("more fields");
    let d = dialogue_of(&s, dialogue_id);
    assert_eq!(d.speaker_character_id.map(|c| c.0), Some(7));
    assert_eq!(d.language.as_deref(), Some("fr"));
    assert_eq!(d.reading_order_index, 2);
    assert!(bad_params(&s.execute("comic.dialogue.update", &json!({"dialogue": dialogue_id, "approval": "maybe"})).expect_err("a bad approval")));
    assert!(s.execute("comic.dialogue.update", &json!({})).is_err());
}

// ---------- undo ----------

#[test]
fn one_user_action_is_one_undo_step_including_a_comic_only_change() {
    let (mut s, _root) = project("undo");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);

    let depth = s.doc().expect("a document").history.undo.len();
    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "wide"})).expect("set_shot");
    assert_eq!(s.doc().expect("a document").history.undo.len(), depth + 1, "exactly one step, for a comic-only change");
    assert_eq!(panel_record(&s, panel).shot_description, "wide");
    s.execute("edit.undo", &json!({})).expect("undo");
    assert_eq!(panel_record(&s, panel).shot_description, "", "undo restores the comic model too");
    s.execute("edit.redo", &json!({})).expect("redo");
    assert_eq!(panel_record(&s, panel).shot_description, "wide");

    // A step that changes the document *and* the model is still one step, and still exact.
    let artboard_before = s.doc().expect("a document").doc.artboards.len();
    let depth = s.doc().expect("a document").history.undo.len();
    let second_asset = {
        let root = open_comic(&s).root.clone();
        add_asset_of(&mut s, &root, "assets/panels/u.png")
    };
    s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": second_asset.0})).expect("replace_art");
    assert_eq!(s.doc().expect("a document").history.undo.len(), depth + 1, "one step for both halves");
    s.execute("edit.undo", &json!({})).expect("undo");
    assert!(panel_record(&s, panel).artwork_refs.is_empty(), "the record went back");
    assert_eq!(s.doc().expect("a document").doc.artboards.len(), artboard_before);

    // Every mutating command is one step: a page takes an artboard and a record together.
    let depth = s.doc().expect("a document").history.undo.len();
    let new_page = s.execute("comic.page.create", &json!({"size": [300.0, 300.0]})).expect("a page");
    assert_eq!(s.doc().expect("a document").history.undo.len(), depth + 1);
    s.execute("edit.undo", &json!({})).expect("undo");
    assert_eq!(open_comic(&s).bundle.pages.len(), 1, "the Page record went back");
    assert!(!open_comic(&s).bundle.pages.contains_key(&vectorcraft_comic_model::PageId(new_page["page"].as_u64().expect("id"))));
}

// ---------- the malformed-call sweep ----------

#[test]
fn every_comic_command_refuses_a_malformed_call_without_panicking() {
    let (mut s, _root) = project("junk");
    let page = s.execute("comic.page.create", &json!({})).expect("a command result")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "x", "at": [120.0, 100.0]}))
        .expect("lettering.add");
    let lettering = balloon["lettering"].as_u64().expect("id");
    let dialogue = lettering_of(&s, lettering).dialogue_id.expect("a dialogue").0;

    // The commands with no required parameter: schema §12 lets extra keys through, so a junk call to
    // them is *valid*, and the assertion for them is only that nothing panics. Every other command
    // must refuse junk — and an empty call — with `BadParams` or a documented `Other`.
    const NO_REQUIRED_PARAM: &[&str] = &["comic.project.save", "comic.project.validate", "comic.project.settings.get"];
    let junk = json!({"page": "x", "panel": [], "lettering": null, "dialogue": -1, "asset": {}, "rect": "nope", "size": "big", "to": "later", "order": 3, "kind": 7, "text": 5});
    for spec in specs() {
        for (label, params) in [("junk", &junk), ("empty", &json!({}))] {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.execute(spec.id, params)));
            assert!(r.is_ok(), "{} panicked on {label} params", spec.id);
            let out = r.expect("checked");
            if label == "junk" && !NO_REQUIRED_PARAM.contains(&spec.id) {
                assert!(matches!(out, Err(EngineError::BadParams { .. }) | Err(EngineError::Other(_))), "{} accepted junk params: {out:?}", spec.id);
            }
            assert!(!s.in_interaction(), "{} left an interaction open", spec.id);
        }
    }
    // The ids the well-formed calls used still resolve, so the junk sweep changed nothing.
    assert!(panel_record(&s, panel).id.0 == panel);
    assert!(lettering_of(&s, lettering).id.0 == lettering);
    assert_eq!(dialogue_of(&s, dialogue).text, "x");
    assert!(!s.in_interaction(), "no interaction is left open");
}

// ---------- round-1 review: what the shipped suite could not see ----------

/// Whether `comic.project.validate` reports an `error` with this subject, so a test can ask "is the
/// project broken right now" without matching the whole report.
fn reported_error(s: &mut Session, subject: &str) -> bool {
    let report = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    report["findings"].as_array().is_some_and(|a| a.iter().any(|f| f["severity"] == "error" && f["subject"] == subject))
}

/// The panels that list `dialogue` in `dialogue_ids` — the owners of one `Dialogue` (design §6.1).
fn dialogue_owners(s: &Session, dialogue: u64) -> Vec<u64> {
    open_comic(s)
        .bundle
        .pages
        .values()
        .flat_map(|f| f.panels.iter())
        .filter(|p| p.dialogue_ids.iter().any(|d| d.0 == dialogue))
        .map(|p| p.id.0)
        .collect()
}

/// §2 of the implementation and [`crate::HistoryEntry::comic`]: a step that recorded **no** comic
/// model must not touch the open project when it is undone or redone.
///
/// This is the test that rejects the plausible simplification
/// `comic: std::mem::replace(&mut st.comic, e.comic.clone())` in `edit.rs`: treating the entry's
/// `comic` field as authoritative even when it is `None` makes undoing a *plain* document edit
/// close the open project — root, model and findings all gone, and every `comic.*` id reporting
/// "no comic project open" afterwards. W4's whole undo design rests on that `match`.
#[test]
fn an_undo_of_a_plain_edit_leaves_the_open_project_alone() {
    let (mut s, root) = project("plain-undo");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "keep me"})).expect("set_shot");
    let artboard_x = s.doc().expect("a document").doc.artboards[0].rect.x0;

    // A plain document edit: `artboard.setProps` is not a comic command, so its step records
    // `comic: None`.
    s.execute("artboard.setProps", &json!({"index": 0, "x": artboard_x + 10.0, "y": 0.0, "width": 612.0, "height": 792.0}))
        .expect("artboard.setProps");
    s.execute("edit.undo", &json!({})).expect("undo the plain edit");
    assert!(s.doc().expect("a document").comic.is_some(), "undoing a plain document edit closed the project");
    assert_eq!(open_comic(&s).root, root, "and the project is still the bundle that was opened");
    assert_eq!(panel_record(&s, panel).shot_description, "keep me", "the model survived the plain step's undo");
    assert_eq!(s.doc().expect("a document").doc.artboards[0].rect.x0, artboard_x, "the plain edit was undone");

    // Redo is symmetric.
    s.execute("edit.redo", &json!({})).expect("redo the plain edit");
    assert!(s.doc().expect("a document").comic.is_some(), "redo of a plain document edit closed the project");
    assert_eq!(panel_record(&s, panel).shot_description, "keep me");
    assert_eq!(s.doc().expect("a document").doc.artboards[0].rect.x0, artboard_x + 10.0);
}

/// §5 of the implementation: a live referential `Severity::Error` does **not** block a save.
///
/// This is the test that rejects the plausible "refuse the write when `validate` reports an
/// Error". Schema §14 exists so a damaged bundle stays openable *and repairable*, and the commands
/// that repair a broken reference are comic commands that save — blocking the write would make
/// repair impossible and risk the user's other edits. The other half is pinned too: the save must
/// carry the Error rather than launder it.
#[test]
fn a_save_with_a_live_referential_error_succeeds_and_keeps_it() {
    let (mut s, root) = project("error-save");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "Do not lose me", "at": [120.0, 100.0]}))
        .expect("lettering.add");
    let dialogue = balloon["dialogue"].as_u64().expect("its dialogue");

    // The one command that leaves an Error on purpose: a reusable dialogue is detached, not deleted.
    s.execute("comic.panel.delete", &json!({"panel": panel, "reuse": [dialogue]})).expect("comic.panel.delete");
    let subject = format!("Dialogue({dialogue}).panel_id");
    assert!(reported_error(&mut s, &subject), "the detachment is a live referential Error before the save");

    let saved = s.execute("comic.project.save", &json!({})).unwrap_or_else(|e| panic!("a live Error blocked the save: {e}"));
    assert!(saved["files"].as_array().is_some_and(|f| !f.is_empty()), "{saved}");

    let mut reopened = Session::new();
    reopened.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open");
    assert_eq!(dialogue_of(&reopened, dialogue).text, "Do not lose me", "the save lost the detached dialogue's words");
    assert!(open_comic(&reopened).bundle.pages.contains_key(&PageId(page)), "the page survived the round trip");
    assert!(reported_error(&mut reopened, &subject), "the save resolved the Error instead of carrying it");
}

/// Design §6.1: one `Dialogue`, **one** owner. A balloon may show another panel's dialogue — that
/// is one record with two renderers, not two independently editable authoritative copies — but it
/// must not claim it, and `comic.dialogue.update` must reach every balloon that shows the record.
#[test]
fn a_balloon_showing_another_panels_dialogue_does_not_own_it() {
    let (mut s, _root) = project("one-owner");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let in_b = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "text": "B says", "at": [340.0, 100.0]}))
        .expect("a balloon in panel B");
    let dialogue = in_b["dialogue"].as_u64().expect("its dialogue");
    let in_a = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "dialogue": dialogue, "at": [120.0, 100.0]}))
        .expect("a balloon in panel A showing B's dialogue");
    let text_node_a = lettering_of(&s, in_a["lettering"].as_u64().expect("id")).text_node_id.0;

    assert_eq!(dialogue_owners(&s, dialogue), vec![b], "a Dialogue has exactly one owner: the panel it names");

    // One record, two renderers: the update reaches both, so no visual object is left behind as a
    // second source of dialogue truth.
    s.execute("comic.dialogue.update", &json!({"dialogue": dialogue, "text": "B says, revised"})).expect("dialogue.update");
    assert_eq!(text_of(&s, text_node_a), "B says, revised", "the balloon outside the owning panel still shows the old words");
    assert_eq!(dialogue_of(&s, dialogue).text, "B says, revised");

    // And the record is not two ownerships to delete through: deleting the owner's panel deletes the
    // dialogue, and the balloon that showed it goes with its words rather than naming a missing
    // record (schema rule 1).
    s.execute("comic.panel.delete", &json!({"panel": b})).expect("comic.panel.delete");
    for id in open_comic(&s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).map(|d| d.id.0) {
        assert_ne!(id, dialogue, "the dialogue went with its owner");
    }
    let violations = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    assert_eq!(violations["errors"].as_u64(), Some(0), "deleting the owner left nothing dangling: {violations}");
}

/// Design §11.1's `Dialogue` row: "deleted with the panel unless flagged reusable, in which case
/// detached and reported, **so the text is moved rather than retyped**". The move has to exist:
/// `Dialogue.panel_id` is required (schema §6.3), so a detached dialogue names a panel that is gone
/// — a rule-1 Error — and `comic.lettering.add {panel, dialogue}` is what re-homes it. Without that
/// the Error is permanent and a normal command has created a project M1 cannot repair.
#[test]
fn lettering_add_readopts_a_dialogue_whose_panel_is_gone() {
    let (mut s, _root) = project("detach-repair");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "text": "Move me", "at": [120.0, 100.0]}))
        .expect("a balloon in panel A");
    let dialogue = balloon["dialogue"].as_u64().expect("its dialogue");

    s.execute("comic.panel.delete", &json!({"panel": a, "reuse": [dialogue]})).expect("comic.panel.delete detaches it");
    let subject = format!("Dialogue({dialogue}).panel_id");
    assert!(reported_error(&mut s, &subject), "the detachment is reported, as §11.1 says");

    s.execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "dialogue": dialogue, "at": [340.0, 100.0]}))
        .expect("the documented way to move the text");
    assert_eq!(dialogue_of(&s, dialogue).text, "Move me", "the words moved, never retyped");
    assert_eq!(dialogue_of(&s, dialogue).panel_id.0, b, "and the record names a panel that exists again");
    assert_eq!(dialogue_owners(&s, dialogue), vec![b], "with one owner, the panel that adopted it");
    assert!(!reported_error(&mut s, &subject), "the repair cleared the rule-1 Error");
}

/// Round-1 F7: `comic.project.save` must not destroy bytes it never staged. A `pages/page-<n>.json`
/// written since the load is not in the generation of record, so the prune leaves it alone — and
/// the manifest this save writes then describes it, because §4 defines `files` as every file.
///
/// W4's round-2 review F9 is why this test goes to **three** saves. Pruning against the previous
/// `manifest.files` (the *declared* set) kept the stray for exactly one save: the save that keeps it
/// also describes it, so the next save found the name in its previous manifest and deleted it
/// silently. The prune must ask what the generation of record **wrote**, which is why
/// `comic-model`'s `files_of_record` is the staged set, not the scanned one. The second half is
/// unchanged: a page the generation of record *wrote* and this save drops still goes.
#[test]
fn a_save_keeps_a_page_file_written_since_the_load() {
    let (mut s, root) = project("keep-stray");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    s.execute("comic.project.save", &json!({})).expect("the first save");

    let stray = root.join("pages/page-99.json");
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &stray).expect("a page file written since the load");
    let out = s.execute("comic.project.save", &json!({})).expect("the second save");
    assert!(stray.is_file(), "the save deleted bytes it never staged: {out}");
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("read manifest.json")).expect("manifest json");
    assert!(manifest["files"].get("pages/page-99.json").is_some(), "and the file it kept is described by `files`: {out}");

    // Save 3: the stray is now *described* by the previous manifest, so a prune that reads the
    // declared set deletes it here. It must survive, and the page the bundle still has must not.
    let third = s.execute("comic.project.save", &json!({})).expect("the third save");
    assert!(stray.is_file(), "the third save deleted a file the generation of record never wrote: describing a file is not writing it: {third}");
    assert!(root.join(format!("pages/page-{page}.json")).is_file(), "the page the bundle still has is still there");
}

/// The other half of what "the generation of record wrote" means: the record is what was **written**,
/// and §11.2's undo snapshot carries the whole bundle — so undoing an edit rewinds
/// `ComicBundle::files_of_record` with it. `comic.project.save` therefore restores the record from
/// the last write (`DocState::saved_comic`, which no undo touches) before pruning. Without that, a
/// save after an undo prunes against a record older than the disk and leaves the file the undone
/// save wrote: the manifest describes it, so the reload adopts the page back and the undo silently
/// fails to stick.
#[test]
fn a_save_after_an_undo_still_removes_the_file_the_undone_save_wrote() {
    let (mut s, root) = project("undo-then-save");
    let first = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    s.execute("comic.project.save", &json!({})).expect("the first save");
    let second = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    s.execute("comic.project.save", &json!({})).expect("the second save");
    let file = root.join(format!("pages/page-{second}.json"));
    assert!(file.is_file(), "the second save wrote the page");

    s.execute("edit.undo", &json!({})).expect("undo the page creation");
    assert!(!open_comic(&s).bundle.pages.contains_key(&vectorcraft_comic_model::PageId(second)), "the model dropped the page");
    s.execute("comic.project.save", &json!({})).expect("the third save");

    assert!(!file.exists(), "a save must remove a page file the bundle no longer has, whatever an undo did to the record");
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("read manifest.json")).expect("manifest json");
    assert!(manifest["files"].get(format!("pages/page-{second}.json")).is_none(), "and the manifest must not describe it");
    assert!(root.join(format!("pages/page-{first}.json")).is_file(), "the page the bundle still has is still there");

    // The consequence the prune exists for: the reopen does not resurrect the undone page.
    let mut reopened = Session::new();
    let out = reopened.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open");
    assert_eq!(out["pages"].as_u64(), Some(1), "the undone page did not come back: {out}");
}

/// W4 round-2 N9 and the ninth finding: `comic.lettering.add {dialogue}` accepts a dialogue owned by
/// a panel on **another page** — schema rule 1 only asks the id to resolve and no rule ties a
/// `Lettering` to its dialogue's page — and it must treat that owner as an owner, not as a missing
/// one.
///
/// This is the pin for a plausible-but-wrong implementation: reading "the owner resolves" as "the
/// owner resolves *on the requested page*" makes a cross-page owner look detached, so the balloon's
/// panel adopts the record and steals it from a live panel on another page. That implementation
/// passes every other test in this suite and in round 1's, so this asserts the *value* it changes:
/// the record still names its own panel, and the balloon's panel is not an owner.
#[test]
fn a_balloon_may_show_a_dialogue_owned_by_a_panel_on_another_page() {
    let (mut s, _root) = project("cross-page-owner");
    let p1 = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let a = panel_on(&mut s, p1, [40.0, 40.0, 200.0, 150.0]);
    let p2 = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    // Page 2's artboard sits one page to the right (612pt + the gutter), so the rect is absolute.
    let c = panel_on(&mut s, p2, [672.0, 40.0, 160.0, 110.0]);

    let owner = s
        .execute("comic.lettering.add", &json!({"page": p2, "panel": c, "kind": "speech", "text": "C says", "at": [750.0, 100.0]}))
        .expect("a balloon on page 2");
    let dialogue = owner["dialogue"].as_u64().expect("its dialogue");

    let far = s
        .execute("comic.lettering.add", &json!({"page": p1, "panel": a, "kind": "speech", "dialogue": dialogue, "at": [120.0, 100.0]}))
        .expect("a balloon on page 1 showing page 2's dialogue");

    assert_eq!(dialogue_of(&s, dialogue).panel_id.0, c, "the owner is unchanged: a resolvable owner is shown, never re-owned");
    assert_eq!(dialogue_owners(&s, dialogue), vec![c], "and the balloon's panel did not claim it");
    assert_eq!(lettering_of(&s, far["lettering"].as_u64().expect("id")).page_id.0, p1, "the balloon is filed on the page it was added to");
    assert_eq!(dialogue_of(&s, dialogue).text, "C says", "the words are the record's, unaltered");

    // Nothing reports it: no rule covers a cross-page dialogue reference, which is what makes the
    // reference legal-but-unspecified rather than a rule the guard should enforce.
    let report = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    assert_eq!(report["errors"].as_u64(), Some(0), "the cross-page reference is not an error: {report}");
    let rules: Vec<u64> = report["findings"].as_array().expect("findings").iter().filter_map(|f| f["rule"].as_u64()).collect();
    assert!(!rules.contains(&1), "rule 1 resolves it: {report}");

    // The wider verb reaches the record wherever it is; `set_text` resolves it in the balloon's own
    // page and refuses instead of reaching across files (the documented N9 asymmetry, one line in
    // its params string). Neither writes the wrong record.
    s.execute("comic.dialogue.update", &json!({"dialogue": dialogue, "text": "C says, revised"})).expect("comic.dialogue.update");
    assert_eq!(dialogue_of(&s, dialogue).text, "C says, revised");
    let refused = s.execute("comic.lettering.set_text", &json!({"lettering": far["lettering"], "text": "retyped"})).expect_err("set_text refuses");
    assert!(refused.to_string().contains("comic.dialogue.update"), "and the refusal names the verb that does reach it: {refused}");
    assert_eq!(dialogue_of(&s, dialogue).text, "C says, revised", "the refused call changed nothing");
}

// ---------- M2-L1: the subject spelling the dedupe key depends on ----------

/// The `subject`s of a command's `findings` array.
fn finding_subjects(out: &Value) -> Vec<String> {
    out["findings"].as_array().expect("findings").iter().filter_map(|f| f["subject"].as_str().map(str::to_string)).collect()
}

/// The `message`s of a command's `findings` array.
fn finding_messages(out: &Value) -> Vec<String> {
    out["findings"].as_array().expect("findings").iter().filter_map(|f| f["message"].as_str().map(str::to_string)).collect()
}

/// **M2-L1.** A geometry fault must be spelled the way `comic-layout`'s `validate::page` spells it —
/// `Panel(<id>)` — because `validate`'s own doc offers `(rule, subject)` as the key a caller running
/// both validators deduplicates on, and two spellings for one fault make that dedupe silently
/// double-report. The index form (`Panel[i]`) was also unresolvable: it indexed the *filtered*
/// rectangle vector, so it was not even the page file's own index.
///
/// The test drives the case the criterion is about — a page resized until two panels nearly touch —
/// and asserts that the command's finding and the validator's finding are **the same pair**, and that
/// the subject names a panel the page really has. Under the old `Panel[0]` spelling both halves fail.
#[test]
fn a_geometry_finding_names_its_panel_the_way_the_validator_does() {
    let (mut s, _root) = project("l1-subject");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let left = panel_on(&mut s, page, [40.0, 40.0, 200.0, 300.0]);
    let right = panel_on(&mut s, page, [242.0, 40.0, 200.0, 300.0]);
    // 2pt apart, against the 6pt minimum: one warning, naming the *left* panel of the pair, because
    // `faults` reports the first of the two it was given.
    let resized = s.execute("comic.panel.resize", &json!({"panel": right, "width": 200.0, "height": 300.0})).expect("comic.panel.resize");
    let subjects = finding_subjects(&resized);
    assert_eq!(subjects.len(), 1, "one warning for one fault: {resized}");
    assert_eq!(subjects[0], format!("Panel({left})"), "the subject is an id, not an index: {resized}");
    assert!(!subjects[0].contains('['), "the index spelling is gone: {resized}");
    let message = &finding_messages(&resized)[0];
    assert!(message.contains(&format!("Panel({right})")), "and the message names the other panel by id too: {message}");

    // The validator's finding for the same rectangles is the *same* `(rule, subject)` — which is what
    // makes a caller's dedupe work — and it is about a panel the page really has.
    let report = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    let pair = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["subject"] == subjects[0].as_str())
        .unwrap_or_else(|| panic!("no finding in validate shares the command's subject: {report}"));
    assert_eq!(pair["rule"], Value::Null, "both are design §12.1 geometry checks, not a referential rule: {pair}");
    assert!(pair["message"].as_str().is_some_and(|m| m.contains("2pt")), "{pair}");
    let ids: Vec<u64> = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.iter().map(|p| p.id.0).collect();
    assert!(ids.contains(&left), "the subject resolves to a panel of this page: {ids:?}");

    // `comic.page.template.apply` reports the same kind of fault through the same helper.
    let applied = s.execute("comic.page.template.apply", &json!({"page": page, "template": "two-tier"})).expect("a two-slot template");
    for subject in finding_subjects(&applied).into_iter().chain(finding_subjects(&resized)) {
        assert!(subject.starts_with("Panel(") && subject.ends_with(')'), "every geometry subject is `Panel(<id>)`: {subject}");
    }
}

// ---------- the lock (§6.2): every row of the table in `panel.rs`'s module doc ----------

/// The refusal a locked panel gets, asserted for content rather than for being *some* error: it must
/// name the panel and the one command that clears the lock.
fn assert_locked_refusal(err: &EngineError, panel: u64) {
    let message = err.to_string();
    assert!(message.contains("locked"), "the refusal says the panel is locked: {message}");
    assert!(message.contains(&format!("panel {panel}")), "it names the panel: {message}");
    assert!(message.contains("comic.panel.set_locked"), "and the command that clears the lock: {message}");
}

#[test]
fn a_new_panel_is_unlocked_and_set_locked_is_the_way_in_and_out() {
    let (mut s, _root) = project("lock-set");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    assert!(!panel_record(&s, panel).locked, "comic.panel.add creates an unlocked panel (§6.2) — nothing is protected until there is one");

    let locked = s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": true})).expect("lock");
    assert_eq!(locked["panel"].as_u64(), Some(panel));
    assert_eq!(locked["page"].as_u64(), Some(page));
    assert_eq!(locked["locked"].as_bool(), Some(true));
    assert!(panel_record(&s, panel).locked);
    s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": false})).expect("unlock");
    assert!(!panel_record(&s, panel).locked);

    assert!(bad_params(&s.execute("comic.panel.set_locked", &json!({"panel": panel})).expect_err("no `locked`")));
    assert!(bad_params(&s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": "yes"})).expect_err("not a bool")));
    assert!(s.execute("comic.panel.set_locked", &json!({"panel": 9999, "locked": true})).is_err(), "no such panel");
}

#[test]
fn a_locked_panel_is_not_moved_resized_or_re_arted_and_the_refusal_acts_on_it() {
    let (mut s, root) = project("lock-geom");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let asset = add_asset(&mut s, &root);
    s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": true})).expect("lock");

    let frame = panel_record(&s, panel).frame_node_id;
    let before = s.doc().expect("a document").doc.node(frame).and_then(|n| n.geometric_bounds()).expect("bounds");
    let nodes = s.doc().expect("a document").doc.node_count();

    assert_locked_refusal(&s.execute("comic.panel.move", &json!({"panel": panel, "dx": 50.0})).expect_err("move"), panel);
    assert_locked_refusal(&s.execute("comic.panel.resize", &json!({"panel": panel, "width": 100.0})).expect_err("resize"), panel);
    assert_locked_refusal(&s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": asset.0})).expect_err("artwork"), panel);

    // The refusals are refusals, not half-applied edits: the frame is where it was, no object was
    // placed or deleted, and nothing was recorded.
    let after = s.doc().expect("a document").doc.node(frame).and_then(|n| n.geometric_bounds()).expect("bounds");
    assert_eq!((before.x0, before.y0, before.width(), before.height()), (after.x0, after.y0, after.width(), after.height()));
    assert_eq!(s.doc().expect("a document").doc.node_count(), nodes, "no object was added or removed");
    assert!(panel_record(&s, panel).artwork_refs.is_empty(), "the artwork the refusal named was not recorded");
}

#[test]
fn a_locked_panel_is_not_deleted_and_neither_is_its_page() {
    let (mut s, _root) = project("lock-delete");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let first = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let second = panel_on(&mut s, page, [300.0, 40.0, 200.0, 150.0]);
    s.execute("comic.panel.set_locked", &json!({"panel": second, "locked": true})).expect("lock the second");

    assert_locked_refusal(&s.execute("comic.panel.delete", &json!({"panel": second})).expect_err("delete the locked panel"), second);
    // `comic.page.delete` refuses on **any** locked panel of the page, and names it — the page is one
    // command away from being deletable, which is what makes the refusal actionable.
    assert_locked_refusal(&s.execute("comic.page.delete", &json!({"page": page})).expect_err("delete the page"), second);
    assert!(
        panel_record(&s, first).id.0 == first && open_comic(&s).bundle.pages.contains_key(&vectorcraft_comic_model::PageId(page)),
        "nothing was deleted"
    );
    // The *unlocked* panel of the same page is still deletable: the lock is per panel.
    s.execute("comic.panel.delete", &json!({"panel": first})).expect("the unlocked panel goes");
}

#[test]
fn a_locked_panel_blocks_a_template_that_would_move_or_replace_it() {
    let (mut s, _root) = project("lock-template");
    let page = s.execute("comic.page.create", &json!({"template": "three-tier"})).expect("a templated page")["page"].as_u64().expect("page id");
    let panel = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels[0].id.0;
    s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": true})).expect("lock one panel");

    // "Refused" is not enough on its own: the re-lay path **moves every frame** and changes no
    // membership, so a check that ran after the lay-out would still refuse *and* leave the page
    // re-laid. The frame rectangles are therefore read before and after, so this pins "refused
    // **before** anything was written" and not merely "refused".
    let frame_rects = |s: &Session| -> Vec<(u64, [f64; 4])> {
        open_comic(s).bundle.pages[&vectorcraft_comic_model::PageId(page)]
            .panels
            .iter()
            .map(|p| {
                let b = s.doc().expect("a document").doc.node(p.frame_node_id).and_then(|n| n.geometric_bounds()).expect("bounds");
                (p.id.0, [b.x0, b.y0, b.width(), b.height()])
            })
            .collect()
    };
    let before = frame_rects(&s);
    assert_eq!(before.len(), 3);

    // The same-count path moves every frame, so it is refused…
    assert_locked_refusal(&s.execute("comic.page.template.apply", &json!({"page": page, "template": "wide-top"})).expect_err("re-lay"), panel);
    // …and `replace: true` deletes them, so it is refused too.
    assert_locked_refusal(
        &s.execute("comic.page.template.apply", &json!({"page": page, "template": "two-tier", "replace": true})).expect_err("replace"),
        panel,
    );
    assert_eq!(open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.len(), 3, "nothing was laid out or deleted");
    assert_eq!(frame_rects(&s), before, "the refusal came before the write: a lay-out that ran first would have moved these frames");

    // A lock on *another* page does not stop a page being laid out from scratch: there is no panel
    // here to protect.
    let fresh = s.execute("comic.page.create", &json!({})).expect("a second page")["page"].as_u64().expect("page id");
    let applied = s.execute("comic.page.template.apply", &json!({"page": fresh, "template": "two-tier"})).expect("create-panels path");
    assert_eq!(applied["panels"].as_array().map(Vec::len), Some(2));
    assert!(applied["panels"].as_array().expect("panels").iter().all(|p| !panel_record(&s, p["panel"].as_u64().expect("id")).locked));
}

#[test]
fn a_lock_does_not_stop_the_brief_the_words_or_the_narrative_order() {
    let (mut s, _root) = project("lock-scope");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let first = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let second = panel_on(&mut s, page, [300.0, 40.0, 200.0, 150.0]);
    s.execute("comic.panel.set_locked", &json!({"panel": first, "locked": true})).expect("lock");

    // The brief: a lock covers the panel as drawn, not what a later job is asked to draw.
    s.execute("comic.panel.set_shot", &json!({"panel": first, "shot": "a locked panel's brief"})).expect("set_shot");
    assert_eq!(panel_record(&s, first).shot_description, "a locked panel's brief");
    // The words: `comic.lettering.add` writes a balloon and a record of its own and moves no panel
    // object, so the lock — which is asked in `comic.panel.move`'s direction — does not apply.
    s.execute("comic.lettering.add", &json!({"page": page, "panel": first, "kind": "narration", "text": "caption", "at": [120.0, 100.0]}))
        .expect("lettering.add on a locked panel");
    // The narrative order: it is page state, not the panel's geometry or artwork. The panel's
    // `reading_order_index` really does change, which is the point — a lock that froze it would make
    // a locked panel unorderable.
    let reversed = vec![second, first];
    let out = s.execute("comic.reading_order.set", &json!({"page": page, "order": reversed})).expect("reading_order.set");
    assert_eq!(out["reading_order"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()), Some(reversed.clone()));
    assert_eq!(panel_record(&s, first).reading_order_index, 1, "the locked panel's index was renumbered like any other");
    assert_eq!(panel_record(&s, second).reading_order_index, 0);
    assert_eq!(open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].page.reading_order[0].0, second);
    // And the geometry is exactly where it was: none of the three touched it.
    let frame = panel_record(&s, first).frame_node_id;
    let bounds = s.doc().expect("a document").doc.node(frame).and_then(|n| n.geometric_bounds()).expect("bounds");
    assert_eq!((bounds.x0, bounds.y0), (40.0, 40.0), "the locked panel never moved: {bounds:?}");
}

#[test]
fn unlocking_lets_the_refused_command_through() {
    let (mut s, _root) = project("lock-remedy");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": true})).expect("lock");
    let refused = s.execute("comic.panel.move", &json!({"panel": panel, "dx": 10.0})).expect_err("refused while locked");
    assert_locked_refusal(&refused, panel);
    // The remedy the refusal names is the remedy that works, and it is one command.
    s.execute("comic.panel.set_locked", &json!({"panel": panel, "locked": false})).expect("unlock");
    s.execute("comic.panel.move", &json!({"panel": panel, "dx": 10.0})).expect("the same call now succeeds");
    let frame = panel_record(&s, panel).frame_node_id;
    let bounds = s.doc().expect("a document").doc.node(frame).and_then(|n| n.geometric_bounds()).expect("bounds");
    assert!((bounds.x0 - 50.0).abs() < 1e-9, "{bounds:?}");
}

// ---------- M2-L2: the project's lettering face ----------

#[test]
fn the_projects_lettering_face_reaches_the_plan_the_record_and_the_missing_face_report() {
    let (mut s, _root) = project("l2-face");
    let page = s.execute("comic.page.create", &json!({})).expect("comic.page.create")["page"].as_u64().expect("page id");
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);

    // A project that names no face letters in the kind's own ADR-017 face.
    let plain = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "one", "at": [120.0, 100.0]}))
        .expect("a balloon");
    assert_eq!(lettering_of(&s, plain["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(), Some("Comic Neue"));
    // …and a sound effect in Bangers, so the pair below is about the *kind*, not about one constant.
    let bang = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "sound_effect", "text": "BAM", "at": [120.0, 100.0]}))
        .expect("a sound effect");
    assert_eq!(lettering_of(&s, bang["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(), Some("Bangers"));

    // Now the project names a face. It must reach three places: the plan's report, the record the
    // command writes, and (through both) the layout. A name no build can have makes the report
    // discriminating with or without `CRAFT_FONTS`: if the setting were ignored the report would name
    // `Bangers Regular` (or nothing, in a build that has it) and never this.
    s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": "No Such Project Face"})).expect("name a face");
    let out = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "sound_effect", "text": "POW", "at": [120.0, 100.0]}))
        .expect("a sound effect in the project's face");
    let messages = finding_messages(&out);
    assert!(
        messages.iter().any(|m| m.contains("No Such Project Face Regular")),
        "the missing-face report names the project's face, family and kind style: {out}"
    );
    // …and the face it says is *missing* is the project's, not the kind's. This pins the clause that
    // names the absent face rather than the absence of a word: the message legitimately *mentions*
    // `Bangers` when the missing face is not one of ADR-017's comic faces, because it lists them
    // (M2-L3, `comic-layout`'s `missing_face_finding`). The clause is what a silent substitution
    // would change, and it is the same sentence in every wording of the trailing explanation.
    assert!(
        messages.iter().any(|m| m.contains("the lettering face No Such Project Face Regular is not available")),
        "the report names the project's face as the one that is missing: {out}"
    );
    assert!(messages.iter().all(|m| !m.contains("the lettering face Bangers")), "the kind's default face is not what was asked for any more: {out}");
    assert_eq!(
        lettering_of(&s, out["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(),
        Some("No Such Project Face"),
        "the record persists the project's face, so a later re-fit keeps it (schema §6.6)"
    );
    // The layout still ran — the CRAFT_FONTS-empty contract is *report and lay out*, never refuse —
    // so the balloon has its frame, its text object and its words.
    let record = lettering_of(&s, out["lettering"].as_u64().expect("id"));
    assert!(s.doc().expect("a document").doc.node(record.frame_node_id).is_some(), "the frame exists");
    assert!(s.doc().expect("a document").doc.node(record.text_node_id).is_some(), "the text object exists");
    assert_eq!(text_of(&s, record.text_node_id.0), "POW");
    // A re-fit reads the record, not the kind, so the face survives `set_text`.
    s.execute("comic.lettering.set_text", &json!({"lettering": record.id.0, "text": "POW!"})).expect("set_text");
    assert_eq!(lettering_of(&s, record.id.0).text_layout_settings.font.as_deref(), Some("No Such Project Face"));

    // An explicit per-lettering `font` still wins: the setting is a default, not an override.
    let own = s
        .execute(
            "comic.lettering.add",
            &json!({"page": page, "panel": panel, "kind": "speech", "text": "mine", "style": {"font": "Some Other Face"}, "at": [120.0, 100.0]}),
        )
        .expect("a balloon with its own face");
    assert_eq!(lettering_of(&s, own["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(), Some("Some Other Face"));

    // And clearing the setting restores the kind's faces.
    s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": null})).expect("clear");
    let back = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "sound_effect", "text": "ZAP", "at": [120.0, 100.0]}))
        .expect("a sound effect again");
    assert_eq!(lettering_of(&s, back["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(), Some("Bangers"));
}

#[test]
fn the_project_settings_are_settable_readable_and_independent() {
    let (mut s, _root) = project("l2-settings");
    let first = s.execute("comic.page.create", &json!({})).expect("comic.page.create");
    assert_eq!(first["artboard_rect"][2].as_f64(), Some(612.0), "the create-time default page size");

    let initial = s.execute("comic.project.settings.get", &json!({})).expect("settings.get");
    assert_eq!(initial["default_lettering_style"], Value::Null, "a project created without a choice names no face");
    assert_eq!(initial["default_template"], Value::Null);
    assert_eq!(initial["default_page_size"]["width"].as_f64(), Some(612.0));

    // Set one thing: the other two are left exactly as they were.
    let set = s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": "My Comic Face"})).expect("settings.set");
    assert_eq!(set["default_lettering_style"].as_str(), Some("My Comic Face"));
    assert_eq!(set["default_page_size"]["height"].as_f64(), Some(792.0), "the omitted key was left alone");
    assert_eq!(s.execute("comic.project.settings.get", &json!({})).expect("settings.get")["default_lettering_style"].as_str(), Some("My Comic Face"));

    // The page size is read too: a new page with no `size` takes the new default, and the page
    // already on disk now differs from it, which rule 15 reports.
    s.execute("comic.project.settings.set", &json!({"defaultPageSize": [200.0, 300.0]})).expect("a new default size");
    let second = s.execute("comic.page.create", &json!({})).expect("a page at the new default");
    assert_eq!(second["artboard_rect"][2].as_f64(), Some(200.0), "comic.page.create reads default_page_size");
    let report = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    assert!(
        report["findings"].as_array().expect("findings").iter().any(|f| f["rule"] == 15),
        "rule 15 compares the first page's artboard against the new default: {report}"
    );

    // `null` clears, and a blank name is refused rather than silently meaning "no face".
    s.execute("comic.project.settings.set", &json!({"defaultPageSize": null})).expect("clear the size");
    assert_eq!(s.execute("comic.project.settings.get", &json!({})).expect("get")["default_page_size"], Value::Null);
    assert!(bad_params(&s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": "   "})).expect_err("blank")));
    assert!(bad_params(&s.execute("comic.project.settings.set", &json!({"defaultLetteringStyle": 5})).expect_err("not a name")));
    assert!(bad_params(
        &s.execute("comic.project.settings.set", &json!({"defaultPageSize": {"width": -1.0, "height": 10.0}})).expect_err("bad size")
    ));
    // A `set` that names nothing this command accepts is refused rather than answered with `Ok`:
    // otherwise a mistyped key is reported as a change that was made. `default_template` is
    // deliberately not among the keys (nothing reads it, M2-W3-F2) — and that is exactly the case
    // this refusal is for.
    assert!(bad_params(&s.execute("comic.project.settings.set", &json!({"defaultTemplate": "three-tier"})).expect_err("a key nothing reads")));
    assert!(bad_params(&s.execute("comic.project.settings.set", &json!({})).expect_err("nothing to set")));
}

// ---------- W1's hand-off: the migration reaches the caller ----------

/// Copy a directory tree — the v1 fixture is **read-only to this test**, because W1's fixture is
/// checked in and byte-pinned, so every write happens in the copy.
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("create the destination");
    for entry in std::fs::read_dir(from).expect("read the fixture").flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy a file");
        }
    }
}

#[test]
fn opening_an_older_bundle_reports_the_migration_and_a_save_retires_it() {
    let v1 = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../comic-model/tests/fixtures/v1/minimal.comiccraft");
    assert!(v1.join("manifest.json").is_file(), "W1's v1 fixture is the anchor for this");
    let pinned = vectorcraft_comic_model::sha256_hex(&std::fs::read(v1.join("manifest.json")).expect("read the fixture manifest"));
    let root = scratch("migration").join("v1-copy.comiccraft");
    copy_tree(&v1, &root);

    let mut s = Session::new();
    let out = s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open on a v1 bundle");
    // §11: a migration is reported, as a **value** and not as a §14 finding.
    let migrations = out["migrations"].as_array().expect("migrations");
    assert_eq!(migrations.len(), 1, "a v1 bundle is read forward once: {out}");
    assert_eq!(migrations[0]["from"].as_u64(), Some(1));
    assert_eq!(migrations[0]["to"].as_u64(), Some(2));
    assert!(migrations[0]["summary"].as_str().is_some_and(|s| s.contains("schema version 1 to 2")), "{out}");
    assert!(migrations[0]["notes"].as_array().is_some_and(|a| !a.is_empty()), "a migration with nothing to report would be unobservable: {out}");
    assert!(out["migrationsNote"].as_str().is_some_and(|s| s.contains("older, not inconsistent")), "{out}");
    assert_eq!(out["findings"].as_array().map(Vec::len), Some(0), "a migrated bundle is older, not inconsistent: {out}");

    // `comic.project.validate` mentions it too, and still not as a finding: the error count must not
    // depend on the age of the file.
    let report = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate");
    assert_eq!(report["migrations"].as_array().map(Vec::len), Some(1), "validate carries the load's migration: {report}");
    assert_eq!(report["errors"].as_u64(), Some(0));

    // A save writes this build's schema version, so the migration it reported is spent — and a later
    // validate that still said "read forward" would be asserting a cause it has not checked.
    s.execute("comic.project.save", &json!({})).expect("save the copy");
    let after = s.execute("comic.project.validate", &json!({})).expect("comic.project.validate after the save");
    assert_eq!(after["migrations"].as_array().map(Vec::len), Some(0), "the save retired it: {after}");
    assert!(after["migrationsNote"].is_null());
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("the saved manifest")).expect("json");
    assert_eq!(manifest["schema_version"].as_u64(), Some(u64::from(vectorcraft_comic_model::SCHEMA_VERSION)), "the copy is current now");
    // Reopening the copy reports nothing to migrate, which is the other half of the same claim.
    let mut again = Session::new();
    let reopened = again.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("reopen the copy");
    assert_eq!(reopened["migrations"].as_array().map(Vec::len), Some(0), "a current bundle is not a migration: {reopened}");

    // The fixture itself was only ever read.
    let after_hash = vectorcraft_comic_model::sha256_hex(&std::fs::read(v1.join("manifest.json")).expect("read the fixture manifest"));
    assert_eq!(pinned, after_hash, "W1's v1 fixture is byte-stable: this test copied it instead of writing to it");
}

#[test]
fn the_v1_fixtures_own_named_face_is_applied_to_a_lettering_on_it() {
    // The same test as `the_projects_lettering_face_reaches…`, on a **real** project that names a
    // face: W1's v1 fixture sets `default_lettering_style: "Comic Neue"`, and `sound_effect`'s kind
    // default is `Bangers`, so the record says which one the code read.
    let v1 = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../comic-model/tests/fixtures/v1/minimal.comiccraft");
    let root = scratch("migration-face").join("v1-copy.comiccraft");
    copy_tree(&v1, &root);
    let mut s = Session::new();
    s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("comic.project.open");
    assert_eq!(open_comic(&s).bundle.project.settings.default_lettering_style.as_deref(), Some("Comic Neue"));
    let (page, panel) = {
        let b = &open_comic(&s).bundle;
        let (id, file) = b.pages.iter().next().expect("the fixture has a page");
        (*id, file.panels[0].id)
    };
    let out = s
        .execute("comic.lettering.add", &json!({"page": page.0, "panel": panel.0, "kind": "sound_effect", "text": "BAM"}))
        .expect("a sound effect on the fixture's page");
    assert_eq!(
        lettering_of(&s, out["lettering"].as_u64().expect("id")).text_layout_settings.font.as_deref(),
        Some("Comic Neue"),
        "the project's named face, not the kind's Bangers: {out}"
    );
}
