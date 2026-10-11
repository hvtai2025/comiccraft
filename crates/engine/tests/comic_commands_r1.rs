//! Round-1 review tests for the M1 W4 `comic.*` command surface, by `code-reviewer`.
//!
//! These are independent of the implementer's inline tests in `crates/engine/src/cmd/comic/tests.rs`
//! and aim at what those do not cover: the doc/model consistency of undo under a bounded history,
//! the stale `artboard_id` that a non-comic command can resurrect, a reading-order gap that one
//! renumbering path repairs silently, a `Dialogue` two panels come to own, a mid-operation failure's
//! atomicity, and the two guarantees whose *plausible but wrong* implementation the brief asks for.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;
use vectorcraft_engine::doc::{Document, NodeId, NodeKind};

// ---------- fixtures ----------

/// A scratch directory inside the workspace's `target/`: never a system temporary directory and
/// never a tracked one (`target/` is git-ignored, so the `assets` and `brands` gates never see a
/// test bundle).
fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-review-r1").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    s.execute("comic.project.create", &json!({"path": root.to_string_lossy(), "title": "R1"})).expect("comic.project.create");
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

fn open_comic_mut(s: &mut Session) -> &mut OpenComic {
    Arc::make_mut(s.doc_mut().expect("a document").comic.as_mut().expect("an open project"))
}

fn doc(s: &Session) -> Arc<Document> {
    s.doc().expect("a document").doc.clone()
}

fn validate(s: &mut Session) -> Value {
    s.execute("comic.project.validate", &json!({})).expect("comic.project.validate")
}

fn rules_of(v: &Value) -> Vec<u64> {
    v["findings"].as_array().expect("findings").iter().filter_map(|f| f["rule"].as_u64()).collect()
}

fn panel_record(s: &Session, id: u64) -> vectorcraft_comic_model::Panel {
    open_comic(s).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == id).cloned().expect("the panel record")
}

fn dialogue_record(s: &Session, id: u64) -> vectorcraft_comic_model::Dialogue {
    open_comic(s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id.0 == id).cloned().expect("the dialogue record")
}

fn shown_text(s: &Session, node: NodeId) -> String {
    match &doc(s).node(node).expect("the text object").kind {
        NodeKind::Text(t) => t.plain_text(),
        other => panic!("not a type object: {other:?}"),
    }
}

/// A PNG on disk plus the `Asset` record that names it (the implementer's own helper: M1 has no
/// `comic.asset.*` command yet).
fn add_asset(s: &mut Session, root: &std::path::Path, relative: &str) -> u64 {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("assets dir");
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 40, 40, 255]));
    img.save(&path).expect("write the png");
    let bytes = std::fs::read(&path).expect("read the png");
    let hash = vectorcraft_comic_model::sha256_hex(&bytes);
    let oc = open_comic_mut(s);
    let id = oc.bundle.allocate::<vectorcraft_comic_model::AssetId>().expect("an asset id");
    oc.bundle.assets.assets.push(vectorcraft_comic_model::Asset {
        id,
        asset_type: vectorcraft_comic_model::AssetType::Source,
        storage_reference: vectorcraft_comic_model::StorageRef::Linked { relative_path: relative.to_string() },
        content_hash: vectorcraft_comic_model::ContentHash { algorithm: "sha256".into(), value: hash },
        media_metadata: vectorcraft_comic_model::MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: None },
        provenance: None,
        parent_asset_ids: Vec::new(),
        approval_status: vectorcraft_comic_model::ApprovalStatus::Pending,
        unknown: Default::default(),
    });
    id.0
}

