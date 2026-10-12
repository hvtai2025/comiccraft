//! M2 W5, criterion 1 — **"panels can be created from templates, reordered and resized with
//! validation warnings"** — as *one* run of the whole cycle on one page.
//!
//! M1 built the pieces: five templates write panel rectangles, and five geometry checks read them
//! back. What this file adds is the cycle the criterion actually states, in order and on one page:
//! **apply** a template → **reorder** the panels → **resize** one — and then the two halves the
//! criterion's own wording turns on:
//!
//! * the warnings that **must** appear for a resize that breaks a rule appear, exactly, with the
//!   numbers a creator can act on;
//! * the warnings that **must not** appear for the template's own plan, for the reorder, and for a
//!   resize that keeps every rule, do not. M1's defect record is full of runs that produced the right
//!   geometry and the wrong warnings, so "no finding here" is asserted as loudly as "this finding
//!   here" — including for the template's own plan, which is exactly where M1's F1 fired a spurious
//!   *warning* (and, at a zero margin, a spurious *error*) on a page the template had just built.
//!
//! Both warning severities design §8.1 gives a resize are exercised — a gutter below the minimum and
//! a panel crossing the safe margin — plus the error halves (below the minimum size) so the
//! severity split is pinned rather than assumed. And the cycle is then run a second time through the
//! **loaded** page — the frames the command wrote into the document — because that is the only path
//! on which an existing panel's geometry can be checked (`comic-layout` never reads the document).
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;

use vectorcraft_comic_layout::layout;
use vectorcraft_comic_layout::template::{LayoutSettings, TemplateName};
use vectorcraft_comic_layout::validate;
use vectorcraft_comic_layout::{PanelRect, reading_order};
use vectorcraft_comic_model::{ArtboardFacts, DocumentFacts, NodeId, PageFile, PageId, PageSize, ReadingDirection, SCHEMA_VERSION, Severity};
use vectorcraft_geom::Rect;

/// US Letter, the size the plan is created at, and the page the artboard mirrors.
fn letter() -> Rect {
    Rect::new(0.0, 0.0, 612.0, 792.0)
}

fn cell(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a legal panel")
}

fn corners(rect: &PanelRect) -> (f64, f64, f64, f64) {
    (rect.x(), rect.y(), rect.width(), rect.height())
}

/// A page file with the given panels — `(panel id, frame node id, reading_order_index)` — and the
/// given reading order, built from JSON so a page's record shape stays `comic-model`'s business and
/// a field added to `Panel` does not move this file.
fn page_file(panels: &[(u64, u64, u16)], order: &[u64]) -> PageFile {
    let records: Vec<String> = panels
        .iter()
        .map(|(id, frame, index)| {
            format!(
                r#"{{ "id": {id}, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": {frame},
                      "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                      "reading_order_index": {index},
                      "continuity_constraints": {{ "characters": [], "wardrobe": null, "time_of_day": null,
                                                   "callbacks": [], "exceptions": [] }},
                      "generation_status": "none" }}"#
            )
        })
        .collect();
    let numbers = |values: &[u64]| values.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
    let ids: Vec<u64> = panels.iter().map(|(id, _, _)| *id).collect();
    serde_json::from_str(&format!(
        r#"{{ "schema_version": {SCHEMA_VERSION},
             "page": {{ "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                        "panel_ids": [{}], "reading_order": [{}], "status": "draft" }},
             "panels": [{}], "dialogue": [], "lettering": [] }}"#,
        numbers(&ids),
        numbers(order),
        records.join(", ")
    ))
    .expect("a test page file")
}

fn document(nodes: &[u64]) -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: letter() }], nodes: nodes.iter().copied().map(NodeId).collect() }
}

