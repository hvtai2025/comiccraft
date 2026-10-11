//! Round-4 `layout-engineer` tests for `crates/comic-layout`'s panel trees.
//!
//! Round 3 came back approved with four `preference` findings, and all four are folded in. Each one is
//! the "a stated guarantee that is not the delivered one" pattern, so each has a test here rather than
//! only a sentence changed:
//!
//! * **1** — `faults` sizes its tolerance from **one scale per call**, so one runaway rectangle sets
//!   the tolerance for every comparison in that call. The doc now says exactly that, and the cost is
//!   measured here instead of left to be inferred.
//! * **2** — `TOLERANCE_ULPS` is a *multiplier*, not a ULP count: `8 * scale * EPSILON` is
//!   `8 * mantissa` ULP. The doc says so; the constant is unchanged.
//! * **3** — the `≤ 1 ULP` bound `template::rows` states is **reachable**, so the old "worst 0.256 ULP"
//!   was a sample and not a bound. Pinned here so a reader sees the bound attained by a legal page.
//! * **4** — an **infinite** page used to satisfy every threshold, because `8 * scale * EPSILON`
//!   overflows to `inf`; a page that is not finite is now skipped deliberately, and `faults` itself
//!   ignores a non-finite coordinate when sizing its scale, so it reports for an infinite page what it
//!   reports for a finite one.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::template::{LayoutSettings, TemplateName, content_box, rows};
use vectorcraft_comic_layout::validate::{Fault, Limits, PanelFrames, TOLERANCE_ULPS, faults, tolerance_at};
use vectorcraft_comic_model::{ArtboardFacts, DocumentFacts, NodeId, PageFile, PageId};
use vectorcraft_geom::Rect;

/// One ULP of `x` — the unit `template::rows`' guarantee and `validate::tolerance_at` are stated in.
fn ulp(x: f64) -> f64 {
    let exponent = ((x.abs().to_bits() >> 52) & 0x7ff) as i64;
    assert!(exponent > 52, "the helper needs a normal number, not {x:e}");
    f64::from_bits(((exponent - 52) as u64) << 52)
}

fn shape(name: TemplateName) -> vectorcraft_comic_layout::template::Shape {
    name.shape().expect("a template name has a shape")
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a legal panel")
}

#[test]
fn one_runaway_rectangle_sets_the_tolerance_for_the_whole_call() {
    // Finding 1. The choice is one scale per call, and the doc's justification for it ("every panel is
    // inside the page") is true of a plan this crate produced and false of `faults`' inputs, which its
    // own doc says come from a caller. This test measures the cost of the choice so a reader can see
    // it: the same call reports a sub-printer breach alone and goes silent with one runaway rectangle
    // in it.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let margin = 18.0;
    let limits = Limits::default();
    // 0.001pt past the safe margin: below anything a printer can place, but representable here.
    let sneaking = rect(margin - 0.001, margin, 100.0, 100.0);
    assert_eq!(faults(page, Some(margin), &[sneaking], &limits), vec![Fault::InSafeMargin { panel: 0 }]);
    // The scale is the call's, and the page alone gives the floor: `tolerance_at(792.0) == 1e-9`.
    assert_eq!(tolerance_at(792.0), tolerance_at(0.0), "a US Letter page sizes the tolerance from the floor");
    let runaway = rect(1e12, 1e12, 100.0, 100.0);
    // One 1e12pt rectangle raises the tolerance for *every* comparison in the call, not just its own.
    assert_eq!(tolerance_at(1e12), TOLERANCE_ULPS * 1e12 * f64::EPSILON, "the runaway panel is now the scale");
    assert!(tolerance_at(1e12) > 0.001, "and the tolerance it sets is past the breach");
    assert_eq!(
        faults(page, Some(margin), &[sneaking, runaway], &limits),
        vec![Fault::OutsidePage { panel: 1 }],
        "with the runaway in the call, only the runaway is reported — the 0.001pt breach is masked"
    );
    // The size of the cost, from the formula: a defect a printer can *see* (0.01pt) needs a rectangle at
    // ~5.6e12pt to be masked, so the choice trades a sub-print breach for a document that contradicts
    // its own page.
    let masking_scale = 0.01 / (TOLERANCE_ULPS * f64::EPSILON);
    assert!(tolerance_at(masking_scale) >= 0.01 && tolerance_at(masking_scale / 10.0) < 0.01, "the boundary is {masking_scale:e}pt");
}

#[test]
fn an_infinite_page_reports_what_a_finite_page_reports() {
    // Finding 4, at `faults`. `tolerance_at` has no ceiling, so a page whose magnitude is `inf` used to
    // give an infinite tolerance and satisfy every threshold — the geometry check was silently off. The
    // scale now ignores a coordinate that is not finite, so the comparison that *is* representable (two
    // finite panels sharing area) is still made.
    let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 50.0, 100.0, 100.0)];
    let limits = Limits { min_gutter: 0.0, ..Limits::default() };
    let finite = Rect::new(0.0, 0.0, 612.0, 792.0);
    let infinite = Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY);
    let expected = vec![Fault::Overlap { first: 0, second: 1 }];
    assert_eq!(faults(finite, None, &panels, &limits), expected, "two panels sharing 50pt on both axes are an overlap");
    assert_eq!(faults(infinite, None, &panels, &limits), expected, "an infinite page must report what a finite one does");
    // The overflow the silence came from, kept visible: the tolerance itself still has no ceiling, so it
    // is the scale that must not be allowed to be infinite.
    assert!(tolerance_at(f64::INFINITY).is_infinite(), "the tolerance has no ceiling at a non-finite scale");
    // Round 5 removed the *finite* ceiling this line used to record: `8 * scale` no longer overflows for
    // a finite scale, because the small constant is multiplied first, so the term is finite even at
    // `f64::MAX` (`round5_layout_engineer.rs` is the failing-before/passing-after test for that).
    assert!(tolerance_at(f64::MAX).is_finite(), "and a finite scale has no ceiling either (round 5)");
    // A finite magnitude is still sized by the relative term, so this is not the checks switched off
    // for every page: `tolerance_at(1e12)` is the relative term's `1.8e-3`pt, not the floor and not
    // `inf`, so nothing about this change lowers the ceiling for a *finite* page.
    assert_eq!(tolerance_at(1e12), TOLERANCE_ULPS * 1e12 * f64::EPSILON, "a finite magnitude is still the relative term");
    assert!(tolerance_at(1e12) > 0.0 && tolerance_at(1e12) < 1.0, "and it is still a sub-point tolerance");
}

