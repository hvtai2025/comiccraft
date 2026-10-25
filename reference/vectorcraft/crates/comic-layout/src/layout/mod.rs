//! The page plan: what a template computes, and what the command applies.
//!
//! **This crate computes and returns a plan. It never takes `&mut Document`** (design §8, §11.2).
//! The engine's comic command module applies the plan to the visual document, which is what keeps a
//! layout result one undoable operation and lets this crate be tested with plain rectangles and no
//! document at all.
//!
//! A [`PagePlan`] carries exactly what design §8.1 lists as the layout's output:
//!
//! * the page size, when the caller is creating the page and the template sets it (`artboard_size`);
//! * one rectangle per panel, in document points (`panels`), which the command turns into a frame
//!   object — the rectangle is **not** stored in a comic record, and the record keeps only
//!   `Panel.frame_node_id` (design §6.1);
//! * the gutter the template used (`settings`);
//! * `Panel.reading_order_index` and `Page.reading_order` (`panels[].reading_order_index` and
//!   `reading_order`);
//! * validation warnings — from [`crate::validate::plan`], which checks the plan **before it is
//!   applied** and reports rather than repairs. They are a function of the plan and not a field of
//!   it, so a caller that wants design §8.1's full output calls both.

pub mod error;

use vectorcraft_comic_model::{PageSize, ReadingDirection};
use vectorcraft_geom::Rect;

use crate::PanelRect;
use crate::panel::{self, PlannedPanel};
use crate::reading_order;
use crate::template;

pub use crate::template::{LayoutSettings, TemplateName};
pub use error::LayoutError;

/// A page to lay out: one that exists, or one being created.
///
/// The distinction is design §8.1's first output — "the artboard `rect`, when the template sets the
/// page size". An artboard that already exists **is** the page's size (§6.1, schema rule 15), and a
/// plan that carried a second copy of it would be the duplicate authority the design exists to
/// prevent; a page that does not exist yet has no size until someone names one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageInput {
    /// A page that already exists: its artboard's rectangle, in document points. The template lays
    /// panels inside it and does not second-guess its size.
    Artboard(Rect),
    /// A page that does not exist yet: the size to create it at. The plan lays panels into a page
    /// whose origin is the document origin, and reports the size back as
    /// [`PagePlan::artboard_size`] so the command can create the artboard.
    NewPage(PageSize),
}

/// What to lay out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanRequest {
    /// The template to lay the page out with.
    pub template: TemplateName,
    /// How many panels the page should have. Must be the template's own count.
    pub panel_count: u32,
    /// The project's reading direction (schema §5.2).
    pub direction: ReadingDirection,
}

impl PlanRequest {
    /// A request for a page of `panel_count` panels.
    #[must_use]
    pub fn new(template: TemplateName, panel_count: u32, direction: ReadingDirection) -> Self {
        Self { template, panel_count, direction }
    }
}

/// A page's composition, before anything is written to the document.
///
/// Returned by [`plan`]. Nothing here is a comic record: the panels are rectangles the command turns
/// into frame objects, and the reading order is the index data the records will hold.
#[derive(Clone, Debug, PartialEq)]
pub struct PagePlan {
    /// The page size the caller should create, when the page does not exist yet — design §8.1's
    /// "the artboard `rect`, when the template sets the page size".
    ///
    /// `None` for a page that already exists: the artboard **is** the page's size (§6.1), and
    /// reporting a size the plan does not own would be a second authority for it.
    pub artboard_size: Option<PageSize>,

    /// The page rectangle the panels were laid out in, in document points.
    pub artboard: Rect,

    /// The page's panels, in **placement order** (top to bottom, left to right within a row), each
    /// with its rectangle and its position in the reading order.
    pub panels: Vec<PlannedPanel>,

    /// The margin and gutter the plan was built with, so a caller validates against the same policy
    /// it was laid out with (design §8.1's "gutter dimensions").
    pub settings: LayoutSettings,

    /// The narrative order, as positions into [`PagePlan::panels`] — `Page.reading_order` in the
    /// order the record holds it, and for `rtl` a right-to-left order (schema §5.2, design §8.1).
    pub reading_order: Vec<u16>,

    /// The direction the order was built for.
    pub direction: ReadingDirection,
}