/// The whole cycle, once, on one page: apply → reorder → resize, with the warnings asserted on both
/// sides of every threshold.
#[test]
fn applying_a_template_then_reordering_then_resizing_warns_only_where_a_rule_is_broken() {
    let limits = validate::Limits::default();

    // ---- 1. apply a template ---------------------------------------------------------------
    // The page does not exist yet, so the plan reports the size the command should create the
    // artboard at (design §8.1's first output).
    let planned = layout::plan(
        layout::PageInput::NewPage(PageSize { width: 612.0, height: 792.0 }),
        &layout::PlanRequest::new(TemplateName::Grid { rows: 2, columns: 2 }, 4, ReadingDirection::Ltr),
        &LayoutSettings::default(),
    )
    .expect("US Letter fits a 2x2 grid");
    assert_eq!(planned.panels.len(), 4);
    assert_eq!(planned.artboard_size, Some(PageSize { width: 612.0, height: 792.0 }));
    assert_eq!(corners(&planned.panels[0].rect), (18.0, 18.0, 282.0, 372.0));
    assert_eq!(corners(&planned.panels[1].rect), (312.0, 18.0, 282.0, 372.0));
    assert_eq!(corners(&planned.panels[3].rect), (312.0, 402.0, 282.0, 372.0));
    // **Must not warn.** The template's own plan is not reported against itself — M1's F1 was exactly
    // this sentence being false (a spurious `Warning` on the last row of every 7-row page, and an
    // `Error` on the same page with a zero margin, which would have had the command refuse a plan the
    // template had just built).
    assert_eq!(validate::plan(&planned, &limits), Vec::new(), "applying a template reports nothing");

    // The record the command writes: one panel per slot, its frame in the document, and the reading
    // order the plan named.
    let slots: Vec<u64> = vec![9, 10, 11, 12];
    let frames_of = |plan: &layout::PagePlan| -> Vec<PanelRect> { plan.panels.iter().map(|panel| panel.rect).collect() };

    // ---- 2. reorder the panels -------------------------------------------------------------
    // The creator rotates the page's order by one: panel 11 is read first, then 12, and 9 and 10
    // follow. Every index shifts with it, which is what being explicit and editable means — and it is
    // **not** the order any template produces for this shape (`narrative_order` gives `[0,1,2,3]` for
    // `ltr` and `[1,0,3,2]` for `rtl`), so the page is genuinely reordered rather than re-planned.
    let rotated = [11_u64, 12, 9, 10];
    let indices_after_rotation: Vec<(u64, u64, u16)> = vec![(9, 100, 2), (10, 101, 3), (11, 102, 0), (12, 103, 1)];
    // The premise, in the units both sides are stated in: as positions into placement order the
    // rotation is `[2, 3, 0, 1]`, and the two orders a 2x2 template can produce are `[0, 1, 2, 3]` and
    // `[1, 0, 3, 2]`. If it were one of those, the step would prove nothing about being editable.
    let in_slots = |order: &[u64]| order.iter().map(|id| slots.iter().position(|slot| slot == id).expect("a known panel")).collect::<Vec<_>>();
    assert_eq!(in_slots(&rotated), vec![2, 3, 0, 1]);
    for direction in [ReadingDirection::Ltr, ReadingDirection::Rtl] {
        assert_ne!(reading_order::narrative_order(&[2, 2], direction), vec![2_u16, 3, 0, 1], "{direction:?}");
    }
    let reordered = page_file(&indices_after_rotation, &rotated);
    // **Must not warn.** A reorder changes narrative data and no rectangle, so it is validated (rule
    // 6 runs, and it is a permutation with every index in step) and reports nothing. A finding here
    // would be the M1 defect shape: the right geometry with a warning nobody should see.
    let frames = validated_frames(&frames_of(&planned));
    let reordered_findings = validate::page(&validate::PageInput {
        pages: &pages(reordered),
        document: &document(&[100, 101, 102, 103]),
        frames: &frames,
        limits: &limits,
    });
    assert_eq!(reordered_findings, Vec::new(), "a legal reorder of an unchanged page reports nothing");

    // ---- 3. resize one, two ways that keep every rule, one that breaks a warning rule ---------
    // (a) a resize that keeps everything: panel 9 (slot 0) loses 10pt of width, so the gutter to its
    //     neighbour grows from 12pt to 22pt and nothing is reported.
    let mut narrower = planned.clone();
    narrower.panels[0].rect = cell(18.0, 18.0, 272.0, 372.0);
    let gap = narrower.panels[1].rect.x() - narrower.panels[0].rect.right();
    assert!(gap >= limits.min_gutter, "the premise of (a): {gap}pt is a legal gutter");
    assert_eq!(validate::plan(&narrower, &limits), Vec::new(), "a resize inside every rule reports nothing");

    // (b) a resize that breaks the gutter: 10pt the other way, so the gap is 2pt against a 6pt
    //     minimum. The check reports the pair the way it measured it, so the message carries the two
    //     panels and the two numbers.
    let mut grown = planned.clone();
    grown.panels[0].rect = cell(18.0, 18.0, 292.0, 372.0);
    let gap = grown.panels[1].rect.x() - grown.panels[0].rect.right();
    assert_eq!(gap, 2.0, "the premise of (b): the resized panels really are 2pt apart");
    assert!(gap < limits.min_gutter, "and {} is below the {}pt minimum", gap, limits.min_gutter);
    let warned = validate::plan(&grown, &limits);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert_eq!(warned[0].severity, Severity::Warning, "a tight pair still renders; a creator may want it");
    assert_eq!(warned[0].rule, None, "a geometry check is not one of the eighteen referential rules");
    assert_eq!(warned[0].subject, "Page.panels[0]");
    assert_eq!(warned[0].message, "sits 2pt from Page.panels[1], closer than the 6pt minimum gutter");
    // The reorder did not move the label, and that is the documented behaviour rather than an accident:
    // the resized panel is slot 0 in **placement** order and is read **second** on this page, and the
    // warning names it by its slot. `validate::plan` says so ("placement order ... It is not the reading
    // order"); a caller that wants the narrative position reads `Panel.reading_order_index`.
    assert_eq!((indices_after_rotation[0].0, indices_after_rotation[0].2), (9, 2), "slot 0 holds panel 9, which this page reads second");

    // (c) the other warning a resize earns: the panel's left edge is pulled out to 10pt, inside the
    //     page but outside the 18pt margin the plan was laid out to.
    let mut bleeding = planned.clone();
    bleeding.panels[0].rect = cell(10.0, 18.0, 290.0, 372.0);
    let warned = validate::plan(&bleeding, &limits);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert_eq!(warned[0].severity, Severity::Warning);
    assert_eq!(warned[0].subject, "Page.panels[0]");
    assert!(warned[0].message.contains("safe margin"), "{}", warned[0].message);

    // (d) and the error half, so the split is pinned rather than assumed: 40pt on both axes is below
    //     the minimum panel size, which is an **error** — a panel that cannot carry artwork or a
    //     balloon. One finding, for the size, and not also a gutter one: a pair that shares no axis
    //     is diagonal and has no gap to measure.
    let mut tiny = planned.clone();
    tiny.panels[0].rect = cell(18.0, 18.0, 40.0, 40.0);
    let errored = validate::plan(&tiny, &limits);
    assert_eq!(errored.len(), 1, "{errored:?}");
    assert_eq!(errored[0].severity, Severity::Error);
    assert!(errored[0].is_error(), "a command must refuse to apply this plan");
    assert_eq!(errored[0].subject, "Page.panels[0]");
    assert!(errored[0].message.contains("below the 48x48pt minimum panel size"), "{}", errored[0].message);

    // ---- 4. the same resize, checked again on the page the document now holds ----------------
    // The command wrote the resized rectangles into the frame objects; a caller reads them back and
    // hands them over, which is the only path on which an existing panel's geometry is checked. The
    // order is the reordered one, and the page is the 2pt-gutter page from (b), so there is exactly
    // one finding and it is the same warning — now labelled by the panel, since a loaded page's panels
    // are named by id and not by slot.
    let resized = {
        let mut plan = planned.clone();
        plan.panels[0].rect = cell(18.0, 18.0, 292.0, 372.0);
        plan
    };
    let frames = validated_frames(&frames_of(&resized));
    // The reorder survives the resize and is still reported clean; the geometry is what is reported.
    let findings = validate::page(&validate::PageInput {
        pages: &pages(page_file(&indices_after_rotation, &rotated)),
        document: &document(&[100, 101, 102, 103]),
        frames: &frames,
        limits: &limits,
    });
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].severity, Severity::Warning);
    assert_eq!(findings[0].subject, "Panel(9)");
    assert_eq!(findings[0].message, "sits 2pt from Panel(10), closer than the 6pt minimum gutter");
}

/// The frames the command wrote: slot `i` is node `100 + i`, at the plan's own rectangle.
fn validated_frames(rects: &[PanelRect]) -> validate::PanelFrames {
    let mut frames = validate::PanelFrames::new();
    for (slot, rect) in rects.iter().enumerate() {
        let node = u64::try_from(slot).expect("four slots") + 100;
        assert!(frames.insert(NodeId(node), *rect).is_none(), "one frame per node");
    }
    frames
}

fn pages(file: PageFile) -> BTreeMap<PageId, PageFile> {
    BTreeMap::from([(file.page.id, file)])
}
