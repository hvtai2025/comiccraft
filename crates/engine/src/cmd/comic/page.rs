//! `comic.page.*` and `comic.reading_order.set`: the pages of a project and the narrative order of
//! a page's panels.
//!
//! A page **is** an artboard (design §5.1.1): there is no `page_size` and no second document. So
//! these commands own the artboard lifecycle (schema §9.1 mitigation 1) and close the artboard-id
//! hazard themselves — `comic.page.create` never hands a page an id the manifest has retired and
//! never one **any page already names** (an artboard the ordinary `artboard.delete` removed is not
//! retired by anyone, and handing its number to a new page would make two pages resolve to one
//! artboard), because `Artboard.id` is `max(id) + 1` with no `fix_next_id()` equivalent
//! (mitigation 4), and `comic.page.delete` retires the id it removes (mitigation 2) so a stale
//! `Page.artboard_id` can never resolve to a different page.
//!
//! `comic.page.reorder` moves a page **within its scene** and puts the document's artboards in book
//! order in the same operation, so a render or a PDF export that walks the artboards follows the
//! comic's reading order. Moving a page *between* scenes is a story edit, and M1 adds no `story.*`
//! command; the artboard is not moved on the canvas either, because a page's artboard is its size
//! and position and sliding it would slide the art with it (`artboard.setProps` does that, and it
//! stays available).

use serde_json::{Value, json};
use vectorcraft_comic_layout::layout::{self, PageInput, PlanRequest};
use vectorcraft_comic_layout::template::{LayoutSettings, TemplateName};
use vectorcraft_comic_model::{ChapterId, ComicBundle, Page, PageFile, PageId, PageStatus, PanelId, SceneId};
use vectorcraft_geom::Rect;

use super::*;
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "comic.page.create",
            "New Comic Page",
            ["Window", "Comic"],
            None,
            "{size?: {width, height} or [width, height] (default: the project's default_page_size; refused when neither is given), chapter?: ChapterId (default: the first), scene?: SceneId (default: that chapter's first), pageNumber?: u32 (default: one past the highest), template?: a template name to lay panels out with} claim a free artboard of the document or allocate a new one whose id no page already names and the manifest has **not** retired (schema §9.1 mitigations 3-4), then write pages/page-<PageId>.json → {page, artboard_id, artboard_rect, page_number, skipped_retired_artboards, panels: [PanelId]}. One undo step",
            has_comic,
            create
        ),
        cmd!(
            "comic.page.delete",
            "Delete Page",
            ["Window", "Comic"],
            None,
            "{page: PageId, reuse?: [DialogueId…] dialogues to detach instead of delete, as comic.panel.delete} delete every panel of the page by the design §11.1 policy, drop the page from its scene, retire its artboard id in the manifest (never pruned: schema rule 17) and remove the artboard from the document → {page, artboard_id, removedNodes, deletedPanels, letteringDeleted, dialogueDeleted, dialogueDetached, assetsKept, jobsStillTargeting, deletion: {the same totals and their notes as one object}}. `letteringDeleted` is every balloon deleted with the page — including a balloon elsewhere that was showing a dialogue the page owned (a dialogue is deleted with its panel, and the balloons that showed it have no words of their own). **Refused while any panel of the page is locked** (§6.2): this deletes them all, so one `comic.panel.set_locked {panel, locked: false}` per locked panel is the way through. One undo step",
            has_comic,
            delete
        ),
        cmd!(
            "comic.page.reorder",
            "Reorder Pages",
            ["Window", "Comic"],
            None,
            "{page: PageId, to: the page's new 0-based position **within its scene**} move the page in Scene.page_ids and put the document's artboards in book order (chapter, scene, page), which is the order a render or PDF export walks → {page, scene, scene_order: [PageId], artboards: [artboard id in book order]}. Moving a page between scenes is a story edit (M2+), not a comic.page one. One undo step",
            has_comic,
            reorder
        ),
        cmd!(
            "comic.page.template.apply",
            "Apply Page Template",
            ["Window", "Comic"],
            None,
            "{page: PageId, template: \"two-tier\"|\"three-tier\"|\"four-tier\"|\"wide-top\"|\"grid-<rows>x<columns>\", margin?: pt (18), gutter?: pt (12), replace?: false} lay the template out over the page's artboard. With no panels, one is created per slot; with the same count, the existing panels keep their ids and narrative index and their frames move to the template's rectangles, and the page's `reading_order` is left exactly as it is — this path renumbers nothing, so a gap stays a gap and stays reported (design §11.1: a gap is never a silent repair); a different count is refused unless replace is true, which deletes them first by the design §11.1 policy. Creating panels or replacing them does renumber, and there a page whose reading_order is not a permutation of its panels is **refused**, not repaired → {page, template, panels: [{panel, frame, index}], reading_order: [PanelId], findings: [geometry warnings]}. **The plan is validated before it is written** and a rejected plan writes nothing (design §8.1). **Refused while any panel of the page is locked** (§6.2): every path touches every panel — with no panels it creates them, at the same count it moves every frame, and `replace: true` deletes them. One undo step",
            has_comic,
            template_apply
        ),
        cmd!(
            "comic.reading_order.set",
            "Set Reading Order",
            ["Window", "Comic"],
            None,
            "{page: PageId, order: [PanelId…] every panel of the page exactly once, in the order they are read} write Page.reading_order and every Panel.reading_order_index in one step → {page, reading_order}. An order that is not a permutation of the page's panels is **refused**, not repaired (design §11.1). **A lock does not apply**: `Panel.locked` (§6.2) protects where a panel is and what it shows, and this writes the page's narrative order, which a locked panel has no claim on. One undo step",
            has_comic,
            reading_order_set
        ),
    ]
}