impl PagePlan {
    /// The gutter the template left between adjacent panels, in points.
    #[must_use]
    pub fn gutter(&self) -> f64 {
        self.settings.gutter()
    }

    /// The safe margin the panels were laid out inside, in points.
    #[must_use]
    pub fn margin(&self) -> f64 {
        self.settings.margin()
    }
}

/// Plan a page: the panel rectangles, the gutter and the reading order (design §8.1).
///
/// Deterministic — the same page, request and settings give the same plan, every time: no AI, no
/// randomness and no wall-clock time. **It returns a plan and touches no document**: the command
/// creates one frame object per panel and writes `Panel.frame_node_id`, and the rectangle is never
/// stored in a comic record (design §6.1, §8.1).
///
/// # Errors
///
/// [`LayoutError`]: the template cannot be resolved or does not fit the page, the panel count is not
/// the template's, or the page rectangle is not one a layout can be built in.
pub fn plan(page: PageInput, request: &PlanRequest, settings: &LayoutSettings) -> Result<PagePlan, LayoutError> {
    let shape = request.template.shape()?;
    let template_count = shape.panel_count();
    if template_count != request.panel_count {
        return Err(LayoutError::PanelCountMismatch { template: request.template.as_name(), requested: request.panel_count, template_count });
    }
    let (artboard, artboard_size) = match page {
        PageInput::Artboard(rect) => (rect, None),
        PageInput::NewPage(size) => (Rect::new(0.0, 0.0, size.width, size.height), Some(size)),
    };
    checked_rectangle(artboard)?;
    let rows = template::rows(artboard, &shape, settings)?;
    Ok(PagePlan {
        artboard_size,
        artboard,
        panels: panel::place(&rows, request.direction),
        settings: *settings,
        reading_order: reading_order::narrative_order(shape.panels_per_row(), request.direction),
        direction: request.direction,
    })
}