/// Every model `NodeId` that does not resolve in the document, plus every frame object in the
/// document that no record names. This is the invariant design §6.1/§11.2 exist to keep.
fn doc_model_violations(s: &Session) -> Vec<String> {
    let d = doc(s);
    let b = &open_comic(s).bundle;
    let mut out = Vec::new();
    for file in b.pages.values() {
        for panel in &file.panels {
            if d.node(panel.frame_node_id).is_none() {
                out.push(format!("Panel({}).frame_node_id NodeId({}) is not in the document", panel.id.0, panel.frame_node_id.0));
            }
            for a in &panel.artwork_refs {
                if d.node(a.node_id).is_none() {
                    out.push(format!("Panel({}).artwork_refs NodeId({}) is not in the document", panel.id.0, a.node_id.0));
                }
            }
        }
        for l in &file.lettering {
            for (what, id) in [("frame_node_id", Some(l.frame_node_id)), ("tail_node_id", l.tail_node_id), ("text_node_id", Some(l.text_node_id))] {
                if let Some(id) = id
                    && d.node(id).is_none()
                {
                    out.push(format!("Lettering({}).{what} NodeId({}) is not in the document", l.id.0, id.0));
                }
            }
        }
    }
    let mut named: Vec<NodeId> = Vec::new();
    d.walk(|n| {
        if n.name.as_deref().is_some_and(|name| name.ends_with(" frame") || name.ends_with(" text")) {
            named.push(n.id);
        }
    });
    for id in named {
        let owned = b.pages.values().flat_map(|f| f.panels.iter()).any(|p| p.frame_node_id == id)
            || b.pages.values().flat_map(|f| f.lettering.iter()).any(|l| l.frame_node_id == id || l.text_node_id == id || l.tail_node_id == Some(id));
        if !owned {
            out.push(format!("document object NodeId({}) is a panel or lettering object no record names", id.0));
        }
    }
    out
}

// ---------- 1. the artboard-id hazard, from the side mitigation 4 does not cover ----------

/// Design §5.3.1 / schema §9.1. The comic layer refuses to **reuse a retired** id (mitigation 4).
/// An artboard removed by the ordinary, still-registered `artboard.delete` (`Window > Artboards`)
/// is never retired, so `comic.page.create` hands that exact number to a new page while the old page
/// still names it — the silent page-identity drift §5.3.1 exists to prevent — and the rule-4 finding
/// that was *reporting* the breakage disappears, which is the reporting half of mitigation 3 gone.
#[test]
fn r1_artboard_delete_then_page_create_does_not_resurrect_a_stale_artboard_id() {
    let (mut s, _root) = project("artboard-resurrect");
    let first = page_of(&mut s);
    let second = s.execute("comic.page.create", &json!({"size": [200.0, 200.0]})).expect("page 2")["page"].as_u64().expect("id");
    assert_ne!(first, second);
    let second_artboard = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(second)].page.artboard_id;
    assert_eq!(second_artboard, 2, "the second page takes the highest artboard");

    let index = doc(&s).artboards.iter().position(|a| a.id == second_artboard).expect("the artboard");
    s.execute("artboard.delete", &json!({"index": index})).expect("artboard.delete");

    let broken = validate(&mut s);
    assert!(rules_of(&broken).contains(&4), "rule 4 reports the missing artboard first: {broken}");
    assert!(
        !open_comic(&s).bundle.manifest.retired_artboard_ids.contains(&second_artboard),
        "the id is not retired: nothing comic owns the deletion"
    );

    let fresh = s.execute("comic.page.create", &json!({"size": [200.0, 200.0]})).expect("a fresh page");
    let fresh_page = fresh["page"].as_u64().expect("id");
    let sharing = open_comic(&s).bundle.pages.values().filter(|f| f.page.artboard_id == second_artboard).count();
    assert_eq!(sharing, 1, "FAIL: pages {second} and {fresh_page} both resolve to artboard {second_artboard} ({fresh})");
    assert!(
        rules_of(&validate(&mut s)).contains(&4),
        "FAIL: page {second}'s dangling artboard_id was silently resolved to page {fresh_page}'s artboard — the \
         rule-4 report vanished instead of the drift being refused (§5.3.1: a stale Page.artboard_id must never \
         silently resolve to a different page)"
    );
}

// ---------- 2. §11.1's reading-order row on the one renumbering path that skips the guard ----------

