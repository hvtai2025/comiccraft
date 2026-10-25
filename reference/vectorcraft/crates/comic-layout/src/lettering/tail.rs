//! Balloon tails: an editable path that stays attached to its balloon.
//!
//! A tail points at something — the speaker's mouth — and it belongs to the balloon, not to the
//! page. Moving a balloon must therefore *preserve or update* its tail and never detach it
//! (design §8.3). This module makes that structural rather than a promise kept by hand:
//!
//! * A [`Tail`] is stored as [`TailKind`] + `attach` + `tip`. The **path is derived** from those
//!   three, so it cannot drift out of step with them.
//! * [`Tail::after_balloon_move`] keeps the tip — the thing being pointed at does not move when a
//!   balloon does — and recomputes the attachment from the **new** outline, so the tail's base
//!   follows the balloon's edge wherever the balloon goes.
//! * [`Tail::retarget`] keeps the balloon and moves the tip.
//!
//! The attachment is the point of the outline nearest the tip, found with
//! [`PathData::nearest`] — so it is exact for every shape (an ellipse, a cloud's lobes, a burst's
//! spikes) rather than a formula that only holds for circles.
//!
//! The two kinds are the two ways a comic points at a speaker: a **wedge** (a tapered flap, used by
//! `speech`, `shout` and `whisper`) and a **chain of bubbles** (`thought`).

use vectorcraft_geom::{Anchor, PathData, Point, Rect, Size, SubPath, Vec2, shapes};

use super::outline::BalloonShape;

/// The shortest tail worth drawing: below this the tail has no direction, and the wedge falls back
/// to pointing down so the path is still a path.
const MIN_LENGTH: f64 = 1.0;

/// A wedge's base width, as a fraction of the tail's length, and the bounds on it.
const WEDGE_BASE: f64 = 0.55;
const WEDGE_MIN_BASE: f64 = 6.0;
const WEDGE_MAX_BASE: f64 = 24.0;

/// How far a wedge's sides bow outward from the straight line to the tip.
const WEDGE_BOW: f64 = 0.12;

/// A bubble chain's radii, as fractions of the tail's length, and the bounds on them.
const BUBBLE_RADIUS: f64 = 0.18;
const BUBBLE_MIN: f64 = 1.2;
const BUBBLE_MAX: f64 = 9.0;
const BUBBLE_RATIO: f64 = 0.55;
const BUBBLE_NEAR: f64 = 0.5;

/// How a tail is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailKind {
    /// A tapered flap from the balloon's edge to the tip: `speech`, `shout`, `whisper`.
    Wedge,
    /// Two bubbles, the larger near the balloon: `thought`.
    Bubble,
}

/// A balloon's tail.
///
/// `path` is derived from `kind`, `attach` and `tip`; build one through [`Tail::new`] rather than
/// filling the fields in, or the path and the three points can disagree.
#[derive(Clone, Debug, PartialEq)]
pub struct Tail {
    /// How it is drawn.
    pub kind: TailKind,
    /// Where it meets the balloon's outline.
    pub attach: Point,
    /// The point it points at — the far end of a wedge, the centre of the last bubble.
    pub tip: Point,
    /// The editable path, in document points.
    pub path: PathData,
}

impl Tail {
    /// The tail of `shape`'s balloon that points at `tip`.
    ///
    /// `tip` outside the balloon is the normal case; a `tip` inside it is not refused — the
    /// attachment is still a point of the outline, so the path stays connected — but the caller
    /// should not expect a tail to read well in that state.
    #[must_use]
    pub fn new(bounds: Rect, shape: BalloonShape, kind: TailKind, tip: Point) -> Self {
        Self::from_outline(&shape.outline(bounds), bounds, kind, tip)
    }

    /// The tail of a balloon whose outline the caller already has.
    ///
    /// This is the constructor a *move* uses: the frame path the plan carries is translated, not
    /// regenerated, so a balloon whose anchors a creator has dragged keeps that shape and its tail
    /// is re-based on the path the balloon actually has.
    #[must_use]
    pub fn from_outline(outline: &PathData, bounds: Rect, kind: TailKind, tip: Point) -> Self {
        let attach = attach_point(outline, tip, bounds);
        let tip = reachable_tip(attach, tip);
        Self { kind, attach, tip, path: path_for(kind, attach, tip) }
    }

    /// The tail after its balloon moves: the tip stays put and the base follows the outline.
    ///
    /// This is the rule design §8.3 states, expressed once: a tail is never detached by moving the
    /// balloon it belongs to, and it never stops pointing at what it pointed at.
    #[must_use]
    pub fn after_balloon_move(&self, bounds: Rect, shape: BalloonShape) -> Self {
        Self::new(bounds, shape, self.kind, self.tip)
    }

