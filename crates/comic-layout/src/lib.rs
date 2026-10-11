//! ComicCraft layout: deterministic page and panel composition, lettering fitting, validation
//! and publishing assembly.
//!
//! **This crate computes and returns a plan. It never takes `&mut Document`.** The engine's
//! comic command modules apply the plan to the visual document, which is what makes a layout
//! result one undoable operation (design §8, §11.2) and keeps this crate testable headlessly,
//! with plain rectangles and no document at all.
//!
//! It is deterministic: no AI, no randomness and no wall-clock time, so its tests are
//! reproducible. AI may *propose* a layout, but the validator here still checks overlap,
//! gutter width, minimum panel size and containment before anything is written (design §8.1).
//!
//! Three module trees live here and have separate owners (see
//! `.github/instructions/comiccraft-work.instructions.md`):
//!
//! | Tree | Owner | Contents |
//! |---|---|---|
//! | `layout`, `panel`, `template`, `reading_order`, `validate` | `layout-engineer` | templates, gutters, panel geometry, reading order, checks |
//! | `lettering`, `fit` | `lettering-engineer` | balloons, tails, text fitting and the overflow policy |
//! | `publish`, `export` | `publishing-engineer` | screen/print/proof presets, the export gate, the package archive |
#![forbid(unsafe_code)]

pub mod fit;
pub mod lettering;

pub mod layout;
pub mod panel;
pub mod reading_order;
pub mod template;
pub mod validate;

use serde::Serialize;

/// Why a [`PanelRect`] could not be built.
///
/// Distinct variants on purpose: design §12.1 requires an actionable message, and a caller
/// needs to tell a **caller** bug (a negative width from a bad template) from a **data** bug
/// (a NaN read from a file). A single `None` cannot say which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelRectError {
    /// A coordinate or size was NaN or infinite.
    NotFinite { field: &'static str },
    /// A width or height was zero or negative.
    NonPositive { field: &'static str },
    /// A width or height is too small to change the far edge at this coordinate, so the
    /// rectangle would have zero area in `f64` arithmetic.
    ///
    /// Rejected because such a rectangle fails to overlap **itself**, which would make every
    /// predicate built on it — the layout validator's overlap check above all — unsound.
    NotRepresentable { field: &'static str },
}

impl std::fmt::Display for PanelRectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite { field } => write!(f, "panel {field} is not a finite number"),
            Self::NonPositive { field } => write!(f, "panel {field} must be greater than zero"),
            Self::NotRepresentable { field } => {
                write!(f, "panel {field} is too small to change the rectangle's extent at this coordinate")
            }
        }
    }
}

impl std::error::Error for PanelRectError {}

