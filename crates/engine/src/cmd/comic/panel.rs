//! `comic.panel.*`: the shots of a page — their frame objects, their artwork, their shot text, and
//! the related-entity policy of design §11.1 when one is deleted.
//!
//! A panel is a **comic record plus a document object**. The record holds the id and the meaning
//! (`Panel.frame_node_id`, `artwork_refs`, `dialogue_ids`); the document object holds the geometry.
//! Nothing here stores a rectangle in a record — design §6.1 forbids the second copy — so every
//! geometric operation is an edit of the frame path and the record never moves.
//!
//! # What a move does, and what a resize does
//!
//! `comic.panel.move` translates the frame **and every object the panel owns** — its artwork clip
//! groups and its lettering frames, tails and text — by the same delta, because the frame clips the
//! artwork (design §8.2): moving the frame alone would silently re-crop every image.
//! `comic.panel.resize` changes the frame's rectangle only and leaves the artwork where it is,
//! which reveals or hides more of the same image without moving it; a creator who wants the art
//! scaled crops or fits it with the existing `object.*` commands. Both keep every id, so the panel,
//! its artwork references, its dialogue and its reading order survive them unchanged (M1 acceptance
//! test 4).
//!
//! # The lock (§6.2's `locked`, schema §5.6)
//!
//! `Panel.locked` is v2's one field, and **this module is what gives it an effect** — a field no
//! command reads is a field that does not exist. The lock is a **command** constraint and not a
//! legality one (schema §13 added no rule for it), so it is applied here, uniformly, by one helper
//! ([`locked_refusal`]) rather than checked where each command finds it convenient.
//!
//! Every command that can move, resize, replace or delete a panel, and the lock's effect on it:
//!
//! | Command | Effect of the lock | Why |
//! |---|---|---|
//! | `comic.panel.add` | **none** — a new panel is created `locked: false` | there is no panel to protect until the command has run, and a lock on a *neighbour* does not stop a page growing |
//! | `comic.panel.move` | **refused** | the frame, and the artwork and lettering the frame clips, would all translate |
//! | `comic.panel.resize` | **refused** | the frame's rectangle, and the crop of the artwork inside it, change |
//! | `comic.panel.replace_art` | **refused** | the artwork it replaces is deleted, and its object does not come back |
//! | `comic.panel.set_shot` | **ignored** | it writes the generation brief and nothing else: no geometry, no object, no artwork. A lock covers the panel as *drawn*, not what a later job is asked to draw |
//! | `comic.panel.delete` | **refused** | losing the deliberate placement is the accident a lock exists to prevent, and a lock that does not stop deletion is a lock in name only |
//! | `comic.page.delete` | **refused** while the page holds one | it deletes every panel of the page by the §11.1 policy; the refusal names the locked panels, which one command clears |
//! | `comic.page.template.apply` | **refused** while the page holds one | every path touches every panel: with no panels it *creates* (no lock to consult), at the same count it **moves** each frame, and `replace: true` **deletes** them |
//! | `comic.reading_order.set` | **ignored** | it renumbers `reading_order_index`, which is narrative state *on the page*, not the panel's geometry or artwork; refusing would make a locked panel unorderable |
//! | `comic.lettering.add` | **ignored** | it adds a balloon and a `Lettering` record of its own and moves no panel object. The lock is asked in `comic.panel.move`'s direction: a locked panel's frame will not move *under* its balloons either |
//!
//! A refusal is **actionable**: it names the panel ids it is about and the one command that clears
//! the lock ([`locked_refusal`]). The lock is settable and clearable through
//! `comic.panel.set_locked`, which is itself never refused — a remedy the lock could block would be
//! a dead end.
//!
//! **Selection is not here.** M2 §3 keeps it out: selection is session state and belongs in M6, so
//! no command in this module writes it, and no lock relates to it.
//!
//! # The lock and the visual document
//!
//! **Nothing here mirrors `locked` into `crates/doc`.** Structural rule 1 forbids a comic field in
//! the visual document, and the document has a node-lock mechanism of its own; whether the comic
//! lock should *drive* it is a design question (reported as M2-W3-F3 in
//! `docs/reviews/M2-command-engineer-locking.md`) and not a decision this module makes.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_comic_model::{
    ArtworkRef, ArtworkRole, AssetId, ComicBundle, ContinuityConstraints, DialogueId, GenerationStatus, JobId, LetteringId, PageId, Panel, PanelId,
    SceneId, StorageRef,
};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, Rect, Vec2, shapes};

