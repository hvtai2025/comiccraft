//! Sizing a balloon to the text it must hold.
//!
//! The search is a bounded, deterministic walk over candidate boxes, and every candidate is judged
//! by [`crate::fit::measure`] — never by an estimate made here:
//!
//! 1. Lay the text out in the widest frame the kind allows. Its ink width is the text's own width,
//!    so a balloon around `Hi!` is tight and one around a sentence starts wide enough to wrap.
//! 2. Walk the widths upward from that preferred width to the maximum. For each width, the height
//!    the block needs (`Measured::needed_height`) is tried first and the maximum height second, so
//!    the first fit is the smallest box on that width.
//! 3. The first box that holds the text and clears the caller's obstacles wins.
//!
//! Two facts make this sound rather than a heuristic:
//!
//! * For a **fixed width** the layout is anchored at the frame's inner top, so a taller frame holds
//!   strictly more: if the maximum height does not fit, no height does.
//! * A candidate is already rejected by [`Measured::fits`], which reads the lines' spans as well as
//!   `TextLayout::overflow` — so a word too long for the width is caught here and the search widens
//!   instead of reporting a fit the reader can see is wrong.
//!
//! When nothing fits, the widest box the limits allow is returned *with* the outcome, never with a
//! silent clamp: a balloon whose text cannot be set is something a creator must be told about.

use vectorcraft_doc::AreaFit;
use vectorcraft_geom::{Point, Rect, Size};
use vectorcraft_text::FontDb;

use super::{FitError, FitOutcome, FitTextStyle, Measured, first_collision, measure, text_frame};

/// How much wider than the text's own ink width a balloon starts, so a snug fit is not a hair too
/// tight for the engine's line breaking.
pub const WIDTH_SLACK: f64 = 1.03;

/// The factor each candidate width grows by.
pub const GROWTH: f64 = 1.25;

/// The most widths tried, so a text that cannot fit terminates in a bounded number of layouts.
pub const MAX_WIDTH_STEPS: usize = 16;

/// Which edge of a frame stays put when it grows.
///
/// A balloon grows about its centre — it sits over a speaker's head and should stay there. A
/// caption box grows downward from its top centre, because that is the edge a reader reads from and
/// the one the artist placed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResizeAnchor {
    /// The centre stays put; the frame grows in all four directions.
    #[default]
    Centre,
    /// The top edge and the horizontal centre stay put; the frame grows downward.
    TopCentre,
}

impl ResizeAnchor {
    /// The box of `size` whose anchor point is `at`.
    #[must_use]
    pub fn box_at(self, at: Point, size: Size) -> Rect {
        match self {
            Self::Centre => Rect::from_center_size(at, size),
            Self::TopCentre => Rect::new(at.x - size.width / 2.0, at.y, at.x + size.width / 2.0, at.y + size.height),
        }
    }

    /// The point of an existing box that this anchor holds fixed.
    #[must_use]
    pub fn anchor_point(self, bounds: Rect) -> Point {
        match self {
            Self::Centre => bounds.center(),
            Self::TopCentre => Point::new(bounds.center().x, bounds.y0),
        }
    }
}

/// Where a box sits: the anchor point and the edge that stays put when it grows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The anchor point in document points.
    pub at: Point,
    /// Which edge stays put.
    pub anchor: ResizeAnchor,
}

impl Placement {
    /// A placement pointing at `at` with `anchor`.
    #[must_use]
    pub fn new(at: Point, anchor: ResizeAnchor) -> Self {
        Self { at, anchor }
    }

    /// The placement of an existing box, which is what keeps a refit in place.
    #[must_use]
    pub fn of(bounds: Rect, anchor: ResizeAnchor) -> Self {
        Self { at: anchor.anchor_point(bounds), anchor }
    }
}

/// The smallest and largest a lettering object's box may be, in document points.
///
/// The limits are on the **outer box** — the balloon a reader sees — not on the text frame inside
/// it: a creator sizes a balloon, and the room left for text follows from the shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The smallest box this kind of lettering may be.
    pub min: Size,
    /// The largest box this kind of lettering may be.
    pub max: Size,
}

