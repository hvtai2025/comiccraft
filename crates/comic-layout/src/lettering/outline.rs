//! Balloon and caption outlines: the editable closed path each lettering kind is drawn as.
//!
//! Every outline is a plain [`PathData`] of anchors with handles, so the balloon a reader sees is
//! the *editable path* the acceptance bar asks for: a creator can drag an anchor and the balloon
//! still re-fits through [`crate::lettering::plan::refit_balloon`].
//!
//! Four forms cover the schema's six kinds (schema §5.5, `LetteringKind`):
//!
//! | Shape | Kinds | Geometry |
//! |---|---|---|
//! | [`BalloonShape::Ellipse`] | `speech`, `whisper` | four smooth anchors; the whisper's dashed outline is a stroke style, not a different path |
//! | [`BalloonShape::Cloud`] | `thought` | lobes pinched at 84% of the ellipse and swollen onto it |
//! | [`BalloonShape::Box`] | `narration` | a caption's four corners |
//! | [`BalloonShape::Burst`] | `shout`, `sound_effect` | a straight-armed star, spikes on the ellipse and valleys at 78% |
//!
//! `speech` and `whisper` share their geometry deliberately: a whisper balloon differs by being
//! drawn with a broken line, and giving it a second path would make two objects to keep in step for
//! no gain.
//!
//! # Bounded by construction
//!
//! Every outline stays inside the box it is given, and every lobe and spike count is derived from
//! the box's own size — no randomness, no wall clock, the same box always gives the same path
//! (design §4). [`BalloonShape::inner_scale`] is the fraction of the box that is inside the
//! outline *everywhere*, which is what the text frame is cut from; the values are the inradius of
//! each form (for an ellipse, `1/√2` — the corner of the inscribed rectangle lies exactly on it).

use std::f64::consts::{FRAC_1_SQRT_2, PI, TAU};

use vectorcraft_geom::{Anchor, PathData, Point, Rect, SubPath, shapes};

/// The largest rectangle inscribed in an ellipse, as a fraction of the box.
const ELLIPSE_INNER_SCALE: f64 = FRAC_1_SQRT_2;

/// The text frame is kept just inside the outline rather than touching it.
///
/// An ellipse's inscribed rectangle has its corners *on* the curve, and a line of type set against
/// that corner would touch the balloon's edge — a hair inside is what a letterer would do by hand.
const INNER_SAFETY: f64 = 0.995;

/// A cloud's lobes meet at this fraction of the ellipse they are arranged on.
const CLOUD_PINCH: f64 = 0.84;

/// The cloud is laid out on an ellipse this much smaller than the box, so the cubic lobes' control
/// points cannot push the curve past the box where the ellipse is at its widest.
const CLOUD_SPAN: f64 = 0.96;

/// Points of perimeter per cloud lobe, and the bounds on how few or many a cloud gets.
const CLOUD_LOBE_STEP: f64 = 26.0;
const CLOUD_MIN_LOBES: u32 = 7;
const CLOUD_MAX_LOBES: u32 = 13;

/// A burst's valleys sit at this fraction of the ellipse its spikes are on.
const BURST_VALLEY: f64 = 0.78;

/// Points of perimeter per burst spike, and the bounds on how few or many a burst gets.
const BURST_SPIKE_STEP: f64 = 44.0;
const BURST_MIN_SPIKES: u32 = 6;
const BURST_MAX_SPIKES: u32 = 14;

/// A balloon's or a caption's outer form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BalloonShape {
    /// An ellipse: `speech` and `whisper`.
    Ellipse,
    /// A cloud of lobes: `thought`.
    Cloud,
    /// A rectangle: `narration`'s caption box.
    Box,
    /// A spiked burst: `shout` and `sound_effect`.
    Burst,
}

impl BalloonShape {
    /// The fraction of the box that is inside the outline in every direction.
    ///
    /// This is the shape's inradius as a fraction of the box, so the rectangle cut from it with
    /// [`crate::fit::inner_rect`] is inside the outline — text can never spill into a lobe or a
    /// spike, which is the failure that would make a balloon look broken rather than tight.
    ///
    /// The cloud and burst figures are conservative: they are measured at the sharpest chord
    /// between a pinch and a peak of the *narrowest* count this module generates, so a larger
    /// balloon with more lobes has margin to spare rather than a spike through its text.
    #[must_use]
    pub fn inner_scale(self) -> f64 {
        let scale = match self {
            Self::Ellipse => ELLIPSE_INNER_SCALE,
            Self::Cloud => 0.57,
            Self::Box => 1.0,
            Self::Burst => 0.51,
        };
        scale * INNER_SAFETY
    }