use super::*;
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "comic.panel.add",
            "Add Panel",
            ["Window", "Comic"],
            None,
            "{page: PageId, rect: [x, y, width, height] (points, in the artboard), scene?: SceneId (default: the page's first scene), shot?: the generation brief} create a frame path object inside the artboard and a Panel record pointing at it → {panel, page, frame, artboard_id, reading_order_index, findings: [geometry warnings]}. The geometry is checked and *reported* (overlap, minimum size, containment) rather than refused, because a creator may place a panel and fix it later (design §12.1), and a finding names its panel as `Panel(<id>)` (see comic.panel.set_locked and the module doc for the lock's effect on each command). The panel is created **unlocked** (`locked: false`, schema §6.2): nothing is protected until there is a panel. One undo step",
            has_comic,
            add
        ),
        cmd!(
            "comic.panel.move",
            "Move Panel",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, dx?, dy? (or x?, y?: put the frame's top-left there)} translate the frame and the objects the panel owns (artwork, lettering) so the panel keeps showing the same crop → {panel, frame, moved: [NodeId], delta}. The id, its artwork references, its dialogue and its reading order are unchanged. **Refused when the panel is locked** (§6.2) — the refusal names the panel and `comic.panel.set_locked {panel, locked: false}`. One undo step",
            has_comic,
            move_panel
        ),
        cmd!(
            "comic.panel.resize",
            "Resize Panel",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, width?: pt, height?: pt, rect?: [x, y, width, height] (wins)} scale the frame path so its bounds become the new rectangle. The artwork is neither scaled nor moved and every id is unchanged → {panel, frame, rect, findings: [geometry warnings, each naming its panel as `Panel(<id>)`]}. **Refused when the panel is locked** (§6.2). One undo step",
            has_comic,
            resize
        ),
        cmd!(
            "comic.panel.set_locked",
            "Lock Panel",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, locked: bool} set or clear `Panel.locked` (§6.2). A locked panel is refused by the commands that would **move**, **resize**, **replace the artwork of** or **delete** it — comic.panel.move, .resize, .replace_art, .delete, comic.page.delete and comic.page.template.apply — and by nothing else: the brief (comic.panel.set_shot), the words (comic.lettering.add) and the page's narrative order (comic.reading_order.set) are unaffected, and this command is never itself refused → {panel, page, locked}. One undo step",
            has_comic,
            set_locked
        ),
        cmd!(
            "comic.panel.replace_art",
            "Replace Panel Artwork",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, asset: AssetId of a Linked asset under the bundle, role?: \"primary\" (default)|\"background\"|\"overlay\"} place the asset's file in the panel, clip it with the panel's own frame, and record an ArtworkRef. The artwork it replaces is deleted; every other object and the previous Asset record are left intact (design §8.2, M1 acceptance test 6) → {panel, asset, role, node: the clip group, image: the placed image object (selected, so object.cropImage acts on it), previous?: the replaced node, asset_kept: true}. An Embedded or Package asset is refused by name, because M1 has no command that could hand over those bytes. **Refused when the panel is locked** (§6.2). One undo step",
            has_comic,
            replace_art
        ),
        cmd!(
            "comic.panel.set_shot",
            "Set Panel Shot",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, shot: the generation brief} set Panel.shot_description. Changes no visual object and no other record, and **a lock does not apply**: `Panel.locked` (§6.2) protects where the panel is and what it shows, not what a later generation job is asked for. `comic.panel.set_locked` is the command that refuses the geometry and artwork commands → {panel, shot}. One undo step",
            has_comic,
            set_shot
        ),
        cmd!(
            "comic.panel.delete",
            "Delete Panel",
            ["Window", "Comic"],
            None,
            "{panel: PanelId, reuse?: [DialogueId…] dialogues to detach instead of delete (a dialogue whose unknown map says \"reusable\": true is detached too)} delete the panel and, per design §11.1: its lettering frame, tail and text objects and its lettering records; its artwork objects; its dialogue records **unless flagged reusable**, which are detached and reported (their panel_id then names a deleted panel, which schema rule 1 reports until `comic.lettering.add {panel, dialogue}` adopts them, which is the move §11.1's row means); its frame; and renumber the reading order in the same operation. A dialogue that is deleted takes **every** balloon showing it, wherever that balloon is: a Lettering has no words of its own, so `letteringDeleted` can name a balloon in another panel. **Asset records are never deleted** — lineage outlives the panel — and a generation job still targeting the panel is left to finish and reported → {panel, page, readingOrder, deletion: {removedNodes, letteringDeleted, dialogueDeleted, dialogueDetached, dialogueDetachedNote, assetsKept, assetsKeptNote, jobsStillTargeting, jobsStillTargetingNote}}. A page whose reading order is not already a permutation of its panels is refused rather than repaired. **Refused when the panel is locked** (§6.2): a lock that did not stop deletion would be a lock in name only — clear it with comic.panel.set_locked. One undo step",
            has_comic,
            delete
        ),
    ]
}

// ---------- parameter helpers ----------

pub(crate) fn raw_id(p: &Value, key: &str, cmd: &str) -> Result<u64> {
    p.get(key).and_then(Value::as_u64).ok_or_else(|| bad(cmd, format!("missing id `{key}` (a non-negative integer)")))
}

/// A rectangle from `[x, y, width, height]` or `{x, y, width, height}`.
pub(crate) fn rect_param(p: &Value, key: &str, cmd: &str) -> Result<Rect> {
    let Some(v) = p.get(key).filter(|v| !v.is_null()) else { return Err(bad(cmd, format!("missing `{key}`"))) };
    let (x, y, w, h) = match v {
        Value::Array(a) if a.len() == 4 => (a[0].as_f64(), a[1].as_f64(), a[2].as_f64(), a[3].as_f64()),
        Value::Object(o) => (
            o.get("x").and_then(Value::as_f64),
            o.get("y").and_then(Value::as_f64),
            o.get("width").and_then(Value::as_f64),
            o.get("height").and_then(Value::as_f64),
        ),
        _ => (None, None, None, None),
    };
    let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) else {
        return Err(bad(cmd, format!("`{key}` must be [x, y, width, height] or {{x, y, width, height}}")));
    };
    Ok(Rect::new(x, y, x + w, y + h))
}