/// Design §11.1: "`Page.reading_order` — renumbered in the same operation; a gap is a validation
/// error, **never a silent repair**". `comic.panel.add`, `comic.panel.delete`, `comic.page.delete`
/// and `comic.reading_order.set` all run `require_permutation` first. `comic.page.template.apply`'s
/// re-lay branch does not, so it heals a gap behind the caller's back.
#[test]
fn r1_the_guarded_paths_refuse_a_reading_order_gap_and_template_apply_does_not_repair_it() {
    let (mut s, _root) = project("gap-repair");
    let page = page_of(&mut s);
    s.execute("comic.page.template.apply", &json!({"page": page, "template": "three-tier"})).expect("three panels");
    let panels: Vec<u64> = open_comic(&s).bundle.pages[&vectorcraft_comic_model::PageId(page)].panels.iter().map(|p| p.id.0).collect();

    // A gap, as an interrupted save or a hand edit leaves it.
    {
        let oc = open_comic_mut(&mut s);
        let file = oc.bundle.pages.get_mut(&vectorcraft_comic_model::PageId(page)).expect("the page file");
        file.page.reading_order.truncate(2);
    }
    assert!(rules_of(&validate(&mut s)).contains(&6), "rule 6 reports the gap before the command runs");

    // The deletion paths refuse, and change nothing. Verified first so the contrast is real.
    // (`comic.reading_order.set` is *given* a full permutation, so writing one is its job; the
    // report's "`require_permutation` runs first" is true of `remove_panel` and of the argument
    // check, not of the page's current state.)
    assert!(s.execute("comic.panel.delete", &json!({"panel": panels[0]})).is_err(), "panel.delete refuses a gapped page");
    assert!(s.execute("comic.page.delete", &json!({"page": page})).is_err(), "page.delete refuses a gapped page");
    assert!(rules_of(&validate(&mut s)).contains(&6), "and the gap is still there");

    // The renumbering path that skips the guard.
    s.execute("comic.page.template.apply", &json!({"page": page, "template": "three-tier"})).expect("re-lay the same template");
    assert!(
        rules_of(&validate(&mut s)).contains(&6),
        "FAIL: comic.page.template.apply silently repaired the reading-order gap — §11.1 says a gap is a validation \
         error, never a silent repair, and the four sibling commands all refuse it"
    );
}

// ---------- 3. one Dialogue, two panels ----------

/// `comic.lettering.add` accepts `dialogue:` and never requires that the dialogue belongs to the
/// panel the balloon is going in. Two panels then list one `Dialogue` in `dialogue_ids` (design §6.1:
/// "avoid storing two independently editable authoritative copies"), `comic.dialogue.update` updates
/// only one of the two balloons that show it, and deleting the owning panel destroys the other
/// panel's words. Every consequence is collected before failing, so the report names all three.
#[test]
fn r1_lettering_add_should_not_let_two_panels_own_one_dialogue() {
    let (mut s, _root) = project("shared-dialogue");
    let page = page_of(&mut s);
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let in_b = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "text": "B says", "at": [340.0, 100.0]}))
        .expect("a balloon in panel B");
    let dialogue_b = in_b["dialogue"].as_u64().expect("the dialogue it made");

    let in_a = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "dialogue": dialogue_b, "at": [120.0, 100.0]}))
        .expect("a balloon in panel A reusing B's dialogue");
    let lettering_a = in_a["lettering"].as_u64().expect("id");
    let text_node_a =
        open_comic(&s).bundle.pages.values().flat_map(|f| f.lettering.iter()).find(|l| l.id.0 == lettering_a).expect("the lettering").text_node_id;

    let mut failures: Vec<String> = Vec::new();

    let owners: Vec<u64> = open_comic(&s)
        .bundle
        .pages
        .values()
        .flat_map(|f| f.panels.iter())
        .filter(|p| p.dialogue_ids.iter().any(|d| d.0 == dialogue_b))
        .map(|p| p.id.0)
        .collect();
    if owners != vec![b] {
        failures.push(format!("dialogue {dialogue_b} is owned by panels {owners:?}: its own {b} and the balloon's {a} (§6.1)"));
    }

    // Consequence 1: one owner, two renderers — the update reaches only one of them.
    s.execute("comic.dialogue.update", &json!({"dialogue": dialogue_b, "text": "B says, revised"})).expect("dialogue.update");
    let shown = shown_text(&s, text_node_a);
    let record = dialogue_record(&s, dialogue_b).text;
    if shown != record {
        failures.push(format!("the balloon in panel {a} still shows {shown:?} while the Dialogue says {record:?} — the visual object became a second source of dialogue truth (§6.1)"));
    }

    // Consequence 2: the panel that owns the record takes the other panel's words with it.
    s.execute("comic.panel.delete", &json!({"panel": b})).expect("comic.panel.delete");
    if open_comic(&s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).any(|d| d.id.0 == dialogue_b) {
        failures.push(format!("dialogue {dialogue_b} survived panel {b}'s deletion"));
    } else if rules_of(&validate(&mut s)).contains(&1) {
        failures.push(format!(
            "deleting panel {b} deleted the shared dialogue, so panel {a}'s balloon now names a missing record (rule 1): the words are gone"
        ));
    }

    assert!(failures.is_empty(), "FAIL: {}", failures.join("; "));
}