#[test]
fn a_non_finite_artboard_is_skipped_the_way_a_degenerate_one_is() {
    // Finding 4, at `validate::page`. The degenerate-artboard guard used to test `<= 0.0` and `is_nan()`
    // and let an **infinite** artboard through, where `faults` then had a magnitude of `inf`. It now
    // measures finiteness, so such an artboard is skipped deliberately — the same treatment a
    // zero-area one gets — and the findings that need no artboard are still reported exactly as on a
    // finite page.
    let file: PageFile = serde_json::from_str(
        r#"{ "schema_version": 1,
          "page": { "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                    "panel_ids": [9, 10, 11], "reading_order": [9, 10, 11], "status": "draft" },
          "panels": [ { "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 2,
                        "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                        "reading_order_index": 0,
                        "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                        "generation_status": "none" },
                      { "id": 10, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 3,
                        "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                        "reading_order_index": 1,
                        "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                        "generation_status": "none" },
                      { "id": 11, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 99,
                        "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                        "reading_order_index": 2,
                        "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                        "generation_status": "none" } ],
          "dialogue": [], "lettering": [] }"#,
    )
    .expect("a test page file");
    let pages: BTreeMap<PageId, PageFile> = [(PageId(8), file)].into_iter().collect();
    // Panel 11's frame is not in the document (rule 7), and 9 and 10 share 50pt on both axes.
    let frames = PanelFrames::new().with(NodeId(2), rect(0.0, 0.0, 100.0, 100.0)).with(NodeId(3), rect(50.0, 50.0, 100.0, 100.0));
    let limits = Limits { min_gutter: 0.0, ..Limits::default() };
    let report = |artboard: Rect| {
        let document =
            DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: artboard }], nodes: [NodeId(2), NodeId(3)].into_iter().collect() };
        vectorcraft_comic_layout::validate::page(&vectorcraft_comic_layout::validate::PageInput {
            pages: &pages,
            document: &document,
            frames: &frames,
            limits: &limits,
        })
    };
    let finite = report(Rect::new(0.0, 0.0, 612.0, 792.0));
    assert_eq!(finite.len(), 2, "a dangling frame and a real overlap: {finite:?}");
    assert_eq!(finite[0].rule, Some(7), "the link finding comes first: {finite:?}");
    assert_eq!(finite[1].message, "overlaps Panel(10): two panels cannot share area", "{finite:?}");
    // The artboard-independent half is reported unchanged on an artboard the geometry check cannot use...
    let infinite = report(Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY));
    assert_eq!(infinite.len(), 1, "an infinite artboard must not silence the link finding: {infinite:?}");
    assert_eq!(infinite[0], finite[0], "and it must report it exactly as the finite page does");
    // ...and that is the *deliberate* skip a zero-area artboard already got, not a tolerance switched off.
    let degenerate = report(Rect::new(0.0, 0.0, 0.0, 792.0));
    assert_eq!(degenerate, infinite, "a non-finite artboard is skipped the way a degenerate one is");
}

#[test]
fn the_one_ulp_bound_is_attained_by_a_legal_page() {
    // Finding 3. `template::rows` promises "within one ULP of the page's coordinate magnitude", and the
    // doc used to give 0.256 ULP over 47 922 probed pages as its worst case. That was a sample: the
    // bound is reachable, and this test is the page that attains it, so the sentence cannot drift back
    // to a number that reads like a bound.
    let mut attained = 0;
    let mut worst = 0.0_f64;
    for gutter in [0.0, 0.1, 3.3, 12.0] {
        let (origin, width, height, fraction) = ((0.1, 0.3), 612.0, 1999.9, 0.499);
        let margin = width * fraction;
        let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
        let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
        let Ok(content) = content_box(page, &settings) else { continue };
        let scale = page.x0.abs().max(page.y0.abs()).max(page.x1.abs()).max(page.y1.abs());
        let Ok(laid) = rows(page, &shape(TemplateName::Grid { rows: 1, columns: 3 }), &settings) else { continue };
        for (row, panels) in laid.iter().enumerate() {
            let last = panels.last().expect("a row holds a panel");
            for (edge, want, what) in [(last.right(), content.x1, "right"), (last.bottom(), content.y1, "bottom")] {
                if edge == want {
                    continue;
                }
                let off = (edge - want).abs() / ulp(scale);
                worst = worst.max(off);
                if off == 1.0 {
                    attained += 1;
                }
                assert!(off <= 1.0, "gutter {gutter}, row {row}: the {what} far edge is {off} ULP off");
            }
        }
    }
    assert!(attained > 0, "no far edge attains the bound, so the doc would be claiming slack: worst {worst} ULP");
}