    /// The tail after its balloon's *outline* moves: the tip stays put and the base follows the
    /// path, whatever shape that path now has.
    #[must_use]
    pub fn after_outline_move(&self, outline: &PathData, bounds: Rect) -> Self {
        Self::from_outline(outline, bounds, self.kind, self.tip)
    }

    /// The tail after its balloon moves *and* the thing it points at moves.
    #[must_use]
    pub fn retarget(&self, bounds: Rect, shape: BalloonShape, tip: Point) -> Self {
        Self::new(bounds, shape, self.kind, tip)
    }

    /// Whether this tail's base is still on `bounds`' outline for `shape`.
    ///
    /// The check a caller makes after moving a balloon, and the one the tests make: the attachment
    /// is a point of the outline to within a hair, so a tail can never have been left behind.
    #[must_use]
    pub fn is_attached_to(&self, bounds: Rect, shape: BalloonShape) -> bool {
        self.is_attached_to_outline(&shape.outline(bounds))
    }

    /// Whether this tail's base is still on `outline` — the check a move makes against the path the
    /// balloon actually has, rather than against a box that only approximates it.
    #[must_use]
    pub fn is_attached_to_outline(&self, outline: &PathData) -> bool {
        outline.nearest(self.attach).is_some_and(|(_, _, _, _, distance)| distance.is_finite() && distance < 1e-6)
    }
}

/// The point of `outline` nearest `tip`, which is where a tail's base sits.
///
/// A `tip` that is not a finite point is aimed **straight down** instead: the probe becomes the point
/// [`MIN_LENGTH`] below the box, so the base lands on the balloon's lowest point — the direction
/// [`reachable_tip`] already gives a tail whose target it cannot use. Falling back to the box's
/// *centre* would put the base inside the balloon, 31 to 40 points off the outline, and the
/// constructor would then build a tail its own [`Tail::is_attached_to`] calls detached (round 1,
/// F6).
///
/// Falls back to the box's centre only when the outline has no segments at all — a degenerate balloon
/// with nothing to attach to — so such a balloon still yields a path instead of a panic.
#[must_use]
pub fn attach_point(outline: &PathData, tip: Point, bounds: Rect) -> Point {
    let centre = bounds.center();
    let probe = if tip.x.is_finite() && tip.y.is_finite() { tip } else { downward_probe(bounds) };
    outline.nearest(probe).map_or(centre, |(_, _, _, point, _)| if point.x.is_finite() && point.y.is_finite() { point } else { centre })
}

/// The point a tail aims at when its target is not a point: [`MIN_LENGTH`] below the box.
///
/// The same direction `direction` falls back to, so a tail with an unusable target hangs down from
/// the balloon's lowest point rather than pointing nowhere.
#[must_use]
pub fn downward_probe(bounds: Rect) -> Point {
    Point::new(bounds.center().x, bounds.y1 + MIN_LENGTH)
}

/// A tip a tail can actually reach: the point itself, or a point [`MIN_LENGTH`] straight below the
/// attachment when the two coincide or the tip is not a finite point.
///
/// The alternative to this normalisation is a path of zero height or a path full of NaNs, and a
/// balloon dropped exactly on its target is a thing a creator does by accident every day.
#[must_use]
pub fn reachable_tip(attach: Point, tip: Point) -> Point {
    let delta = tip - attach;
    if delta.hypot() > MIN_LENGTH && delta.x.is_finite() && delta.y.is_finite() { tip } else { attach + Vec2::new(0.0, MIN_LENGTH) }
}

/// The path for a tail from `attach` to `tip`.
#[must_use]
pub fn path_for(kind: TailKind, attach: Point, tip: Point) -> PathData {
    match kind {
        TailKind::Wedge => wedge(attach, tip),
        TailKind::Bubble => bubbles(attach, tip),
    }
}

/// The unit vector from `attach` to `tip`, or straight down when they coincide.
///
/// Down, because a comic tail that points nowhere specific still reads as a balloon's tail hanging
/// below it; the alternative — a NaN direction — is the one outcome this must not produce.
fn direction(attach: Point, tip: Point) -> (Vec2, f64) {
    let delta = tip - attach;
    let length = delta.hypot();
    if length > MIN_LENGTH && length.is_finite() { (delta / length, length) } else { (Vec2::new(0.0, 1.0), 0.0) }
}