// ---------- 4. undo, the `comic` field, and a bounded history ----------

/// Design §11.2: `comic_edit` records **one** history entry per user action. It collapses the
/// document edits the command made by truncating history to the length it saw — which does nothing
/// once `push_undo` has already dropped entries off the front because `history.limit` was reached.
/// At the limit the inner edit survives: one user action becomes two undo steps (Undo twice shows no
/// change), and a genuine earlier step is evicted to make room for the duplicate.
#[test]
fn r1_a_comic_action_at_the_history_limit_is_exactly_one_undo_step() {
    let (mut s, _root) = project("limit");
    let page = page_of(&mut s);
    panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);

    // A legitimate state: `history.undo.len() == history.limit`. The default is 500 (a real session
    // reaches it), and this is the same boundary with the numbers made small.
    let limit = s.doc().expect("a document").history.undo.len();
    assert_eq!(limit, 2, "the page create and the panel add are the two recorded steps");
    s.doc_mut().expect("a document").history.limit = limit;

    let pages_before = open_comic(&s).bundle.pages.len();
    let artboards_before: Vec<u32> = doc(&s).artboards.iter().map(|a| a.id).collect();
    s.execute("comic.page.delete", &json!({"page": page})).expect("comic.page.delete");
    assert_eq!(open_comic(&s).bundle.pages.len(), 0, "the page is gone");
    assert!(doc_model_violations(&s).is_empty(), "and the delete itself is consistent");

    // Adding one step to a full journal evicts exactly one step. The inner document edits the
    // command made must be collapsed into it, so the journal is ["Add Panel", "Delete Page"].
    let labels: Vec<String> = s.doc().expect("a document").history.undo.iter().map(|e| e.label.clone()).collect();
    let mut failures: Vec<String> = Vec::new();
    if labels != ["Add Panel", "Delete Page"] {
        failures.push(format!("the journal after one action on a full history of {limit} is {labels:?}, not [\"Add Panel\", \"Delete Page\"]"));
    }

    // Undo until the state before the action comes back, and count the steps it took.
    let mut steps = 0;
    let mut model_sizes = Vec::new();
    for _ in 0..4 {
        if s.execute("edit.undo", &json!({})).is_err() {
            break;
        }
        steps += 1;
        let st = s.doc().expect("a document");
        let pages = st.comic.as_ref().map(|c| c.bundle.pages.len()).unwrap_or(0);
        model_sizes.push(pages);
        if pages == pages_before && st.doc.artboards.iter().map(|a| a.id).collect::<Vec<_>>() == artboards_before {
            break;
        }
    }
    if steps != 1 {
        failures.push(format!(
            "comic.page.delete took {steps} undo steps against a history limit of {limit} (model page counts per step: {model_sizes:?})"
        ));
    }
    let violations = doc_model_violations(&s);
    if !violations.is_empty() {
        failures.push(format!("doc/model desync after undoing back: {violations:?}"));
    }
    assert!(
        failures.is_empty(),
        "FAIL (§11.2, one logical unit per user action; `comic_edit`'s truncate cannot undo an eviction `push_undo` \
         already made): {}",
        failures.join("; ")
    );
}

/// The other half of the same boundary: the duplicate entry costs an unrelated step its place.
#[test]
fn r1_a_comic_action_at_the_history_limit_does_not_evict_an_unrelated_undo_step() {
    let (mut s, _root) = project("limit-evict");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let limit = s.doc().expect("a document").history.undo.len();
    assert_eq!(limit, 2, "New Comic Page and Add Panel");
    s.doc_mut().expect("a document").history.limit = limit;

    // `comic.panel.move` translates the frame through `Session::edit`, so it makes one inner
    // document edit that `comic_edit` must collapse into the one step the user sees.
    s.execute("comic.panel.move", &json!({"panel": panel, "dx": 5.0, "dy": 5.0})).expect("move");
    let labels: Vec<String> = s.doc().expect("a document").history.undo.iter().map(|e| e.label.clone()).collect();
    assert_eq!(
        labels.iter().filter(|l| l.as_str() == "Move Panel").count(),
        1,
        "FAIL: one Move Panel action is recorded {labels:?} — the inner document edit was not collapsed (§11.2)"
    );
    assert_eq!(
        labels,
        ["Add Panel", "Move Panel"],
        "FAIL: one step added to a full journal must evict exactly one step, not the whole history: {labels:?}"
    );
}