    /// The outline for a box, in document points.
    ///
    /// Always a closed [`PathData`] inside `bounds`. A degenerate box (a zero or negative side)
    /// gives a degenerate path rather than a panic; the callers all validate their sizes first.
    #[must_use]
    pub fn outline(self, bounds: Rect) -> PathData {
        match self {
            Self::Ellipse => shapes::ellipse(bounds),
            Self::Box => shapes::rectangle(bounds),
            Self::Cloud => cloud(bounds),
            Self::Burst => burst(bounds),
        }
    }
}

/// A cloud of lobes around the box.
fn cloud(bounds: Rect) -> PathData {
    let centre = bounds.center();
    let (rx, ry) = (bounds.width() / 2.0 * CLOUD_SPAN, bounds.height() / 2.0 * CLOUD_SPAN);
    // The lobes pinch at CLOUD_PINCH and swell back onto the ellipse the cloud is arranged on:
    // a symmetric cubic whose midpoint is the peak needs its controls 1/0.75 as far out as the
    // peak is from the pinch.
    let control = CLOUD_PINCH + (1.0 - CLOUD_PINCH) / 0.75;
    let count = count_for((bounds.width() + bounds.height()) * 2.0, CLOUD_LOBE_STEP, CLOUD_MIN_LOBES, CLOUD_MAX_LOBES);
    let step = TAU / f64::from(count);
    let at = |angle: f64, radius: f64| Point::new(centre.x + rx * radius * angle.cos(), centre.y + ry * radius * angle.sin());

    let anchors = (0..count)
        .map(|i| {
            // A lobe's swelling is centred on `-90° + i * step`, so the first lobe points straight
            // up and — whenever the count is a multiple of four — the cloud reaches both axes of
            // its box as well.
            let mid = f64::from(i) * step - PI / 2.0;
            let angle = mid - step / 2.0;
            // Each lobe runs from its pinch to the next; both of its controls sit on the lobe's own
            // mid-direction, so the curve swells outward and comes back down to the next pinch.
            Anchor::with_handles(at(angle, CLOUD_PINCH), at(mid - step, control), at(mid, control))
        })
        .collect();
    PathData::single(SubPath::new(anchors, true))
}

/// A straight-armed star: spikes on the box's ellipse, valleys at [`BURST_VALLEY`].
fn burst(bounds: Rect) -> PathData {
    let centre = bounds.center();
    let (rx, ry) = (bounds.width() / 2.0, bounds.height() / 2.0);
    let spikes = count_for((bounds.width() + bounds.height()) * 2.0, BURST_SPIKE_STEP, BURST_MIN_SPIKES, BURST_MAX_SPIKES);
    let step = PI / f64::from(spikes);
    let points = (0..spikes * 2)
        .map(|i| {
            let radius = if i % 2 == 0 { 1.0 } else { BURST_VALLEY };
            let angle = f64::from(i) * step - PI / 2.0;
            Point::new(centre.x + rx * radius * angle.cos(), centre.y + ry * radius * angle.sin())
        })
        .collect::<Vec<Point>>();
    PathData::single(SubPath::polyline(&points, true))
}