impl Limits {
    /// Limits from a minimum and a maximum, rejecting a degenerate or inverted pair.
    pub fn new(min: Size, max: Size) -> Result<Self, FitError> {
        for (value, field) in
            [(min.width, "minimum width"), (min.height, "minimum height"), (max.width, "maximum width"), (max.height, "maximum height")]
        {
            if !value.is_finite() {
                return Err(FitError::NotFinite { field });
            }
            if value <= 0.0 {
                return Err(FitError::NonPositive { field });
            }
        }
        if min.width > max.width || min.height > max.height {
            return Err(FitError::LimitsInverted);
        }
        Ok(Self { min, max })
    }

    /// These limits narrowed to the room `available` offers, never past the minimum.
    ///
    /// A balloon cannot be wider than the panel it is in; the floor stays the kind's minimum even
    /// when the space is smaller than that, because a box below the minimum is not a balloon this
    /// kind can draw — the caller sees the collision finding instead.
    #[must_use]
    pub fn within(self, available: Size) -> Self {
        // `max`/`min` rather than `clamp`, which panics on an inverted pair; `within` must hold for
        // every `Limits` a caller can build, including one built through the public fields.
        let width = available.width.max(self.min.width).min(self.max.width);
        let height = available.height.max(self.min.height).min(self.max.height);
        Self { min: self.min, max: Size::new(width, height) }
    }

    /// Whether the minimum is inside the maximum in both directions.
    #[must_use]
    pub fn is_ordered(self) -> bool {
        self.min.width <= self.max.width && self.min.height <= self.max.height
    }
}

/// What to size: the text, the type, the shape's room and where the box goes.
#[derive(Clone, Copy, Debug)]
pub struct SizeRequest<'a> {
    /// The words, borrowed — the `Dialogue` record owns them (design §6.1).
    pub text: &'a str,
    /// The style to measure them in.
    pub style: &'a FitTextStyle,
    /// The fraction of the box the shape leaves for text (`BalloonShape::inner_scale`).
    pub inner_scale: f64,
    /// The smallest and largest box allowed.
    pub limits: Limits,
    /// Where the box sits.
    pub place: Placement,
    /// Rectangles the grown box must not overlap — other panels and other lettering (design §8.3).
    pub obstacles: &'a [Rect],
}

/// A sized box and the measurement that justified it.
#[derive(Clone, Debug)]
pub struct Sized {
    /// The outer box: the balloon or caption as the reader sees it.
    pub bounds: Rect,
    /// The text laid out in this box's frame.
    pub measured: Measured,
    /// How sizing resolved. A box that only fitted where it collided reports
    /// [`FitOutcome::Blocked`] with the obstacle's index, so the outcome is the one place a caller
    /// reads the collision from.
    pub outcome: FitOutcome,
}

/// The frame inside `bounds`, or [`FitError::PaddingTooLarge`] when the box cannot hold text at all.
fn frame_of(bounds: Rect, inner_scale: f64, padding: f64) -> Result<Rect, FitError> {
    text_frame(bounds, inner_scale, padding)
}

/// Widths to try, ascending from `start` to `max` in [`GROWTH`] steps.
///
/// Bounded by [`MAX_WIDTH_STEPS`] so a text that fits nowhere terminates instead of walking away
/// from the limits.
fn widths_from(start: f64, max: f64) -> Vec<f64> {
    let mut out = vec![start];
    let mut width = start;
    while width < max && out.len() < MAX_WIDTH_STEPS {
        let next = (width * GROWTH).min(max);
        // A width that cannot advance ends the walk: `width` and `max` are finite, and a NaN here
        // would otherwise loop for ever.
        if !next.is_finite() || next <= width {
            break;
        }
        out.push(next);
        width = next;
    }
    out
}

/// The heights to try at one width, ascending and deduplicated.
///
/// The needed height comes first so the smallest box on that width wins; the maximum height second,
/// because for a fixed width it holds strictly more than any shorter frame.
fn heights_from(needed: f64, min: f64, max: f64) -> Vec<f64> {
    let first = if needed.is_finite() { needed.max(min).min(max) } else { max };
    let mut out = vec![first];
    if max - first > 1e-9 {
        out.push(max);
    }
    if min - first > 1e-9 {
        out.insert(0, min);
    }
    out
}

