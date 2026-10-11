//! Page templates: the deterministic shape rules that turn a page and a panel count into panel
//! rectangles (design §8.1).
//!
//! A template is **a function of the page rectangle and the layout settings**, so it is testable
//! with plain rectangles and no document at all. It returns rectangles; it never creates the frame
//! objects, and it never stores a rectangle anywhere (design §6.1, §8.1).
//!
//! M1's set is the deterministic grids. Every one of them is a **row structure** — how many panels
//! sit in each row, top to bottom — and a row divides the content width equally among its panels.
//! That is all the geometry the templates need; a shape that wanted a wide panel beside a narrow
//! one would need per-panel weights, and none of these has one.

pub mod error;

use vectorcraft_geom::Rect;

use crate::PanelRect;
use crate::template::error::TemplateError;

/// The largest grid side a `grid-<rows>x<columns>` template may name.
///
/// See [`TemplateError::UnusableGrid`] for why the bound exists. 64 is past any comic page — a
/// 64x64 grid is 4096 panels — and small enough that the shape is a `Vec` worth allocating.
pub const MAX_GRID_SIDE: u32 = 64;

/// The margin a page is laid out with when the caller names none, in points.
///
/// **Policy, not schema**: the frozen schema has no margin or gutter field (schema §5.3), so this
/// is the crate's default and a project may replace it. 18pt (a quarter inch) is a printable
/// margin on US Letter.
pub const DEFAULT_MARGIN: f64 = 18.0;

/// The gutter a page is laid out with when the caller names none, in points.
///
/// Policy, like [`DEFAULT_MARGIN`]: 12pt reads as two panels at print size.
pub const DEFAULT_GUTTER: f64 = 12.0;

/// The templates this crate implements, by name.
///
/// `ProjectSettings.default_template` is a free-form string (schema §5.3), so a name is a string in
/// the project and a type here: [`TemplateName::from_name`] resolves it and [`TemplateName::as_name`]
/// writes it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateName {
    /// Two tiers, one panel each: `two-tier`.
    TwoTier,
    /// Three tiers, one panel each: `three-tier` — M1's three-panel page.
    ThreeTier,
    /// Four tiers, one panel each: `four-tier`.
    FourTier,
    /// A wide panel across the top and two below it: `wide-top`.
    WideTop,
    /// A uniform `rows x columns` grid: `grid-<rows>x<columns>`.
    Grid {
        /// How many rows.
        rows: u32,
        /// How many panels each row holds.
        columns: u32,
    },
}

impl TemplateName {
    /// Every fixed-shape template.
    ///
    /// `Grid` is absent because it is parameterised. A test walks this list, so a template cannot
    /// be added without its shape being checked.
    pub const FIXED: [Self; 4] = [Self::TwoTier, Self::ThreeTier, Self::FourTier, Self::WideTop];

    /// The template's name, as `ProjectSettings.default_template` spells it (schema §5.3).
    #[must_use]
    pub fn as_name(self) -> String {
        match self {
            Self::TwoTier => "two-tier".to_string(),
            Self::ThreeTier => "three-tier".to_string(),
            Self::FourTier => "four-tier".to_string(),
            Self::WideTop => "wide-top".to_string(),
            Self::Grid { rows, columns } => format!("grid-{rows}x{columns}"),
        }
    }

