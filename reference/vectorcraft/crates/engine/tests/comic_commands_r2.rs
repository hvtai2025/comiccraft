//! Round-2 review tests for the M1 W4 `comic.*` command surface, by `code-reviewer`.
//!
//! Round 1 found eight bugs. Round 2 verifies the fixes against round 1's own tests (they are the
//! acceptance tests) and probes the four behaviours the round-1 fixes *introduced* or left
//! unspecified: the new `prune_stale` rule's reach, the cross-page `Lettering -> Dialogue`
//! reference, and the state an adoption leaves behind.
// Integration tests: unwrapping is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::{Value, json};
use vectorcraft_comic_model::{DialogueId, PageId};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;

// ---------- fixtures (the same shape as `comic_commands_r1.rs`) ----------

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-review-r2").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
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

fn rules_of(v: &Value) -> Vec<u64> {
    v["findings"].as_array().expect("findings").iter().filter_map(|f| f["rule"].as_u64()).collect()
}

fn dialogue_record(s: &Session, id: u64) -> vectorcraft_comic_model::Dialogue {
    open_comic(s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id.0 == id).cloned().expect("the dialogue record")
}

fn panel_record(s: &Session, id: u64) -> vectorcraft_comic_model::Panel {
    open_comic(s).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == id).cloned().expect("the panel record")
}

fn read_manifest(root: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("read manifest.json")).expect("manifest json")
}

// ---------- (1) F7's remedy, judged by what happens on the *next* save ----------

/// Round 1's F7: `comic.project.save` must not destroy bytes it never staged, and the round-1 fix
/// makes `prune_stale` skip a `pages/` file the previous `manifest.files` did not describe. But the
/// save *writes a manifest that then describes the file* (§4: `files` covers every file in the
/// bundle). So the protection lasts exactly one save: the file the fix just preserved is declared
/// by the generation of record, and the **next** save prunes it under the ordinary rule. If so, the
/// loss round 1 reported is not removed — it is deferred by one save, and the second save is still
/// silent.
#[test]
fn r2_a_page_file_written_since_the_load_is_destroyed_by_the_next_save() {
    let (mut s, root) = project("stray-two-saves");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");

    let n = 99u64;
    let stray = root.join(format!("pages/page-{n}.json"));
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &stray).expect("a page file written since the load");

    s.execute("comic.project.save", &json!({})).expect("save 2");
    assert!(stray.is_file(), "save 2 must keep it — this is round 1's F7 acceptance criterion");
    assert!(read_manifest(&root)["files"].get(format!("pages/page-{n}.json")).is_some(), "and the manifest save 2 wrote describes it (§4)");

    // The very next save has a *previous* manifest that describes the file, so `prune_stale`'s
    // existing branch applies. The round-1 finding's own ground — a file the save never staged is
    // destroyed with no report — is unchanged by the fix if this assertion fails.
    let out = s.execute("comic.project.save", &json!({})).expect("save 3");
    let _ = out;
    assert!(
        stray.is_file(),
        "FAIL: the third save silently deleted pages/page-{n}.json — save 2's manifest declared it, so `prune_stale` \
         treated it as 'a page this save replaced or dropped' and destroyed bytes the bundle never held. The F7 fix \
         protects the file for exactly one save; §12's promise (a load-and-save does not destroy what it does not \
         understand) is a property of every save, not of the first one"
    );
}

// ---------- (2) the ninth finding: a cross-page `Lettering -> Dialogue` ----------