/// The claim in §2 of the implementation: `HistoryEntry::comic == None` is a **no-op** for every
/// step an existing command records — undoing a plain document edit must not clear an open project.
/// This is the test that rejects the plausible-wrong `st.comic = e.comic.clone()` in `edit.undo`.
#[test]
fn r1_undoing_a_plain_document_edit_leaves_the_open_project_alone() {
    let (mut s, _root) = project("plain-undo");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "keep me"})).expect("set_shot");

    let depth = s.doc().expect("a document").history.undo.len();
    let artboard_x = doc(&s).artboards[0].rect.x0;
    // A plain document edit: `artboard.setProps` is not a comic command and records `comic: None`.
    s.execute("artboard.setProps", &json!({"index": 0, "x": artboard_x + 10.0, "y": 0.0, "width": 612.0, "height": 792.0}))
        .expect("artboard.setProps");
    assert_eq!(s.doc().expect("a document").history.undo.len(), depth + 1, "the plain edit is its own step");
    assert_eq!(doc(&s).artboards[0].rect.x0, artboard_x + 10.0);

    s.execute("edit.undo", &json!({})).expect("undo the plain edit");
    assert!(s.doc().expect("a document").comic.is_some(), "FAIL: undoing a plain document edit closed the project");
    assert_eq!(doc(&s).artboards[0].rect.x0, artboard_x, "the plain edit was undone");
    assert_eq!(panel_record(&s, panel).shot_description, "keep me", "FAIL: the comic model was cleared by an undo of a plain document edit");

    // And redo is symmetric.
    s.execute("edit.redo", &json!({})).expect("redo the plain edit");
    assert!(s.doc().expect("a document").comic.is_some(), "FAIL: redo of a plain document edit closed the project");
    assert_eq!(panel_record(&s, panel).shot_description, "keep me");
    assert_eq!(doc(&s).artboards[0].rect.x0, artboard_x + 10.0);

    // A comic step still round-trips its model.
    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "changed"})).expect("set_shot");
    s.execute("edit.undo", &json!({})).expect("undo");
    assert_eq!(panel_record(&s, panel).shot_description, "keep me", "the comic step's undo restored the model");
    s.execute("edit.redo", &json!({})).expect("redo");
    assert_eq!(panel_record(&s, panel).shot_description, "changed", "and redo put it back");
}

// ---------- 5. `Severity::Error` and the save ----------

/// The claim in §5 of the implementation: a live referential `Severity::Error` does **not** block a
/// save (schema §14 exists so a damaged bundle stays openable *and repairable*). This is the test
/// that rejects the plausible-wrong "refuse the write when `validate` reports an Error": it asserts
/// the save succeeds, and that the Error, the records and the page all survive the round trip, so
/// nothing was silently dropped to make the write clean.
#[test]
fn r1_a_live_referential_error_does_not_block_a_save_and_survives_it() {
    let (mut s, root) = project("error-save");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "speech", "text": "Do not lose me", "at": [120.0, 100.0]}))
        .expect("a balloon");
    let dialogue = balloon["dialogue"].as_u64().expect("its dialogue");
    // The one command that deliberately leaves an Error: a reusable dialogue is detached, not deleted.
    s.execute("comic.panel.delete", &json!({"panel": panel, "reuse": [dialogue]})).expect("comic.panel.delete");

    let before = validate(&mut s);
    assert!(before["errors"].as_u64().unwrap_or(0) >= 1, "there is a live referential Error: {before}");
    assert_eq!(before["severityErrorBlocksSave"].as_bool(), Some(false));
    let assets_before = open_comic(&s).bundle.assets.assets.len();

    let saved = s.execute("comic.project.save", &json!({})).unwrap_or_else(|e| panic!("FAIL: a live Error blocked the save ({e})"));
    assert!(saved["files"].as_array().is_some_and(|f| !f.is_empty()), "{saved}");

    let mut reopened = Session::new();
    reopened.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("reopen");
    let after = validate(&mut reopened);
    assert_eq!(dialogue_record(&reopened, dialogue).text, "Do not lose me", "FAIL: the save lost the detached dialogue's words");
    assert!(open_comic(&reopened).bundle.pages.contains_key(&vectorcraft_comic_model::PageId(page)), "the page survived");
    assert_eq!(open_comic(&reopened).bundle.assets.assets.len(), assets_before, "no asset was dropped");
    assert!(after["errors"].as_u64().unwrap_or(0) >= 1, "FAIL: the save resolved a referential Error instead of carrying it: {after}");
}