// ---------- chapters and scenes ----------

fn chapter_for(s: &Session, given: Option<u64>, cmd: &str) -> Result<ChapterId> {
    let b = &open(s)?.bundle;
    let id = match given {
        Some(raw) => ChapterId(raw),
        None => *b.project.chapter_ids.first().ok_or_else(|| bad(cmd, "the project has no chapter; comic.project.create makes one"))?,
    };
    if !b.chapters.chapters.iter().any(|c| c.id == id) {
        return Err(bad(cmd, format!("no such chapter {}", id.0)));
    }
    Ok(id)
}

fn scene_for(s: &Session, chapter: ChapterId, given: Option<u64>, cmd: &str) -> Result<SceneId> {
    let b = &open(s)?.bundle;
    let id = match given {
        Some(raw) => SceneId(raw),
        None => b
            .scenes
            .scenes
            .iter()
            .find(|sc| sc.chapter_id == chapter)
            .map(|sc| sc.id)
            .ok_or_else(|| bad(cmd, format!("chapter {} has no scene; the project has none to put a page in", chapter.0)))?,
    };
    if !b.scenes.scenes.iter().any(|sc| sc.id == id) {
        return Err(bad(cmd, format!("no such scene {}", id.0)));
    }
    Ok(id)
}

// ---------- book order ----------

/// Every page of the project in book order: chapters in the project's order, each chapter's scenes,
/// each scene's pages. A page no scene lists comes last, ordered by id, because it is still on disk
/// (schema rule 14 makes that a warning, not an error).
pub(crate) fn book_order(b: &ComicBundle) -> Vec<PageId> {
    let mut out: Vec<PageId> = Vec::new();
    let push = |id: PageId, out: &mut Vec<PageId>| {
        if !out.contains(&id) {
            out.push(id);
        }
    };
    let chapter_ids: Vec<ChapterId> = b.project.chapter_ids.iter().copied().chain(b.chapters.chapters.iter().map(|c| c.id)).collect();
    for chapter in chapter_ids {
        let scene_ids: Vec<SceneId> = b
            .chapters
            .chapters
            .iter()
            .find(|c| c.id == chapter)
            .map(|c| c.scene_ids.clone())
            .unwrap_or_default()
            .into_iter()
            .chain(b.scenes.scenes.iter().filter(|s| s.chapter_id == chapter).map(|s| s.id))
            .collect();
        for scene in scene_ids {
            if let Some(s) = b.scenes.scenes.iter().find(|s| s.id == scene) {
                for page in s.page_ids.iter().copied() {
                    push(page, &mut out);
                }
            }
        }
    }
    for id in b.pages.keys() {
        push(*id, &mut out);
    }
    out
}

// ---------- artboards ----------

/// An artboard a page may claim: one the document holds that no page uses and the manifest has not
/// retired (schema §9.1 mitigation 4).
fn free_artboards(s: &Session) -> Result<Vec<u32>> {
    let b = &open(s)?.bundle;
    let used: std::collections::BTreeSet<u32> = b.pages.values().map(|f| f.page.artboard_id).collect();
    Ok(s.doc()?.doc.artboards.iter().map(|a| a.id).filter(|id| !used.contains(id) && !b.manifest.is_artboard_retired(*id)).collect())
}