/// The ninth finding (round 1's report §R-13): `comic.lettering.add {dialogue}` accepts a dialogue
/// owned by a panel on **another page**. Schema rule 1 only requires `Lettering.dialogue_id` to
/// resolve, schema rule 5 checks only the lettering's own artboard, and no §13 rule ties a
/// `Lettering` to its dialogue's page. So the reference is *unforbidden*, and this characterises
/// what M1 does with it: does it resolve, does it validate clean, does the owning panel's deletion
/// clean the balloon up, and does `comic.lettering.set_text` still reach the words?
#[test]
fn r2_lettering_add_accepts_a_dialogue_from_another_page_and_nothing_reports_it() {
    let (mut s, _root) = project("cross-page");
    let p1 = page_of(&mut s);
    let a = panel_on(&mut s, p1, [40.0, 40.0, 200.0, 150.0]);
    let p2 = page_of(&mut s);
    // Page 2's artboard sits at x = 632 in document coordinates, and a panel rect is
    // [x, y, width, height], absolute.
    let c = panel_on(&mut s, p2, [672.0, 40.0, 160.0, 110.0]);

    let in_c = s
        .execute("comic.lettering.add", &json!({"page": p2, "panel": c, "kind": "speech", "text": "C says", "at": [750.0, 100.0]}))
        .expect("a balloon on page 2");
    let d = in_c["dialogue"].as_u64().expect("its dialogue");

    // A balloon on page 1 that *shows* page 2's dialogue.
    let in_a = s
        .execute("comic.lettering.add", &json!({"page": p1, "panel": a, "kind": "speech", "dialogue": d, "at": [120.0, 100.0]}))
        .expect("a balloon on page 1 showing a dialogue owned by a panel on page 2");

    // The record keeps its owner, exactly as the in-page case does: one owner, two renderers.
    assert_eq!(dialogue_record(&s, d).panel_id.0, c, "the owner is unchanged");
    assert!(panel_record(&s, a).dialogue_ids.is_empty(), "the balloon's panel did not claim it");
    assert!(panel_record(&s, c).dialogue_ids.contains(&DialogueId(d)), "the owner still lists it");
    assert_eq!(panel_record(&s, a).lettering_ids.len(), 1, "the lettering is on the balloon's panel");
    let l = open_comic(&s).bundle.pages[&PageId(p1)].lettering.iter().find(|x| x.id.0 == in_a["lettering"].as_u64().unwrap()).unwrap().clone();
    assert_eq!(l.page_id.0, p1, "the Lettering is filed on page 1");
    assert_eq!(l.artboard_id, open_comic(&s).bundle.pages[&PageId(p1)].page.artboard_id, "rule 5 is satisfied");

    // Nothing reports it: no rule covers a cross-page dialogue reference.
    let v = validate(&mut s);
    assert_eq!(v["errors"].as_u64(), Some(0), "the cross-page reference is not an error: {v}");
    assert!(!rules_of(&v).contains(&1), "rule 1 resolves it: {v}");

    // `comic.dialogue.update` reaches the balloon on the other page (R-3's bundle-wide scan).
    s.execute("comic.dialogue.update", &json!({"dialogue": d, "text": "C says, revised"})).expect("dialogue.update");
    let shown = {
        let node = l.text_node_id;
        match &s.doc().unwrap().doc.node(node).unwrap().kind {
            vectorcraft_engine::doc::NodeKind::Text(t) => t.plain_text(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(shown, "C says, revised", "the balloon on page 1 shows the record's new words");

    // `comic.lettering.set_text` is the asymmetry: it looks the dialogue up on the *lettering's*
    // page, so a cross-page balloon cannot be retyped through it. It errors rather than corrupts.
    let set = s.execute("comic.lettering.set_text", &json!({"lettering": in_a["lettering"], "text": "retyped"}));
    assert!(set.is_err(), "set_text refuses a cross-page balloon rather than editing the wrong record: {set:?}");

    // Deleting the owning panel cleans the balloon up, wherever it is (R-3's third change).
    s.execute("comic.panel.delete", &json!({"panel": c})).expect("comic.panel.delete");
    assert!(
        !open_comic(&s).bundle.pages[&PageId(p1)].lettering.iter().any(|x| x.id.0 == in_a["lettering"].as_u64().unwrap()),
        "the balloon went with its words"
    );
    let after = validate(&mut s);
    assert_eq!(after["errors"].as_u64(), Some(0), "and the cross-page balloon left nothing dangling: {after}");
}

// ---------- (3) the state an adoption leaves ----------

/// F8's adoption (`comic.lettering.add` re-homes a detached dialogue) must leave the record as an
/// owner's record: the adopting panel's `dialogue_ids` gains it, so a later `comic.panel.delete`
/// applies §11.1 to it. Round 1's own F8 test and the implementer's pin the owner and the Error;
/// this pins the *list* — the value, not the direction — so an "adopt by setting `panel_id` only"
/// implementation is rejected.
#[test]
fn r2_adoption_records_the_dialogue_in_the_adopting_panels_dialogue_ids() {
    let (mut s, _root) = project("adopt-list");
    let page = page_of(&mut s);
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "text": "Move me", "at": [120.0, 100.0]}))
        .expect("a balloon");
    let d = balloon["dialogue"].as_u64().expect("its dialogue");

    s.execute("comic.panel.delete", &json!({"panel": a, "reuse": [d]})).expect("detach");
    s.execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "dialogue": d, "at": [340.0, 100.0]})).expect("adopt");

    assert_eq!(dialogue_record(&s, d).panel_id.0, b, "the record names B");
    assert!(panel_record(&s, b).dialogue_ids.contains(&DialogueId(d)), "FAIL: the adopting panel does not list the dialogue it owns");

    // The consequence the list exists for: deleting B applies §11.1 to the dialogue.
    s.execute("comic.panel.delete", &json!({"panel": b})).expect("delete the adopter");
    assert!(
        !open_comic(&s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).any(|x| x.id.0 == d),
        "FAIL: the dialogue survived its owner's deletion — it was never in the owner's dialogue_ids"
    );
    assert!(!rules_of(&validate(&mut s)).contains(&1), "and nothing dangles");
}