/// The *lossy* write the implementation does refuse (the duplicate page file), and the same loss it
/// does not. Both are `prune_stale` destroying bytes the save never staged; only one is guarded.
#[test]
fn r1_save_refuses_only_one_of_the_two_ways_pruning_destroys_bytes() {
    let (mut s, root) = project("prune");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("the first save");

    // (a) two files, one id — refused, and the loser's bytes survive.
    let duplicate = root.join(format!("pages/page-{page:03}.json"));
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &duplicate).expect("a duplicate");
    let err = s.execute("comic.project.save", &json!({})).expect_err("two files encode one id");
    assert!(err.to_string().contains("both encode id"), "{err}");
    assert!(duplicate.is_file(), "the refused save left the loser alone");
    std::fs::remove_file(&duplicate).expect("clean up");

    // (b) one file, an id of its own, written since the load — the same destruction, unguarded.
    let stray = root.join("pages/page-99.json");
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &stray).expect("a page file written since the load");
    assert!(stray.is_file());
    s.execute("comic.project.save", &json!({})).expect("this save succeeds");
    assert!(
        stray.is_file(),
        "FAIL: comic.project.save silently deleted pages/page-99.json — bytes it never staged, destroyed by the same \
         prune the duplicate rule exists to catch (the refusal is an id collision, not the loss)"
    );
}

// ---------- 6. atomicity of a failure *after* the command has begun to mutate ----------

/// `comic_edit` snapshots document, selection and model and restores all three on error. The
/// implementer's malformed-call sweep only exercises failures that happen before any mutation, so
/// this drives one that happens after: `comic.panel.replace_art` removes the previous artwork and
/// places the new image, and only then finds that the panel's frame cannot clip.
#[test]
fn r1_a_failure_after_a_partial_change_restores_document_selection_and_model() {
    let (mut s, root) = project("atomic");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let first = add_asset(&mut s, &root, "assets/panels/one.png");
    let placed = s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": first})).expect("artwork");
    let group = NodeId(placed["node"].as_u64().expect("the clip group"));
    let second = add_asset(&mut s, &root, "assets/panels/two.png");

    // Break the frame so the clip step cannot run, *after* the command has already changed things.
    {
        let frame = panel_record(&s, panel).frame_node_id;
        let st = s.doc_mut().expect("a document");
        let d = Arc::make_mut(&mut st.doc);
        let node = d.node_mut(frame).expect("the frame");
        node.kind = NodeKind::Group { children: Vec::new(), clip: false };
    }

    let refs_before = panel_record(&s, panel).artwork_refs.clone();
    let selection_before = s.doc().expect("a document").selection.objects.clone();
    let history_before = s.doc().expect("a document").history.undo.len();
    let node_count_before = {
        let mut n = 0;
        doc(&s).walk(|_| n += 1);
        n
    };

    s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": second})).expect_err("the frame cannot clip");

    let mut failures: Vec<String> = Vec::new();
    if panel_record(&s, panel).artwork_refs != refs_before {
        failures.push(format!("the model's artwork_refs changed: {refs_before:?} -> {:?}", panel_record(&s, panel).artwork_refs));
    }
    if s.doc().expect("a document").selection.objects != selection_before {
        failures.push(format!("the selection changed: {selection_before:?} -> {:?}", s.doc().expect("a document").selection.objects));
    }
    if s.doc().expect("a document").history.undo.len() != history_before {
        failures.push("a history entry was recorded for a failed command".to_string());
    }
    if doc(&s).node(group).is_none() {
        failures.push(format!("the previous artwork NodeId({}) was not restored", group.0));
    }
    let node_count_after = {
        let mut n = 0;
        doc(&s).walk(|_| n += 1);
        n
    };
    if node_count_after != node_count_before {
        failures.push(format!("the document gained or lost objects: {node_count_before} -> {node_count_after}"));
    }
    assert!(failures.is_empty(), "FAIL: a failed comic command left a half-applied change (comic_edit must restore all three): {failures:?}");
}

// ---------- 7. the claims the existing sweeps must be able to see ----------