/// Claim a free artboard, or allocate fresh ones until the id is neither retired nor **named by a
/// page**.
///
/// `Artboard.id` is `max(id) + 1` (audit §13 U1): deleting the highest artboard frees its number, so
/// a page that took it would be permanently untrusted (rule 4) and permanently unfixable (rule 17).
/// Each allocated *picket* stays in the document while the next id is taken, which is what advances
/// the counter past the number being skipped; the pickets are removed once a fresh id is found.
///
/// **The ids pages name are claimed, not merely the ones the document holds.** Mitigation 4 retires
/// the ids `comic.page.delete` removed, but `artboard.delete` (`crates/engine/src/cmd/layer.rs`,
/// `Window > Artboards`) removes an artboard *without* retiring it and stays registered and
/// reachable — including when a comic project is open. A page whose artboard that command removed
/// still names the number, so handing that exact number to a new page would make two pages resolve
/// to one artboard *and* delete the rule-4 report that was showing the first page's breakage
/// (design §5.3.1). Skipping it is the conservative half of the same rule: an id a page names is
/// taken whether or not the artboard behind it exists.
fn claim_artboard(s: &mut Session, rect: Rect, name: String, retired: &[u32]) -> Result<(u32, u32)> {
    let free: Vec<u32> = free_artboards(s)?;
    let named: Vec<u32> = open(s)?.bundle.pages.values().map(|f| f.page.artboard_id).collect();
    if let Some(id) = free.first() {
        let id = *id;
        s.edit("New Page", |d, _| {
            if let Some(a) = d.artboards.iter_mut().find(|a| a.id == id) {
                a.rect = Rect::new(a.rect.x0, a.rect.y0, a.rect.x0 + rect.width(), a.rect.y0 + rect.height());
                a.name = name.clone();
            }
            Ok(())
        })?;
        return Ok((id, 0));
    }
    let retired = retired.to_vec();
    let (id, skipped) = s.edit("New Page", |d, _| {
        let last = d.artboards.last().map(|a| a.rect).unwrap_or(Rect::new(0.0, 0.0, rect.width(), rect.height()));
        let placed = Rect::new(last.x1 + 20.0, last.y0, last.x1 + 20.0 + rect.width(), last.y0 + rect.height());
        let mut pickets: Vec<u32> = Vec::new();
        let id;
        loop {
            let candidate = d.next_artboard_id();
            d.artboards.push(vectorcraft_doc::Artboard {
                id: candidate,
                name: name.clone(),
                rect: placed,
                show_center_mark: false,
                show_cross_hairs: false,
            });
            if retired.contains(&candidate) || named.contains(&candidate) {
                pickets.push(candidate);
                continue;
            }
            id = candidate;
            break;
        }
        // The pickets are never the highest artboard once `id` exists, so removing them cannot hand
        // their numbers back to the next allocation.
        let count = pickets.len();
        d.artboards.retain(|a| !pickets.contains(&a.id));
        Ok((id, u32::try_from(count).unwrap_or(u32::MAX)))
    })?;
    Ok((id, skipped))
}

