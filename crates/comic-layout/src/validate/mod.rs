//! Checks over a plan and over the facts a loaded page gives (design §12.1).
//!
//! **Everything here is read-only, and nothing repairs.** A check that reported *and* fixed would
//! hide the inconsistency from the creator (schema §13), so a finding names what failed and why and
//! leaves the decision to the caller (design §6.3).
//!
//! Three entry points, and each takes only the facts it needs:
//!
//! * [`faults`] — the geometric core. It measures a set of rectangles against a page and returns
//!   [`Fault`]s, with no text and no policy but the thresholds it is given. This is the check design
//!   §12.1 calls "invalid panel geometry": overlap, gutter width, minimum panel size and containment.
//! * [`plan`] — a [`PagePlan`] checked **before it is applied** (design §8.1), which is where a
//!   template that asked for a gutter below the minimum is caught.
//! * [`page`] — a loaded page checked against [`DocumentFacts`], which is the only place the geometry
//!   of *existing* frame objects can be checked, because this crate never reads the document itself.

pub mod fault;

use std::collections::BTreeMap;

use vectorcraft_comic_model::{DocumentFacts, Finding, NodeId, PageFile, PageId};
use vectorcraft_geom::Rect;

use crate::PanelRect;
use crate::layout::PagePlan;
use crate::reading_order;
pub use fault::Fault;

/// The thresholds a layout is checked against.
///
/// **Policy, not schema.** The frozen schema has no panel-size or gutter field (schema §5.3), so
/// these are this crate's defaults and a caller may replace them. They are the values design §12.1
/// means by "minimum panel size" and "gutter width": 48pt is two thirds of an inch, below which a
/// panel cannot carry a readable balloon or a legible image at print size, and 6pt is the tightest
/// gap that still reads as two panels rather than one.
///
/// The fields are public and there is no invariant to keep: a nonsense threshold — negative, or
/// `NaN` — simply never fires, because every check is a comparison against a measured length. Setting
/// one to `0.0` therefore switches that check off, which is how a project that does not want a
/// minimum says so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The smallest width a panel may have, in points.
    pub min_panel_width: f64,
    /// The smallest height a panel may have, in points.
    pub min_panel_height: f64,
    /// The narrowest gap two neighbouring panels may leave, in points.
    pub min_gutter: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { min_panel_width: 48.0, min_panel_height: 48.0, min_gutter: 6.0 }
    }
}

/// The frame rectangles a caller read out of the visual document, by node id.
///
/// `comic-layout` never reads the document (design §4.3, §6.1): a caller that holds the loaded
/// document reads each panel frame's rectangle and hands it over. A frame that is **absent** was not
/// supplied, so the checks that need its geometry are skipped for that panel rather than guessed at —
/// a rectangle this crate invented would be exactly the second authority design §6.1 forbids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelFrames {
    rects: BTreeMap<NodeId, PanelRect>,
}

impl PanelFrames {
    /// No frames: every geometric check over a loaded page is skipped.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same set with one more frame, for a caller that builds it in one expression.
    #[must_use]
    pub fn with(mut self, node: NodeId, rect: PanelRect) -> Self {
        self.rects.insert(node, rect);
        self
    }

    /// Add a frame, returning the rectangle it replaced, if any.
    pub fn insert(&mut self, node: NodeId, rect: PanelRect) -> Option<PanelRect> {
        self.rects.insert(node, rect)
    }

    /// The frame rectangle for a node, if the caller supplied one.
    #[must_use]
    pub fn get(&self, node: NodeId) -> Option<PanelRect> {
        self.rects.get(&node).copied()
    }