/// §3 of the implementation: `has_comic` gates all nineteen, so a comic id is `Disabled` — never
/// `Ok(Err(NoDocument))` — in a session with no document and in one with a plain document.
#[test]
fn r1_every_comic_command_is_disabled_without_a_project() {
    let ids: Vec<&str> = vectorcraft_engine::command_specs().iter().filter(|c| c.id.starts_with("comic.")).map(|c| c.id).collect();
    assert_eq!(ids.len(), 19, "the surface is nineteen commands: {ids:?}");

    // Two of the nineteen are deliberately `always`-gated (`create` and `open` — a project has to
    // start somewhere), which is a justifiable deviation from the report's "one predicate gates all
    // nineteen". Neither may ever report `NoDocument`.
    const ALWAYS: [&str; 2] = ["comic.project.create", "comic.project.open"];
    let mut empty = Session::new();
    for id in &ids {
        let err = empty.execute(id, &json!({})).expect_err("no document");
        assert!(
            !matches!(err, vectorcraft_engine::EngineError::NoDocument),
            "{id} with no document returned {err:?} — the existing sweep would see Ok(Err(NoDocument))"
        );
        if ALWAYS.contains(id) {
            assert!(matches!(err, vectorcraft_engine::EngineError::BadParams { .. }), "{id}: {err:?}");
        } else {
            assert!(matches!(err, vectorcraft_engine::EngineError::Disabled { .. }), "{id} with no document returned {err:?}, not Disabled");
        }
        assert!(!empty.in_interaction(), "{id} left an interaction open");
    }

    // A plain document, no project.
    let mut plain = Session::new();
    plain.execute("file.new", &json!({"width": 400, "height": 400})).expect("a plain document");
    assert!(plain.doc().expect("a document").comic.is_none());
    for id in &ids {
        let err = plain.execute(id, &json!({})).expect_err("no comic project");
        assert!(!matches!(err, vectorcraft_engine::EngineError::NoDocument), "{id} with a plain document returned {err:?}");
        if ALWAYS.contains(id) {
            continue;
        }
        match err {
            vectorcraft_engine::EngineError::Disabled { .. } => assert!(err.to_string().contains("comic project"), "{id}: {err}"),
            other => panic!("{id} with a plain document returned {other:?}, not Disabled"),
        }
    }
}

/// Focus §5: "the `params` string must match the JSON actually accepted". The output-side half of
/// that for `comic.page.delete`, whose params string names the deletion totals as top-level keys.
#[test]
fn r1_page_delete_returns_the_keys_its_params_string_documents() {
    let (mut s, _root) = project("contract");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.lettering.add", &json!({"page": page, "panel": panel, "kind": "narration", "text": "caption"})).expect("a caption");

    let out = s.execute("comic.page.delete", &json!({"page": page})).expect("comic.page.delete");
    const DOCUMENTED: [&str; 9] = [
        "page",
        "artboard_id",
        "deletedPanels",
        "removedNodes",
        "letteringDeleted",
        "dialogueDeleted",
        "dialogueDetached",
        "assetsKept",
        "jobsStillTargeting",
    ];
    let missing: Vec<&str> = DOCUMENTED.iter().copied().filter(|k| out.get(*k).is_none()).collect();
    assert!(
        missing.is_empty(),
        "FAIL: the params string documents {DOCUMENTED:?} but the result nests the deletion totals under `deletion`, \
         so {missing:?} are missing from {out}"
    );
}

/// §11.1's `Asset` row: lineage outlives the panel *and* the page. `comic.page.delete` applies the
/// per-panel policy, so it must keep every `Asset` record too.
#[test]
fn r1_page_delete_keeps_every_asset_record() {
    let (mut s, root) = project("page-assets");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let asset = add_asset(&mut s, &root, "assets/panels/p.png");
    s.execute("comic.panel.replace_art", &json!({"panel": panel, "asset": asset})).expect("artwork");

    let before = open_comic(&s).bundle.assets.assets.len();
    let nodes_before = {
        let mut n = 0;
        doc(&s).walk(|_| n += 1);
        n
    };
    let out = s.execute("comic.page.delete", &json!({"page": page})).expect("comic.page.delete");
    assert_eq!(open_comic(&s).bundle.assets.assets.len(), before, "FAIL: comic.page.delete removed an Asset record: {out}");
    let kept = out["deletion"]["assetsKept"].as_array().map(Vec::len).unwrap_or(0);
    assert_eq!(kept, 1, "and it reports the asset it kept: {out}");
    // §11.1 row 1/3: the lettering frame, tail, text AND the artwork objects go with the panel —
    // the whole clip group subtree, not just the group node. Only the artboard's own decoration
    // may remain, so the document must be empty of panel objects afterwards.
    let nodes_after = {
        let mut n = 0;
        doc(&s).walk(|_| n += 1);
        n
    };
    assert!(nodes_after < nodes_before, "the panel's objects were not removed ({nodes_before} -> {nodes_after})");
    assert!(doc_model_violations(&s).is_empty(), "FAIL: orphaned panel objects remain after comic.page.delete: {:?}", doc_model_violations(&s));
}