/// Reject a page rectangle a layout cannot be built in.
///
/// [`PanelRect::new`] is this crate's one rectangle invariant — finite, positive, with each far edge
/// beyond its origin — and this reuses it rather than repeating the predicates. The value is **not**
/// kept: the plan stores the page as the model spells it (`geom::Rect`, mirroring
/// `PageGeometry.rect`), because [`PanelRect`] documents that it only ever describes a panel *inside*
/// the page, and using it for the page would make that sentence false.
fn checked_rectangle(page: Rect) -> Result<(), crate::PanelRectError> {
    PanelRect::new(page.x0, page.y0, page.width(), page.height()).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_comic_model::ReadingDirection;
    use vectorcraft_geom::Size;

    /// US Letter, the size the fixture's project defaults to, with the crate's default margin and
    /// gutter.
    fn letter() -> (Rect, LayoutSettings) {
        (Rect::new(0.0, 0.0, 612.0, 792.0), LayoutSettings::default())
    }

    fn three_panels(direction: ReadingDirection) -> PlanRequest {
        PlanRequest::new(TemplateName::ThreeTier, 3, direction)
    }

    fn rects(plan: &PagePlan) -> Vec<(f64, f64, f64, f64)> {
        plan.panels.iter().map(|panel| (panel.rect.x(), panel.rect.y(), panel.rect.width(), panel.rect.height())).collect()
    }

    #[test]
    fn a_three_panel_page_is_planned_with_rectangles_and_a_reading_order() {
        // M1's acceptance test 2 lays out three panels; this is the layout half of it.
        let (page, settings) = letter();
        let plan = plan(PageInput::Artboard(page), &three_panels(ReadingDirection::Ltr), &settings).expect("US Letter fits three tiers");
        assert_eq!(plan.panels.len(), 3);
        assert_eq!(rects(&plan), vec![(18.0, 18.0, 576.0, 244.0), (18.0, 274.0, 576.0, 244.0), (18.0, 530.0, 576.0, 244.0)]);
        assert_eq!(plan.reading_order, vec![0, 1, 2]);
        assert_eq!(plan.panels.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(plan.gutter(), 12.0);
        assert_eq!(plan.margin(), 18.0);
        assert_eq!(plan.artboard, page);
        assert_eq!(plan.artboard_size, None, "an existing artboard is the page's size, not the plan's to report");
    }

    #[test]
    fn a_new_page_reports_the_size_the_caller_should_create() {
        let (_, settings) = letter();
        let plan =
            plan(PageInput::NewPage(Size::new(612.0, 792.0)), &three_panels(ReadingDirection::Ltr), &settings).expect("a page of that size fits");
        assert_eq!(plan.artboard_size, Some(Size::new(612.0, 792.0)));
        assert_eq!(plan.artboard, Rect::new(0.0, 0.0, 612.0, 792.0), "a new page is created at the document origin");
        assert_eq!(rects(&plan)[0], (18.0, 18.0, 576.0, 244.0));
    }

    #[test]
    fn a_right_to_left_page_plans_a_right_to_left_order() {
        // `wide-top`: one panel over two. Reading right to left, the second panel of the bottom row
        // is read before the first, and the order says so while the rectangles do not move.
        let (page, settings) = letter();
        let request = PlanRequest::new(TemplateName::WideTop, 3, ReadingDirection::Rtl);
        let ltr = plan(PageInput::Artboard(page), &PlanRequest { direction: ReadingDirection::Ltr, ..request }, &settings)
            .expect("US Letter fits a wide top");
        let rtl = plan(PageInput::Artboard(page), &request, &settings).expect("US Letter fits a wide top");
        assert_eq!(ltr.reading_order, vec![0, 1, 2]);
        assert_eq!(rtl.reading_order, vec![0, 2, 1]);
        assert_eq!(rtl.panels.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![0, 2, 1]);
        assert_eq!(rects(&ltr), rects(&rtl), "the direction orders the page; it does not move it");
    }

    #[test]
    fn a_panel_count_the_template_does_not_place_is_refused() {
        let (page, settings) = letter();
        for requested in [0, 2, 4] {
            let request = PlanRequest::new(TemplateName::ThreeTier, requested, ReadingDirection::Ltr);
            assert_eq!(
                plan(PageInput::Artboard(page), &request, &settings),
                Err(LayoutError::PanelCountMismatch { template: "three-tier".to_string(), requested, template_count: 3 })
            );
        }
    }

    #[test]
    fn an_unknown_template_name_is_refused_before_anything_is_planned() {
        // The name comes from `ProjectSettings.default_template`, a free-form string (schema §5.3).
        let error = TemplateName::from_name("three-tiers").expect_err("no template answers to that name");
        assert!(matches!(error, crate::template::error::TemplateError::UnknownTemplate { .. }));
        assert!(TemplateName::from_name("three-tier").is_ok());
    }

    #[test]
    fn a_page_rectangle_a_layout_cannot_be_built_in_is_refused() {
        let settings = LayoutSettings::default();
        for page in [
            Rect::new(0.0, 0.0, f64::NAN, 792.0),
            Rect::new(0.0, 0.0, 612.0, f64::INFINITY),
            Rect::new(0.0, 0.0, 0.0, 792.0),
            Rect::new(0.0, 0.0, -612.0, 792.0),
        ] {
            let error = plan(PageInput::Artboard(page), &three_panels(ReadingDirection::Ltr), &settings).expect_err("that page cannot be laid out");
            assert!(matches!(error, LayoutError::Artboard(_)), "{page:?} gave {error:?}");
        }
        // A page that is merely small is not refused here: it fails as "no room", with a message a
        // creator can act on, rather than as a malformed rectangle.
        let tiny = plan(PageInput::Artboard(Rect::new(0.0, 0.0, 30.0, 30.0)), &three_panels(ReadingDirection::Ltr), &settings);
        assert!(matches!(tiny, Err(LayoutError::Template(crate::template::error::TemplateError::NoContentArea { .. }))));
    }

    #[test]
    fn a_grid_plans_its_own_panel_count() {
        let (page, settings) = letter();
        let request = PlanRequest::new(TemplateName::Grid { rows: 2, columns: 3 }, 6, ReadingDirection::Ltr);
        let plan = plan(PageInput::Artboard(page), &request, &settings).expect("US Letter fits a 2x3 grid");
        assert_eq!(plan.panels.len(), 6);
        assert_eq!(plan.reading_order, vec![0, 1, 2, 3, 4, 5]);
    }
}