/// A dialogue-id array parameter.
pub(crate) fn dialogue_ids_param(p: &Value, key: &str, cmd: &str) -> Result<Vec<DialogueId>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_u64().map(DialogueId).ok_or_else(|| bad(cmd, format!("`{key}` is an array of dialogue ids (non-negative integers)"))))
            .collect(),
        Some(_) => Err(bad(cmd, format!("`{key}` must be an array of dialogue ids"))),
    }
}

pub(crate) fn artboard_of(s: &Session, page: PageId) -> Result<u32> {
    Ok(page_file(&open(s)?.bundle, page)?.page.artboard_id)
}

/// The scene a new panel goes in: the one given, else the page's first, else the first scene of the
/// page's chapter.
pub(crate) fn scene_for(s: &Session, page: PageId, given: Option<u64>, cmd: &str) -> Result<SceneId> {
    let b = &open(s)?.bundle;
    let file = page_file(b, page)?;
    if let Some(raw) = given {
        let id = SceneId(raw);
        if !b.scenes.scenes.iter().any(|sc| sc.id == id) {
            return Err(bad(cmd, format!("no such scene {raw}")));
        }
        if !file.page.scene_ids.contains(&id) {
            return Err(bad(cmd, format!("scene {raw} is not one of page {}'s scenes", page.0)));
        }
        return Ok(id);
    }
    if let Some(first) = file.page.scene_ids.first() {
        return Ok(*first);
    }
    if let Some(first) = b.scenes.scenes.iter().find(|sc| sc.chapter_id == file.page.chapter_id) {
        return Ok(first.id);
    }
    Err(bad(cmd, format!("page {} has no scene: the project has none for its chapter", page.0)))
}

// ---------- the frame object ----------

/// The paint a panel frame is created with: a white frame with a black outline, which is what a
/// panel reads as until the artwork covers it.
pub(crate) fn frame_appearance() -> Appearance {
    Appearance::basic(Paint::solid(Color::WHITE), Paint::solid(Color::BLACK), 1.0)
}

/// Create a frame path object for `rect` on the current layer.
pub(crate) fn create_frame(s: &mut Session, label: &str, rect: Rect) -> Result<NodeId> {
    s.edit(label, |d, _| {
        let parent = d.default_layer();
        let id = d.alloc_id();
        let mut n = Node::path(id, shapes::rectangle(rect), frame_appearance());
        n.name = Some("Panel frame".to_string());
        d.insert(parent, usize::MAX, n)?;
        Ok(id)
    })
}

/// Replace the frame object's path with a scaled copy of it, so a frame that is not a plain
/// rectangle keeps its shape.
pub(crate) fn set_frame_rect(d: &mut Document, frame: NodeId, target: Rect) -> Result<()> {
    let node = d.node(frame).ok_or(EngineError::NoNode(frame))?;
    let current = node.geometric_bounds().ok_or_else(|| EngineError::Other(format!("the frame object {} has no geometry to resize", frame.0)))?;
    let path = node.path_data().cloned().ok_or_else(|| EngineError::Other(format!("the frame object {} is not a path", frame.0)))?;
    let (sx, sy) = (target.width() / current.width(), target.height() / current.height());
    let m = Affine::new([sx, 0.0, 0.0, sy, target.x0 - current.x0 * sx, target.y0 - current.y0 * sy]);
    if let Some(n) = d.node_mut(frame)
        && let NodeKind::Path { path: p, .. } = &mut n.kind
    {
        *p = path.transformed(m);
    }
    Ok(())
}

/// Translate a node's geometry in place.
fn translate(d: &mut Document, id: NodeId, delta: Vec2) {
    if let Some(node) = d.node_mut(id) {
        node.transform(Affine::translate(delta), false);
    }
}

/// The objects a panel owns: its artwork and its lettering's frame, tail and text.
pub(crate) fn owned_objects(b: &ComicBundle, d: &Document, panel: &Panel) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = panel.artwork_refs.iter().map(|a| a.node_id).collect();
    for id in &panel.lettering_ids {
        if let Some(l) = b.pages.values().flat_map(|f| f.lettering.iter()).find(|l2| l2.id == *id) {
            ids.push(l.frame_node_id);
            if let Some(t) = l.tail_node_id {
                ids.push(t);
            }
            ids.push(l.text_node_id);
        }
    }
    ids.retain(|id| d.node(*id).is_some());
    ids
}

// ---------- geometry findings ----------