    /// How many frames were supplied.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Whether no frames were supplied.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

/// The floor of the tolerance every threshold comparison in [`faults`] allows, in points.
///
/// **What a tolerance is for here.** Two lengths this crate compares are never the same number: a
/// panel's edge and a page's edge, or a measured gap and a minimum, are each the result of a few
/// roundings, and comparing them strictly reports arithmetic as a defect. A validator that cries wolf
/// on arithmetic is worse than no validator, and an `Error` from one would have a command refuse a plan
/// the template just built.
///
/// **This floor is absolute, and it is not enough on its own.** `1e-9`pt is seven orders of magnitude
/// below the smallest mark a printer can place (~0.01pt), and four above the `~1e-13` drift the
/// template arithmetic used to produce — but a fixed epsilon cannot work when the coordinates are
/// unbounded: at `1e7`pt a gap comes out `3.7e-10` under the gutter and at `1e8` `6.0e-9` under, both
/// past this floor, and the crate's own plan then reported **24 spurious `GutterTooThin` warnings** at
/// `1e8` (round 2's R2). So the tolerance that is actually used is [`tolerance_at`], which grows with
/// the coordinate magnitude and falls back to this floor for ordinary pages.
pub const GEOMETRY_TOLERANCE: f64 = 1e-9;

/// The multiple of one ULP of the coordinate magnitude that [`tolerance_at`] grows by.
///
/// **Not "eight ULP".** The term is `TOLERANCE_ULPS * scale * EPSILON`, and one ULP of `scale` is
/// `mantissa * scale * EPSILON` with the mantissa in `[1, 2)` — so this constant is `8 * mantissa` ULP
/// of `scale`: **between 8 and 16 ULP**, measured 9.3 … 15.3 over `1e6` … `1e10`pt (the name counts the
/// multiplier, not the unit; `the_tolerance_grows_with_the_magnitude_and_keeps_the_floor_near_the_origin`
/// pins the range). Even the low end is comfortably more than the handful of roundings a gap or an edge
/// involves — the worst drift measured is **1.0 ULP** of the magnitude, over 294 420 far edges — and it
/// is still far below any length a page can express: the relative term stays under a printer's smallest
/// mark (0.01pt) until the coordinates pass ~`5.6e12`pt — a page `1.99e9`m across, **wider than the
/// Sun** (`1.39e9`m across) and still absurd next to anything a creator has.
pub const TOLERANCE_ULPS: f64 = 8.0;

/// The tolerance for geometry at coordinates of a given magnitude, in points.
///
/// [`GEOMETRY_TOLERANCE`], or [`TOLERANCE_ULPS`] times one ULP of the magnitude when that is larger —
/// the magnitude being the scale of the numbers the arithmetic started from, which is what the error
/// scales with. A page near the origin gets the absolute floor; a page at `1e8` gets `1.8e-7`pt, which
/// is still nothing next to a gutter. A non-finite `scale` has no ceiling — but a **finite** one always
/// does not, at any magnitude, because the product below is ordered to stay finite (round 5).
///
/// [`faults`] sizes it from `magnitude`, which never hands a non-finite coordinate here.
#[must_use]
pub fn tolerance_at(scale: f64) -> f64 {
    // **The order of this product is load-bearing, not a style choice — do not "tidy" it back to the
    // left-to-right form.** Written `TOLERANCE_ULPS * scale.abs() * f64::EPSILON`, `(8.0 * scale)` is
    // computed first and overflows to `+inf` for a finite `scale` past `f64::MAX / 8` (~`2.247e307`pt),
    // *before* epsilon is ever multiplied in; the whole term is then `inf`, `max(inf)` is `inf`, every
    // threshold holds and the geometry check is **silently switched off for a finite page** — the same
    // failure mode round 4 closed for a non-finite one, and the reason a `5e299`pt overlap on a
    // `2.25e307`pt page went unreported. Multiplying the small constant first (round 5) keeps the term
    // finite for *every* finite `scale`: `8 * EPSILON` is `1.776e-15`, and times even `f64::MAX` it is
    // `3.19e293`. The two orders are bit-identical for every ordinary page (equal at `1e3` … `1e12`, and
    // `tolerance_at(1e12)` is still exactly `0.0017763568394002505`), so only the huge end of the range
    // tells them apart — and it tells them apart by going silent.
    GEOMETRY_TOLERANCE.max(TOLERANCE_ULPS * f64::EPSILON * scale.abs())
}

/// The geometric checks design §12.1 gives this crate, as structured faults.
///
/// Every pair of panels is examined once: a pair that shares area is an overlap, and a pair that is
/// separated on one axis while their extents on the other meet is a **neighbour**, whose gap is
/// measured. A pair apart on both axes is diagonal, and there is no gap between them to measure.
/// Panels that merely touch are neighbours with a gap of zero, which is a *thin gutter* rather than
/// an overlap — and a shared sliver within the tolerance is read the same way, because arithmetic is
/// what puts it there.
///
/// `safe_margin` is the margin a panel should stay inside, or `None` when the caller has no such
/// policy to check: a loaded page has none, because the schema stores no per-page margin (§5.3), and
/// the check is then not made rather than made up.
///
/// Every comparison allows the tolerance from [`tolerance_at`], because the rectangles a template produces are its
/// own arithmetic and can land an ULP off a threshold — see that constant for the measurement. The scale is
/// `magnitude`'s and there is **one for the whole call**, so one runaway rectangle sets the tolerance for
/// everything in it; a coordinate that is not finite does not set it at all. The comparisons include
/// the **overlap** test, which is why this function does not use [`PanelRect::overlaps`]:
/// that predicate is strict by contract (touching is not overlapping), and with a zero gutter the
/// drift makes two stacked panels share a `1e-13`-deep sliver — a spurious *error* that would have a
/// command refuse a plan the template just built. Here a shared area counts only when its depth on
/// both axes is past the tolerance, and a sliver within it is reported the way touching is: as a
/// gutter of zero.
#[must_use]
pub fn faults(page: Rect, safe_margin: Option<f64>, panels: &[PanelRect], limits: &Limits) -> Vec<Fault> {
    let mut out = Vec::new();
    let tolerance = tolerance_at(magnitude(page, panels));
    for (index, panel) in panels.iter().enumerate() {
        if panel.x() < page.x0 - tolerance
            || panel.y() < page.y0 - tolerance
            || panel.right() > page.x1 + tolerance
            || panel.bottom() > page.y1 + tolerance
        {
            out.push(Fault::OutsidePage { panel: index });
        } else if let Some(margin) = safe_margin {
            // Only when the panel is inside the page: a panel that is off the page has already been
            // reported, and saying it also crosses the margin would be a second finding for one fault.
            if panel.x() < page.x0 + margin - tolerance
                || panel.y() < page.y0 + margin - tolerance
                || panel.right() > page.x1 - margin + tolerance
                || panel.bottom() > page.y1 - margin + tolerance
            {
                out.push(Fault::InSafeMargin { panel: index });
            }
        }
        if panel.width() < limits.min_panel_width - tolerance || panel.height() < limits.min_panel_height - tolerance {
            out.push(Fault::TooSmall {
                panel: index,
                width: panel.width(),
                height: panel.height(),
                min_width: limits.min_panel_width,
                min_height: limits.min_panel_height,
            });
        }
    }
    for first in 0..panels.len() {
        for second in (first + 1)..panels.len() {
            let (a, b) = (&panels[first], &panels[second]);
            let depth_x = overlap_depth(a.x(), a.right(), b.x(), b.right());
            let depth_y = overlap_depth(a.y(), a.bottom(), b.y(), b.bottom());
            if depth_x > tolerance && depth_y > tolerance {
                out.push(Fault::Overlap { first, second });
            } else if let Some(gap) = neighbour_gap(depth_x, depth_y, tolerance)
                && gap < limits.min_gutter - tolerance
            {
                out.push(Fault::GutterTooThin { first, second, gap, minimum: limits.min_gutter });
            }
        }
    }
    out
}

/// The largest absolute **finite** coordinate a check is about: the scale every tolerance in one call
/// grows with.
///
/// **One scale for the whole call, not one per comparison — so one runaway rectangle sets the
/// tolerance for everything in the call.** For a plan this crate produced that is the same thing as
/// sizing the tolerance per comparison, because containment keeps every panel inside the page, so the
/// page's coordinates are the largest in play. [`faults`] is public and its own doc says a caller hands
/// it rectangles this crate did not compute, so the assumption is *stated* rather than relied on
/// silently: on a 612x792pt page at the origin, a panel 0.001pt past its safe margin is a finding on
/// its own, and the same call with one 1e12pt rectangle in it reports only the runaway — the runaway
/// sets the tolerance (`1.8e-3`pt) for every comparison in that call. The cost is measured rather than
/// left to be inferred (`one_runaway_rectangle_sets_the_tolerance_for_the_whole_call`), and masking a
/// defect a printer can *see* (0.01pt) needs a rectangle at ~`5.6e12`pt (`0.01 / (8 * EPSILON)`),
/// which is a caller whose own rectangle contradicts its page.
///
/// A coordinate that is **not finite** is skipped instead of being allowed to set the scale:
/// [`tolerance_at`] has no ceiling, so an infinite page would make every threshold hold and switch the
/// checks off entirely (round 4). **There is no *finite* ceiling.** Round 3 recorded one at
/// ~`2.25e307`pt and round 4 left it standing on the argument that a defect there is below one ULP
/// (`~2e292`pt) and so unrepresentable — that argument was **wrong**: `8.0 * scale` was overflowing to
/// `inf` *before* the multiply by epsilon, so a perfectly representable `5e299`pt overlap on a
/// `2.25e307`pt page was silently unreported. Round 5 reorders the product in [`tolerance_at`] (small
/// constant first), which keeps the tolerance finite at every finite magnitude, so the only skip here
/// is a coordinate that is not finite at all.
fn magnitude(page: Rect, panels: &[PanelRect]) -> f64 {
    let mut scale = 0.0_f64;
    let mut consider = |value: f64| {
        if value.is_finite() {
            scale = scale.max(value.abs());
        }
    };
    for value in [page.x0, page.y0, page.x1, page.y1] {
        consider(value);
    }
    for panel in panels {
        for value in [panel.x(), panel.y(), panel.right(), panel.bottom()] {
            consider(value);
        }
    }
    scale
}

/// How much two intervals share, in points: positive when they overlap, zero when they touch, and
/// negative when they are apart by that distance.
fn overlap_depth(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> f64 {
    a_end.min(b_end) - a_start.max(b_start)
}

/// The gap between two panels that sit side by side or one above the other, or `None` when they are
/// not neighbours.
///
/// A pair is a neighbour when one axis separates them — or they touch on it, within the tolerance —
/// and their extents on the other axis meet. A pair apart on both axes is diagonal, and there is no
/// gutter between them to measure.
fn neighbour_gap(depth_x: f64, depth_y: f64, tolerance: f64) -> Option<f64> {
    match (separation(depth_x, tolerance), separation(depth_y, tolerance)) {
        (Some(gap), _) if depth_y > -tolerance => Some(gap),
        (_, Some(gap)) if depth_x > -tolerance => Some(gap),
        _ => None,
    }
}

/// The gap between two intervals: `None` when they share real area on this axis — which is an overlap
/// and not a gutter — and otherwise the distance between them, reported as `0.0` when they touch or
/// share no more than the tolerance.
fn separation(depth: f64, tolerance: f64) -> Option<f64> {
    if depth > tolerance {
        None
    } else if depth >= 0.0 {
        Some(0.0)
    } else {
        Some(-depth)
    }
}

/// Check a plan **before anything is written** (design §8.1).
///
/// The plan's own margin is the safe area it was laid out to, so a correctly laid-out plan reports
/// nothing — `a_planned_three_panel_page_reports_nothing` holds that. The checks earn their keep on a
/// plan whose settings ask for a gutter below the minimum, and on [`faults`]' callers, who check
/// rectangles this crate did not compute.
///
/// Panels are labelled by their position in [`PagePlan::panels`] — **placement order**, top to bottom
/// and left to right within a row — because the faults are about rectangles, and that is the order
/// the rectangles are in. It is not the reading order, and for an `rtl` page the two differ: the
/// reading position is [`PlannedPanel::reading_order_index`](crate::panel::PlannedPanel::reading_order_index), which the label does not imply.
#[must_use]
pub fn plan(plan: &PagePlan, limits: &Limits) -> Vec<Finding> {
    let panels: Vec<PanelRect> = plan.panels.iter().map(|panel| panel.rect).collect();
    faults(plan.artboard, Some(plan.margin()), &panels, limits).iter().map(|fault| fault.describe(&|index| format!("Page.panels[{index}]"))).collect()
}

/// What [`page`] needs besides the pages themselves.
#[derive(Clone, Copy, Debug)]
pub struct PageInput<'a> {
    /// The pages to check, by id — `ComicBundle::pages`. Narrower than the bundle on purpose: these
    /// are the facts the check reads, and nothing else about a project changes its answer.
    pub pages: &'a BTreeMap<PageId, PageFile>,
    /// The artboards and node ids of the loaded visual document (schema §9).
    pub document: &'a DocumentFacts,
    /// The frame rectangles the caller read from the document, by node id.
    pub frames: &'a PanelFrames,
    /// The thresholds to check against.
    pub limits: &'a Limits,
}

/// Check the pages of a loaded bundle against the facts the caller supplies.
///
/// Read-only, and it repairs nothing (schema §13, design §6.3): a dangling frame link is reported so
/// a creator can repair it, never silently replaced, and no rectangle is written anywhere.
///
/// Three things are checked, and the third is the one the comic model cannot check for itself:
///
/// 1. every `Panel.frame_node_id` resolves in the document the caller handed over (schema rule 7) —
///    a panel whose frame is gone has no geometry, and the layout cannot check what is not there;
/// 2. the page's explicit reading order is a permutation of its panels, and every panel's
///    `reading_order_index` agrees with its position (schema rule 6, [`reading_order::check`]);
/// 3. the geometry of the frames supplied in [`PanelFrames`]: overlap, gutter width, minimum panel
///    size and containment in the page (design §12.1 — the only check here that needs a rectangle).
///
/// (1) and (2) are also reported by `comic-model`'s own `validate` from the same facts, because
/// schema §13 gives those predicates to the model and design §12.1 gives the *layout* half of the
/// checks to this crate. The subjects are spelled as `comic-model` spells them — the layout findings
/// use `Panel(<id>)` — so a caller that runs both can deduplicate them.
///
/// **Deduplicate on `(rule, subject)`, not on the whole [`Finding`].** The messages differ on purpose
/// — this crate's message says what the layout consequence is, `comic-model`'s names the schema rule —
/// so a caller comparing a `Finding` field by field would report one defect twice. Round 1 checked the
/// claim against a mutated fixture and found the pair does deduplicate; the struct does not. The
/// geometry findings need no key at all: their `rule` is `None` and their subjects (`Panel(<id>)`) are
/// this crate's own, so nothing else produces them.
///
/// The safe margin is **not** checked here: a loaded page has no margin to check against (§5.3 stores
/// none), so the check is not made rather than made up.
#[must_use]
pub fn page(input: &PageInput<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    for file in input.pages.values() {
        for panel in &file.panels {
            if !input.document.nodes.contains(&panel.frame_node_id) {
                out.push(Finding::error(
                    Some(7),
                    format!("Panel({}).frame_node_id", panel.id.0),
                    format!("node {} does not exist in the visual document, so the panel has no geometry to check", panel.frame_node_id.0),
                ));
            }
        }
        out.extend(reading_order::check(&file.page, &file.panels));

        // A page whose artboard is missing, has no area, or is not **finite** is skipped, and that is a
        // **deliberate choice** rather than something `faults` forces. `faults` *can* size a tolerance
        // from such an artboard and would answer it correctly: `magnitude` skips a non-finite
        // coordinate (round 4), so the primitive reports the panels' overlap and their size findings
        // with no spurious containment, exactly as it does for a finite page. The skip is made because
        // it is the same treatment a **degenerate** artboard already gets — a page that is not a
        // rectangle has no rectangle to measure a panel against, so the containment half is meaningless
        // and the overlap half would answer about a page whose artboard is already corrupt — and because
        // the malformed artboard is named one layer up, by `comic-model`'s rule 15 against
        // `default_page_size`, not by this crate (design §12.1 gives "unsupported page dimensions" to
        // the model). Reporting every panel of such a page as off it would bury that one finding under a
        // pageful of noise. (`is_finite()` covers the `NaN` side too; `plan` and `rows` refuse a
        // non-finite artboard, so reaching here means a caller's rectangle or a malformed document.)
        let Some(artboard) = input.document.artboard(file.page.artboard_id) else { continue };
        let (width, height) = (artboard.rect.width(), artboard.rect.height());
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            continue;
        }
        let mut labels = Vec::new();
        let mut rects = Vec::new();
        for panel in &file.panels {
            if let Some(rect) = input.frames.get(panel.frame_node_id) {
                labels.push(format!("Panel({})", panel.id.0));
                rects.push(rect);
            }
        }
        for fault in faults(artboard.rect, None, &rects, input.limits) {
            out.push(fault.describe(&|index| labels.get(index).cloned().unwrap_or_else(|| format!("panels[{index}]"))));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{PageInput as PlanPage, PlanRequest, plan as plan_page};
    use crate::template::{LayoutSettings, TemplateName};
    use vectorcraft_comic_model::{ArtboardFacts, ReadingDirection};
    use vectorcraft_geom::Size;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
        PanelRect::new(x, y, width, height).expect("a test rectangle")
    }

    fn letter() -> Rect {
        Rect::new(0.0, 0.0, 612.0, 792.0)
    }

    #[test]
    fn a_page_whose_rows_do_not_divide_evenly_is_not_reported_as_a_defect() {
        // Round 1's F1. `612x1000pt` with seven rows does not divide evenly, and with the nominal
        // height used for the last row too, that page's last row landed `1.1e-13` past the content box:
        // a strict comparison called it "crosses the safe margin" (a warning), and with no margin it
        // called it "not inside the page" (an **error**, which would have had the command refuse a plan
        // the template just built). `template::rows` now takes the remainder for the last row, so the
        // edge is exact — and this pins that the page is one where the division is inexact, or the case
        // proves nothing.
        for (margin, gutter) in [(18.0, 12.0), (0.0, 12.0)] {
            let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
            let planned = plan_page(
                PlanPage::Artboard(Rect::new(0.0, 0.0, 612.0, 1000.0)),
                &PlanRequest::new(TemplateName::Grid { rows: 7, columns: 1 }, 7, ReadingDirection::Ltr),
                &settings,
            )
            .expect("612x1000pt fits seven rows");
            let content = crate::template::content_box(planned.artboard, &settings).expect("a content box");
            let step = content.height() - gutter * 6.0;
            assert!(step % 7.0 != 0.0, "margin {margin}: this page must be one whose division is inexact");
            let last = planned.panels.last().expect("seven rows");
            assert_eq!(last.rect.bottom(), content.y1, "margin {margin}: the last row's edge is the content box's");
            assert_eq!(plan(&planned, &Limits::default()), Vec::new(), "margin {margin}, gutter {gutter}");
        }
    }

    #[test]
    fn a_zero_gutter_page_whose_rows_do_not_divide_evenly_is_not_an_overlap() {
        // The sharper half of F1: with no gutter the rows are meant to **touch**. The nominal height
        // put a `1.1e-13`-deep sliver between two of them, and `PanelRect::overlaps` is strict, so the
        // strict check returned an error — the one finding that would have stopped the command. With the
        // remainder taken for the last row the rows touch exactly, which is what a zero gutter means.
        let settings = LayoutSettings::new(18.0, 0.0).expect("a zero gutter is legal");
        let planned = plan_page(
            PlanPage::Artboard(Rect::new(0.0, 0.0, 612.0, 1000.0)),
            &PlanRequest::new(TemplateName::Grid { rows: 7, columns: 1 }, 7, ReadingDirection::Ltr),
            &settings,
        )
        .expect("612x1000pt fits seven rows");
        // `Grid { rows: 7, columns: 1 }`, so consecutive panels are consecutive rows, and each row is
        // placed from the one above it — so a zero gutter means they meet *exactly*, not within the
        // tolerance. That is what makes a zero gutter a deliberate borderless pair rather than an
        // overlap, which is the finding that would have stopped the command.
        let touching = planned.panels.windows(2).all(|pair| pair[1].rect.y() == pair[0].rect.bottom());
        assert!(touching, "with no gutter every row must start exactly where the one above ends");
        // `min_gutter: 0.0` so the thin-gutter warning is off: the overlap is what is under test.
        assert_eq!(plan(&planned, &Limits { min_gutter: 0.0, ..Limits::default() }), Vec::new());
        // And with the default minimum, touching is reported as a gutter of zero — a warning, never as
        // a shared area.
        let findings = plan(&planned, &Limits::default());
        assert!(findings.iter().all(|finding| !finding.is_error()), "{findings:?}");
        assert!(findings.iter().any(|finding| finding.message.contains("0pt from")), "{findings:?}");
    }

    #[test]
    fn no_page_geometry_this_crate_can_produce_is_reported_as_a_defect() {
        // Round 1 measured the drift over 70 005 heights: 12 629 drifted, giving 6 313 spurious
        // warnings and 6 367 spurious errors. This sweeps that space through the public path — plan a
        // page, then check it — so the *class* stays fixed and not only the case in the finding.
        //
        // The limits are the page's own gutter and no size floor, so a finding here can only be a real
        // defect: an overlap, a panel off the page, or a panel in the margin.
        let mut checked = 0;
        let mut drifted = 0;
        for rows in 2..=9u32 {
            for columns in 1..=3u32 {
                for origin in [(0.0, 0.0), (100.0, 50.0), (1e6, 1e6), (1e7, 1e7), (1e8, 1e8)] {
                    for height in [500.0, 792.0, 964.3, 1000.0, 1234.5, 1099.7] {
                        for margin in [0.0, 3.3, 18.0] {
                            for gutter in [0.0, 3.3, 12.0] {
                                let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                                let request = PlanRequest::new(TemplateName::Grid { rows, columns }, rows * columns, ReadingDirection::Ltr);
                                let page = Rect::new(origin.0, origin.1, origin.0 + 611.0, origin.1 + height);
                                let planned = plan_page(PlanPage::Artboard(page), &request, &settings).expect("the page has room for the grid");
                                // The arithmetic is exact now, so no combination may drift: a non-zero count
                                // here is a **second** cause, and the tolerance would be hiding it.
                                let content_bottom = planned.artboard.y1 - margin;
                                let overshoot = planned.panels.last().map_or(0.0, |last| last.rect.bottom() - content_bottom);
                                // Across row boundaries only: consecutive panels are two panels of the *same*
                                // row in a multi-column grid, and those always share a top edge.
                                let columns = columns as usize;
                                let sliver = (1..rows as usize).any(|row| {
                                    let below = planned.panels[row * columns].rect;
                                    let above = planned.panels[row * columns - 1].rect;
                                    below.y() < above.bottom()
                                });
                                if overshoot != 0.0 || sliver {
                                    drifted += 1;
                                    eprintln!(
                                        "drifted: {rows}x{columns} in 611x{height}pt, margin {margin}, gutter {gutter}, overshoot {overshoot}, sliver {sliver}"
                                    );
                                }
                                let limits = Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: gutter };
                                assert_eq!(
                                    plan(&planned, &limits),
                                    Vec::new(),
                                    "{rows}x{columns} in 611x{height}pt, margin {margin}, gutter {gutter}"
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 8 * 3 * 5 * 6 * 3 * 3, "the sweep must cover every combination it claims to");
        assert_eq!(
            drifted, 0,
            "{drifted} of {checked} combinations still drift: `template::rows` promises exact far edges, so this is a second cause"
        );
    }

    #[test]
    fn a_page_far_from_the_origin_is_not_reported_for_its_own_arithmetic() {
        // Round 2's R2. The tolerance was absolute, so the crate's own plan was reported for a gap it
        // had just computed: at `1e7` two `GutterTooThin` warnings ("sits 3.299999998882413pt …,
        // closer than the 3.3pt minimum gutter") and 24 at `1e8`. The position of a page is unbounded,
        // so the tolerance grows with the coordinate magnitude instead.
        for scale in [1e6, 1e7, 1e8] {
            let settings = LayoutSettings::new(0.0, 3.3).expect("legal settings");
            let planned = plan_page(
                PlanPage::Artboard(Rect::new(scale, scale, scale + 612.0, scale + 792.0)),
                &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr),
                &settings,
            )
            .expect("the page has room");
            // The page's own gutter is the minimum, so only a real defect may be reported.
            let limits = Limits { min_gutter: 3.3, ..Limits::default() };
            assert_eq!(plan(&planned, &limits), Vec::new(), "at {scale:e}pt");
            assert!(tolerance_at(scale) > GEOMETRY_TOLERANCE, "the tolerance must grow with the magnitude at {scale:e}pt");
        }
        // The floor still applies where the magnitude is small, and the growth is the documented shape.
        assert_eq!(tolerance_at(0.0), GEOMETRY_TOLERANCE);
        assert_eq!(tolerance_at(612.0), GEOMETRY_TOLERANCE, "a US Letter page is far below the floor's reach");
        assert_eq!(tolerance_at(1e8), TOLERANCE_ULPS * 1e8 * f64::EPSILON);
    }

    #[test]
    fn a_gap_one_ulp_under_the_minimum_is_not_reported() {
        // The gutter half of F1: a measured gap can come out one ULP under the requested gutter
        // (`3.299999999999997` for `3.3`), so a caller whose minimum is their own gutter must not be
        // told their page is thin. The drift is produced here the way a page produces it — by placing
        // the second panel one gutter below the first — rather than written as a literal.
        let second = 100.0 + 3.3;
        assert!(second - 100.0 < 3.3, "the test needs a gap that is really one ULP under the minimum");
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(0.0, second, 100.0, 100.0)];
        assert_eq!(faults(letter(), None, &panels, &Limits { min_gutter: 3.3, ..Limits::default() }), Vec::new());
        // A gap a thousandth of a point under the minimum is still a thin gutter: the tolerance is
        // far below a point, so it cannot mask a real defect.
        let thin = [rect(0.0, 0.0, 100.0, 100.0), rect(0.0, 100.0 + 3.299, 100.0, 100.0)];
        assert_eq!(faults(letter(), None, &thin, &Limits { min_gutter: 3.3, ..Limits::default() }).len(), 1);
    }

    #[test]
    fn a_plan_whose_panel_crosses_the_safe_margin_is_reported() {
        // F5: `PagePlan` is plain public data, so a caller may hand-build one — design §8.1's
        // "optional fixed panel geometry" arrives that way — and a panel inside the page but inside
        // the margin is exactly what this branch is for. It is **live**, and it has to be built by
        // hand to be seen: a plan this crate's templates produce is laid out inside the content box
        // and never reaches it.
        let settings = LayoutSettings::default();
        let mut planned = plan_page(PlanPage::Artboard(letter()), &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr), &settings)
            .expect("US Letter fits three tiers");
        planned.panels[0].rect = rect(5.0, 5.0, 100.0, 100.0);
        let findings = plan(&planned, &Limits::default());
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].subject, "Page.panels[0]");
        assert!(findings[0].message.contains("safe margin"), "{}", findings[0].message);
        assert!(!findings[0].is_error(), "a panel that bleeds deliberately is a warning, not a blocker");
    }

    #[test]
    fn a_planned_three_panel_page_reports_nothing() {
        // The template's own output must pass the crate's own checks: a plan that warned about itself
        // would make every caller either ignore warnings or refuse to lay out a correct page.
        let settings = LayoutSettings::default();
        let planned = plan_page(PlanPage::Artboard(letter()), &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr), &settings)
            .expect("US Letter fits three tiers");
        assert_eq!(plan(&planned, &Limits::default()), Vec::new());
    }