    /// Resolve a name.
    ///
    /// `_` is read as `-`, surrounding whitespace is trimmed and the match is case-insensitive,
    /// because the schema stores a free-form string: a bundle written by another tool may spell
    /// `three_tier`, and refusing it would be a format rule the schema does not have.
    pub fn from_name(name: &str) -> Result<Self, TemplateError> {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "two-tier" => Ok(Self::TwoTier),
            "three-tier" => Ok(Self::ThreeTier),
            "four-tier" => Ok(Self::FourTier),
            "wide-top" => Ok(Self::WideTop),
            _ => Self::parse_grid(&normalized, name),
        }
    }

    /// The `grid-<rows>x<columns>` half of [`TemplateName::from_name`].
    fn parse_grid(normalized: &str, original: &str) -> Result<Self, TemplateError> {
        let unknown = || TemplateError::UnknownTemplate { name: original.to_string() };
        let sides = normalized.strip_prefix("grid-").ok_or_else(unknown)?;
        let (rows, columns) = sides.split_once('x').ok_or_else(unknown)?;
        let (rows, columns) = match (rows.parse::<u32>(), columns.parse::<u32>()) {
            (Ok(rows), Ok(columns)) => (rows, columns),
            _ => return Err(unknown()),
        };
        Self::grid(rows, columns)
    }

    /// A grid template, if its sides are usable.
    fn grid(rows: u32, columns: u32) -> Result<Self, TemplateError> {
        if rows == 0 || columns == 0 || rows > MAX_GRID_SIDE || columns > MAX_GRID_SIDE {
            return Err(TemplateError::UnusableGrid { rows, columns, max: MAX_GRID_SIDE });
        }
        Ok(Self::Grid { rows, columns })
    }

    /// The template's row structure.
    ///
    /// Fallible because `Grid` is a public variant a caller can build with any two `u32`s: the
    /// bound is enforced here as well as in [`TemplateName::from_name`], so a hand-built
    /// `Grid { rows: u32::MAX, .. }` is refused rather than allocated for.
    pub fn shape(self) -> Result<Shape, TemplateError> {
        match self {
            Self::TwoTier => Ok(Shape { panels_per_row: vec![1, 1] }),
            Self::ThreeTier => Ok(Shape { panels_per_row: vec![1, 1, 1] }),
            Self::FourTier => Ok(Shape { panels_per_row: vec![1, 1, 1, 1] }),
            Self::WideTop => Ok(Shape { panels_per_row: vec![1, 2] }),
            Self::Grid { rows, columns } => {
                Self::grid(rows, columns)?;
                Ok(Shape { panels_per_row: vec![columns; rows as usize] })
            }
        }
    }
}

/// A template's shape: **how many panels sit in each row**, top to bottom.
///
/// A row divides the content width equally among its panels, with one gutter between each pair;
/// rows are equal in height, separated by the same gutter. The fields are private and every
/// construction site is in [`TemplateName::shape`], so "at least one row, and every row at least
/// one panel" holds by construction — `every_template_shape_is_well_formed` checks that for every
/// name, so a new template cannot quietly divide by zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    panels_per_row: Vec<u32>,
}

impl Shape {
    /// How many panels sit in each row, top to bottom.
    #[must_use]
    pub fn panels_per_row(&self) -> &[u32] {
        &self.panels_per_row
    }

    /// How many rows the shape has.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.panels_per_row.len()
    }

    /// How many panels the shape places.
    #[must_use]
    pub fn panel_count(&self) -> u32 {
        self.panels_per_row.iter().sum()
    }
}

/// The margin and gutter a template lays panels out with, in document points (design §8.1).
///
/// The fields are private and the only constructor is [`LayoutSettings::new`], for the same reason
/// [`PanelRect`] hides its own: a `NaN` margin would produce `NaN` rectangles that fail every later
/// comparison silently, and a negative gutter would produce panels that overlap. [`Default`] is
/// [`DEFAULT_MARGIN`] and [`DEFAULT_GUTTER`] — this crate's policy, not a schema value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSettings {
    margin: f64,
    gutter: f64,
}

impl LayoutSettings {
    /// Settings from a margin and a gutter, in points.
    pub fn new(margin: f64, gutter: f64) -> Result<Self, TemplateError> {
        for (value, field) in [(margin, "margin"), (gutter, "gutter")] {
            if !value.is_finite() || value < 0.0 {
                return Err(TemplateError::InvalidSetting { field, value });
            }
        }
        Ok(Self { margin, gutter })
    }

    /// The margin kept between the page edge and the panel area, in points.
    #[must_use]
    pub fn margin(&self) -> f64 {
        self.margin
    }

    /// The gap left between adjacent panels, in points.
    #[must_use]
    pub fn gutter(&self) -> f64 {
        self.gutter
    }
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self { margin: DEFAULT_MARGIN, gutter: DEFAULT_GUTTER }
    }
}

