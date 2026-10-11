//! The overflow policy, applied as a plan.
//!
//! `Lettering.overflow_policy` is `warn`, `shrink` or `resize_frame` (schema §5.5). This module
//! answers what each one does when the text no longer fits the frame it is in, and it answers with
//! data: a box, an [`AreaFit`] and the findings — never a mutation. The command layer writes the
//! record and the objects from that answer, which is what makes one undoable step (design §8,
//! §11.2).
//!
//! | Policy | What the plan says | What the record gets |
//! |---|---|---|
//! | `warn` | report the overflow, change nothing | `TextLayoutSettings::fit_to_frame = false` |
//! | `shrink` | `vectorcraft-text`'s shrink-to-fit for this frame | `fit_to_frame = true` |
//! | `resize_frame` | the smallest larger frame that holds the text | `fit_to_frame = false` |
//!
//! [`FitTextStyle::shrink_percent`] is the floor `shrink` may go to; below it the text still
//! overflows and the plan says so rather than scaling type a reader cannot read.
//!
//! **The collision case.** Automatic resizing must never move unrelated page elements (design
//! §8.3). A frame that would have to overlap another panel or another lettering object is
//! **reported, not applied**: the plan keeps the frame it was given and returns
//! [`FitOutcome::Blocked`] naming the obstacle's index in the caller's list.
//!
//! [`FitTextStyle::shrink_percent`]: crate::fit::FitTextStyle::shrink_percent

use vectorcraft_comic_model::OverflowPolicy;
use vectorcraft_doc::AreaFit;
use vectorcraft_geom::Rect;
use vectorcraft_text::FontDb;

use super::sizing::{Limits, Placement, ResizeAnchor, SizeRequest, size_to_text};
use super::{FitError, FitOutcome, FitTextStyle, Measured, area_fit, first_collision, measure, overflow_finding, text_frame};

/// What applying a policy decided.
#[derive(Clone, Debug)]
pub struct FitDecision {
    /// The box the caller should write: the one it passed in, or the grown one.
    pub bounds: Rect,
    /// The measurement behind the decision, in `bounds`' frame.
    pub measured: Measured,
    /// How the policy resolved.
    pub outcome: FitOutcome,
    /// What to report — empty when the text fits, and when a resize or a shrink succeeded.
    pub findings: Vec<vectorcraft_comic_model::Finding>,
}

/// Everything a policy is applied to.
#[derive(Clone, Copy, Debug)]
pub struct FitRequest<'a> {
    /// The words, borrowed — the `Dialogue` record owns them (design §6.1).
    pub text: &'a str,
    /// The style to measure them in.
    pub style: &'a FitTextStyle,
    /// The frame as it is, in document points.
    pub bounds: Rect,
    /// The fraction of the box the shape leaves for text (`BalloonShape::inner_scale`).
    pub inner_scale: f64,
    /// The policy from the record.
    pub policy: OverflowPolicy,
    /// How large the frame may grow when the policy is `resize_frame`.
    pub limits: Limits,
    /// Which edge stays put when the frame grows — the balloon's centre, or a caption's top.
    pub anchor: ResizeAnchor,
    /// Rectangles the grown frame must not overlap — other panels and other lettering.
    pub obstacles: &'a [Rect],
    /// What the finding's subject names, e.g. `lettering speech`.
    pub subject: &'a str,
}

/// Apply `request.policy` to a frame that already exists.
pub fn apply(db: &FontDb, request: &FitRequest<'_>) -> Result<FitDecision, FitError> {
    request.style.validate()?;
    let frame = text_frame(request.bounds, request.inner_scale, request.style.padding)?;
    let measured = measure(db, request.text, request.style, &frame, AreaFit::None)?;

    if measured.fits() {
        return Ok(FitDecision { bounds: request.bounds, measured, outcome: FitOutcome::Fits { fit_scale: 1.0 }, findings: Vec::new() });
    }

    match request.policy {
        OverflowPolicy::Warn => Ok(report(request, measured, FitOutcome::Warned, overflow_message(request, false))),
        OverflowPolicy::Shrink => shrink(db, request, frame),
        OverflowPolicy::ResizeFrame => resize(db, request),
    }
}