/// A tapered flap whose base is centred on the attachment.
///
/// The base straddles the outline, so the tail meets the balloon with no gap at either corner —
/// which is also why the two base anchors are the pair that a creator drags to reshape a tail.
fn wedge(attach: Point, tip: Point) -> PathData {
    let tip = reachable_tip(attach, tip);
    let (unit, length) = direction(attach, tip);
    let normal = Vec2::new(-unit.y, unit.x);
    let base = (length * WEDGE_BASE).clamp(WEDGE_MIN_BASE, WEDGE_MAX_BASE);
    let left = attach + normal * (base / 2.0);
    let right = attach - normal * (base / 2.0);
    let quarter = (tip - attach) * 0.5;
    let bow = normal * (base * WEDGE_BOW);

    let anchors = vec![
        Anchor::with_handles(left, left, left + quarter + bow),
        Anchor::with_handles(tip, tip - quarter + bow, tip - quarter - bow),
        Anchor::with_handles(right, right - quarter - bow, right),
    ];
    PathData::single(SubPath::new(anchors, true))
}

/// Two bubbles: the larger part-way along, the smaller at the tip.
fn bubbles(attach: Point, tip: Point) -> PathData {
    let tip = reachable_tip(attach, tip);
    let (unit, length) = direction(attach, tip);
    let far_radius = (length * BUBBLE_RADIUS).clamp(BUBBLE_MIN, BUBBLE_MAX);
    let near_radius = (far_radius * BUBBLE_RATIO).max(BUBBLE_MIN);
    let near = attach + unit * (length * BUBBLE_NEAR);

    let circle = |centre: Point, radius: f64| Rect::from_center_size(centre, Size::new(radius * 2.0, radius * 2.0));
    let mut subpaths = shapes::ellipse(circle(near, near_radius)).subpaths;
    subpaths.extend(shapes::ellipse(circle(tip, far_radius)).subpaths);
    PathData::new(subpaths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Shape;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    fn balloon() -> Rect {
        rect(100.0, 100.0, 120.0, 80.0)
    }

    #[test]
    fn a_tail_starts_on_the_balloons_outline_and_ends_at_its_target() {
        for shape in [BalloonShape::Ellipse, BalloonShape::Cloud, BalloonShape::Box, BalloonShape::Burst] {
            let bounds = balloon();
            let target = Point::new(160.0, 260.0);
            let tail = Tail::new(bounds, shape, TailKind::Wedge, target);
            assert!(tail.is_attached_to(bounds, shape), "{shape:?}: the tail does not meet the outline");
            assert!(bounds.inflate(1e-6, 1e-6).contains(tail.attach), "{shape:?}: the attachment is not on the balloon");
            assert_eq!(tail.tip, target, "{shape:?}: a wedge ends at the point it points at");
            assert!(tail.path.is_closed(), "{shape:?}: a wedge is a closed path");
            assert_eq!(tail.path.anchor_count(), 3, "{shape:?}: three anchors a creator can drag");
        }
    }

    #[test]
    fn moving_the_balloon_updates_the_base_and_keeps_the_target() {
        // Design §8.3: moving a balloon must preserve or update its tail, never detach it.
        let shape = BalloonShape::Ellipse;
        let target = Point::new(160.0, 260.0);
        let tail = Tail::new(balloon(), shape, TailKind::Wedge, target);
        let moved = rect(400.0, 40.0, 120.0, 80.0);
        let after = tail.after_balloon_move(moved, shape);
        assert_eq!(after.tip, target, "the thing being pointed at did not move");
        assert_ne!(after.attach, tail.attach, "the base followed the balloon");
        assert!(after.is_attached_to(moved, shape));
        // And it is attached to the balloon's *near* side: the target is now below and to the
        // left of the balloon, so that is where the tail leaves it.
        assert!(after.attach.y > moved.center().y, "the tail leaves the lower edge");
        assert!(after.attach.x < moved.center().x, "the tail leaves the left edge");
        // The base sits on the side of the balloon the target is on: the dot product of the base's
        // offset with the target's direction is positive.
        let outward = after.attach - moved.center();
        let towards = target - moved.center();
        assert!(outward.dot(towards) > 0.0, "the tail no longer leans at its target");
    }

    #[test]
    fn a_tail_that_would_be_left_behind_is_re_attached_on_every_edge() {
        // Four moves in four directions, each one checked against the outline the balloon now has.
        let shape = BalloonShape::Ellipse;
        let tail = Tail::new(balloon(), shape, TailKind::Wedge, Point::new(160.0, 300.0));
        for moved in [rect(0.0, 0.0, 120.0, 80.0), rect(300.0, 0.0, 120.0, 80.0), rect(0.0, 400.0, 120.0, 80.0), rect(500.0, 400.0, 60.0, 40.0)] {
            let after = tail.after_balloon_move(moved, shape);
            assert!(after.is_attached_to(moved, shape), "{moved:?}: detached");
            assert!(moved.inflate(1e-6, 1e-6).contains(after.attach), "{moved:?}: the base is off the balloon");
        }
    }

    #[test]
    fn retargeting_moves_the_tip_and_keeps_the_balloon() {
        let shape = BalloonShape::Ellipse;
        let tail = Tail::new(balloon(), shape, TailKind::Wedge, Point::new(160.0, 300.0));
        let after = tail.retarget(balloon(), shape, Point::new(80.0, 300.0));
        assert_eq!(after.tip, Point::new(80.0, 300.0));
        assert!(after.is_attached_to(balloon(), shape));
        assert!(after.attach.x < tail.attach.x, "the base swung to the far side");
    }

    #[test]
    fn a_thought_tail_is_a_chain_of_bubbles_that_ends_at_its_target() {
        let shape = BalloonShape::Cloud;
        let tail = Tail::new(balloon(), shape, TailKind::Bubble, Point::new(160.0, 300.0));
        assert_eq!(tail.path.subpaths.len(), 2, "a thought tail is two bubbles");
        assert!(tail.path.subpaths.iter().all(|sp| sp.closed));
        let mut far_path = vectorcraft_geom::BezPath::new();
        if let Some(sub) = tail.path.subpaths.last() {
            sub.to_bezpath_into(&mut far_path);
        }
        let far = far_path.bounding_box();
        assert!((far.center().x - tail.tip.x).abs() < 1e-9 && (far.center().y - tail.tip.y).abs() < 1e-9, "the last bubble is centred on the target");
        assert!(far.width() > 0.0 && far.height() > 0.0);
        assert!(tail.is_attached_to(balloon(), shape));
    }

    #[test]
    fn a_target_inside_the_balloon_still_gives_a_connected_path() {
        // Not a state a tail reads well in, but it must not produce a NaN or a detached tail: the
        // nearest outline point to an interior point is on the outline.
        for shape in [BalloonShape::Ellipse, BalloonShape::Box, BalloonShape::Cloud, BalloonShape::Burst] {
            let bounds = balloon();
            let tail = Tail::new(bounds, shape, TailKind::Wedge, bounds.center());
            assert!(tail.is_attached_to(bounds, shape), "{shape:?}");
            assert!(tail.path.bounds().is_some(), "{shape:?}: the path has geometry");
            let b = tail.path.bounds().expect("bounds");
            assert!(b.width().is_finite() && b.height().is_finite(), "{shape:?}: {b:?}");
        }
    }

    #[test]
    fn a_target_on_the_attachment_falls_back_to_straight_down_instead_of_a_nan() {
        // The degenerate case, reached when a balloon is dropped exactly on its target.
        let attach = Point::new(10.0, 10.0);
        let path = path_for(TailKind::Wedge, attach, attach);
        let bounds = path.bounds().expect("bounds");
        assert!(bounds.width().is_finite() && bounds.height().is_finite(), "{bounds:?}");
        assert!(bounds.y1 > bounds.y0, "the tail still hangs down");
        let bubbles = path_for(TailKind::Bubble, attach, attach);
        let bb = bubbles.bounds().expect("bounds");
        assert!(bb.width().is_finite() && bb.height().is_finite());
        assert_eq!(bubbles.subpaths.len(), 2);
    }

    #[test]
    fn the_same_balloon_and_target_always_give_the_same_tail() {
        let tail = Tail::new(balloon(), BalloonShape::Ellipse, TailKind::Wedge, Point::new(160.0, 300.0));
        let again = Tail::new(balloon(), BalloonShape::Ellipse, TailKind::Wedge, Point::new(160.0, 300.0));
        assert_eq!(tail, again);
        assert_eq!(tail.after_balloon_move(balloon(), BalloonShape::Ellipse), tail);
    }

    #[test]
    fn a_tail_built_from_a_non_finite_target_hangs_from_the_outline_not_from_the_centre() {
        // The caller validates its points (see `crate::lettering::plan`); one arriving here anyway
        // must still produce a tail that touches its balloon. The centre of the box is *not* a point
        // of any outline, so a tail based there is one this module's own `is_attached_to` calls
        // detached, 31 to 40 points off — which is what an earlier version of this test asserted,
        // pinning the defect rather than catching it (round 1, F6).
        let bounds = balloon();
        for shape in [BalloonShape::Ellipse, BalloonShape::Cloud, BalloonShape::Box, BalloonShape::Burst] {
            for kind in [TailKind::Wedge, TailKind::Bubble] {
                let tail = Tail::new(bounds, shape, kind, Point::new(f64::NAN, f64::NAN));
                assert!(tail.path.bounds().is_some(), "{shape:?} {kind:?}: the path is a path");
                assert!(tail.is_attached_to(bounds, shape), "{shape:?} {kind:?}: a constructor's own tail is detached");
                assert_ne!(tail.attach, bounds.center(), "{shape:?} {kind:?}: the base is the box's centre, which is inside the outline");
                // It hangs down, the direction the module falls back to for a target it cannot use.
                assert!(tail.tip.y > tail.attach.y, "{shape:?} {kind:?}: {tail:?}");
                assert!((tail.tip - tail.attach).hypot().is_finite());
            }
        }
        // The fallback probe is the point the tail aims at, so the base is the outline's lowest
        // point rather than an arbitrary one.
        let outline = BalloonShape::Ellipse.outline(bounds);
        let attach = attach_point(&outline, Point::new(f64::NAN, f64::NAN), bounds);
        assert!(outline.nearest(attach).is_some_and(|(_, _, _, _, distance)| distance < 1e-6), "{attach:?} is off the outline");
        assert!((attach.x - bounds.center().x).abs() < 1e-9, "the base is under the balloon: {attach:?}");
        assert!(attach.y > bounds.center().y, "the base is on the lower half: {attach:?}");
    }

    #[test]
    fn a_non_finite_target_is_answered_by_the_downward_probe_and_not_by_nearest() {
        // The property the guard above rests on, stated so a change to `nearest` cannot disarm it:
        // a non-finite target is *replaced* by `downward_probe(bounds)` before `nearest` sees it.
        //
        // It matters because `nearest` answers a NaN query with the path's first anchor — a point
        // that is itself on the outline, so a test that only asked "is it attached?" would not notice
        // the difference. Round 2's mutation F6a (`probe = tip`) is caught here and was not caught by
        // an attachment-only assertion.
        let bounds = balloon();
        let nan = Point::new(f64::NAN, f64::NAN);
        for shape in [BalloonShape::Ellipse, BalloonShape::Cloud, BalloonShape::Box, BalloonShape::Burst] {
            let outline = shape.outline(bounds);
            let probe = downward_probe(bounds);
            let attach = attach_point(&outline, nan, bounds);
            assert_eq!(attach, attach_point(&outline, probe, bounds), "{shape:?}: a non-finite target did not become the downward probe");
            assert_eq!(attach, outline.nearest(probe).map(|(_, _, _, point, _)| point).expect("an outline has segments"), "{shape:?}");
            assert_ne!(attach, bounds.center(), "{shape:?}: the base is the box's centre");
            // Not what a NaN query returns: `nearest` gives the path's first anchor, which is a
            // different point of the outline on every one of these shapes.
            let from_nan = outline.nearest(nan).map(|(_, _, _, point, _)| point);
            assert_ne!(Some(attach), from_nan, "{shape:?}: the guard let a NaN reach `nearest`");
        }
        // A finite target is passed through untouched.
        let outline = BalloonShape::Ellipse.outline(bounds);
        let target = Point::new(160.0, 300.0);
        assert_eq!(attach_point(&outline, target, bounds), outline.nearest(target).map(|(_, _, _, point, _)| point).expect("segments"));
    }

    #[test]
    fn attachment_is_measured_against_the_path_and_can_say_no() {
        // Without a negative case every attachment assertion in this file would pass if
        // `is_attached_to_outline` simply returned `true`. A base a quarter of a point off the
        // outline is not attached; one on it is.
        let bounds = balloon();
        let outline = BalloonShape::Ellipse.outline(bounds);
        let tail = Tail::new(bounds, BalloonShape::Ellipse, TailKind::Wedge, Point::new(160.0, 300.0));
        assert!(tail.is_attached_to_outline(&outline));
        let nudged = Tail { attach: tail.attach + Vec2::new(0.25, 0.0), ..tail.clone() };
        assert!(!nudged.is_attached_to_outline(&outline), "a base a quarter point off the outline is not attached");
        // The check is about the base, not about the path: a tail whose path was replaced still
        // reports its base.
        let pathless = Tail { path: PathData::default(), ..tail.clone() };
        assert!(pathless.is_attached_to_outline(&outline));
    }
}