// ---------- comic.page.create ----------

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.page.create";
    let chapter = chapter_for(s, p.get("chapter").and_then(Value::as_u64), C)?;
    let scene = scene_for(s, chapter, p.get("scene").and_then(Value::as_u64), C)?;
    let size = match super::project::page_size_param(p, "size", C)? {
        Some(size) => size,
        None => open(s)?
            .bundle
            .project
            .settings
            .default_page_size
            .ok_or_else(|| bad(C, "give `size`, or create the project with a `pageSize` so it has a default"))?,
    };
    let page_number = p.get("pageNumber").and_then(Value::as_u64).map(|n| u32::try_from(n).unwrap_or(u32::MAX));
    let template = str_param(p, "template").map(str::to_string);
    let retired = open(s)?.bundle.manifest.retired_artboard_ids.clone();
    comic_edit(s, "New Comic Page", |s| {
        let page_number = page_number.unwrap_or_else(|| u32::try_from(open(s).map(|o| o.bundle.pages.len()).unwrap_or(0) + 1).unwrap_or(u32::MAX));
        let rect = Rect::new(0.0, 0.0, size.width, size.height);
        let (artboard_id, skipped) = claim_artboard(s, rect, format!("Page {page_number}"), &retired)?;
        let artboard_rect = s
            .doc()?
            .doc
            .artboards
            .iter()
            .find(|a| a.id == artboard_id)
            .map(|a| a.rect)
            .ok_or_else(|| EngineError::Other(format!("artboard {artboard_id} was not created")))?;
        let page = {
            let oc = open_mut(s)?;
            let page = oc.bundle.allocate::<PageId>().map_err(|e| EngineError::Other(format!("{e}")))?;
            oc.bundle.pages.insert(
                page,
                PageFile {
                    schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
                    page: Page {
                        id: page,
                        chapter_id: chapter,
                        scene_ids: vec![scene],
                        artboard_id,
                        page_number,
                        panel_ids: Vec::new(),
                        reading_order: Vec::new(),
                        status: PageStatus::Draft,
                        unknown: Default::default(),
                    },
                    panels: Vec::new(),
                    dialogue: Vec::new(),
                    lettering: Vec::new(),
                    unknown: Default::default(),
                },
            );
            if let Some(sc) = oc.bundle.scenes.scenes.iter_mut().find(|sc| sc.id == scene) {
                sc.page_ids.push(page);
            }
            page
        };
        let panels = match &template {
            Some(name) => {
                let applied = apply_template(s, page, name, 18.0, 12.0, false)?;
                applied
                    .get("panels")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.get("panel").and_then(Value::as_u64)).collect())
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };
        Ok(json!({
            "page": page.0,
            "chapter": chapter.0,
            "scene": scene.0,
            "artboard_id": artboard_id,
            "artboard_rect": [artboard_rect.x0, artboard_rect.y0, artboard_rect.width(), artboard_rect.height()],
            "page_number": page_number,
            "skipped_retired_artboards": skipped,
            "panels": panels,
        }))
    })
}

// ---------- comic.page.delete ----------

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.page.delete";
    let page = PageId(super::panel::raw_id(p, "page", C)?);
    let reuse = super::panel::dialogue_ids_param(p, "reuse", C)?;
    let locked = super::panel::locked_on_page(&open(s)?.bundle, page);
    if !locked.is_empty() {
        return Err(super::panel::locked_refusal(C, "the page is not deleted", &locked));
    }
    comic_edit(s, "Delete Page", |s| {
        let artboard_id = super::panel::artboard_of(s, page)?;
        // Every panel of the page, by the design §11.1 policy. The order is the page's, so the
        // renumbering is deterministic.
        let panel_ids: Vec<PanelId> = page_file(&open(s)?.bundle, page)?.panels.iter().map(|p2| p2.id).collect();
        let mut all: Vec<Value> = Vec::new();
        let mut nodes: Vec<vectorcraft_doc::NodeId> = Vec::new();
        for id in &panel_ids {
            let out = {
                let oc = open_mut(s)?;
                super::panel::remove_panel(&mut oc.bundle, page, *id, &reuse)?
            };
            nodes.extend(out.nodes.iter().copied());
            all.push(super::panel::deletion_json(&out));
        }
        {
            let oc = open_mut(s)?;
            oc.bundle.pages.remove(&page);
            for sc in &mut oc.bundle.scenes.scenes {
                sc.page_ids.retain(|p2| *p2 != page);
            }
            oc.bundle.manifest.retire_artboard_id(artboard_id);
        }
        s.edit("Delete Page", |d, sel| {
            drop_nodes(d, &nodes);
            let index = d.artboards.iter().position(|a| a.id == artboard_id);
            if let Some(i) = index {
                let id = d.artboards.remove(i).id;
                d.retain_guides(sel, |_, g| g.artboard != Some(id));
            }
            Ok(())
        })?;
        let merged = merge_deletions(&all);
        // The totals are the result's own keys, exactly as the `params` string documents them — a
        // `params` string is the documentation an agent reads (§10.2), so the result must not make
        // it look up a level the string never mentioned. `deletion` carries the same object for
        // callers that want the §11.1 per-panel report as one block.
        let mut out = merged.clone();
        out["page"] = json!(page.0);
        out["artboard_id"] = json!(artboard_id);
        out["deletedPanels"] = json!(panel_ids.iter().map(|p2| p2.0).collect::<Vec<_>>());
        out["deletion"] = merged;
        Ok(out)
    })
}