/// `shrink`: hand the frame to the engine's shrink-to-fit and read `fit_scale` back.
fn shrink(db: &FontDb, request: &FitRequest<'_>, frame: Rect) -> Result<FitDecision, FitError> {
    let fit = area_fit(OverflowPolicy::Shrink, request.style);
    let shrunk = measure(db, request.text, request.style, &frame, fit)?;
    let (scale, fits) = (shrunk.layout.fit_scale, shrunk.fits());
    let outcome = FitOutcome::Shrunk { fit_scale: scale, fits };
    if fits {
        // Shrinking succeeded: the frame keeps its size and the plan says by how much the type was
        // scaled, which is what `TextLayout.fit_scale` reports.
        return Ok(FitDecision { bounds: request.bounds, measured: shrunk, outcome, findings: Vec::new() });
    }
    // The floor the *engine* was given, which is `AreaFit::parse`'s clamp of the style's number:
    // the message must quote the policy that was applied, not the one the style asked for.
    let floor = fit.min_scale().unwrap_or(1.0);
    let message = shrink_message(request, floor);
    Ok(report(request, shrunk, outcome, message))
}

/// `resize_frame`: grow the frame to the text, or report why it could not.
fn resize(db: &FontDb, request: &FitRequest<'_>) -> Result<FitDecision, FitError> {
    // The floor is the frame as it is, never below the kind's own minimum and never above the
    // maximum: automatic resizing grows a frame to hold its text and never shrinks one under it,
    // and a frame a creator has dragged past the kind's maximum is still re-fitted rather than
    // refused.
    let floor = vectorcraft_geom::Size::new(
        request.bounds.width().max(request.limits.min.width).min(request.limits.max.width),
        request.bounds.height().max(request.limits.min.height).min(request.limits.max.height),
    );
    let limits = Limits::new(floor, request.limits.max)?;
    let sized = size_to_text(
        db,
        &SizeRequest {
            text: request.text,
            style: request.style,
            inner_scale: request.inner_scale,
            limits,
            place: Placement::of(request.bounds, request.anchor),
            obstacles: request.obstacles,
        },
    )?;
    match sized.outcome {
        FitOutcome::Fits { .. } => {
            // Growing is silent when it works: the plan carries the box, and `Resized` is the record
            // of what happened for the caller and the tests.
            Ok(FitDecision {
                bounds: sized.bounds,
                measured: sized.measured,
                outcome: FitOutcome::Resized { bounds: sized.bounds },
                findings: Vec::new(),
            })
        }
        FitOutcome::Blocked { obstacle } => {
            let message = format!(
                "the frame would have to grow to {}x{} points to hold the text, which overlaps another object; reported, not applied",
                round(sized.bounds.width()),
                round(sized.bounds.height())
            );
            let measured = in_the_given_frame(db, request)?;
            Ok(report(request, measured, FitOutcome::Blocked { obstacle }, message))
        }
        _ => {
            let measured = in_the_given_frame(db, request)?;
            Ok(report(request, measured, FitOutcome::Unfittable, unfittable_message(request)))
        }
    }
}

/// The text measured in the frame of the box the caller gave — the box a reported decision returns.
///
/// A grow that is **not applied** leaves the frame exactly as it was, so the measurement the caller
/// reads must describe *that* frame. `size_to_text` measured the text in the largest box the limits
/// allow before it gave up, and handing that measurement back would put `measured.frame`,
/// `needed_height` and the plan's `text_bounds` in a box nobody is writing (round 1, F1).
fn in_the_given_frame(db: &FontDb, request: &FitRequest<'_>) -> Result<Measured, FitError> {
    let frame = text_frame(request.bounds, request.inner_scale, request.style.padding)?;
    measure(db, request.text, request.style, &frame, AreaFit::None)
}

/// A decision that changed nothing and reports why: the box the caller gave, a measurement in *that*
/// box's frame, and one finding.
fn report(request: &FitRequest<'_>, measured: Measured, outcome: FitOutcome, message: String) -> FitDecision {
    FitDecision { bounds: request.bounds, measured, outcome, findings: vec![overflow_finding(request.subject, message)] }
}

/// A deterministic whole-point size for a message: a finding is read by a person, and
/// `142.7344…` points of chalk is noise.
fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn overflow_message(request: &FitRequest<'_>, _shrunk: bool) -> String {
    format!(
        "the text overflows its {}x{} point frame; overflow_policy is warn, so nothing was changed",
        round(request.bounds.width()),
        round(request.bounds.height())
    )
}