/// The smallest box inside `request.limits` whose text frame holds `request.text` at the style's own
/// size, at the placement asked for.
///
/// Never returns [`Err`] for text that does not fit: the widest allowed box comes back with
/// [`FitOutcome::Unfittable`] (or [`FitOutcome::Blocked`]) so the caller can report it and the
/// creator still has a balloon to drag.
pub fn size_to_text(db: &FontDb, request: &SizeRequest<'_>) -> Result<Sized, FitError> {
    request.style.validate()?;
    if !request.place.at.x.is_finite() || !request.place.at.y.is_finite() {
        return Err(FitError::NotFinite { field: "placement" });
    }
    if !(request.inner_scale.is_finite() && request.inner_scale > 0.0 && request.inner_scale <= 1.0) {
        return Err(FitError::NonPositive { field: "inner scale" });
    }
    if !request.limits.is_ordered() {
        return Err(FitError::LimitsInverted);
    }
    // Validated through the smallest box, which is the one most likely to leave no room.
    frame_of(request.place.anchor.box_at(request.place.at, request.limits.min), request.inner_scale, request.style.padding)?;

    let biggest = request.place.anchor.box_at(request.place.at, request.limits.max);
    let natural = measure(db, request.text, request.style, &frame_of(biggest, request.inner_scale, request.style.padding)?, AreaFit::None)?;
    let preferred = ((natural.layout.bounds.width() * WIDTH_SLACK + 2.0 * request.style.padding) / request.inner_scale)
        .max(request.limits.min.width)
        .min(request.limits.max.width);

    let mut collision: Option<usize> = None;
    for width in widths_from(preferred, request.limits.max.width) {
        let probe_box = request.place.anchor.box_at(request.place.at, Size::new(width, request.limits.max.height));
        let probe = measure(db, request.text, request.style, &frame_of(probe_box, request.inner_scale, request.style.padding)?, AreaFit::None)?;
        // `needed_height` is the height the *text frame* needs; the candidates are *box* heights, and
        // the frame is the box reduced by the shape's inner scale. Dividing here is what stops a
        // balloon from jumping from its minimum straight to its maximum: a 12 pt line with 7 points
        // of padding needs about 25 points of frame, which is about 36 points of ellipse.
        let needed_box = probe.needed_height() / request.inner_scale;
        for height in heights_from(needed_box, request.limits.min.height, request.limits.max.height) {
            let bounds = request.place.anchor.box_at(request.place.at, Size::new(width, height));
            let measured = measure(db, request.text, request.style, &frame_of(bounds, request.inner_scale, request.style.padding)?, AreaFit::None)?;
            if !measured.fits() {
                continue;
            }
            match first_collision(bounds, request.obstacles) {
                None => return Ok(Sized { bounds, measured, outcome: FitOutcome::Fits { fit_scale: 1.0 } }),
                Some(index) => collision = Some(collision.map_or(index, |first| first.min(index))),
            }
        }
    }

    let measured = measure(db, request.text, request.style, &frame_of(biggest, request.inner_scale, request.style.padding)?, AreaFit::None)?;
    let outcome = match collision {
        Some(obstacle) => FitOutcome::Blocked { obstacle },
        None => FitOutcome::Unfittable,
    };
    Ok(Sized { bounds: biggest, measured, outcome })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Point;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    fn style(size: f64) -> FitTextStyle {
        FitTextStyle::new("Comic Neue", "Regular", size)
    }

    fn request<'a>(text: &'a str, style: &'a FitTextStyle, limits: Limits, obstacles: &'a [Rect]) -> SizeRequest<'a> {
        SizeRequest {
            text,
            style,
            inner_scale: std::f64::consts::FRAC_1_SQRT_2,
            limits,
            place: Placement::new(Point::new(200.0, 150.0), ResizeAnchor::Centre),
            obstacles,
        }
    }

    fn limits(min: (f64, f64), max: (f64, f64)) -> Limits {
        Limits::new(Size::new(min.0, min.1), Size::new(max.0, max.1)).expect("valid limits")
    }

    #[test]
    fn a_balloon_grows_past_its_minimum_to_hold_a_sentence() {
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 44.0), (280.0, 220.0));
        let sized = size_to_text(db, &request("The storm broke over the harbour at dawn, and nobody spoke.", &s, l, &[])).expect("sizable");
        assert_eq!(sized.outcome, FitOutcome::Fits { fit_scale: 1.0 });
        assert!(sized.measured.fits());
        assert!(sized.bounds.width() > l.min.width, "the box grew");
        assert!(sized.bounds.height() > l.min.height, "the box grew");
        assert!(sized.bounds.width() <= l.max.width + 1e-9);
        assert!(sized.bounds.height() <= l.max.height + 1e-9);
        // The frame is inside the box and the box is centred on the placement.
        let frame = text_frame(sized.bounds, std::f64::consts::FRAC_1_SQRT_2, s.padding).expect("frame");
        assert!(
            sized.bounds.x0 <= frame.x0 && sized.bounds.y0 <= frame.y0 && frame.x1 <= sized.bounds.x1 && frame.y1 <= sized.bounds.y1,
            "the text frame is inside the box: {frame:?} in {:?}",
            sized.bounds
        );
        assert!((sized.bounds.center().x - 200.0).abs() < 1e-9, "a centred balloon stays centred");
        assert!((sized.bounds.center().y - 150.0).abs() < 1e-9);
    }

    #[test]
    fn the_box_that_comes_back_is_tight_enough_to_matter() {
        // Not a claim of global minimality — the search is over a grid — but the size that comes
        // back must be one whose 20%-smaller version genuinely does not fit, or the grid would be
        // returning a box a reader would call loose.
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 32.0), (400.0, 300.0));
        let text = "The storm broke over the harbour at dawn.";
        let sized = size_to_text(db, &request(text, &s, l, &[])).expect("sizable");
        assert!(sized.measured.fits());
        let smaller = Rect::from_center_size(sized.bounds.center(), Size::new(sized.bounds.width() * 0.8, sized.bounds.height() * 0.8));
        let frame = text_frame(smaller, std::f64::consts::FRAC_1_SQRT_2, s.padding).expect("frame");
        let shrunk = measure(db, text, &s, &frame, AreaFit::None).expect("measurable");
        assert!(!shrunk.fits(), "a box 20% smaller holds less text, so the search overshot");
    }

    #[test]
    fn an_empty_string_gets_the_smallest_box_the_kind_allows() {
        // An empty dialogue is a balloon a creator has just made, so the answer is the kind's own
        // minimum — the box the style guarantees holds one line, which is the line the caret sits
        // on until words arrive.
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 44.0), (280.0, 220.0));
        let sized = size_to_text(db, &request("", &s, l, &[])).expect("sizable");
        assert_eq!(sized.bounds.width(), l.min.width);
        assert_eq!(sized.bounds.height(), l.min.height);
        assert_eq!(sized.outcome, FitOutcome::Fits { fit_scale: 1.0 });
        assert!(sized.measured.fits());
    }

    #[test]
    fn a_long_word_widens_the_balloon_rather_than_reporting_overflow() {
        // The width search exists for this: the preferred width comes from the text's own ink
        // extent, but a word longer than the frame has to be answered by a wider frame, not by
        // `Measured::fits` failing at every height.
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 32.0), (400.0, 300.0));
        let sized = size_to_text(db, &request("Unforgettable", &s, l, &[])).expect("sizable");
        assert_eq!(sized.outcome, FitOutcome::Fits { fit_scale: 1.0 });
        assert!(sized.measured.fits());
        assert!(sized.bounds.width() > l.min.width);
    }

    #[test]
    fn text_that_cannot_fit_any_box_is_reported_with_the_widest_box() {
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 32.0), (60.0, 40.0));
        let sized = size_to_text(db, &request("Incomprehensibilities", &s, l, &[])).expect("sizable");
        assert_eq!(sized.outcome, FitOutcome::Unfittable);
        assert!(!sized.measured.fits());
        assert_eq!(sized.bounds.width(), l.max.width, "the widest allowed box comes back, not a clamp");
        assert_eq!(sized.bounds.height(), l.max.height);
    }

    #[test]
    fn an_obstacle_that_covers_the_only_fitting_box_blocks_it() {
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 32.0), (280.0, 220.0));
        let covering = [rect(0.0, 0.0, 1000.0, 1000.0)];
        let sized = size_to_text(db, &request("Hold still.", &s, l, &covering)).expect("sizable");
        assert_eq!(sized.outcome, FitOutcome::Blocked { obstacle: 0 });
        assert_eq!(sized.bounds.width(), l.max.width, "a blocked grow is reported, not applied");
    }

    #[test]
    fn a_top_anchored_caption_grows_downward_and_keeps_its_top_edge() {
        let db = FontDb::global();
        let mut s = style(10.0);
        s.padding = 5.0;
        let l = limits((90.0, 26.0), (420.0, 150.0));
        let mut req = request("Earlier that evening, in a quieter part of town.", &s, l, &[]);
        req.place = Placement::new(Point::new(200.0, 60.0), ResizeAnchor::TopCentre);
        let sized = size_to_text(db, &req).expect("sizable");
        assert!(sized.measured.fits());
        assert!((sized.bounds.y0 - 60.0).abs() < 1e-9, "the top edge stays where it was placed");
        assert!((sized.bounds.center().x - 200.0).abs() < 1e-9);
        assert!(sized.bounds.height() > l.min.height);
    }

    #[test]
    fn a_degenerate_or_inverted_limit_pair_is_an_error_not_a_panic() {
        assert_eq!(Limits::new(Size::new(0.0, 10.0), Size::new(10.0, 10.0)), Err(FitError::NonPositive { field: "minimum width" }));
        assert_eq!(Limits::new(Size::new(f64::NAN, 10.0), Size::new(10.0, 10.0)), Err(FitError::NotFinite { field: "minimum width" }));
        assert_eq!(Limits::new(Size::new(20.0, 10.0), Size::new(10.0, 10.0)), Err(FitError::LimitsInverted));
    }

    #[test]
    fn limits_are_narrowed_to_the_room_available_without_going_below_the_minimum() {
        let l = limits((48.0, 32.0), (280.0, 220.0));
        assert_eq!(l.within(Size::new(100.0, 90.0)).max, Size::new(100.0, 90.0));
        assert_eq!(l.within(Size::new(1000.0, 1000.0)).max, l.max);
        // Smaller than the minimum: the minimum stands, so the caller sees a collision finding
        // rather than a box this kind cannot draw.
        assert_eq!(l.within(Size::new(10.0, 10.0)).max, l.min);
    }

    #[test]
    fn padding_that_swallows_the_minimum_box_is_reported() {
        let db = FontDb::global();
        let mut s = style(12.0);
        s.padding = 30.0;
        let l = limits((48.0, 32.0), (280.0, 220.0));
        let err = size_to_text(db, &request("Hi!", &s, l, &[])).expect_err("the minimum box leaves no room");
        assert!(matches!(err, FitError::PaddingTooLarge { .. }), "{err}");
    }

    #[test]
    fn a_non_finite_placement_is_rejected() {
        let db = FontDb::global();
        let s = style(12.0);
        let l = limits((48.0, 32.0), (280.0, 220.0));
        let mut req = request("Hi!", &s, l, &[]);
        req.place = Placement::new(Point::new(f64::INFINITY, 0.0), ResizeAnchor::Centre);
        assert_eq!(size_to_text(db, &req).err(), Some(FitError::NotFinite { field: "placement" }));
    }

    #[test]
    fn widths_are_ascending_bounded_and_reach_the_maximum() {
        let w = widths_from(100.0, 1000.0);
        assert!(w.len() <= MAX_WIDTH_STEPS);
        assert!(w.windows(2).all(|p| p[1] > p[0]), "{w:?}");
        assert_eq!(w.last().copied(), Some(1000.0));
        assert_eq!(widths_from(1000.0, 1000.0), vec![1000.0]);
        // A start above the maximum cannot walk away from it.
        assert_eq!(widths_from(2000.0, 1000.0), vec![2000.0]);
    }

    #[test]
    fn heights_are_ascending_and_end_at_the_maximum() {
        assert_eq!(heights_from(50.0, 32.0, 220.0), vec![50.0, 220.0]);
        assert_eq!(heights_from(10.0, 32.0, 220.0), vec![32.0, 220.0]);
        assert_eq!(heights_from(400.0, 32.0, 220.0), vec![220.0]);
        assert_eq!(heights_from(f64::NAN, 32.0, 220.0), vec![220.0]);
    }
}