/// Merge the per-panel §11.1 reports of a page deletion into one, so the caller sees the page's
/// totals rather than one block per panel.
fn merge_deletions(parts: &[Value]) -> Value {
    let collect =
        |key: &str| -> Vec<Value> { parts.iter().flat_map(|p| p.get(key).and_then(Value::as_array).cloned().unwrap_or_default()).collect() };
    let detached = collect("dialogueDetached");
    let jobs = collect("jobsStillTargeting");
    json!({
        "removedNodes": collect("removedNodes"),
        "letteringDeleted": collect("letteringDeleted"),
        "dialogueDeleted": collect("dialogueDeleted"),
        "dialogueDetached": detached,
        "dialogueDetachedNote": if detached.is_empty() { Value::Null } else { json!("kept, not retyped: their panel_id names a deleted panel, so schema rule 1 reports them until comic.lettering.add {panel, dialogue} adopts them") },
        "assetsKept": collect("assetsKept"),
        "assetsKeptNote": "Asset records are never deleted: lineage and provenance outlive the panel (design §11.1)",
        "jobsStillTargeting": jobs,
        "jobsStillTargetingNote": if jobs.is_empty() { Value::Null } else { json!("left to finish by design; each is reported as targeting a missing entity once it lands") },
    })
}

// ---------- comic.page.reorder ----------

fn reorder(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.page.reorder";
    let page = PageId(super::panel::raw_id(p, "page", C)?);
    let to = usize::try_from(p.get("to").and_then(Value::as_u64).ok_or_else(|| bad(C, "missing `to` (the page's new position in its scene)"))?)
        .map_err(|_| bad(C, "`to` is out of range"))?;
    comic_edit(s, "Reorder Pages", |s| {
        let scene = page_file(&open(s)?.bundle, page)?
            .page
            .scene_ids
            .first()
            .copied()
            .ok_or_else(|| EngineError::Other(format!("page {} is in no scene, so it has no position to move within", page.0)))?;
        {
            let oc = open_mut(s)?;
            let sc = oc
                .bundle
                .scenes
                .scenes
                .iter_mut()
                .find(|sc| sc.id == scene)
                .ok_or_else(|| EngineError::Other(format!("no such scene {}", scene.0)))?;
            let from = sc
                .page_ids
                .iter()
                .position(|p2| *p2 == page)
                .ok_or_else(|| EngineError::Other(format!("page {} is not listed in scene {}", page.0, scene.0)))?;
            if to >= sc.page_ids.len() {
                return Err(bad(C, format!("`to` is {to}, outside scene {} ({} pages)", scene.0, sc.page_ids.len())));
            }
            let moved = sc.page_ids.remove(from);
            sc.page_ids.insert(to, moved);
        }
        // The document's artboards follow the book order, so a render or a PDF that walks them
        // follows the comic's reading order (M1 acceptance test 10).
        let order = book_order(&open(s)?.bundle);
        let artboards: Vec<u32> = {
            let b = &open(s)?.bundle;
            order.iter().filter_map(|p2| b.pages.get(p2).map(|f| f.page.artboard_id)).collect()
        };
        let wanted = artboards.clone();
        s.edit("Reorder Pages", |d, _| {
            let rest: Vec<u32> = d.artboards.iter().map(|a| a.id).filter(|id| !wanted.contains(id)).collect();
            let mut all = wanted.clone();
            all.extend(rest);
            d.artboards.sort_by_key(|a| all.iter().position(|id| *id == a.id).unwrap_or(usize::MAX));
            Ok(())
        })?;
        let scene_order = {
            let b = &open(s)?.bundle;
            b.scenes.scenes.iter().find(|sc| sc.id == scene).map(|sc| sc.page_ids.iter().map(|p2| p2.0).collect::<Vec<_>>()).unwrap_or_default()
        };
        Ok(json!({ "page": page.0, "scene": scene.0, "scene_order": scene_order, "artboards": artboards }))
    })
}

// ---------- comic.reading_order.set ----------