/// The `comic-layout` geometry findings for a page's panel rectangles, as JSON.
///
/// **The subject is `Panel(<id>)`, and that spelling is a contract.** `comic-layout`'s
/// `validate::page` labels the same faults the same way and documents `(rule, subject)` as the key a
/// caller running both validators deduplicates on (`validate/mod.rs`, "The subjects are spelled as
/// `comic-model` spells them"). This used to spell them `Panel[i]` — the index in the vector handed
/// to `faults`, which is *not* even the page file's own index, because [`page_frames`] skips a panel
/// whose frame node the document cannot answer for. Two spellings for one fault meant a caller that
/// deduplicated got the same warning twice, and a response saying "sits 2pt from `Panel[1]`" could
/// not be resolved to a panel without the file in hand (M2-L1). The index form survives nowhere: a
/// caller who wants a position has the page and can count it, and the position it was reporting was
/// a filtered one.
pub(crate) fn geometry_findings(page_rect: Rect, panels: &[LabeledFrame]) -> Vec<Value> {
    let rects: Vec<vectorcraft_comic_layout::PanelRect> = panels.iter().map(|(_, rect)| *rect).collect();
    vectorcraft_comic_layout::validate::faults(page_rect, None, &rects, &vectorcraft_comic_layout::validate::Limits::default())
        .iter()
        .map(|f| finding_json(&f.describe(&|i| panels.get(i).map(|(label, _)| label.clone()).unwrap_or_else(|| format!("panels[{i}]")))))
        .collect()
}

/// A panel's frame rectangle and the label a finding about it carries: `Panel(<id>)`.
pub(crate) type LabeledFrame = (String, vectorcraft_comic_layout::PanelRect);

/// Every frame rectangle of a page that the document can answer for, each labelled `Panel(<id>)`.
fn page_frames(s: &Session, page: PageId) -> Result<Vec<LabeledFrame>> {
    let b = &open(s)?.bundle;
    let d = &s.doc()?.doc;
    Ok(page_file(b, page)?.panels.iter().filter_map(|p| node_frame(d, p.frame_node_id).map(|rect| (format!("Panel({})", p.id.0), rect))).collect())
}

// ---------- the lock (§6.2, schema §5.6) ----------

/// Whether `panel` is locked. A panel that does not exist is not locked: a missing panel is another
/// command's refusal, and answering `false` keeps this helper from inventing a second one.
pub(crate) fn is_locked(b: &ComicBundle, panel: PanelId) -> bool {
    b.pages.values().flat_map(|f| f.panels.iter()).any(|p| p.id == panel && p.locked)
}

/// The panel, if it is locked — the shape a single-panel command needs, so the refusal and the
/// ordinary "no such panel" answer stay distinguishable.
pub(crate) fn locked_panel(b: &ComicBundle, panel: PanelId) -> Vec<PanelId> {
    if is_locked(b, panel) { vec![panel] } else { Vec::new() }
}

/// The locked panels of a page, in the page's own panel order.
pub(crate) fn locked_on_page(b: &ComicBundle, page: PageId) -> Vec<PanelId> {
    b.pages.get(&page).map(|f| f.panels.iter().filter(|p| p.locked).map(|p| p.id).collect()).unwrap_or_default()
}

/// The refusal a locked panel gets (schema §5.6, §6.2).
///
/// One message for both shapes — one panel and a pageful — because the remedy is the same command.
/// It says **which** panels stand in the way and **what to do about it**, in the same sentence: the
/// lock's whole point is that a creator learns why an edit did not happen, and a bare "refused"
/// would leave them hunting for which of a page's panels is the obstacle.
pub(crate) fn locked_refusal(cmd: &str, action: &str, locked: &[PanelId]) -> EngineError {
    let ids: Vec<String> = locked.iter().map(|p| p.0.to_string()).collect();
    let (what, remedy) = match ids.len() {
        1 => (format!("panel {} is locked", ids[0]), format!("comic.panel.set_locked {{panel: {}, locked: false}}", ids[0])),
        _ => (format!("panels {} are locked", ids.join(", ")), "comic.panel.set_locked {panel: <id>, locked: false} for each".to_string()),
    };
    EngineError::Other(format!(
        "{cmd}: {what}, so {action} — a locked panel does not move, resize, take new artwork or go away. Unlock it with {remedy}, then retry"
    ))
}