/// The content box: the page inset by the margin on every side.
///
/// Returned as the model's own rectangle spelling — `geom::Rect`, which is what
/// `PageGeometry.rect` and `ArtboardFacts.rect` are — because a page rectangle is **not** a panel
/// rectangle: [`PanelRect`] documents that it only ever describes a panel inside the page, and
/// using it for the page would make that sentence false.
pub fn content_box(page: Rect, settings: &LayoutSettings) -> Result<Rect, TemplateError> {
    let margin = settings.margin();
    let (width, height) = (page.width() - 2.0 * margin, page.height() - 2.0 * margin);
    // `NaN` is checked explicitly, because `NaN <= 0.0` is false and a `NaN` content area would
    // otherwise reach the rectangle arithmetic.
    if width <= 0.0 || height <= 0.0 || width.is_nan() || height.is_nan() {
        return Err(TemplateError::NoContentArea { margin, width: page.width(), height: page.height() });
    }
    Ok(Rect::new(page.x0 + margin, page.y0 + margin, page.x1 - margin, page.y1 - margin))
}

/// Lay a shape into a page: the panel rectangles, grouped by row.
///
/// Placement order is top to bottom, and left to right within a row — the order the panels are
/// **drawn** in, which is not the reading order: `rtl` reads a row right to left, and that is
/// [`crate::reading_order`]'s business, not a coordinate comparison made here (design §8.1).
///
/// The rectangles are in **document points**, offset by the page's own origin, so a caller can hand
/// one straight to the command that creates the frame object. They are the only copy: nothing here
/// stores a rectangle in a comic record (design §6.1).
///
/// # The last row and the last panel reach the content box, to within one ULP
///
/// Rows are nominally equal in height, and a row's panels are nominally equal in width — but the
/// **last** row takes the remainder to the content box's far edge (`content.y1 - row_top`) and the
/// **last** panel in a row takes the remainder to its right edge (`content.x1 - panel_left`). The rows
/// and the panels are then placed from the previous edge (`next = this_far_edge + gutter`) rather than
/// by multiplying an index by a rounded step, so the gap between two rows is the gutter itself.
///
/// That is as far as `f64` goes, and the guarantee is stated at exactly that strength:
///
/// * the far edges are the content box's **to within one ULP of the page's coordinate magnitude** —
///   the scale of the numbers the arithmetic started from. The bound is **reachable**, not slack: a
///   `grid-1x3` on a 612x1999.9pt page anchored at `(0.1, 0.3)` with a margin of 49.9% of the page
///   lands **exactly one ULP** past its content box, and 0 of 294 420 other far edges exceed it
///   (`the_one_ulp_guarantee_is_attained_and_never_exceeded`). The figure this sentence used to give —
///   **0.256 ULP** over 47 922 probed pages — was a *sample*, not a bound, and came from a narrower
///   sweep; the wider one is the one that attains the bound. The width axis and the templates the wide
///   sweep leaves out are still covered by `the_last_panel_of_every_row_reaches_the_content_box`;
/// * they are **exact** when the page's rectangle starts at the document origin, which is where a new
///   page is created — measured **0 inexact of 978** origin-anchored pages, and pinned by
///   `the_last_row_of_every_swept_page_height_lands_exactly_on_the_content_box` (70 005 heights).
///
/// **Why not exact everywhere.** `a + (b - a) == b` is not an identity in `f64`: the remainder `b - a`
/// is rounded, and adding it back rounds again. When the page is anchored at the origin the operands
/// are small multiples of the same ULP and both steps come out exact — which is why that case is worth
/// pinning — but a page at `Rect::new(0.1, 0.3, 50.6, 33.6)` lands one ULP past its edge, and no
/// arrangement of `(position, size)` avoids it, because a `PanelRect` *is* a position and a size.
/// A single row with no accumulation is enough to falsify it, so this is a property of the
/// representation and not of the placement. A reader who re-derives this the hard way has been warned;
/// the alternative was to promise what the type cannot deliver, which round 1 caught this code doing.
pub fn rows(page: Rect, shape: &Shape, settings: &LayoutSettings) -> Result<Vec<Vec<PanelRect>>, TemplateError> {
    let content = content_box(page, settings)?;
    let gutter = settings.gutter();
    let row_count = u32::try_from(shape.rows()).unwrap_or(u32::MAX);
    let nominal_row_height = (content.height() - gutter * f64::from(row_count.saturating_sub(1))) / f64::from(row_count);
    if nominal_row_height <= 0.0 || nominal_row_height.is_nan() {
        return Err(TemplateError::NoRoomDown { rows: row_count, available: content.height(), gutter });
    }
    let mut laid = Vec::with_capacity(shape.rows());
    let mut top = content.y0;
    for (row, panels) in shape.panels_per_row().iter().enumerate() {
        let nominal_panel_width = (content.width() - gutter * f64::from(panels.saturating_sub(1))) / f64::from(*panels);
        if nominal_panel_width <= 0.0 || nominal_panel_width.is_nan() {
            return Err(TemplateError::NoRoomAcross { panels: *panels, available: content.width(), gutter });
        }
        // The last row gets the remainder, so the guard has to measure **that** and not the nominal
        // height: guarding the nominal let a page through which then failed building a rectangle nobody
        // asked for — 64 rows in a 10.05pt content height with a 0.16pt gutter came back as
        // `Rect(NonPositive { height })` instead of `NoRoomDown` (round 2's R3).
        let height = if row + 1 == shape.rows() { content.y1 - top } else { nominal_row_height };
        if height <= 0.0 || height.is_nan() {
            return Err(TemplateError::NoRoomDown { rows: row_count, available: content.height(), gutter });
        }
        let mut placed = Vec::with_capacity(*panels as usize);
        let mut left = content.x0;
        for column in 0..*panels {
            let width = if column + 1 == *panels { content.x1 - left } else { nominal_panel_width };
            if width <= 0.0 || width.is_nan() {
                return Err(TemplateError::NoRoomAcross { panels: *panels, available: content.width(), gutter });
            }
            placed.push(PanelRect::new(left, top, width, height)?);
            left += width + gutter;
        }
        laid.push(placed);
        top += height + gutter;
    }
    Ok(laid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Point;

    /// US Letter, the fixture's page size, with the crate's default margin and gutter.
    fn letter() -> (Rect, LayoutSettings) {
        (Rect::new(0.0, 0.0, 612.0, 792.0), LayoutSettings::default())
    }

    fn shape(name: TemplateName) -> Shape {
        name.shape().expect("a template name has a shape")
    }

    fn corners(rect: &PanelRect) -> (f64, f64, f64, f64) {
        (rect.x(), rect.y(), rect.width(), rect.height())
    }

    #[test]
    fn every_fixed_template_has_the_panel_count_it_is_named_for() {
        assert_eq!(shape(TemplateName::TwoTier).panel_count(), 2);
        assert_eq!(shape(TemplateName::ThreeTier).panel_count(), 3);
        assert_eq!(shape(TemplateName::FourTier).panel_count(), 4);
        assert_eq!(shape(TemplateName::WideTop).panel_count(), 3);
        assert_eq!(shape(TemplateName::Grid { rows: 2, columns: 3 }).panel_count(), 6);
    }

    #[test]
    fn wide_top_is_not_three_tiers() {
        // The two three-panel templates must not be the same shape, or one of the names is a lie.
        assert_eq!(shape(TemplateName::ThreeTier).panels_per_row(), &[1, 1, 1]);
        assert_eq!(shape(TemplateName::WideTop).panels_per_row(), &[1, 2]);
    }

    #[test]
    fn every_template_shape_is_well_formed() {
        // A row of zero panels would divide by zero in `rows`, so the invariant is checked for
        // every name — including the parameterised one, whose bound is what keeps it finite.
        let names = TemplateName::FIXED
            .into_iter()
            .chain([TemplateName::Grid { rows: 1, columns: 1 }, TemplateName::Grid { rows: MAX_GRID_SIDE, columns: MAX_GRID_SIDE }]);
        for name in names {
            let shape = shape(name);
            assert!(shape.rows() >= 1, "{name:?} has no rows");
            assert!(shape.panels_per_row().iter().all(|&panels| panels >= 1), "{name:?} has an empty row");
        }
    }

    #[test]
    fn names_round_trip_and_are_normalized() {
        for name in TemplateName::FIXED.into_iter().chain([TemplateName::Grid { rows: 2, columns: 3 }]) {
            assert_eq!(TemplateName::from_name(&name.as_name()), Ok(name), "{name:?} must round-trip through its name");
        }
        // The schema stores a free-form string (§5.3), so the spellings a project may carry resolve.
        for spelling in ["three_tier", "THREE-TIER", "  three-tier  ", "Three_Tier"] {
            assert_eq!(TemplateName::from_name(spelling), Ok(TemplateName::ThreeTier), "{spelling}");
        }
        assert_eq!(TemplateName::from_name("grid_2x3"), Ok(TemplateName::Grid { rows: 2, columns: 3 }));
    }

    #[test]
    fn a_name_no_template_answers_to_is_refused_rather_than_guessed() {
        // The fixture's default template is a real one; everything else must be reported, because a
        // silent fallback to a default shape would lay out a page the project did not ask for.
        assert_eq!(TemplateName::from_name("three-tier"), Ok(TemplateName::ThreeTier));
        for name in ["", "three", "grid", "grid-", "grid-x", "grid-2", "grid-2x", "grid-2x3x4", "grid-twoxthree", "tiers-3"] {
            assert!(matches!(TemplateName::from_name(name), Err(TemplateError::UnknownTemplate { .. })), "`{name}` must be reported as unknown");
        }
    }

    #[test]
    fn a_grid_side_is_bounded_because_the_name_is_untrusted_input() {
        for (rows, columns) in [(0, 2), (2, 0), (0, 0), (65, 1), (1, 65), (u32::MAX, u32::MAX)] {
            assert_eq!(
                TemplateName::from_name(&format!("grid-{rows}x{columns}")),
                Err(TemplateError::UnusableGrid { rows, columns, max: MAX_GRID_SIDE }),
                "grid-{rows}x{columns} must be refused"
            );
            // And the same bound holds for a caller that builds the variant directly, which is why
            // `shape` is fallible: a hand-built grid must not allocate for `u32::MAX` rows.
            assert!(TemplateName::Grid { rows, columns }.shape().is_err(), "Grid {{ {rows}, {columns} }} must be refused");
        }
        assert_eq!(TemplateName::Grid { rows: MAX_GRID_SIDE, columns: MAX_GRID_SIDE }.shape().map(|s| s.panel_count()), Ok(4096));
    }

    #[test]
    fn three_tiers_lay_three_equal_full_width_panels() {
        let (page, settings) = letter();
        let laid = rows(page, &shape(TemplateName::ThreeTier), &settings).expect("US Letter fits three tiers");
        assert_eq!(laid.len(), 3);
        // Content 576x756; (756 - 2*12) / 3 = 244; each row starts 244 + 12 lower.
        assert_eq!(corners(&laid[0][0]), (18.0, 18.0, 576.0, 244.0));
        assert_eq!(corners(&laid[1][0]), (18.0, 274.0, 576.0, 244.0));
        assert_eq!(corners(&laid[2][0]), (18.0, 530.0, 576.0, 244.0));
        // The last row's bottom edge is the content box's, so nothing drifts by rounding.
        assert_eq!(laid[2][0].bottom(), 774.0);
    }

    #[test]
    fn wide_top_puts_a_full_width_panel_over_two_half_width_ones() {
        let (page, settings) = letter();
        let laid = rows(page, &shape(TemplateName::WideTop), &settings).expect("US Letter fits a wide top");
        assert_eq!(laid.len(), 2);
        assert_eq!(corners(&laid[0][0]), (18.0, 18.0, 576.0, 372.0));
        // (576 - 12) / 2 = 282, and the second panel starts one gutter later.
        assert_eq!(corners(&laid[1][0]), (18.0, 402.0, 282.0, 372.0));
        assert_eq!(corners(&laid[1][1]), (312.0, 402.0, 282.0, 372.0));
    }

    #[test]
    fn a_grid_lays_equal_cells() {
        let (page, settings) = letter();
        let laid = rows(page, &shape(TemplateName::Grid { rows: 2, columns: 2 }), &settings).expect("US Letter fits a 2x2 grid");
        assert_eq!(laid.len(), 2);
        assert_eq!(laid.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2]);
        assert_eq!(corners(&laid[0][0]), (18.0, 18.0, 282.0, 372.0));
        assert_eq!(corners(&laid[0][1]), (312.0, 18.0, 282.0, 372.0));
        assert_eq!(corners(&laid[1][0]), (18.0, 402.0, 282.0, 372.0));
        assert_eq!(corners(&laid[1][1]), (312.0, 402.0, 282.0, 372.0));
    }

    #[test]
    fn a_zero_margin_uses_the_whole_page_and_a_zero_gutter_leaves_no_gap() {
        let (page, _) = letter();
        let tight = LayoutSettings::new(0.0, 0.0).expect("zero is a legal margin and gutter");
        let laid = rows(page, &shape(TemplateName::WideTop), &tight).expect("a full-bleed page still fits");
        assert_eq!(corners(&laid[0][0]), (0.0, 0.0, 612.0, 396.0));
        // Touching, not overlapping: the gutter is zero, so the second panel starts where the
        // first ends. This is the case the validator must call a thin gutter rather than an overlap.
        assert_eq!(corners(&laid[1][0]), (0.0, 396.0, 306.0, 396.0));
        assert_eq!(corners(&laid[1][1]), (306.0, 396.0, 306.0, 396.0));
        assert!(!laid[1][0].overlaps(&laid[1][1]));
    }

    #[test]
    fn panels_are_offset_by_the_pages_own_origin() {
        // A page's artboard is not necessarily at the document origin (design §5.1.1: it is an
        // artboard, at whatever place the document put it), so the rectangles must follow it.
        let page = Rect::new(100.0, 50.0, 712.0, 842.0);
        let settings = LayoutSettings::default();
        let laid = rows(page, &shape(TemplateName::TwoTier), &settings).expect("the page has room");
        assert_eq!(corners(&laid[0][0]), (118.0, 68.0, 576.0, 372.0));
        assert_eq!(corners(&laid[1][0]), (118.0, 452.0, 576.0, 372.0));
        // And the remainder is taken against this page's own edges, wherever it sits.
        let content = content_box(page, &settings).expect("a content box");
        assert_eq!(laid[0][0].right(), content.x1);
        assert_eq!(laid[1][0].bottom(), content.y1);
    }

    #[test]
    fn the_last_row_of_every_swept_page_height_lands_exactly_on_the_content_box() {
        // The same sweep as the reviewer's `the_last_row_of_every_page_height_lands_exactly_on_the_content_box`
        // — every height from 600.0 to 2000.0 in tenths, rows 3/5/6/7/8, US Letter at the default margin
        // and gutter — kept here as well so this crate's own contract is checked without depending on
        // the reviewer's file. It is an **exact** equality: the last row takes the remainder to the
        // content box's far edge, so the edge is the boundary by construction and not by luck of a
        // division that happens to come out even.
        let settings = LayoutSettings::default();
        let mut checked = 0;
        for tenth in 6000..=20000 {
            let height = f64::from(tenth) / 10.0;
            for row_count in [3u32, 5, 6, 7, 8] {
                let page = Rect::new(0.0, 0.0, 612.0, height);
                let content = content_box(page, &settings).expect("a content box");
                let laid = rows(page, &shape(TemplateName::Grid { rows: row_count, columns: 1 }), &settings).expect("the page has room");
                let last = laid.last().and_then(|row| row.last()).expect("a row of a panel");
                assert_eq!(last.bottom(), content.y1, "height {height}, {row_count} rows");
                checked += 1;
            }
        }
        assert!(checked >= 70_000, "the sweep must run: {checked}");
    }

    /// How far apart two coordinates are, in ULP of `scale` — the measure `rows`' guarantee is stated
    /// in, and the one a caller can check without knowing which ULP of the edge is in play.
    fn ulps(a: f64, b: f64, scale: f64) -> f64 {
        if a == b { 0.0 } else { (a - b).abs() / (scale.abs().max(f64::MIN_POSITIVE) * f64::EPSILON) }
    }

    /// The largest absolute coordinate of a page — the magnitude every far-edge bound is relative to.
    fn magnitude(page: Rect) -> f64 {
        page.x0.abs().max(page.y0.abs()).max(page.x1.abs()).max(page.y1.abs())
    }

    #[test]
    fn the_last_panel_of_every_row_reaches_the_content_box() {
        // The axes the height sweep does not cover: the width axis, unequal rows (`wide-top`), other
        // templates, a zero gutter, and pages away from the origin. The far edges are the content box's
        // **exactly** when the page is anchored at the origin and **to within one ULP of the page's
        // coordinate magnitude** elsewhere — `a + (b - a) == b` is not an identity in `f64`, so no
        // arrangement of (position, size) makes the second case exact (round 2's R1).
        let mut checked = 0;
        for height in [500.0, 600.1, 792.0, 964.3, 1000.0, 1234.5, 1999.9] {
            for width in [611.0, 612.3, 595.7, 1000.1] {
                for margin in [0.0, 3.3, 18.0] {
                    for gutter in [0.0, 3.3, 12.0] {
                        let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                        for template in
                            [TemplateName::TwoTier, TemplateName::ThreeTier, TemplateName::WideTop, TemplateName::Grid { rows: 3, columns: 5 }]
                        {
                            for origin in [(0.0, 0.0), (100.0, 50.0), (0.1, 0.3), (-1000.5, -2000.25), (1e7, 1e7), (1e8, 1e8)] {
                                let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                                let content = content_box(page, &settings).expect("a content box");
                                let laid = rows(page, &shape(template), &settings).expect("the page has room");
                                let scale = magnitude(page);
                                let anchored = origin == (0.0, 0.0);
                                let case = format!("{template:?} {width}x{height}pt at {origin:?}, margin {margin}, gutter {gutter}");
                                for (row, panels) in laid.iter().enumerate() {
                                    let last = panels.last().expect("a row holds at least one panel");
                                    let off = ulps(last.right(), content.x1, scale);
                                    assert!(off <= 1.0, "{case}, row {row}: right edge is {off} ULP off");
                                    if anchored {
                                        assert_eq!(off, 0.0, "{case}, row {row}: an origin-anchored page must be exact");
                                    }
                                    if row + 1 == laid.len() {
                                        let off = ulps(last.bottom(), content.y1, scale);
                                        assert!(off <= 1.0, "{case}, last row: bottom edge is {off} ULP off");
                                        if anchored {
                                            assert_eq!(off, 0.0, "{case}, last row: an origin-anchored page must be exact");
                                        }
                                    }
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked >= 6000, "the sweep must run: {checked}");
    }

    #[test]
    fn a_page_away_from_the_origin_reaches_its_content_box_to_within_one_ulp() {
        // The two cases round 2's R1 named. Both are the same fact — `a + (b - a) == b` is not an f64
        // identity — and neither is a defect to chase: the guarantee is one ULP, and it is stated at
        // that strength rather than promised exact.
        let settings = LayoutSettings::new(3.3, 12.0).expect("legal settings");
        let page = Rect::new(0.1, 0.3, 50.6, 33.6);
        let content = content_box(page, &settings).expect("a content box");
        // A **single row**, so nothing accumulates: the identity itself is what fails.
        let laid = rows(page, &shape(TemplateName::Grid { rows: 1, columns: 3 }), &settings).expect("the page has room");
        let bottom = laid[0][0].bottom();
        assert_ne!(bottom, content.y1, "this is the case that falsifies the identity, so it must not be exact");
        // One ULP of the edge, as R1 measured: the edge is in [16, 32), so its ULP is `EPSILON * 16`.
        assert_eq!(bottom - content.y1, f64::EPSILON * 16.0, "one ULP past the edge");
        assert!(ulps(bottom, content.y1, magnitude(page)) <= 1.0, "and within one ULP of the page's magnitude");

        // A negative origin with no gutter: the right edges are one ULP past `content.x1`.
        let tight = LayoutSettings::new(0.0, 0.0).expect("legal settings");
        let page = Rect::new(-1000.5, -2000.25, -388.5, -1208.25);
        let content = content_box(page, &tight).expect("a content box");
        let laid = rows(page, &shape(TemplateName::TwoTier), &tight).expect("the page has room");
        for (row, panels) in laid.iter().enumerate() {
            let off = ulps(panels[0].right(), content.x1, magnitude(page));
            assert!(off <= 1.0, "row {row}: right edge is {off} ULP off");
        }
    }

    #[test]
    fn a_zero_gutter_row_has_its_panels_meet_exactly() {
        // The column axis of the same guarantee, and the case round 2's M-C survives: a mutation that
        // multiplies the column index by a rounded step leaves every *far* edge exact and the first
        // panel where it was, so only the edges **between** panels tell it apart. Placing each panel
        // from the previous one's right edge makes a zero gutter mean exactly zero.
        let settings = LayoutSettings::new(3.3, 0.0).expect("legal settings");
        for width in [611.0, 612.3, 595.7, 1000.1, 612.0] {
            for columns in [2u32, 3, 4, 5, 7, 9] {
                let page = Rect::new(0.0, 0.0, width, 1000.0);
                let laid = rows(page, &shape(TemplateName::Grid { rows: 3, columns }), &settings).expect("the page has room");
                for (row, panels) in laid.iter().enumerate() {
                    for (column, pair) in panels.windows(2).enumerate() {
                        assert_eq!(
                            pair[1].x(),
                            pair[0].right(),
                            "{width}pt in {columns} columns, row {row}: panel {} must start exactly where panel {column} ends",
                            column + 1
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_page_too_short_for_its_rows_is_refused_as_no_room() {
        // Round 2's R3: the guard measured the *nominal* row height while the last row takes the
        // unguarded remainder, so 64 rows in a 10.05pt content height with a 0.16pt gutter came back as
        // an error about a rectangle nobody built. The guard now measures what the last row gets.
        let settings = LayoutSettings::new(0.0, 0.15952380952380796).expect("a legal gutter");
        let refused = rows(Rect::new(0.0, 0.0, 100.0, 10.05), &shape(TemplateName::Grid { rows: 64, columns: 1 }), &settings);
        assert!(
            matches!(refused, Err(TemplateError::NoRoomDown { rows: 64, .. })),
            "the page has no room, and that is what must be reported: {refused:?}"
        );
    }

    #[test]
    fn a_margin_that_consumes_the_page_is_refused() {
        let page = Rect::new(0.0, 0.0, 30.0, 30.0);
        let settings = LayoutSettings::new(18.0, 2.0).expect("18pt is a legal margin");
        assert_eq!(
            rows(page, &shape(TemplateName::TwoTier), &settings),
            Err(TemplateError::NoContentArea { margin: 18.0, width: 30.0, height: 30.0 })
        );
        // A page whose rectangle is inverted has no width at all, and is refused the same way
        // rather than producing negative rectangles.
        assert!(content_box(Rect::new(0.0, 0.0, -10.0, 30.0), &settings).is_err());
    }

    #[test]
    fn gutters_that_leave_no_room_are_refused_on_each_axis() {
        let settings = LayoutSettings::new(1.0, 30.0).expect("30pt is a legal gutter");
        // Three panels across a 58pt content width need 60pt of gutters: no room across.
        let across = rows(Rect::new(0.0, 0.0, 60.0, 100.0), &shape(TemplateName::Grid { rows: 1, columns: 3 }), &settings);
        assert_eq!(across, Err(TemplateError::NoRoomAcross { panels: 3, available: 58.0, gutter: 30.0 }));
        // Three rows down a 58pt content height need the same: no room down.
        let down = rows(Rect::new(0.0, 0.0, 100.0, 60.0), &shape(TemplateName::ThreeTier), &settings);
        assert_eq!(down, Err(TemplateError::NoRoomDown { rows: 3, available: 58.0, gutter: 30.0 }));
    }

    #[test]
    fn settings_are_checked_because_a_nan_margin_would_poison_every_rectangle() {
        // The field is checked for the finite cases; `NaN` is matched structurally, because
        // `NaN != NaN` and an error that carries it can never compare equal to itself.
        assert_eq!(LayoutSettings::new(-1.0, 12.0), Err(TemplateError::InvalidSetting { field: "margin", value: -1.0 }));
        assert_eq!(LayoutSettings::new(18.0, -1.0), Err(TemplateError::InvalidSetting { field: "gutter", value: -1.0 }));
        assert!(matches!(LayoutSettings::new(f64::NAN, 12.0), Err(TemplateError::InvalidSetting { field: "margin", .. })));
        assert!(matches!(LayoutSettings::new(18.0, f64::NAN), Err(TemplateError::InvalidSetting { field: "gutter", .. })));
        // Infinity is not a length either.
        assert!(matches!(LayoutSettings::new(f64::INFINITY, 12.0), Err(TemplateError::InvalidSetting { field: "margin", .. })));
    }

    #[test]
    fn the_content_box_is_the_page_inset_by_the_margin() {
        let (page, settings) = letter();
        let content = content_box(page, &settings).expect("US Letter has a content area");
        assert_eq!((content.x0, content.y0, content.x1, content.y1), (18.0, 18.0, 594.0, 774.0));
        assert_eq!((content.width(), content.height()), (576.0, 756.0));
        assert_eq!(content.center(), Point::new(306.0, 396.0));
    }
}