fn reading_order_set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.reading_order.set";
    let page = PageId(super::panel::raw_id(p, "page", C)?);
    let raw = p.get("order").and_then(Value::as_array).ok_or_else(|| bad(C, "missing `order` (an array of panel ids)"))?;
    let order: Vec<PanelId> = raw
        .iter()
        .map(|v| v.as_u64().map(PanelId).ok_or_else(|| bad(C, "`order` is an array of panel ids (non-negative integers)")))
        .collect::<Result<Vec<_>>>()?;
    comic_edit(s, "Set Reading Order", |s| {
        let oc = open_mut(s)?;
        let file = page_file_mut(&mut oc.bundle, page)?;
        let mut wanted = order.clone();
        let mut members = file.page.panel_ids.clone();
        wanted.sort_by_key(|p2| p2.0);
        members.sort_by_key(|p2| p2.0);
        if wanted != members {
            return Err(bad(
                C,
                format!("`order` must name every panel of page {} exactly once ({} panels, {} given)", page.0, members.len(), order.len()),
            ));
        }
        apply_reading_order(file, order);
        Ok(json!({ "page": page.0, "reading_order": reading_ids_of(file) }))
    })
}

fn reading_ids_of(file: &vectorcraft_comic_model::PageFile) -> Vec<u64> {
    file.page.reading_order.iter().map(|p| p.0).collect()
}

// ---------- comic.page.template.apply ----------