/// How many lobes or spikes a perimeter of `perimeter` points gets, within a floor and a ceiling.
///
/// A function of the box alone, so the same balloon always draws the same outline (there is no
/// random or time-based input anywhere in this module).
fn count_for(perimeter: f64, step: f64, min: u32, max: u32) -> u32 {
    if !(perimeter.is_finite() && step.is_finite() && step > 0.0) {
        return min;
    }
    let n = (perimeter / step).round();
    if !n.is_finite() {
        return min;
    }
    (n as i64).clamp(i64::from(min), i64::from(max)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::FillRule;
    use vectorcraft_geom::hit::fill_contains;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    const SHAPES: [BalloonShape; 4] = [BalloonShape::Ellipse, BalloonShape::Cloud, BalloonShape::Box, BalloonShape::Burst];

    #[test]
    fn every_outline_is_closed_and_inside_its_box() {
        for shape in SHAPES {
            for bounds in [rect(0.0, 0.0, 120.0, 90.0), rect(-50.0, 20.0, 41.0, 37.0), rect(3.5, 7.25, 300.0, 40.0)] {
                let path = shape.outline(bounds);
                assert!(path.is_closed(), "{shape:?} is not a closed path");
                assert!(path.anchor_count() >= 3, "{shape:?} has too few anchors to be a shape");
                let b = path.bounds().expect("a non-empty path has bounds");
                assert!(
                    b.x0 >= bounds.x0 - 1e-9 && b.x1 <= bounds.x1 + 1e-9 && b.y0 >= bounds.y0 - 1e-9 && b.y1 <= bounds.y1 + 1e-9,
                    "{shape:?} leaves its box: {b:?} outside {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn every_outline_fills_most_of_its_box() {
        // The box is the balloon's size, so an outline that fell far short of it would leave a gap
        // between what the reader sees and what the collision check measures. It never exceeds the
        // box (checked above); a scallop or a star misses the corner of its box by construction,
        // and the axis extents are within a few per cent.
        let bounds = rect(0.0, 0.0, 120.0, 90.0);
        for shape in SHAPES {
            let b = shape.outline(bounds).bounds().expect("bounds");
            assert!(b.width() >= bounds.width() * 0.9, "{shape:?} is only {} wide of 120", b.width());
            assert!(b.height() >= bounds.height() * 0.9, "{shape:?} is only {} tall of 90", b.height());
        }
    }

    #[test]
    fn the_inner_rectangle_is_inside_the_outline_for_every_shape() {
        // This is what `inner_scale` promises: the text frame cut from it cannot spill into a lobe
        // or a spike. Checked by fill-testing the frame's four corners and its edge midpoints.
        for shape in SHAPES {
            for bounds in [rect(0.0, 0.0, 120.0, 90.0), rect(200.0, 90.0, 60.0, 140.0), rect(-30.0, -40.0, 220.0, 55.0)] {
                let outline = shape.outline(bounds);
                let bez = outline.to_bezpath();
                let inner = crate::fit::inner_rect(bounds, shape.inner_scale());
                let probes = [
                    (inner.x0, inner.y0),
                    (inner.x1, inner.y0),
                    (inner.x1, inner.y1),
                    (inner.x0, inner.y1),
                    (inner.center().x, inner.y0),
                    (inner.center().x, inner.y1),
                    (inner.x0, inner.center().y),
                    (inner.x1, inner.center().y),
                ];
                for (x, y) in probes {
                    assert!(
                        fill_contains(&bez, FillRule::NonZero, Point::new(x, y)),
                        "{shape:?}: the text frame's point ({x}, {y}) is outside the outline at {bounds:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_ellipse_is_four_smooth_anchors_whose_inscribed_rectangle_sits_just_inside_it() {
        let bounds = rect(0.0, 0.0, 100.0, 50.0);
        let path = BalloonShape::Ellipse.outline(bounds);
        assert_eq!(path.anchor_count(), 4);
        let inner = crate::fit::inner_rect(bounds, BalloonShape::Ellipse.inner_scale());
        // The corner is a hair inside the ellipse: (x/rx)^2 + (y/ry)^2 is just below 1.
        let (dx, dy) = ((inner.x1 - 50.0) / 50.0, (inner.y1 - 25.0) / 25.0);
        let radius = dx * dx + dy * dy;
        assert!(radius < 1.0, "the corner is on or outside the ellipse: {radius}");
        assert!(radius > 0.98, "and it is not timidly small: {radius}");
    }

    #[test]
    fn a_caption_box_is_a_plain_rectangle() {
        let bounds = rect(10.0, 20.0, 80.0, 30.0);
        let path = BalloonShape::Box.outline(bounds);
        assert_eq!(path.anchor_count(), 4);
        assert_eq!(path.bounds(), Some(bounds));
        // A caption box leaves (very nearly) all of itself for text — the hair is the safety factor
        // every shape keeps so the frame is inside the outline rather than on it.
        assert!(BalloonShape::Box.inner_scale() >= 0.99, "{}", BalloonShape::Box.inner_scale());
        assert_eq!(BalloonShape::Box.inner_scale(), BalloonShape::Ellipse.inner_scale() / std::f64::consts::FRAC_1_SQRT_2);
    }

    #[test]
    fn the_same_box_always_draws_the_same_outline() {
        // Determinism is a design rule (§4), and the lobe and spike counts are the only place a
        // "looks about right" formula could have crept in.
        for shape in SHAPES {
            let bounds = rect(12.0, 34.0, 210.0, 96.0);
            assert_eq!(shape.outline(bounds), shape.outline(bounds), "{shape:?} is not deterministic");
        }
    }

    #[test]
    fn counts_are_clamped_and_never_panic_on_hostile_input() {
        assert_eq!(count_for(0.0, 26.0, 7, 13), 7);
        assert_eq!(count_for(f64::NAN, 26.0, 7, 13), 7);
        assert_eq!(count_for(1.0e9, 26.0, 7, 13), 13);
        assert_eq!(count_for(100.0, 0.0, 7, 13), 7, "a zero step is not a division by zero");
        assert_eq!(count_for(100.0, f64::INFINITY, 7, 13), 7);
    }

    #[test]
    fn a_bigger_balloon_gets_more_lobes_and_spikes() {
        let small = rect(0.0, 0.0, 60.0, 40.0);
        let large = rect(0.0, 0.0, 300.0, 220.0);
        assert!(
            BalloonShape::Cloud.outline(large).anchor_count() > BalloonShape::Cloud.outline(small).anchor_count(),
            "a cloud scales its lobes with its size"
        );
        assert!(BalloonShape::Burst.outline(large).anchor_count() > BalloonShape::Burst.outline(small).anchor_count());
    }
}