/// The ninth finding's minor half: an adopted record keeps the `reading_order_index` it was given
/// against its *old* panel, so it can collide with an index the adopting panel already used.
/// Nothing validates dialogue index uniqueness. This pins the collision and its (inert) effect:
/// `readingOrderIndex` is reported back but no command or rule consumes it, so the defect is a
/// *note* rather than a bug — unless a future export orders by it.
#[test]
fn r2_adoption_can_collide_a_reading_order_index_and_nothing_validates_it() {
    let (mut s, _root) = project("adopt-index");
    let page = page_of(&mut s);
    let a = panel_on(&mut s, page, [40.0, 40.0, 200.0, 150.0]);
    let b = panel_on(&mut s, page, [260.0, 40.0, 200.0, 150.0]);
    // B already has one dialogue at index 0.
    s.execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "text": "B first", "at": [340.0, 100.0]}))
        .expect("B's own dialogue");
    // A's dialogue is index 0 against A, and is then adopted by B — index 0 twice.
    let balloon = s
        .execute("comic.lettering.add", &json!({"page": page, "panel": a, "kind": "speech", "text": "Move me", "at": [120.0, 100.0]}))
        .expect("A's dialogue");
    let d = balloon["dialogue"].as_u64().expect("id");
    assert_eq!(dialogue_record(&s, d).reading_order_index, 0, "A gave it index 0");

    s.execute("comic.panel.delete", &json!({"panel": a, "reuse": [d]})).expect("detach");
    s.execute("comic.lettering.add", &json!({"page": page, "panel": b, "kind": "speech", "dialogue": d, "at": [340.0, 120.0]})).expect("adopt");

    let b_indices: Vec<u16> =
        open_comic(&s).bundle.pages[&PageId(page)].dialogue.iter().filter(|x| x.panel_id.0 == b).map(|x| x.reading_order_index).collect();
    assert_eq!(b_indices, vec![0, 0], "the adopted record collides with B's own index 0");
    // Nothing reports it, and nothing reads it — which is what makes this a note.
    let v = validate(&mut s);
    assert_eq!(v["errors"].as_u64(), Some(0), "no rule governs Dialogue.reading_order_index uniqueness: {v}");
}

// ---------- (4) F7's re-grounding: is the duplicate refusal still about loss? ----------

/// §R-7 re-grounds the duplicate-id refusal on *indistinguishability* (rule 13) and says "the bytes
/// themselves are safe, because `prune_stale` no longer removes a file the generation of record
/// never wrote". For a pair that was **already in the tree at load**, the generation of record
/// *does* declare both files, so the prune's protection does not apply: the refusal is still what
/// keeps the loser's bytes. This pins the refusal and the survival, and is the test that shows the
/// re-grounding is a re-framing rather than a fact.
#[test]
fn r2_a_load_time_duplicate_pair_is_refused_and_the_loser_survives() {
    let (mut s, root) = project("dup-load");
    let page = page_of(&mut s);
    s.execute("comic.project.save", &json!({})).expect("save 1");

    let twin = root.join(format!("pages/page-{page:03}.json"));
    std::fs::copy(root.join(format!("pages/page-{page}.json")), &twin).expect("a padded twin, present before the next load");

    // A hand-repaired bundle's manifest describes the twin (§4: `files` covers every file).
    let manifest_path = root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).expect("read")).expect("manifest json");
    let bytes = std::fs::read(&twin).expect("read the twin");
    manifest["files"][format!("pages/page-{page:03}.json")] = json!({"sha256": vectorcraft_comic_model::sha256_hex(&bytes), "bytes": bytes.len()});
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).expect("serialize")).expect("write the manifest");

    // Re-open, so the in-memory generation of record declares both files.
    s.execute("comic.project.open", &json!({"path": root.to_string_lossy()})).expect("reopen");
    let err = s.execute("comic.project.save", &json!({})).expect_err("two files encode one id must be refused");
    assert!(err.to_string().contains("both encode id"), "and the message names the pair: {err}");
    assert!(err.to_string().contains(&format!("pages/page-{page}.json")), "it names the first file: {err}");
    assert!(err.to_string().contains(&format!("pages/page-{page:03}.json")), "and the second: {err}");
    assert!(twin.is_file(), "the refused save left the loser's bytes alone");
}