/// Lay a template over a page (design §8.1). The plan is validated **before anything is written**,
/// and a plan with an error writes nothing.
pub(crate) fn apply_template(s: &mut Session, page: PageId, name: &str, margin: f64, gutter: f64, replace: bool) -> Result<Value> {
    const C: &str = "comic.page.template.apply";
    let template = TemplateName::from_name(name).map_err(|e| bad(C, format!("{e}")))?;
    let shape = template.shape().map_err(|e| bad(C, format!("{e}")))?;
    let settings = LayoutSettings::new(margin, gutter).map_err(|e| bad(C, format!("{e}")))?;
    let rect = checked_page_rect(page, page_rect(s, page)?)?;
    let direction = open(s)?.bundle.project.metadata.reading_direction;
    let request = PlanRequest::new(template, shape.panel_count(), direction);
    let plan = layout::plan(PageInput::Artboard(rect), &request, &settings).map_err(|e| bad(C, format!("{e}")))?;
    let plan_findings = vectorcraft_comic_layout::validate::plan(&plan, &vectorcraft_comic_layout::validate::Limits::default());
    if let Some(error) = plan_findings.iter().find(|f| f.is_error()) {
        return Err(bad(C, format!("the plan is refused, nothing was written: {error}")));
    }
    let warnings: Vec<Value> = plan_findings.iter().map(finding_json).collect();

    let existing: Vec<PanelId> = page_file(&open(s)?.bundle, page)?.panels.iter().map(|p2| p2.id).collect();
    // The lock (§6.2), before anything is written. **Every** path below touches every panel: the
    // create path only runs on a page with no panels, the re-lay path moves each frame, and
    // `replace: true` deletes them — so one check on the page's panels is the whole rule, and it is
    // asked here rather than in one branch so a later branch cannot quietly skip it.
    let locked = super::panel::locked_on_page(&open(s)?.bundle, page);
    if !locked.is_empty() {
        return Err(super::panel::locked_refusal(C, "the template is not applied", &locked));
    }
    if !existing.is_empty() && existing.len() != plan.panels.len() && !replace {
        return Err(bad(
            C,
            format!(
                "page {} has {} panels and template `{}` places {}; pass replace: true to delete them first",
                page.0,
                existing.len(),
                template.as_name(),
                plan.panels.len()
            ),
        ));
    }
    // **A gap is a validation error, never a silent repair** (§11.1). A page with no panels and a
    // panel count that is replaced both *renumber* the page's order — new panels have to be placed
    // in it — so those two paths run the same guard `comic.panel.add` and `comic.panel.delete` run,
    // and refuse a page whose `reading_order` is not a permutation of its panels. The re-lay below
    // changes no membership and therefore renumbers nothing; see its branch.
    if existing.is_empty() || replace {
        require_permutation(&page_file(&open(s)?.bundle, page)?.page)?;
    }
    if replace && !existing.is_empty() {
        for id in &existing {
            let out = {
                let oc = open_mut(s)?;
                super::panel::remove_panel(&mut oc.bundle, page, *id, &[])?
            };
            let nodes = out.nodes.clone();
            s.edit("Apply Page Template", |d, _| {
                drop_nodes(d, &nodes);
                Ok(())
            })?;
        }
    }

    let scene = page_file(&open(s)?.bundle, page)?
        .page
        .scene_ids
        .first()
        .copied()
        .ok_or_else(|| EngineError::Other(format!("page {} is in no scene, so a panel cannot record which scene it belongs to", page.0)))?;
    let artboard_id = super::panel::artboard_of(s, page)?;
    let mut table: Vec<Value> = Vec::new();
    if existing.is_empty() || replace {
        // One frame per plan slot, in placement order; the narrative index is the plan's.
        let mut created: Vec<(u16, PanelId)> = Vec::new();
        for slot in &plan.panels {
            let frame = super::panel::create_frame(s, "Apply Page Template", slot_rect(slot))?;
            let panel = {
                let oc = open_mut(s)?;
                let id = oc.bundle.allocate::<PanelId>().map_err(|e| EngineError::Other(format!("{e}")))?;
                super::panel::add_panel_record(&mut oc.bundle, id, page, artboard_id, scene, frame, slot.reading_order_index)?;
                id
            };
            created.push((slot.reading_order_index, panel));
            table.push(json!({ "panel": panel.0, "frame": frame.0, "index": slot.reading_order_index }));
        }
        let mut order: Vec<PanelId> = created.iter().map(|(_, id)| *id).collect();
        order.sort_by_key(|id| created.iter().find(|(_, i)| i == id).map(|(i, _)| *i).unwrap_or(u16::MAX));
        let oc = open_mut(s)?;
        apply_reading_order(page_file_mut(&mut oc.bundle, page)?, order);
    } else {
        // Re-lay the panels that exist: the one read first keeps the first slot, and so on. Every id
        // and every narrative index survives, and **`reading_order` is not written**: this path
        // changes no membership, so it has nothing to renumber, and writing the order it can derive
        // from the panels' indices would be exactly the silent repair §11.1 forbids — it would heal
        // a gap (and overwrite a hand-edited order) behind the caller's back. A gapped page stays
        // gapped and stays rule-6 reported; the frames still move.
        let by_index: std::collections::BTreeMap<u16, PanelId> =
            page_file(&open(s)?.bundle, page)?.panels.iter().map(|p2| (p2.reading_order_index, p2.id)).collect();
        for slot in &plan.panels {
            let Some(panel) = by_index.get(&slot.reading_order_index) else {
                return Err(bad(C, "the page's reading order does not cover the template's slots; pass replace: true"));
            };
            let frame = {
                let b = &open(s)?.bundle;
                let index = locate_panel(b, *panel)?.1;
                page_file(b, page)?.panels[index].frame_node_id
            };
            let target = slot_rect(slot);
            s.edit("Apply Page Template", |d, _| super::panel::set_frame_rect(d, frame, target))?;
            table.push(json!({ "panel": panel.0, "frame": frame.0, "index": slot.reading_order_index }));
        }
    }
    let findings = {
        let page_rect = checked_page_rect(page, page_rect(s, page)?)?;
        let b = &open(s)?.bundle;
        let d = &s.doc()?.doc;
        let rects: Vec<super::panel::LabeledFrame> = page_file(b, page)?
            .panels
            .iter()
            .filter_map(|p2| node_frame(d, p2.frame_node_id).map(|rect| (format!("Panel({})", p2.id.0), rect)))
            .collect();
        let mut all = warnings;
        all.extend(super::panel::geometry_findings(page_rect, &rects));
        all
    };
    let reading_order = reading_order_ids(page_file(&open(s)?.bundle, page)?);
    Ok(json!({
        "page": page.0,
        "template": template.as_name(),
        "panels": table,
        "reading_order": reading_order,
        "findings": findings,
    }))
}

fn slot_rect(slot: &vectorcraft_comic_layout::panel::PlannedPanel) -> Rect {
    let r = slot.rect;
    Rect::new(r.x(), r.y(), r.x() + r.width(), r.y() + r.height())
}

fn template_apply(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.page.template.apply";
    let page = PageId(super::panel::raw_id(p, "page", C)?);
    let name = str_param(p, "template").ok_or_else(|| bad(C, "missing `template`"))?.to_string();
    let margin = f64_or(p, "margin", 18.0);
    let gutter = f64_or(p, "gutter", 12.0);
    let replace = bool_or(p, "replace", false);
    comic_edit(s, "Apply Page Template", |s| apply_template(s, page, &name, margin, gutter, replace))
}