    #[test]
    fn a_gutter_below_the_minimum_warns_rather_than_blocks() {
        // 2pt between panels still renders, so the plan is not refused; a creator is told.
        let settings = LayoutSettings::new(18.0, 2.0).expect("2pt is a legal gutter");
        let planned = plan_page(PlanPage::Artboard(letter()), &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr), &settings)
            .expect("US Letter fits three tiers");
        let findings = plan(&planned, &Limits::default());
        assert_eq!(findings.len(), 2, "the two stacked pairs are both too close: {findings:?}");
        assert_eq!(findings[0].subject, "Page.panels[0]");
        assert_eq!(findings[0].message, "sits 2pt from Page.panels[1], closer than the 6pt minimum gutter");
        assert!(!findings[0].is_error());
    }

    #[test]
    fn a_panel_below_the_minimum_blocks() {
        let settings = LayoutSettings::new(5.0, 5.0).expect("a legal margin and gutter");
        let planned = plan_page(
            PlanPage::Artboard(Rect::new(0.0, 0.0, 200.0, 200.0)),
            &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr),
            &settings,
        )
        .expect("a 200x200pt page has room for three tiers");
        // The page gives each panel 60pt of height; a 100pt minimum makes every one of them too
        // small, and the 5pt gutter is below the 6pt default as well.
        let limits = Limits { min_panel_height: 100.0, ..Limits::default() };
        let findings = plan(&planned, &limits);
        let too_small: Vec<&Finding> = findings.iter().filter(|finding| finding.message.contains("minimum panel size")).collect();
        assert_eq!(too_small.len(), 3, "{findings:?}");
        assert!(too_small.iter().all(|finding| finding.is_error() && finding.subject.starts_with("Page.panels[")));
        assert!(too_small[0].message.contains("below the 48x100pt minimum panel size"), "{}", too_small[0].message);
        // And the gutter is a warning, not a blocker: 5pt still renders.
        assert!(findings.iter().any(|finding| !finding.is_error() && finding.message.contains("minimum gutter")), "{findings:?}");
    }

    #[test]
    fn overlap_outranks_a_thin_gutter_for_the_same_pair() {
        // One finding per pair: a pair that shares area is an overlap, not an overlap *and* a gap of
        // minus something.
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 0.0, 100.0, 100.0)];
        let faults = faults(letter(), None, &panels, &Limits::default());
        assert_eq!(faults, vec![Fault::Overlap { first: 0, second: 1 }]);
    }

    #[test]
    fn touching_panels_are_a_zero_gutter_not_an_overlap() {
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(100.0, 0.0, 100.0, 100.0)];
        let faults = faults(letter(), None, &panels, &Limits::default());
        assert_eq!(faults, vec![Fault::GutterTooThin { first: 0, second: 1, gap: 0.0, minimum: 6.0 }]);
    }

    #[test]
    fn a_diagonal_pair_is_not_neighbours_and_has_no_gap() {
        // Separated on both axes: no edge is shared, so there is no gutter to measure — even with a
        // minimum gutter wide enough that a naive check would report it.
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(200.0, 200.0, 100.0, 100.0)];
        assert_eq!(faults(letter(), None, &panels, &Limits { min_gutter: 150.0, ..Limits::default() }), Vec::new());
    }

    #[test]
    fn a_wide_enough_gutter_is_not_reported() {
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(112.0, 0.0, 100.0, 100.0)];
        assert_eq!(faults(letter(), None, &panels, &Limits::default()), Vec::new());
    }

    #[test]
    fn containment_is_checked_against_the_page_and_against_the_safe_margin() {
        let inside_margin = rect(5.0, 5.0, 100.0, 100.0);
        let off_page = rect(-1.0, 5.0, 100.0, 100.0);
        // Inside the page but crossing a 10pt margin: a warning, once.
        assert_eq!(faults(letter(), Some(10.0), &[inside_margin], &Limits::default()), vec![Fault::InSafeMargin { panel: 0 }]);
        // Off the page: an error, and *not* also a margin finding for the same panel.
        assert_eq!(faults(letter(), Some(10.0), &[off_page], &Limits::default()), vec![Fault::OutsidePage { panel: 0 }]);
        // With no margin policy, only the page is checked.
        assert_eq!(faults(letter(), None, &[inside_margin], &Limits::default()), Vec::new());
    }

    #[test]
    fn the_checks_are_deterministic_and_ordered_by_panel() {
        // Three stacked panels: (0,1) and (1,2) are neighbours with a zero gap, and (0,2) is not a
        // neighbour at all — they are 100pt apart, and a pair separated by more than the gutter is
        // simply not a finding.
        let panels = [rect(0.0, 0.0, 100.0, 100.0), rect(0.0, 100.0, 100.0, 100.0), rect(0.0, 200.0, 100.0, 100.0)];
        let first = faults(letter(), None, &panels, &Limits::default());
        assert_eq!(first, faults(letter(), None, &panels, &Limits::default()));
        assert_eq!(
            first,
            vec![
                Fault::GutterTooThin { first: 0, second: 1, gap: 0.0, minimum: 6.0 },
                Fault::GutterTooThin { first: 1, second: 2, gap: 0.0, minimum: 6.0 },
            ]
        );
    }

    /// One page, from JSON, so a page's record shape stays `comic-model`'s business.
    fn page_file(json: &str) -> PageFile {
        serde_json::from_str(json).expect("a test page file")
    }

    fn one_panel_page(frame_node_id: u64, reading_order_index: u16) -> PageFile {
        page_file(&format!(
            r#"{{
              "schema_version": 1,
              "page": {{ "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                         "panel_ids": [9], "reading_order": [9], "status": "draft" }},
              "panels": [ {{ "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4,
                             "frame_node_id": {frame_node_id}, "artwork_refs": [], "shot_description": "",
                             "dialogue_ids": [], "lettering_ids": [], "reading_order_index": {reading_order_index},
                             "continuity_constraints": {{ "characters": [], "wardrobe": null, "time_of_day": null,
                                                          "callbacks": [], "exceptions": [] }},
                             "generation_status": "none" }} ],
              "dialogue": [], "lettering": []
            }}"#
        ))
    }

    fn pages(files: Vec<PageFile>) -> BTreeMap<PageId, PageFile> {
        files.into_iter().map(|file| (file.page.id, file)).collect()
    }

    fn document(nodes: &[u64]) -> DocumentFacts {
        DocumentFacts {
            artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }],
            nodes: nodes.iter().copied().map(NodeId).collect(),
        }
    }

    #[test]
    fn a_page_whose_frame_node_is_gone_is_reported_and_its_geometry_is_skipped() {
        // The panel's frame object was deleted outside a comic command (design §6.3): the link
        // dangles, and the geometry cannot be checked because there is nothing to read.
        let pages = pages(vec![one_panel_page(99, 0)]);
        let frames = PanelFrames::new().with(NodeId(99), rect(0.0, 0.0, 100.0, 100.0));
        let findings = page(&PageInput { pages: &pages, document: &document(&[2]), frames: &frames, limits: &Limits::default() });
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, Some(7));
        assert_eq!(findings[0].subject, "Panel(9).frame_node_id");
        assert!(findings[0].is_error());
    }

    #[test]
    fn a_resolved_page_with_supplied_frames_is_checked_for_geometry() {
        // Two panels of one page that share area: the model cannot see this — it holds no rectangle —
        // and the caller's frames are the only way to find it.
        let two = page_file(
            r#"{ "schema_version": 1,
              "page": { "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                        "panel_ids": [9, 10], "reading_order": [9, 10], "status": "draft" },
              "panels": [ { "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 2,
                            "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                            "reading_order_index": 0,
                            "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                            "generation_status": "none" },
                          { "id": 10, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 3,
                            "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                            "reading_order_index": 1,
                            "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                            "generation_status": "none" } ],
              "dialogue": [], "lettering": [] }"#,
        );
        let pages = pages(vec![two]);
        let frames = PanelFrames::new().with(NodeId(2), rect(18.0, 18.0, 300.0, 244.0)).with(NodeId(3), rect(200.0, 18.0, 300.0, 244.0));
        let findings = page(&PageInput { pages: &pages, document: &document(&[2, 3]), frames: &frames, limits: &Limits::default() });
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].subject, "Panel(9)");
        assert_eq!(findings[0].message, "overlaps Panel(10): two panels cannot share area");
        assert_eq!(findings[0].rule, None);
    }

    #[test]
    fn a_page_with_no_supplied_frames_is_still_checked_for_its_links_and_order() {
        let pages = pages(vec![one_panel_page(2, 3)]);
        let findings = page(&PageInput { pages: &pages, document: &document(&[2]), frames: &PanelFrames::new(), limits: &Limits::default() });
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, Some(6));
        assert_eq!(findings[0].subject, "Panel(9).reading_order_index");
        assert_eq!(findings[0].message, "is 3 but the panel is at position 0 in `Page.reading_order`");
    }

    #[test]
    fn a_page_whose_artboard_is_missing_is_left_to_rule_4() {
        // The artboard is gone, so there is no page rectangle to check against. Reporting each panel
        // as off-page would be a pageful of noise over one real finding, which rule 4 owns.
        let pages = pages(vec![one_panel_page(2, 0)]);
        let frames = PanelFrames::new().with(NodeId(2), rect(0.0, 0.0, 100.0, 100.0));
        let orphaned = DocumentFacts { artboards: Vec::new(), nodes: [NodeId(2)].into_iter().collect() };
        let findings = page(&PageInput { pages: &pages, document: &orphaned, frames: &frames, limits: &Limits::default() });
        assert_eq!(findings, Vec::new());
    }

    #[test]
    fn an_empty_page_set_reports_nothing() {
        // The scan must not pass vacuously: an empty set is a legal answer, and this pins that the
        // function does not invent a finding to look busy.
        let pages = BTreeMap::new();
        let findings = page(&PageInput { pages: &pages, document: &document(&[]), frames: &PanelFrames::new(), limits: &Limits::default() });
        assert_eq!(findings, Vec::new());
    }

    #[test]
    fn frames_are_a_map_a_caller_can_build_and_read_back() {
        let frames = PanelFrames::new().with(NodeId(2), rect(1.0, 2.0, 3.0, 4.0)).with(NodeId(3), rect(5.0, 6.0, 7.0, 8.0));
        assert_eq!(frames.len(), 2);
        assert!(!frames.is_empty());
        assert_eq!(frames.get(NodeId(2)).map(|rect| rect.x()), Some(1.0));
        assert_eq!(frames.get(NodeId(9)), None);
        assert!(PanelFrames::new().is_empty());
        let mut replaced = frames.clone();
        assert_eq!(replaced.insert(NodeId(2), rect(9.0, 9.0, 9.0, 9.0)).map(|rect| rect.x()), Some(1.0));
        assert_eq!(replaced.get(NodeId(2)).map(|rect| rect.x()), Some(9.0));
    }

    #[test]
    fn a_new_page_is_planned_at_the_size_it_was_given() {
        // The one place a plan sets the page size (design §8.1); the caller creates the artboard with it.
        let settings = LayoutSettings::default();
        let planned =
            plan_page(PlanPage::NewPage(Size::new(612.0, 792.0)), &PlanRequest::new(TemplateName::WideTop, 3, ReadingDirection::Rtl), &settings)
                .expect("a page of that size fits a wide top");
        assert_eq!(planned.artboard_size, Some(Size::new(612.0, 792.0)));
        assert_eq!(plan(&planned, &Limits::default()), Vec::new());
    }
}