/// A panel frame rectangle, in **document points**.
///
/// Points, not pixels, because it mirrors `Artboard.rect` and every other length in the
/// document model. The page's size is the artboard's; this type only ever describes a panel
/// *inside* it, and it is never stored in a comic record — the document object owns the real
/// geometry (design §6.1).
///
/// # The invariant is enforced, not documented
///
/// Every `PanelRect` has four **finite, strictly positive** dimensions, and a positive
/// representable area: each far edge is strictly beyond its origin in `f64` arithmetic, so a
/// rectangle always overlaps itself. The fields are private and the only constructor is
/// [`PanelRect::new`], so the invariant cannot be bypassed by a struct literal or by the wire.
/// `Deserialize` is deliberately **not** implemented: `PanelRect` is a layout primitive, not a
/// persisted type — schema §6.2 gives `Panel` no geometry because the document owns it — so
/// there is no wire path that needs to build one, and an unvalidated one would be a defect.
/// `Serialize` is safe: it cannot construct an invalid value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PanelRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl PanelRect {
    /// A rectangle from its corner and size.
    ///
    /// Rejects a non-finite coordinate or size, and a width or height that is zero or
    /// negative, rather than silently normalizing it — either means the caller has a bug
    /// worth surfacing. Also rejects a size too small to advance the far edge at this
    /// coordinate, which would give the rectangle zero area in `f64` arithmetic.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, PanelRectError> {
        for (value, field) in [(x, "x"), (y, "y"), (width, "width"), (height, "height")] {
            if !value.is_finite() {
                return Err(PanelRectError::NotFinite { field });
            }
        }
        if width <= 0.0 {
            return Err(PanelRectError::NonPositive { field: "width" });
        }
        if height <= 0.0 {
            return Err(PanelRectError::NonPositive { field: "height" });
        }
        // Guard the arithmetic the predicates are built on: at a large enough coordinate,
        // `x + width == x`, and a rectangle that cannot advance its far edge fails to
        // overlap itself — which would make the validator's overlap check unsound.
        if x + width <= x {
            return Err(PanelRectError::NotRepresentable { field: "width" });
        }
        if y + height <= y {
            return Err(PanelRectError::NotRepresentable { field: "height" });
        }
        Ok(Self { x, y, width, height })
    }

    /// The left edge.
    #[must_use]
    pub fn x(&self) -> f64 {
        self.x
    }

    /// The top edge.
    #[must_use]
    pub fn y(&self) -> f64 {
        self.y
    }

    /// The width, always greater than zero.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.width
    }

    /// The height, always greater than zero.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.height
    }

    /// The right edge.
    #[must_use]
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Whether this rectangle and `other` share any area.
    ///
    /// Rectangles that merely touch along an edge do **not** overlap: two panels sharing a
    /// gutter boundary is normal, and reporting it would make the validator useless. Both
    /// axes are compared strictly, so the check is symmetric and correct whichever way the
    /// rectangles are stacked.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }

    /// Whether `other` lies entirely within this rectangle, touching edges allowed.
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.bottom() <= self.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> PanelRect {
        PanelRect::new(x, y, w, h).expect("valid test rectangle")
    }

    #[test]
    fn new_reports_which_field_is_wrong() {
        // A caller must be able to tell a caller bug from a data bug (design §12.1).
        assert_eq!(PanelRect::new(0.0, 0.0, -1.0, 10.0), Err(PanelRectError::NonPositive { field: "width" }));
        assert_eq!(PanelRect::new(0.0, 0.0, 10.0, 0.0), Err(PanelRectError::NonPositive { field: "height" }));
        assert_eq!(PanelRect::new(f64::NAN, 0.0, 10.0, 10.0), Err(PanelRectError::NotFinite { field: "x" }));
        assert_eq!(PanelRect::new(0.0, 0.0, 10.0, f64::INFINITY), Err(PanelRectError::NotFinite { field: "height" }));
    }

    #[test]
    fn a_size_too_small_to_advance_the_far_edge_is_rejected() {
        // At this magnitude `1.0` is below the ULP, so `x + 1.0 == x`: the rectangle would
        // have zero area and would fail to overlap itself.
        let huge = f64::MAX / 4.0;
        assert_eq!(PanelRect::new(huge, 0.0, 1.0, 10.0), Err(PanelRectError::NotRepresentable { field: "width" }));
        assert_eq!(PanelRect::new(0.0, huge, 10.0, 1.0), Err(PanelRectError::NotRepresentable { field: "height" }));
        assert_eq!(PanelRect::new(-huge, 0.0, 1.0, 10.0), Err(PanelRectError::NotRepresentable { field: "width" }));
        // A size at or above the ULP is fine, so the guard is about representability, not size.
        assert!(PanelRect::new(huge, 0.0, huge, 10.0).is_ok());
    }

    #[test]
    fn rejects_degenerate_and_non_finite_rectangles() {
        for (x, y, w, h) in [
            (0.0, 0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0, 10.0),
            (0.0, 0.0, 10.0, 0.0),
            (0.0, 0.0, -0.0, 10.0),
            (0.0, 0.0, 10.0, -1.0),
            (f64::NAN, 0.0, 10.0, 10.0),
            (0.0, f64::NAN, 10.0, 10.0),
            (0.0, 0.0, f64::NAN, 10.0),
            (0.0, 0.0, 10.0, f64::NAN),
            (f64::INFINITY, 0.0, 10.0, 10.0),
            (0.0, 0.0, f64::NEG_INFINITY, 10.0),
        ] {
            assert!(PanelRect::new(x, y, w, h).is_err(), "({x}, {y}, {w}, {h}) must be rejected");
        }
        // The smallest legal rectangle is accepted: the rejection is on the sign, not the size.
        assert!(PanelRect::new(0.0, 0.0, f64::MIN_POSITIVE, f64::MIN_POSITIVE).is_ok());
    }

    #[test]
    fn touching_edges_do_not_overlap_on_either_axis() {
        // Both axes matter. A `<`/`<=` slip on the y axis alone would leave every
        // *vertically* stacked pair reported as overlapping, so testing only side-by-side
        // rectangles is not enough.
        let a = rect(0.0, 0.0, 10.0, 10.0);
        // Shared vertical edge.
        let beside = rect(10.0, 0.0, 10.0, 10.0);
        assert!(!a.overlaps(&beside));
        assert!(!beside.overlaps(&a));
        // Shared horizontal edge — the case a single-axis test misses.
        let below = rect(0.0, 10.0, 10.0, 10.0);
        assert!(!a.overlaps(&below), "a shared horizontal edge is a gutter, not an overlap");
        assert!(!below.overlaps(&a));
        // Shared corner.
        assert!(!a.overlaps(&rect(10.0, 10.0, 10.0, 10.0)));
    }

    #[test]
    fn the_overlap_boundary_sits_strictly_between_touching_and_shared_area() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        assert!(a.overlaps(&rect(9.0, 0.0, 10.0, 10.0)), "a one-unit horizontal overlap is an overlap");
        assert!(a.overlaps(&rect(0.0, 9.0, 10.0, 10.0)), "a one-unit vertical overlap is an overlap");
        assert!(!a.overlaps(&rect(10.0, 0.0, 10.0, 10.0)));
        assert!(!a.overlaps(&rect(0.0, 10.0, 10.0, 10.0)));
    }

    #[test]
    fn genuine_overlap_is_reported_in_both_directions() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(5.0, 5.0, 10.0, 10.0);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn containment_is_not_overlap() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let panel = rect(10.0, 10.0, 20.0, 20.0);
        assert!(page.contains(&panel));
        assert!(!panel.contains(&page));
        assert!(page.overlaps(&panel), "a contained rectangle does share area");
    }

    #[test]
    fn containment_fails_when_straddling_any_single_edge() {
        // All four edges, not just the right one.
        let page = rect(0.0, 0.0, 100.0, 100.0);
        assert!(page.contains(&rect(0.0, 0.0, 100.0, 100.0)), "flush on every edge is contained");
        assert!(!page.contains(&rect(-1.0, 0.0, 10.0, 10.0)), "straddling the left edge");
        assert!(!page.contains(&rect(0.0, -1.0, 10.0, 10.0)), "straddling the top edge");
        assert!(!page.contains(&rect(0.0, 0.0, 101.0, 10.0)), "straddling the right edge");
        assert!(!page.contains(&rect(0.0, 0.0, 10.0, 101.0)), "straddling the bottom edge");
    }

    #[test]
    fn arithmetic_holds_at_a_negative_origin() {
        // Document points may be negative; no comparison may assume a non-negative origin.
        let page = rect(-50.0, -50.0, 100.0, 100.0);
        assert_eq!(page.right(), 50.0);
        assert_eq!(page.bottom(), 50.0);
        assert!(page.contains(&rect(-50.0, -50.0, 100.0, 100.0)));
        assert!(!page.contains(&rect(-60.0, -10.0, 20.0, 20.0)));
        let sub_unit = rect(0.0, 0.0, 0.5, 0.25);
        assert_eq!(sub_unit.right(), 0.5, "sub-unit sizes must not round away");
        assert_eq!(sub_unit.bottom(), 0.25);
    }
}