// ---------- comic.panel.add ----------

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.add";
    let page = PageId(raw_id(p, "page", C)?);
    let rect = rect_param(p, "rect", C)?;
    checked_frame(C, rect)?;
    let scene = scene_for(s, page, p.get("scene").and_then(Value::as_u64), C)?;
    let shot = str_param(p, "shot").unwrap_or("").to_string();
    let page_rect = checked_page_rect(page, page_rect(s, page)?)?;
    // MUTATION A (round-2 review): `comic.panel.add` over-applies the page's lock.
    let locked = locked_on_page(&open(s)?.bundle, page);
    if !locked.is_empty() {
        return Err(locked_refusal(C, "the panel is not added", &locked));
    }
    comic_edit(s, "Add Panel", |s| {
        let artboard_id = artboard_of(s, page)?;
        let frame = create_frame(s, "Add Panel", rect)?;
        let (panel, order_index) = {
            let oc = open_mut(s)?;
            let id = oc.bundle.allocate::<PanelId>().map_err(|e| EngineError::Other(format!("{e}")))?;
            let index = {
                let file = page_file_mut(&mut oc.bundle, page)?;
                require_permutation(&file.page)?;
                let index = u16::try_from(file.page.reading_order.len()).unwrap_or(u16::MAX);
                file.page.panel_ids.push(id);
                file.page.reading_order.push(id);
                file.panels.push(Panel {
                    id,
                    page_id: page,
                    artboard_id,
                    scene_id: scene,
                    frame_node_id: frame,
                    artwork_refs: Vec::new(),
                    shot_description: shot.clone(),
                    dialogue_ids: Vec::new(),
                    lettering_ids: Vec::new(),
                    reading_order_index: index,
                    continuity_constraints: ContinuityConstraints {
                        characters: Vec::new(),
                        wardrobe: None,
                        time_of_day: None,
                        callbacks: Vec::new(),
                        exceptions: Vec::new(),
                    },
                    generation_status: GenerationStatus::None,
                    // v2's one field (§6.2): a panel created here starts unlocked. W3's commands
                    // are what give `locked` its effect; this literal only has to exist.
                    locked: false,
                    unknown: Default::default(),
                });
                index
            };
            (id, index)
        };
        let findings = geometry_findings(page_rect, &page_frames(s, page)?);
        Ok(json!({
            "panel": panel.0,
            "page": page.0,
            "frame": frame.0,
            "artboard_id": artboard_id,
            "reading_order_index": order_index,
            "findings": findings,
        }))
    })
}

// ---------- comic.panel.move ----------

fn move_panel(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.move";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let locked = locked_panel(&open(s)?.bundle, panel);
    if !locked.is_empty() {
        return Err(locked_refusal(C, "nothing is moved", &locked));
    }
    comic_edit(s, "Move Panel", |s| {
        let (page, frame, delta) = {
            let b = &open(s)?.bundle;
            let (page, index) = locate_panel(b, panel)?;
            let frame = page_file(b, page)?.panels[index].frame_node_id;
            let current = s
                .doc()?
                .doc
                .node(frame)
                .and_then(|n| n.geometric_bounds())
                .ok_or_else(|| EngineError::Other(format!("the panel's frame {} has no geometry", frame.0)))?;
            let delta = match (p.get("x").and_then(Value::as_f64), p.get("y").and_then(Value::as_f64)) {
                (Some(x), Some(y)) => Vec2::new(x - current.x0, y - current.y0),
                (None, None) => Vec2::new(f64_or(p, "dx", 0.0), f64_or(p, "dy", 0.0)),
                _ => return Err(bad(C, "give both `x` and `y`, or `dx` and `dy`")),
            };
            if !(delta.x.is_finite() && delta.y.is_finite()) {
                return Err(bad(C, "the move must be finite"));
            }
            (page, frame, delta)
        };
        let owned = {
            let b = &open(s)?.bundle;
            let d = &s.doc()?.doc;
            let index = locate_panel(b, panel)?.1;
            owned_objects(b, d, &page_file(b, page)?.panels[index])
        };
        let moved = s.edit("Move Panel", |d, _| {
            translate(d, frame, delta);
            for id in &owned {
                translate(d, *id, delta);
            }
            Ok(std::iter::once(frame.0).chain(owned.iter().map(|n| n.0)).collect::<Vec<u64>>())
        })?;
        Ok(json!({ "panel": panel.0, "frame": frame.0, "moved": moved, "delta": [delta.x, delta.y] }))
    })
}

// ---------- comic.panel.resize ----------

fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.resize";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let locked = locked_panel(&open(s)?.bundle, panel);
    if !locked.is_empty() {
        return Err(locked_refusal(C, "nothing is resized", &locked));
    }
    comic_edit(s, "Resize Panel", |s| {
        let (page, frame, target, page_rect) = {
            let b = &open(s)?.bundle;
            let (page, index) = locate_panel(b, panel)?;
            let frame = page_file(b, page)?.panels[index].frame_node_id;
            let current = s
                .doc()?
                .doc
                .node(frame)
                .and_then(|n| n.geometric_bounds())
                .ok_or_else(|| EngineError::Other(format!("the panel's frame {} has no geometry to resize", frame.0)))?;
            let target = if p.get("rect").is_some() {
                let r = rect_param(p, "rect", C)?;
                checked_frame(C, r)?;
                r
            } else {
                let w = p.get("width").and_then(Value::as_f64).unwrap_or_else(|| current.width());
                let h = p.get("height").and_then(Value::as_f64).unwrap_or_else(|| current.height());
                let r = Rect::new(current.x0, current.y0, current.x0 + w, current.y0 + h);
                checked_frame(C, r)?;
                r
            };
            (page, frame, target, checked_page_rect(page, page_rect(s, page)?)?)
        };
        s.edit("Resize Panel", |d, _| set_frame_rect(d, frame, target))?;
        let findings = geometry_findings(page_rect, &page_frames(s, page)?);
        Ok(json!({ "panel": panel.0, "frame": frame.0, "rect": [target.x0, target.y0, target.width(), target.height()], "findings": findings }))
    })
}

// ---------- comic.panel.replace_art ----------