/// The message for a shrink that hit its floor.
///
/// Both numbers are the **applied** floor (`AreaFit::min_scale`), never the style's raw
/// `shrink_percent`: `AreaFit::parse` clamps into 10..=100, so a style asking for `1.0` is given
/// `10` and a message quoting `1` would state a policy the engine never saw (round 1, F2). A
/// non-finite percent never reaches here — [`FitTextStyle::validate`] refuses it.
fn shrink_message(request: &FitRequest<'_>, floor: f64) -> String {
    format!(
        "the text still overflows its {}x{} point frame at {}% — the smallest size the policy allows ({}%) — so it was left as it is",
        round(request.bounds.width()),
        round(request.bounds.height()),
        round(floor * 100.0),
        round(floor * 100.0)
    )
}

fn unfittable_message(request: &FitRequest<'_>) -> String {
    format!(
        "the text cannot be set in any frame up to {}x{} points; the frame was not grown",
        round(request.limits.max.width),
        round(request.limits.max.height)
    )
}

/// Whether growing `bounds` to hold `text` would overlap one of `obstacles`, and which.
///
/// Exposed so a caller can ask the collision question without asking for a plan — for instance when
/// it drags a balloon and wants to warn before the drop.
#[must_use]
pub fn collision(bounds: Rect, obstacles: &[Rect]) -> Option<usize> {
    first_collision(bounds, obstacles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::{Point, Size};

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    fn style() -> FitTextStyle {
        FitTextStyle::new("Comic Neue", "Regular", 12.0)
    }

    fn limits() -> Limits {
        Limits::new(Size::new(48.0, 32.0), Size::new(280.0, 220.0)).expect("valid limits")
    }

    fn request<'a>(text: &'a str, style: &'a FitTextStyle, bounds: Rect, policy: OverflowPolicy, obstacles: &'a [Rect]) -> FitRequest<'a> {
        FitRequest {
            text,
            style,
            bounds,
            inner_scale: std::f64::consts::FRAC_1_SQRT_2,
            policy,
            limits: limits(),
            anchor: ResizeAnchor::Centre,
            obstacles,
            subject: "lettering speech",
        }
    }

    /// A frame the given text overflows at 12 pt: its frame is far too small for this sentence.
    fn tight() -> Rect {
        rect(180.0, 140.0, 48.0, 32.0)
    }

    const LONG: &str = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";

    #[test]
    fn text_that_fits_is_not_reported_at_all() {
        let db = FontDb::global();
        let s = style();
        let bounds = rect(100.0, 100.0, 260.0, 200.0);
        let decision = apply(db, &request("Hi!", &s, bounds, OverflowPolicy::Warn, &[])).expect("applied");
        assert_eq!(decision.outcome, FitOutcome::Fits { fit_scale: 1.0 });
        assert!(decision.findings.is_empty(), "a fitting balloon reports nothing");
        assert_eq!(decision.bounds, bounds, "and changes nothing");
    }

    #[test]
    fn warn_reports_the_overflow_and_leaves_the_frame_alone() {
        let db = FontDb::global();
        let s = style();
        let decision = apply(db, &request(LONG, &s, tight(), OverflowPolicy::Warn, &[])).expect("applied");
        assert_eq!(decision.outcome, FitOutcome::Warned);
        assert_eq!(decision.bounds, tight(), "warn changes nothing");
        assert_eq!(decision.findings.len(), 1);
        assert_eq!(decision.findings[0].rule, None, "not one of the eighteen referential rules");
        assert_eq!(decision.findings[0].subject, "lettering speech");
        assert!(decision.findings[0].message.contains("overflow"), "{}", decision.findings[0].message);
        assert!(decision.findings[0].severity == vectorcraft_comic_model::Severity::Warning);
    }

    #[test]
    fn shrink_keeps_the_frame_and_reports_the_scale_the_engine_used() {
        let db = FontDb::global();
        let mut s = style();
        s.shrink_percent = 10.0;
        let decision = apply(db, &request(LONG, &s, tight(), OverflowPolicy::Shrink, &[])).expect("applied");
        let FitOutcome::Shrunk { fit_scale, fits } = decision.outcome else {
            panic!("shrink must report a scale, got {:?}", decision.outcome);
        };
        assert_eq!(decision.bounds, tight(), "shrink never moves the frame");
        assert!((0.1..1.0).contains(&fit_scale), "the engine scaled the type down: {fit_scale}");
        assert!(fits, "10% is small enough for these four lines");
        assert!(decision.findings.is_empty(), "a shrink that works is not a finding");
    }

    #[test]
    fn shrink_that_cannot_help_reports_instead_of_scaling_type_into_a_smudge() {
        let db = FontDb::global();
        let mut s = style();
        // An unbreakable word wider than the frame at any size the floor allows.
        s.shrink_percent = 90.0;
        let decision = apply(db, &request("Incomprehensibilities", &s, tight(), OverflowPolicy::Shrink, &[])).expect("applied");
        let FitOutcome::Shrunk { fit_scale, fits } = decision.outcome else {
            panic!("shrink must report a scale, got {:?}", decision.outcome);
        };
        assert!(!fits);
        assert!((fit_scale - 0.9).abs() < 1e-9, "the floor is the reported scale: {fit_scale}");
        assert_eq!(decision.findings.len(), 1);
        assert!(decision.findings[0].message.contains("90"), "{}", decision.findings[0].message);
    }

    #[test]
    fn resize_frame_grows_the_frame_about_its_centre() {
        let db = FontDb::global();
        let s = style();
        let before = tight();
        let decision = apply(db, &request(LONG, &s, before, OverflowPolicy::ResizeFrame, &[])).expect("applied");
        let FitOutcome::Resized { bounds } = decision.outcome else {
            panic!("resize must report the new box, got {:?}", decision.outcome);
        };
        assert_eq!(bounds, decision.bounds);
        assert!(decision.bounds.width() > before.width() && decision.bounds.height() > before.height());
        assert!(decision.measured.fits(), "the grown frame holds the text");
        assert!((decision.bounds.center().x - before.center().x).abs() < 1e-9, "a balloon grows about its centre");
        assert!((decision.bounds.center().y - before.center().y).abs() < 1e-9);
        assert!(decision.findings.is_empty(), "a resize that works is not a finding");
    }

    #[test]
    fn resize_frame_never_shrinks_a_frame_that_already_fits_the_text() {
        // The frame is wider than the text needs; `resize_frame` grows to fit, it does not tidy up.
        let db = FontDb::global();
        let s = style();
        let roomy = rect(50.0, 50.0, 260.0, 200.0);
        let decision = apply(db, &request("Hi!", &s, roomy, OverflowPolicy::ResizeFrame, &[])).expect("applied");
        assert_eq!(decision.outcome, FitOutcome::Fits { fit_scale: 1.0 });
        assert_eq!(decision.bounds, roomy);
    }

    #[test]
    fn resize_frame_that_would_overlap_another_object_is_reported_not_applied() {
        // Design §8.3: growing a frame that would collide with another panel or lettering object is
        // reported, not applied.
        let db = FontDb::global();
        let s = style();
        // The neighbouring balloon sits to the right of the tight frame, in the way of growing.
        let neighbour = rect(232.0, 140.0, 48.0, 32.0);
        let decision = apply(db, &request(LONG, &s, tight(), OverflowPolicy::ResizeFrame, &[neighbour])).expect("applied");
        assert_eq!(decision.outcome, FitOutcome::Blocked { obstacle: 0 });
        assert_eq!(decision.bounds, tight(), "blocked means the frame is exactly as it was");
        // The measurement describes the box that comes back, not the largest box the search tried:
        // a grow that was not applied cannot report where the text would have landed (round 1, F1).
        assert_eq!(
            decision.measured.frame,
            text_frame(decision.bounds, std::f64::consts::FRAC_1_SQRT_2, s.padding).expect("frame"),
            "the measurement is not in the returned box's frame"
        );
        assert_eq!(decision.findings.len(), 1);
        assert!(decision.findings[0].message.contains("another object"), "{}", decision.findings[0].message);
        assert!(decision.findings[0].message.contains("not applied"), "{}", decision.findings[0].message);
    }

    #[test]
    fn resize_frame_reports_when_no_frame_within_the_limits_holds_the_text() {
        let db = FontDb::global();
        let s = style();
        let mut req = request("Incomprehensibilities", &s, tight(), OverflowPolicy::ResizeFrame, &[]);
        req.limits = Limits::new(Size::new(48.0, 32.0), Size::new(60.0, 40.0)).expect("valid limits");
        let decision = apply(db, &req).expect("applied");
        assert_eq!(decision.outcome, FitOutcome::Unfittable);
        assert_eq!(decision.bounds, tight());
        assert_eq!(
            decision.measured.frame,
            text_frame(decision.bounds, std::f64::consts::FRAC_1_SQRT_2, s.padding).expect("frame"),
            "the measurement is not in the returned box's frame"
        );
        assert_eq!(decision.findings.len(), 1);
        assert!(decision.findings[0].message.contains("cannot be set"), "{}", decision.findings[0].message);
    }

    #[test]
    fn a_caption_grows_downward_from_its_top_edge() {
        let db = FontDb::global();
        let mut s = style();
        s.align = vectorcraft_comic_model::TextAlign::Left;
        let before = rect(60.0, 40.0, 90.0, 26.0);
        let mut req = request(LONG, &s, before, OverflowPolicy::ResizeFrame, &[]);
        req.anchor = ResizeAnchor::TopCentre;
        req.limits = Limits::new(Size::new(90.0, 26.0), Size::new(400.0, 200.0)).expect("valid limits");
        let decision = apply(db, &req).expect("applied");
        assert!(matches!(decision.outcome, FitOutcome::Resized { .. }));
        assert!((decision.bounds.y0 - before.y0).abs() < 1e-9, "the top edge stays put");
        assert!(decision.bounds.height() > before.height(), "the box grew downward");
    }

    #[test]
    fn an_out_of_range_shrink_percent_is_clamped_and_quoted_as_the_policy_applied() {
        // `AreaFit::parse` clamps into 10..=100, so the message and the plan must quote the clamped
        // floor rather than the style's raw number: a finding that said "the smallest size the
        // policy allows (1)" would name a policy the engine was never given (round 1, F2).
        let db = FontDb::global();
        let mut s = style();
        s.size = 200.0;
        s.leading = Some(240.0);
        for (given, applied) in [(1.0, 10.0), (0.0, 10.0), (-5.0, 10.0), (1.0e9, 100.0), (f64::NAN, 10.0)] {
            s.shrink_percent = given;
            let request = request("Incomprehensibilities", &s, tight(), OverflowPolicy::Shrink, &[]);
            if given.is_nan() {
                assert_eq!(
                    apply(db, &request).err(),
                    Some(FitError::NotFinite { field: "shrink percent" }),
                    "a NaN percent is a data bug, not a policy"
                );
                continue;
            }
            let decision = apply(db, &request).expect("applied");
            let FitOutcome::Shrunk { fit_scale, fits } = decision.outcome else {
                panic!("{given}: {:?}", decision.outcome);
            };
            assert!(!fits, "{given}: nothing at this size fits a {:?} frame", tight());
            assert!((fit_scale - applied / 100.0).abs() < 1e-9, "{given}: the plan reports {fit_scale}, the applied floor is {}", applied / 100.0);
            let quoted = format!("({}%)", (applied * 10.0).round() / 10.0);
            assert!(
                decision.findings.iter().any(|finding| finding.message.contains(&quoted)),
                "{given}: no finding quotes the floor that was applied ({quoted}): {:?}",
                decision.findings.iter().map(|finding| &finding.message).collect::<Vec<_>>()
            );
            assert!(!decision.findings.iter().any(|finding| finding.message.contains("NaN")), "{given}");
        }
    }

    #[test]
    fn a_non_finite_frame_is_rejected_before_anything_is_measured() {
        let db = FontDb::global();
        let s = style();
        let bad = Rect::new(0.0, 0.0, f64::INFINITY, 40.0);
        assert_eq!(apply(db, &request("Hi!", &s, bad, OverflowPolicy::Warn, &[])).err(), Some(FitError::NotFinite { field: "bounds" }));
    }

    #[test]
    fn collision_answers_the_same_question_the_planner_asks() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(collision(a, &[]), None);
        assert_eq!(collision(a, &[rect(10.0, 0.0, 5.0, 5.0)]), None, "touching is not a collision");
        assert_eq!(collision(a, &[rect(9.0, 0.0, 5.0, 5.0)]), Some(0));
    }

    #[test]
    fn a_point_placement_of_an_existing_box_is_its_centre_or_its_top() {
        let b = rect(10.0, 20.0, 40.0, 30.0);
        assert_eq!(Placement::of(b, ResizeAnchor::Centre).at, Point::new(30.0, 35.0));
        assert_eq!(Placement::of(b, ResizeAnchor::TopCentre).at, Point::new(30.0, 20.0));
    }
}