// ---------- 8. the detached-dialogue Error: is a normal action allowed to break validation forever? ----------

/// Design §11.1's `Dialogue` row: "deleted with the panel unless flagged reusable, in which case
/// detached and reported, **so the text is moved rather than retyped**". Schema §6.3 makes
/// `Dialogue.panel_id` a required `PanelId`, so "detached" can only mean "still names the panel that
/// was just deleted" — which is why the command leaves a live rule-1 Error. The row's own promise is
/// that the words are *moved*; this checks that the move is actually possible with M1's commands, by
/// doing the documented repair (reuse the same Dialogue from another panel) and asking validation
/// again. If it cannot be moved, `comic.panel.delete {reuse: ...}` hands the creator a project that
/// no M1 command can bring back to a clean state.
#[test]
fn r1_the_documented_repair_for_a_detached_dialogue_exists_in_m1() {
    let (mut s, _root) = project("detach-repair");
    let page = page_of(&mut s);
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "text": "Move me", "at": [120.0, 100.0]}))
        .expect("a balloon");
    let dialogue = balloon["dialogue"].as_u64().expect("its dialogue");

    s.execute("comic.panel.delete", &json!({"panel": a, "reuse": [dialogue]})).expect("comic.panel.delete");
    let detached = validate(&mut s);
    assert!(detached["errors"].as_u64().unwrap_or(0) >= 1, "the detachment is reported, as §11.1 says: {detached}");
    assert!(rules_of(&detached).contains(&1), "{detached}");

    // The move: put the same words in another panel, reusing the detached Dialogue.
    s.execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "dialogue": dialogue, "at": [340.0, 100.0]}))
        .expect("the documented way to move the text");
    assert_eq!(dialogue_record(&s, dialogue).text, "Move me", "the words survived, which is the point of detaching");

    let moved = validate(&mut s);
    assert!(
        !rules_of(&moved).contains(&1),
        "FAIL: after the only repair M1 offers, the detached Dialogue still names the deleted panel {} — nothing in \
         the command surface can re-attach it (comic.dialogue.update has no panel, comic.lettering.add links a new \
         balloon but leaves Dialogue.panel_id alone, and there is no comic.dialogue.move), so 'detached and reported' \
         is a one-way door into a project that fails validation for good: {moved}",
        a
    );
}

// ---------- 9. is the open project tracked as unsaved work? ----------

/// Focus §1's last question — "is `DocState::comic` a second source of truth that can drift from the
/// bundle?" — from the save side. It is: the model is a working copy the on-disk bundle does not
/// know about until `comic.project.save`, and nothing here marks the document modified when only the
/// model changed. `DocState::is_dirty()` is what the title-bar asterisk, the Save enablement and the
/// quit prompt read (`Is that true?` — `crates/ui-egui/src/{chrome,graphics,menus}.rs`), and
/// `comic_edit` advances `revision` without touching `saved_doc`, so a comic-only edit leaves the
/// document "clean" and closing the tab discards it without asking.
#[test]
fn r1_a_comic_only_edit_counts_as_unsaved_work() {
    let (mut s, _root) = project("dirty");
    let page = page_of(&mut s);
    let panel = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    s.execute("comic.project.save", &json!({})).expect("save");
    assert!(!s.doc().expect("a document").is_dirty(), "a saved project is clean");

    s.execute("comic.panel.set_shot", &json!({"panel": panel, "shot": "only the model changed"})).expect("set_shot");
    let dirty = s.doc().expect("a document").is_dirty();
    // Undo it, then change only the model again, so this is not about the document half.
    assert!(
        dirty,
        "FAIL: a comic-only change (Panel.shot_description) leaves the document clean, so is_dirty() is \
                    false: no asterisk, Save stays disabled and the quit prompt does not fire — the edit is lost on close"
    );

    s.execute("comic.reading_order.set", &json!({"page": page, "order": [panel]})).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.doc().expect("a document").is_dirty(), "FAIL: a reading-order-only change is likewise invisible");
}