fn artwork_role(p: &Value, cmd: &str) -> Result<ArtworkRole> {
    match str_param(p, "role") {
        None | Some("primary") => Ok(ArtworkRole::Primary),
        Some("background") => Ok(ArtworkRole::Background),
        Some("overlay") => Ok(ArtworkRole::Overlay),
        Some(other) => Err(bad(cmd, format!("`role` is primary, background or overlay, not {other:?}"))),
    }
}

fn role_name(role: ArtworkRole) -> &'static str {
    match role {
        ArtworkRole::Primary => "primary",
        ArtworkRole::Background => "background",
        ArtworkRole::Overlay => "overlay",
    }
}

fn replace_art(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.replace_art";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let asset_id = AssetId(raw_id(p, "asset", C)?);
    let role = artwork_role(p, C)?;
    let locked = locked_panel(&open(s)?.bundle, panel);
    if !locked.is_empty() {
        return Err(locked_refusal(C, "the artwork is not replaced", &locked));
    }
    let (page, frame, asset_file) = {
        let bundle_root = root(s)?;
        let b = &open(s)?.bundle;
        let (page, index) = locate_panel(b, panel)?;
        let frame = page_file(b, page)?.panels[index].frame_node_id;
        let asset = b.assets.assets.iter().find(|a| a.id == asset_id).ok_or_else(|| bad(C, format!("no such asset {}", asset_id.0)))?;
        let relative = match &asset.storage_reference {
            StorageRef::Linked { relative_path } => relative_path.clone(),
            // The base is the bundle root (schema §3, rule 9), never the document folder.
            StorageRef::Embedded { .. } => {
                return Err(bad(
                    C,
                    format!(
                        "asset {} is Embedded in a document's image map and M1 has no asset command that could hand those bytes over; give it a Linked storage reference under the bundle",
                        asset_id.0
                    ),
                ));
            }
            StorageRef::Package { relative_path } => {
                return Err(bad(
                    C,
                    format!("asset {} lives in a packaged archive ({relative_path}), which M1 does not open (design §7.5)", asset_id.0),
                ));
            }
        };
        (page, frame, bundle_root.join(relative))
    };
    if !asset_file.is_file() {
        return Err(bad(C, format!("asset {} names {}, which is not a file in the bundle", asset_id.0, asset_file.display())));
    }
    comic_edit(s, "Replace Panel Artwork", |s| {
        let previous = {
            let oc = open_mut(s)?;
            let index = locate_panel(&oc.bundle, panel)?.1;
            let file = page_file_mut(&mut oc.bundle, page)?;
            file.panels[index].artwork_refs.iter().position(|a| a.role == role).map(|i| file.panels[index].artwork_refs.remove(i).node_id)
        };
        if let Some(old) = previous {
            s.edit("Replace Panel Artwork", |d, _| {
                drop_nodes(d, &[old]);
                Ok(())
            })?;
        }
        let rect = s
            .doc()?
            .doc
            .node(frame)
            .and_then(|n| n.geometric_bounds())
            .ok_or_else(|| EngineError::Other(format!("the panel's frame {} has no geometry to place art in", frame.0)))?;
        let placed = s.execute(
            "file.place",
            &json!({
                "path": asset_file.to_string_lossy(),
                "link": false,
                "rect": [rect.x0, rect.y0, rect.width(), rect.height()],
            }),
        )?;
        let ids: Vec<NodeId> =
            placed.get("ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(NodeId).collect()).unwrap_or_default();
        if ids.is_empty() {
            return Err(EngineError::Other(format!("placing asset {} produced no object", asset_id.0)));
        }
        let group = s.edit("Clip Panel Artwork", |d, _| {
            let frame_path = d
                .node(frame)
                .and_then(|n| n.path_data().cloned())
                .ok_or_else(|| EngineError::Other(format!("the panel's frame {} is not a path, so it cannot clip the artwork", frame.0)))?;
            let (par, idx, _) = d.position(ids[0]).ok_or(EngineError::NoNode(ids[0]))?;
            let mut clip = Node::path(d.alloc_id(), frame_path, Appearance::basic(Paint::None, Paint::None, 0.0));
            if let NodeKind::Path { clipping, .. } = &mut clip.kind {
                *clipping = true;
            }
            let gid = d.alloc_id();
            d.insert(par, idx + 1, Node::new(gid, NodeKind::Group { children: vec![], clip: true }))?;
            d.insert(Some(gid), usize::MAX, clip)?;
            for id in &ids {
                d.move_node(*id, Some(gid), usize::MAX)?;
            }
            Ok(gid)
        })?;
        {
            let oc = open_mut(s)?;
            let index = locate_panel(&oc.bundle, panel)?.1;
            page_file_mut(&mut oc.bundle, page)?.panels[index].artwork_refs.push(ArtworkRef { asset_id, node_id: group, role });
        }
        // Select the image itself, so `object.cropImage` acts on it (M1 acceptance test 3).
        s.select(|_, sel| sel.set(ids.clone()))?;
        Ok(json!({
            "panel": panel.0,
            "asset": asset_id.0,
            "role": role_name(role),
            "node": group.0,
            "image": ids[0].0,
            "previous": previous.map(|id| id.0),
            "asset_kept": true,
        }))
    })
}

// ---------- comic.panel.set_shot ----------

fn set_shot(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.set_shot";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let shot = str_param(p, "shot").ok_or_else(|| bad(C, "missing `shot` (the generation brief)"))?.to_string();
    comic_edit(s, "Set Panel Shot", |s| {
        let oc = open_mut(s)?;
        let (page, index) = locate_panel(&oc.bundle, panel)?;
        page_file_mut(&mut oc.bundle, page)?.panels[index].shot_description = shot.clone();
        Ok(json!({ "panel": panel.0, "shot": shot }))
    })
}

// ---------- comic.panel.set_locked ----------

fn set_locked(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.set_locked";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let locked = p.get("locked").and_then(Value::as_bool).ok_or_else(|| bad(C, "missing `locked` (true to lock the panel, false to unlock it)"))?;
    comic_edit(s, if locked { "Lock Panel" } else { "Unlock Panel" }, |s| {
        let oc = open_mut(s)?;
        let (page, index) = locate_panel(&oc.bundle, panel)?;
        page_file_mut(&mut oc.bundle, page)?.panels[index].locked = locked;
        Ok(json!({ "panel": panel.0, "page": page.0, "locked": locked }))
    })
}

// ---------- a panel record for a template slot ----------

/// Add a panel record for a frame the template just placed, with the plan's narrative index.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_panel_record(
    bundle: &mut ComicBundle,
    id: PanelId,
    page: PageId,
    artboard_id: u32,
    scene: SceneId,
    frame: NodeId,
    index: u16,
) -> Result<()> {
    let file = page_file_mut(bundle, page)?;
    file.page.panel_ids.push(id);
    file.panels.push(Panel {
        id,
        page_id: page,
        artboard_id,
        scene_id: scene,
        frame_node_id: frame,
        artwork_refs: Vec::new(),
        shot_description: String::new(),
        dialogue_ids: Vec::new(),
        lettering_ids: Vec::new(),
        reading_order_index: index,
        continuity_constraints: ContinuityConstraints {
            characters: Vec::new(),
            wardrobe: None,
            time_of_day: None,
            callbacks: Vec::new(),
            exceptions: Vec::new(),
        },
        generation_status: GenerationStatus::None,
        // v2's one field (§6.2); see the note on the literal above.
        locked: false,
        unknown: Default::default(),
    });
    Ok(())
}

// ---------- comic.panel.delete, and design §11.1 ----------

/// What deleting one panel did, per design §11.1.
#[derive(Clone, Debug, Default)]
pub(crate) struct Deletion {
    /// Document objects deleted with the panel.
    pub nodes: Vec<NodeId>,
    /// Lettering records deleted with the panel — including a balloon **elsewhere** (another panel,
    /// another page) that was showing a dialogue this panel owned and the policy deleted.
    pub lettering: Vec<LetteringId>,
    /// Dialogue records deleted with the panel.
    pub dialogue_deleted: Vec<DialogueId>,
    /// Dialogue records **detached and reported** rather than deleted (§11.1).
    pub dialogue_detached: Vec<DialogueId>,
    /// Asset records this looked at and did **not** delete (§11.1: lineage outlives the panel).
    pub assets_kept: Vec<AssetId>,
    /// Jobs still targeting the deleted panel, left to finish and reported (§11.1).
    pub jobs_still_targeting: Vec<JobId>,
}

/// Apply design §11.1 to one panel's **records**: delete the panel and every related record the
/// policy says to delete, detach and report the reusable dialogue, keep every `Asset` record, and
/// renumber the reading order in the same operation. The document objects it names are returned in
/// [`Deletion::nodes`] for the caller to remove, so the model and the document are each mutated in
/// one place.
pub(crate) fn remove_panel(b: &mut ComicBundle, page: PageId, panel: PanelId, reuse: &[DialogueId]) -> Result<Deletion> {
    let file = page_file_mut(b, page)?;
    require_permutation(&file.page)?;
    let index =
        file.panels.iter().position(|p| p.id == panel).ok_or_else(|| EngineError::Other(format!("no such panel {} on page {}", panel.0, page.0)))?;
    let record = file.panels.remove(index);
    file.page.panel_ids.retain(|p| *p != panel);
    let order = order_without(file, panel);
    apply_reading_order(file, order);

    let mut out = Deletion::default();
    // Artwork objects go with the panel; the `Asset` records they name do not (§11.1).
    for artwork in &record.artwork_refs {
        out.nodes.push(artwork.node_id);
        if !out.assets_kept.contains(&artwork.asset_id) {
            out.assets_kept.push(artwork.asset_id);
        }
    }
    out.nodes.push(record.frame_node_id);
    // Lettering: the record and its frame, tail and text objects are the panel's presentation.
    for id in &record.lettering_ids {
        if let Some(i) = file.lettering.iter().position(|l| l.id == *id) {
            let l = file.lettering.remove(i);
            out.nodes.push(l.frame_node_id);
            out.nodes.push(l.text_node_id);
            if let Some(t) = l.tail_node_id {
                out.nodes.push(t);
            }
            out.lettering.push(l.id);
        }
    }
    // Dialogue: deleted unless flagged reusable, in which case detached and reported.
    for id in &record.dialogue_ids {
        let Some(i) = file.dialogue.iter().position(|x| x.id == *id) else { continue };
        let flagged = reuse.contains(id) || file.dialogue[i].unknown.get("reusable").and_then(Value::as_bool).unwrap_or(false);
        if flagged {
            out.dialogue_detached.push(*id);
        } else {
            file.dialogue.remove(i);
            out.dialogue_deleted.push(*id);
        }
    }
    for job in b.provenance.jobs.iter().chain(b.jobs.values().map(|f| &f.job)) {
        if job.target_panel_id == Some(panel) && !out.jobs_still_targeting.contains(&job.id) {
            out.jobs_still_targeting.push(job.id);
        }
    }
    // A dialogue this panel owned and the policy deleted takes **every** balloon that was showing
    // it, so a deleted record never leaves a `Lettering.dialogue_id` naming nothing.
    for id in out.dialogue_deleted.clone() {
        remove_balloons_showing(b, id, &mut out)?;
    }
    Ok(out)
}

/// Delete every balloon that **shows** `dialogue`, wherever it is (design §6.1, §11.1).
///
/// A `Dialogue` owns the words and a `Lettering` is only where they are drawn, so when the record
/// goes the balloons that showed it have no words left: they go with it, and their frame, tail and
/// text objects with them. Such a balloon need not be the deleted panel's — `comic.lettering.add
/// {dialogue}` lets a balloon show a record its own panel does not own, and the record can even be
/// on another page — so this looks through the whole bundle. Leaving one behind would name a
/// missing record: a live rule-1 Error that only `comic.lettering.add` could clear, which is the
/// dead end design §11.1's row is not allowed to create.
fn remove_balloons_showing(b: &mut ComicBundle, dialogue: DialogueId, out: &mut Deletion) -> Result<()> {
    let mut pages: Vec<PageId> =
        b.pages.iter().filter(|(_, f)| f.lettering.iter().any(|l| l.dialogue_id == Some(dialogue))).map(|(id, _)| *id).collect();
    pages.sort_by_key(|p| p.0);
    for id in pages {
        let file = page_file_mut(b, id)?;
        let mut i = 0;
        while i < file.lettering.len() {
            if file.lettering[i].dialogue_id == Some(dialogue) {
                let l = file.lettering.remove(i);
                out.nodes.push(l.frame_node_id);
                out.nodes.push(l.text_node_id);
                if let Some(tail) = l.tail_node_id {
                    out.nodes.push(tail);
                }
                for panel in &mut file.panels {
                    panel.lettering_ids.retain(|x| *x != l.id);
                }
                if !out.lettering.contains(&l.id) {
                    out.lettering.push(l.id);
                }
            } else {
                i += 1;
            }
        }
    }
    Ok(())
}

/// The JSON of a [`Deletion`], with the reports the policy requires.
pub(crate) fn deletion_json(out: &Deletion) -> Value {
    json!({
        "removedNodes": out.nodes.iter().map(|n| n.0).collect::<Vec<_>>(),
        "letteringDeleted": out.lettering.iter().map(|l| l.0).collect::<Vec<_>>(),
        "dialogueDeleted": out.dialogue_deleted.iter().map(|d| d.0).collect::<Vec<_>>(),
        "dialogueDetached": out.dialogue_detached.iter().map(|d| d.0).collect::<Vec<_>>(),
        "dialogueDetachedNote": if out.dialogue_detached.is_empty() { Value::Null } else { json!("kept, not retyped: their panel_id names the panel that was just deleted, so schema rule 1 reports them until comic.lettering.add {panel, dialogue} adopts them") },
        "assetsKept": out.assets_kept.iter().map(|a| a.0).collect::<Vec<_>>(),
        "assetsKeptNote": "Asset records are never deleted: lineage and provenance outlive the panel (design §11.1)",
        "jobsStillTargeting": out.jobs_still_targeting.iter().map(|j| j.0).collect::<Vec<_>>(),
        "jobsStillTargetingNote": if out.jobs_still_targeting.is_empty() { Value::Null } else { json!("left to finish by design; each is reported as targeting a missing entity once it lands") },
    })
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.panel.delete";
    let panel = PanelId(raw_id(p, "panel", C)?);
    let reuse = dialogue_ids_param(p, "reuse", C)?;
    let locked = locked_panel(&open(s)?.bundle, panel);
    if !locked.is_empty() {
        return Err(locked_refusal(C, "nothing is deleted", &locked));
    }
    comic_edit(s, "Delete Panel", |s| {
        let page = locate_panel(&open(s)?.bundle, panel)?.0;
        let out = {
            let oc = open_mut(s)?;
            remove_panel(&mut oc.bundle, page, panel, &reuse)?
        };
        s.edit("Delete Panel", |d, _| {
            drop_nodes(d, &out.nodes);
            Ok(())
        })?;
        let order = reading_order_ids(page_file(&open(s)?.bundle, page)?);
        Ok(json!({ "panel": panel.0, "page": page.0, "readingOrder": order, "deletion": deletion_json(&out) }))
    })
}
