//! Round-1 reviewer tests for W3 — `src/{fit,lettering}/**` (`lettering-engineer`).
//!
//! Written by `code-reviewer`, independently of the implementer's inline tests, aimed at the
//! behaviour the implementer's own tests do not pin. The `finding_*` tests are the reproduction for
//! a round-1 finding: each encodes what the design, the frozen schema or the module's own
//! documented contract requires, and each fails until the owner fixes it.
//!
//! Every test here runs with `CRAFT_FONTS_DIR` unset, which is the configuration the gate uses:
//! the comic faces are absent and the lettering must still lay out and report them.

use vectorcraft_comic_layout::fit::sizing::{Limits, Placement, ResizeAnchor, SizeRequest, size_to_text};
use vectorcraft_comic_layout::fit::{
    FitError, FitOutcome, FitRequest, FitTextStyle, Measured, apply, area_fit, measure, overflow_finding, text_frame,
};
use vectorcraft_comic_layout::lettering::outline::BalloonShape;
use vectorcraft_comic_layout::lettering::plan::{LetteringPlan, LetteringRequest, TextSource, new_balloon, refit_balloon};
use vectorcraft_comic_layout::lettering::style::style_of;
use vectorcraft_comic_layout::lettering::tail::{Tail, TailKind, attach_point, downward_probe, path_for, reachable_tip};
use vectorcraft_comic_layout::lettering::{LetteringKindExt, fonts};
use vectorcraft_comic_model::{LetteringKind, OverflowPolicy, Severity, TextAlign};
use vectorcraft_geom::hit::fill_contains;
use vectorcraft_geom::{Affine, FillRule, PathData, Point, Rect, Size, Vec2};
use vectorcraft_text::{AreaFit, FALLBACK_FAMILY, FontDb, FontMatch};

const KINDS: [LetteringKind; 6] = [
    LetteringKind::Speech,
    LetteringKind::Thought,
    LetteringKind::Narration,
    LetteringKind::Shout,
    LetteringKind::Whisper,
    LetteringKind::SoundEffect,
];

const SHAPES: [BalloonShape; 4] = [BalloonShape::Ellipse, BalloonShape::Cloud, BalloonShape::Box, BalloonShape::Burst];

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn source_for(kind: LetteringKind, text: &str) -> TextSource<'_> {
    if kind.is_spoken() { TextSource::Dialogue(text) } else { TextSource::Own(text) }
}

/// The findings that are not the CRAFT_FONTS-empty missing-face report.
fn not_font(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| !finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

fn font_reports(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

fn ballpark(text: &str) -> LetteringRequest<'_> {
    LetteringRequest::new(LetteringKind::Speech, TextSource::Dialogue(text)).placed_at(Point::new(200.0, 150.0)).with_tail(Point::new(160.0, 300.0))
}

/// Limits from a minimum and a maximum pair, with the implementer's own fallback for the pair the
/// tests build (which is well formed); `expect` is not available to a helper that is not itself a
/// `#[test]` (clippy.toml's `allow-expect-in-tests`).
fn limits_of(min: (f64, f64), max: (f64, f64)) -> Limits {
    Limits::new(Size::new(min.0, min.1), Size::new(max.0, max.1)).unwrap_or(Limits { min: Size::new(min.0, min.1), max: Size::new(max.0, max.1) })
}

fn fit_request<'a>(text: &'a str, style: &'a FitTextStyle, bounds: Rect, policy: OverflowPolicy, obstacles: &'a [Rect]) -> FitRequest<'a> {
    FitRequest {
        text,
        style,
        bounds,
        inner_scale: BalloonShape::Ellipse.inner_scale(),
        policy,
        limits: limits_of((48.0, 32.0), (360.0, 300.0)),
        anchor: ResizeAnchor::Centre,
        obstacles,
        subject: "lettering speech",
    }
}

// ---------------------------------------------------------------------------------------------
// Independent coverage: the geometry, the frame, the degenerate inputs
// ---------------------------------------------------------------------------------------------

#[test]
fn the_inner_rectangle_is_inside_every_outline_over_a_dense_grid() {
    // The implementer probes the frame's four corners and four edge midpoints at three boxes. A
    // scallop or a star can part company with the inscribed rectangle *between* those probes, so
    // this walks a grid, at boxes whose aspect ratio is far from the ones the table was measured
    // on. It is the promise `BalloonShape::inner_scale` makes, checked where it is weakest.
    let boxes = [
        rect(0.0, 0.0, 120.0, 90.0),
        rect(0.0, 0.0, 600.0, 18.0),
        rect(0.0, 0.0, 14.0, 420.0),
        rect(0.0, 0.0, 9.0, 9.0),
        rect(-1400.0, 620.0, 320.0, 240.0),
        rect(1.0e6, 1.0e6, 200.0, 120.0),
    ];
    for shape in SHAPES {
        for bounds in boxes {
            let outline = shape.outline(bounds);
            let bez = outline.to_bezpath();
            let inner = vectorcraft_comic_layout::fit::inner_rect(bounds, shape.inner_scale());
            for i in 0..=8 {
                for j in 0..=8 {
                    let x = inner.x0 + inner.width() * f64::from(i) / 8.0;
                    let y = inner.y0 + inner.height() * f64::from(j) / 8.0;
                    // The frame's edge itself may sit a hair outside where a curve is flattest, so
                    // the corners are checked inwards by a thousandth of the frame.
                    let t = 0.999;
                    let px = inner.center().x + (x - inner.center().x) * t;
                    let py = inner.center().y + (y - inner.center().y) * t;
                    assert!(
                        fill_contains(&bez, FillRule::NonZero, Point::new(px, py)),
                        "{shape:?}: ({px}, {py}) is inside the inner rectangle but outside the outline at {bounds:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_kind_plans_finite_positive_geometry_for_degenerate_inputs() {
    // Empty string, one character, an overlong word, a long sentence, an emoji, RTL and CJK — each
    // kind's plan must be a real, finite, positive box with the text frame inside it. A NaN here
    // would travel into `PathData` and into the document.
    let db = FontDb::global();
    let texts = [
        "",
        "A",
        "Unforgettable",
        "The storm broke over the harbour at dawn, and nobody spoke for a long while.",
        "😀",
        "مرحبا بالعالم",
        "日本語のテキスト",
    ];
    for kind in KINDS {
        for text in texts {
            let request = LetteringRequest::new(kind, source_for(kind, text)).placed_at(Point::new(200.0, 150.0)).with_tail(Point::new(120.0, 320.0));
            let plan = new_balloon(db, &request).unwrap_or_else(|error| panic!("{kind:?} {text:?}: {error}"));
            let bounds = plan.bounds;
            assert!(
                bounds.x0.is_finite() && bounds.y0.is_finite() && bounds.x1.is_finite() && bounds.y1.is_finite(),
                "{kind:?} {text:?}: {bounds:?}"
            );
            assert!(bounds.width() > 0.0 && bounds.height() > 0.0, "{kind:?} {text:?}: {bounds:?}");
            assert!(plan.text_frame.x0 >= bounds.x0 && plan.text_frame.y0 >= bounds.y0, "{kind:?} {text:?}: the frame left its box");
            assert!(plan.text_frame.x1 <= bounds.x1 && plan.text_frame.y1 <= bounds.y1, "{kind:?} {text:?}: the frame left its box");
            assert!(plan.text_frame.width() > 0.0 && plan.text_frame.height() > 0.0, "{kind:?} {text:?}: a degenerate frame");
            let frame = plan.frame.bounds().expect("the outline has geometry");
            assert!(frame.x0.is_finite() && frame.x1.is_finite() && frame.width() > 0.0, "{kind:?} {text:?}: {frame:?}");
            assert!(plan.text_bounds.x0.is_finite() && plan.text_bounds.y1.is_finite(), "{kind:?} {text:?}: {plan:?}", plan = plan.text_bounds);
            if let Some(tail) = &plan.tail {
                let path = tail.path.bounds().expect("a tail has geometry");
                assert!(path.width().is_finite() && path.height().is_finite(), "{kind:?} {text:?}: tail {path:?}");
            }
        }
    }
}

#[test]
fn a_huge_and_a_tiny_font_size_still_yield_a_finite_plan() {
    let db = FontDb::global();
    for size in [0.1, 4.0, 400.0, 40_000.0] {
        let mut request = ballpark("Look out!");
        request.style.text.size = size;
        request.style.text.leading = Some(size * 1.2);
        let plan = new_balloon(db, &request).unwrap_or_else(|error| panic!("size {size}: {error}"));
        assert!(plan.bounds.width().is_finite() && plan.bounds.height().is_finite(), "size {size}: {:?}", plan.bounds);
        assert!(plan.bounds.width() > 0.0 && plan.bounds.height() > 0.0, "size {size}: {:?}", plan.bounds);
        assert!(plan.text_bounds.x1.is_finite(), "size {size}: {:?}", plan.text_bounds);
    }
}

#[test]
fn the_padding_reaches_the_text_area_exactly_once() {
    // The frame is the shape's full inscribed rectangle — the padding is *not* taken off it — and
    // the layout inset moves the text in by exactly one padding. If the padding were applied twice
    // the left edge of a left-aligned line would sit at `x0 + 2 * padding`.
    let db = FontDb::global();
    let mut style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    style.align = TextAlign::Left;
    style.padding = 6.0;
    let frame = rect(400.0, 300.0, 120.0, 80.0);
    let measured = measure(db, "Harbour dawn", &style, &frame, AreaFit::None).expect("measurable");
    let line = measured.layout.lines.first().expect("a line");
    assert!(
        (line.x0 - (frame.x0 + style.padding)).abs() < 1e-6,
        "the line starts {} from the frame's left edge, not one padding ({}): {line:?}",
        line.x0 - frame.x0,
        style.padding
    );
    assert_eq!(measured.frame, frame, "the frame is returned as given, not already inset");
    assert_eq!(measured.inset, style.padding);
    // And `text_frame` keeps the shape's whole inscribed rectangle.
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    assert_eq!(text_frame(outer, 0.5, 6.0), Ok(rect(25.0, 25.0, 50.0, 50.0)));
}

#[test]
fn the_height_a_short_line_needs_does_not_jump_to_the_kinds_maximum() {
    // This is the bug the implementer says probing caught: the height the *text frame* needs was
    // compared against *box* heights, so a balloon jumped from its minimum to its maximum. One line
    // of 12 pt type needs about 39 points of ellipse, which is less than the kind's 44-point
    // minimum, so the smallest box on the preferred width is the minimum itself — exactly. A
    // division left out (or done twice) moves the number to 56 and this fails.
    let db = FontDb::global();
    let plan = new_balloon(db, &ballpark("Hi!")).expect("planned");
    let style = style_of(LetteringKind::Speech);
    assert!(plan.outcome.fits(), "{:?}", plan.outcome);
    assert!(plan.bounds.height() < style.limits.max.height, "the box jumped to the maximum: {:?}", plan.bounds);
    assert_eq!(plan.bounds.height(), style.limits.min.height, "one line fits the minimum box of {:?} exactly: {:?}", style.limits.min, plan.bounds);
    assert!(plan.bounds.width() >= style.limits.min.width);
    assert_eq!(plan.bounds.width(), style.limits.min.width, "and it does not grow for two letters: {:?}", plan.bounds);
    // The frame inside it is the same box scaled by the shape, so it is the frame the box promised.
    assert_eq!(plan.text_frame, text_frame(plan.bounds, BalloonShape::Ellipse.inner_scale(), style.text.padding).expect("frame"));
}

#[test]
fn a_tail_built_through_its_constructor_has_the_path_its_own_fields_imply() {
    // The derived-path claim, checked for every shape and every kind of tail, in every direction —
    // including the degenerate ones. `path_for` is the definition; a `Tail` that does not agree
    // with it has drifted.
    let bounds = rect(100.0, 100.0, 120.0, 80.0);
    let tips = [
        Point::new(160.0, 300.0),
        Point::new(160.0, -200.0),
        Point::new(-400.0, 140.0),
        Point::new(900.0, 140.0),
        bounds.center(),
        bounds.center() + Vec2::new(0.25, 0.25),
    ];
    for shape in SHAPES {
        for kind in [TailKind::Wedge, TailKind::Bubble] {
            for tip in tips {
                let tail = Tail::new(bounds, shape, kind, tip);
                assert_eq!(
                    tail.path,
                    path_for(kind, tail.attach, tail.tip),
                    "{shape:?} {kind:?} {tip:?}: the path is not derived from the three fields"
                );
                assert_eq!(tail.tip, reachable_tip(tail.attach, tip), "{shape:?} {kind:?} {tip:?}: the tip is not the reachable one");
                assert!(tail.is_attached_to(bounds, shape), "{shape:?} {kind:?} {tip:?}: not attached");
            }
        }
    }
}

#[test]
fn attachment_is_measured_against_the_path_not_the_box() {
    // `is_attached_to_outline` must be falsifiable: a tail whose base is a thousandth of a point
    // off the outline is not attached, and one on it is. If the test could not tell them apart it
    // would be decorative.
    let bounds = rect(100.0, 100.0, 120.0, 80.0);
    let outline = BalloonShape::Ellipse.outline(bounds);
    let tail = Tail::new(bounds, BalloonShape::Ellipse, TailKind::Wedge, Point::new(160.0, 300.0));
    assert!(tail.is_attached_to_outline(&outline));
    let nudged = Tail { attach: tail.attach + Vec2::new(0.25, 0.0), ..tail.clone() };
    assert!(!nudged.is_attached_to_outline(&outline), "a base a quarter point off the outline is not attached");
    let translated = Tail { attach: tail.attach, path: PathData::default(), ..tail.clone() };
    assert!(translated.is_attached_to_outline(&outline), "the check tests attachment, not the path");
}

#[test]
fn the_text_source_must_match_the_kind_for_every_kind_through_both_entry_points() {
    // Rule 8, exhaustively: all six kinds against both sources, so exactly the four spoken kinds
    // reject their own text and the two carrying their own reject the dialogue's — through
    // `new_balloon` *and* through `refit_balloon`, which is the path M1's step 5 takes and the one a
    // single-entry-point test would leave unguarded.
    let db = FontDb::global();
    let frame = rect(200.0, 160.0, 120.0, 80.0);
    for kind in KINDS {
        let expected = FitError::TextSourceMismatch { kind: kind.name(), spoken: kind.is_spoken() };
        for own in [true, false] {
            let source = if own { TextSource::Own("words") } else { TextSource::Dialogue("words") };
            let mismatched = own == kind.is_spoken();
            let request = LetteringRequest::new(kind, source).placed_at(Point::new(200.0, 150.0));
            let planned = new_balloon(db, &request).err();
            let refitted = refit_balloon(db, &request, frame).err();
            if mismatched {
                assert_eq!(planned.as_ref(), Some(&expected), "{kind:?} own={own} accepted through new_balloon");
                assert_eq!(refitted.as_ref(), Some(&expected), "{kind:?} own={own} accepted through refit_balloon");
            } else {
                assert_eq!(planned, None, "{kind:?} own={own} refused through new_balloon");
                assert_eq!(refitted, None, "{kind:?} own={own} refused through refit_balloon");
            }
        }
    }
}

#[test]
fn every_kind_gets_a_tail_from_a_plan_exactly_when_the_schema_requires_one() {
    // The table at the module head, checked at the *plan* (not the style) for all six kinds.
    let db = FontDb::global();
    for kind in KINDS {
        let mut request =
            LetteringRequest::new(kind, source_for(kind, "Hi!")).placed_at(Point::new(200.0, 150.0)).with_tail(Point::new(160.0, 300.0));
        request.overflow_policy = OverflowPolicy::default();
        let plan = new_balloon(db, &request).expect("planned");
        assert_eq!(plan.tail.is_some(), kind.is_spoken(), "{kind:?}: {:?}", plan.tail);
        assert!(plan.text_layout.font.is_some(), "{kind:?}: the record names a face");
    }
}

#[test]
fn the_missing_face_report_is_conditional_on_the_face_really_being_absent() {
    // The CRAFT_FONTS contract, asserted *both* ways: with the faces present there must be no
    // report at all (not merely "skip the assertion"), and with them absent exactly one report per
    // face, naming the face the kind's style asks for.
    let db = FontDb::global();
    for kind in KINDS {
        let style = style_of(kind);
        let face = style.face();
        let absent = !fonts::missing_faces(db, std::slice::from_ref(&face)).is_empty();
        let plan = new_balloon(db, &LetteringRequest::new(kind, source_for(kind, "Hi!")).placed_at(Point::new(200.0, 150.0))).expect("planned");
        let reported = font_reports(&plan);
        if absent {
            assert_eq!(reported.len(), 1, "{kind:?} asks for {face:?}: {reported:?}");
            assert!(reported[0].contains(&face.family), "{kind:?}: {reported:?}");
            assert!(reported[0].contains(&face.style), "{kind:?}: {reported:?}");
        } else {
            assert!(reported.is_empty(), "{kind:?} has its face, so nothing is reported: {reported:?}");
        }
        assert_eq!(plan.text_layout.font.as_deref(), Some(face.family.as_str()), "{kind:?}: the record names the face it asked for");
        assert!(plan.text_bounds.width() > 0.0, "{kind:?}: the layout ran either way");
    }
}

#[test]
fn every_finding_a_plan_or_a_policy_returns_is_a_warning_and_never_a_rule() {
    // §12.1 calls text overflow a deterministic *check*; a check reports. `Severity::Error` is the
    // schema's referential inconsistency, and none of these findings is one.
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let long = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";
    let tight = rect(180.0, 140.0, 48.0, 32.0);
    let covering = [rect(0.0, 0.0, 1000.0, 1000.0)];
    for (policy, obstacles) in [(OverflowPolicy::Warn, &[] as &[Rect]), (OverflowPolicy::Shrink, &[]), (OverflowPolicy::ResizeFrame, &covering)] {
        for bounds in [tight, rect(400.0, 400.0, 40.0, 30.0)] {
            let decision = apply(db, &fit_request(long, &style, bounds, policy, obstacles)).expect("applied");
            for finding in &decision.findings {
                assert_eq!(finding.severity, Severity::Warning, "{policy:?}: {finding:?}");
                assert_eq!(finding.rule, None, "overflow is not one of the eighteen rules: {finding:?}");
                assert_eq!(finding.subject, "lettering speech");
                assert!(!finding.message.contains("NaN"), "{policy:?}: {finding:?}");
            }
            assert_eq!(
                overflow_finding("lettering speech", decision.findings.first().map(|f| f.message.clone()).unwrap_or_default()).severity,
                Severity::Warning
            );
        }
    }
}

#[test]
fn an_obstacle_that_only_touches_the_frame_does_not_block_a_grow() {
    // A frame flush with its neighbour is a layout, not a collision — the same rule
    // `PanelRect::overlaps` uses — so `size_to_text` must be free to use a box that shares an edge
    // with an obstacle, and must refuse one that overlaps by a single point. The empty balloon is
    // the case where the first candidate width is the kind's own minimum, so the box stays exactly
    // where it was placed and the edge is the only thing that can decide the answer.
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let limits = Limits::new(Size::new(48.0, 44.0), Size::new(280.0, 220.0)).expect("valid limits");
    let place = Placement::new(Point::new(200.0, 150.0), ResizeAnchor::Centre);
    let min_box = rect(176.0, 128.0, 48.0, 44.0);
    let touching = [rect(min_box.x1, min_box.y0, 60.0, 44.0)];
    let request = SizeRequest { text: "", style: &style, inner_scale: BalloonShape::Ellipse.inner_scale(), limits, place, obstacles: &touching };
    let sized = size_to_text(db, &request).expect("sizable");
    assert!(sized.outcome.fits(), "a shared edge is not an overlap: {:?}", sized.outcome);
    assert_eq!(sized.bounds, min_box, "the minimum box touches the neighbour and nothing else: {:?}", sized.bounds);

    let overlapping = [rect(min_box.x1 - 1.0, min_box.y0, 61.0, 44.0)];
    let request = SizeRequest { obstacles: &overlapping, ..request };
    let sized = size_to_text(db, &request).expect("sizable");
    assert!(matches!(sized.outcome, FitOutcome::Blocked { obstacle: 0 }), "{:?}", sized.outcome);
    assert_eq!(sized.bounds, Rect::from_center_size(place.at, limits.max), "a blocked grow still comes back at the maximum, reported not applied");

    // And through the policy: a `resize_frame` that would collide is reported, and the frame the
    // caller gave is the frame it gets back.
    let long = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";
    let before = rect(180.0, 140.0, 48.0, 32.0);
    let covering = [rect(0.0, 0.0, 1000.0, 1000.0)];
    let blocked = apply(db, &fit_request(long, &style, before, OverflowPolicy::ResizeFrame, &covering)).expect("applied");
    assert!(matches!(blocked.outcome, FitOutcome::Blocked { .. }), "{:?}", blocked.outcome);
    assert_eq!(blocked.bounds, before, "a blocked grow is reported, not applied");
}

#[test]
fn the_search_stays_inside_the_limits_and_inside_the_obstacle_free_space() {
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let limits = Limits::new(Size::new(48.0, 44.0), Size::new(300.0, 220.0)).expect("valid limits");
    let request = SizeRequest {
        text: "The storm broke over the harbour at dawn.",
        style: &style,
        inner_scale: BalloonShape::Ellipse.inner_scale(),
        limits,
        place: Placement::new(Point::new(240.0, 200.0), ResizeAnchor::Centre),
        obstacles: &[],
    };
    let sized = size_to_text(db, &request).expect("sizable");
    assert!(sized.outcome.fits(), "{:?}", sized.outcome);
    assert!(sized.bounds.width() >= limits.min.width - 1e-9 && sized.bounds.width() <= limits.max.width + 1e-9, "{:?}", sized.bounds);
    assert!(sized.bounds.height() >= limits.min.height - 1e-9 && sized.bounds.height() <= limits.max.height + 1e-9, "{:?}", sized.bounds);
    assert_eq!(sized.measured.frame, text_frame(sized.bounds, request.inner_scale, style.padding).expect("frame"));
    // A top-anchored box keeps its top edge and grows down.
    let caption = SizeRequest {
        text: "Earlier that evening, in a quieter part of town.",
        style: &style,
        inner_scale: BalloonShape::Box.inner_scale(),
        limits: Limits::new(Size::new(90.0, 26.0), Size::new(420.0, 150.0)).expect("valid limits"),
        place: Placement::new(Point::new(230.0, 60.0), ResizeAnchor::TopCentre),
        obstacles: &[],
    };
    let sized = size_to_text(db, &caption).expect("sizable");
    assert!((sized.bounds.y0 - 60.0).abs() < 1e-9, "{:?}", sized.bounds);
    assert!((sized.bounds.center().x - 230.0).abs() < 1e-9, "{:?}", sized.bounds);
    assert_eq!(sized.measured.frame, text_frame(sized.bounds, caption.inner_scale, style.padding).expect("frame"));
}

// ---------------------------------------------------------------------------------------------
// Round-1 findings: each of these is the reproduction for a finding in the review record
// ---------------------------------------------------------------------------------------------

/// **F1.** `FitDecision::measured` is documented as "the measurement behind the decision, **in
/// `bounds`' frame**", and `LetteringPlan::text_bounds` as "where the text actually landed". On the
/// `resize_frame` path that cannot hold: `fit::policy::resize` returns `request.bounds` as the
/// decision's box but the measurement `size_to_text` took *inside the largest box the limits
/// allow*, because it was the maximally-grown box that was measured, not the one that comes back.
#[test]
fn finding_1_the_measurement_must_be_taken_in_the_frame_of_the_box_that_comes_back() {
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let long = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";
    let tight = rect(180.0, 140.0, 48.0, 32.0);
    let covering = [rect(0.0, 0.0, 1000.0, 1000.0)];
    let cases: [(&str, Rect, OverflowPolicy, &[Rect]); 5] = [
        ("fits", rect(100.0, 100.0, 260.0, 200.0), OverflowPolicy::Warn, &[]),
        ("warn", tight, OverflowPolicy::Warn, &[]),
        ("shrink", tight, OverflowPolicy::Shrink, &[]),
        ("resize ok", tight, OverflowPolicy::ResizeFrame, &[]),
        ("resize blocked", tight, OverflowPolicy::ResizeFrame, &covering),
    ];
    let mut wrong = vec![];
    for (name, bounds, policy, obstacles) in cases {
        let decision = apply(db, &fit_request(long, &style, bounds, policy, obstacles)).expect("applied");
        let expected = text_frame(decision.bounds, BalloonShape::Ellipse.inner_scale(), style.padding).expect("frame");
        if decision.measured.frame != expected {
            wrong.push(format!(
                "{name}: outcome {:?}, bounds {:?} but measured in {:?} (expected {:?})",
                decision.outcome, decision.bounds, decision.measured.frame, expected
            ));
        }
    }
    assert!(wrong.is_empty(), "the measurement is not in the returned box's frame — {wrong:#?}");

    // The same defect, seen through the plan the command layer writes: measuring the text in the
    // plan's own text frame must reproduce the plan's `text_bounds`.
    let mut request = ballpark(long);
    request.overflow_policy = OverflowPolicy::ResizeFrame;
    request.obstacles = &covering;
    let plan = refit_balloon(db, &request, tight).expect("refitted");
    assert!(matches!(plan.outcome, FitOutcome::Blocked { .. }), "{:?}", plan.outcome);
    let in_plan_frame = measure(db, long, &plan_style(&plan), &plan.text_frame, plan.fit).expect("measurable");
    assert_eq!(
        plan.text_bounds, in_plan_frame.layout.bounds,
        "the plan reports text at {:?} but its box holds the text at {:?}",
        plan.text_bounds, in_plan_frame.layout.bounds
    );
}

/// **F2.** `FitTextStyle::validate` is the documented guard that stops a bad style reaching a layout
/// call, but it never checks `shrink_percent`, which `area_fit` feeds straight to
/// `AreaFit::parse("shrinkText", Some(percent))`. `parse` clamps into `AreaFit::MIN_PERCENT..100`
/// and replaces a non-finite value with the default, so the engine and the finding's own message
/// disagree: the message quotes the *style's* number as "the smallest size the policy allows" while
/// the engine used a different one — and a `NaN` reaches user-visible text.
#[test]
fn finding_2_a_shrink_percent_outside_the_engines_range_is_neither_validated_nor_reported_honestly() {
    let db = FontDb::global();
    // A frame and a type size chosen so that no size the engine may go down to fits: the reported
    // scale is then the floor itself, and the message's parenthetical quotes the floor.
    let tight = rect(180.0, 140.0, 48.0, 32.0);
    let mut base = FitTextStyle::new("Comic Neue", "Regular", 200.0);
    base.leading = Some(240.0);
    let unbreakable = "Incomprehensibilities";
    let mut wrong = vec![];
    for percent in [f64::NAN, 1.0, 0.0, -5.0, 1e9] {
        let mut style = base.clone();
        style.shrink_percent = percent;
        let decision = match apply(db, &fit_request(unbreakable, &style, tight, OverflowPolicy::Shrink, &[])) {
            Ok(decision) => decision,
            Err(error) => {
                // Refusing the style is a fix, not a defect: `validate` is the documented guard.
                assert!(percent.is_nan() || !(10.0..=100.0).contains(&percent), "{percent}: an in-range style was refused: {error}");
                continue;
            }
        };
        let FitOutcome::Shrunk { fit_scale, fits } = decision.outcome else {
            wrong.push(format!("{percent}: {:?}", decision.outcome));
            continue;
        };
        let engine_floor = area_fit(OverflowPolicy::Shrink, &style).min_scale().expect("shrink text");
        for finding in &decision.findings {
            let message = &finding.message;
            if message.contains("NaN") || message.contains("inf") {
                wrong.push(format!("{percent}: the finding reads {message:?}"));
            }
        }
        if !fits {
            // The message says "at {}% — the smallest size the policy allows ({})": the first number
            // is what the engine reported, the second is the style's, and the two must be the same
            // floor or the finding states a policy the engine was never given.
            if (fit_scale - engine_floor).abs() >= 1e-9 {
                wrong.push(format!("{percent}: the plan reports fit_scale {fit_scale}, the engine's floor is {engine_floor}"));
            }
            let quoted = format!("({}%)", (engine_floor * 100.0 * 10.0).round() / 10.0);
            if !decision.findings.iter().any(|finding| finding.message.contains(&quoted)) {
                wrong.push(format!(
                    "{percent}: no finding quotes the floor the engine used ({quoted}): {:?}",
                    decision.findings.iter().map(|f| &f.message).collect::<Vec<_>>()
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "shrink_percent is not validated, and the report does not match the engine — {wrong:#?}");

    // `shrink_percent` is the one number `area_fit` feeds to `AreaFit::parse` without the style's
    // own guard having looked at it.
    let mut style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    style.shrink_percent = f64::NAN;
    assert!(measure(db, "Hi!", &style, &tight, AreaFit::None).is_err(), "a NaN shrink_percent passed the style's own validation");
}

/// **F3.** `LetteringPlan::moved_by` translates the plan's geometry but copies `outcome` verbatim,
/// and `FitOutcome::Resized { bounds }` names a box. After a move the outcome names the box the
/// plan used to be in, so `outcome` and `bounds` contradict each other.
#[test]
fn finding_3_a_move_must_not_leave_the_outcome_naming_the_box_it_left() {
    let db = FontDb::global();
    let plan = new_balloon(db, &ballpark("Hi!")).expect("planned");
    let before = plan.bounds;
    let longer = ballpark("Wait — did you hear that? It came from the cellar, I am sure of it.").with_policy(OverflowPolicy::ResizeFrame);
    let refit = refit_balloon(db, &longer, before).expect("refitted");
    assert!(matches!(refit.outcome, FitOutcome::Resized { .. }), "{:?}", refit.outcome);
    let moved = refit.moved_by(Vec2::new(120.0, -80.0)).expect("moved");
    let FitOutcome::Resized { bounds } = moved.outcome else {
        panic!("the moved plan lost its outcome: {:?}", moved.outcome);
    };
    assert_eq!(bounds, moved.bounds, "the outcome names the pre-move box {:?}; the plan is at {:?}", bounds, moved.bounds);
}

/// **F4.** `LetteringRequest::style` is the caller's, and `with_style` accepts any
/// `LetteringStyle`. A spoken kind planned with a style whose `tail` is `None` produces a plan with
/// `tail: null` and no finding at all, which the command layer then writes as a `speech` lettering
/// whose `tail_node_id` is `null` — which schema §6.4 forbids ("required for kinds that have a
/// tail"). The module reports the *missing target* but not the *missing tail*, which is the
/// asymmetry.
#[test]
fn finding_4_a_spoken_kind_with_a_tailless_style_must_be_reported() {
    let db = FontDb::global();
    let mut request = ballpark("Hi!");
    request.style.tail = None;
    let plan = new_balloon(db, &request).expect("planned");
    match plan.tail {
        Some(_) => {}
        None => assert!(
            not_font(&plan).iter().any(|message| message.to_lowercase().contains("tail")),
            "a {:?} lettering planned without a tail is reported by nothing: {:?}",
            plan.kind,
            not_font(&plan)
        ),
    }

    // The same, reached the way a project's `default_lettering_style` would reach it.
    let mut swapped = ballpark("Hi!");
    swapped.style = style_of(LetteringKind::Narration);
    let plan = new_balloon(db, &swapped).expect("planned");
    assert_eq!(plan.kind, LetteringKind::Speech);
    assert!(plan.tail.is_some() || !not_font(&plan).is_empty(), "a speech balloon wearing a caption's style: {:?}", not_font(&plan));
}

/// **F5.** `Measured::needed_height` is "the frame height this block needs". The engine reads
/// `overflow` and, for a frame too narrow for one character, places **no lines at all** — the state
/// `Measured::fits` documents as the answer it relies on. In that state `bottom()` falls back to
/// `layout.bounds.y1`, which is `0.0` in text space rather than the frame's own top, so the "height
/// needed" comes back *negative*. Nothing in `size_to_text` is fooled (it clamps `needed` into the
/// limits), but the accessor is public and its answer is not a height.
#[test]
fn finding_5_the_height_a_frame_that_places_no_lines_needs_is_not_negative() {
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    // The empty balloon is fine — the engine places one empty line for it.
    let frame = rect(400.0, 300.0, 80.0, 40.0);
    let measured = measure(db, "", &style, &frame, AreaFit::None).expect("measurable");
    assert!(
        measured.needed_height() >= 0.0,
        "an empty balloon needs {:?} points of height, with {} lines placed",
        measured.needed_height(),
        measured.layout.lines.len()
    );
    // This one is not.
    let narrow = measure(db, "M", &style, &rect(400.0, 300.0, 8.0, 200.0), AreaFit::None).expect("measurable");
    assert!(narrow.layout.lines.is_empty(), "the frame is not narrow enough to drop the line");
    assert!(
        narrow.needed_height() >= 0.0,
        "a frame too narrow for one character needs {:?} points of height (bottom {:?}, frame {:?})",
        narrow.needed_height(),
        narrow.bottom(),
        narrow.frame
    );
}

/// **F6.** `Tail::new` with a non-finite tip returns a tail that its own `is_attached_to` says is
/// **not attached to its balloon**. `attach_point`'s documented fallback is "the box's centre when
/// the outline has no segments at all" — but the same fallback is taken for a non-finite tip with a
/// perfectly good outline, and the box's centre is not a point of any of the four outlines. The
/// module head's claim is that "the base follows the balloon's edge wherever the balloon goes", and
/// `is_attached_to`'s that "a tail can never have been left behind". The implementer's own
/// `a_nan_target_is_carried_as_a_finite_fallback_not_as_a_broken_path` asserts the centre, so the
/// wrong answer is pinned by a test rather than merely produced.
#[test]
fn finding_6_a_tail_built_from_a_non_finite_target_must_still_touch_its_balloon() {
    let bounds = rect(100.0, 100.0, 120.0, 80.0);
    let mut detached = vec![];
    for shape in SHAPES {
        for kind in [TailKind::Wedge, TailKind::Bubble] {
            let tail = Tail::new(bounds, shape, kind, Point::new(f64::NAN, f64::NAN));
            assert!(tail.path.bounds().is_some(), "{shape:?} {kind:?}: the path is a path");
            if !tail.is_attached_to(bounds, shape) {
                detached.push(format!(
                    "{shape:?} {kind:?}: attach {:?} is {:?} from the outline",
                    tail.attach,
                    shape.outline(bounds).nearest(tail.attach).map(|(_, _, _, _, d)| d)
                ));
            }
        }
    }
    assert!(detached.is_empty(), "a tail the constructor built is detached from its balloon — {detached:#?}");
}

/// A one-line balloon must not jump to the maximum, and neither must a sentence: if the height the
/// *frame* needs is compared against *box* heights the candidate is too short, does not fit, and
/// the search takes the next candidate — which is the maximum.
#[test]
fn a_sentence_balloon_does_not_jump_to_the_kinds_maximum_height() {
    let db = FontDb::global();
    let style = style_of(LetteringKind::Speech);
    for text in ["Hi!", "Look out!", "The storm broke over the harbour at dawn, and nobody spoke."] {
        let plan = new_balloon(db, &ballpark(text)).expect("planned");
        assert!(plan.outcome.fits(), "{text:?}: {:?}", plan.outcome);
        assert!(
            plan.bounds.height() < style.limits.max.height,
            "{text:?}: the box jumped to the maximum height ({:?} of {:?})",
            plan.bounds,
            style.limits.max
        );
        assert!(plan.bounds.width() <= style.limits.max.width + 1e-9, "{text:?}: wider than the kind allows");
    }
    // A short line does not need the kind's maximum width either — only a sentence that genuinely
    // has to wrap across the widest box does.
    for text in ["Hi!", "Look out!"] {
        let plan = new_balloon(db, &ballpark(text)).expect("planned");
        assert!(plan.bounds.width() < style.limits.max.width, "{text:?}: the box is as wide as the kind allows ({:?})", plan.bounds);
    }
}
/// **F7 — preference.** The module doc says a project's lettering report and an export's font
/// warning "name the same thing", and `MissingFace` is "field for field what `text.missingFonts`
/// reports". The four fields are identical, but the *filter* is not: the exporter
/// (`crates/engine/src/cmd/fontfiles.rs:25-37`, `:111`) also leaves out the CSS generic families and
/// names longer than 256 bytes, and `missing_faces` filters only blank names. A project whose
/// `text_layout_settings.font` is `sans-serif` — which schema §6.6 permits, since the field is any
/// family name — is reported by the lettering and not by the exporter, which is the one thing the
/// mirroring claim rules out.
#[test]
fn finding_7_the_face_report_leaves_out_what_the_exporter_leaves_out() {
    let db = FontDb::global();
    let generics: Vec<fonts::Face> = [
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "ui-serif",
        "ui-sans-serif",
        "ui-monospace",
        "ui-rounded",
        "math",
        "emoji",
    ]
    .iter()
    .map(|family| fonts::Face::new(*family, fonts::REGULAR))
    .collect();
    let reported = fonts::missing_faces(db, &generics);
    assert!(
        reported.is_empty(),
        "the exporter omits the CSS generic families; the lettering report names {:?}",
        reported.iter().map(|face| face.family.clone()).collect::<Vec<_>>()
    );
    let long = "x".repeat(257);
    assert!(fonts::missing_faces(db, &[fonts::Face::new(long, fonts::REGULAR)]).is_empty(), "the exporter drops a family name longer than 256 bytes");
}

/// The attachment check is exact at any coordinate a document reaches, so a balloon may be dragged
/// from where a page starts into a shared pasteboard and the tail comes with it.
#[test]
fn attachment_holds_across_a_document_scale_move() {
    let db = FontDb::global();
    let plan = new_balloon(db, &ballpark("Look out!")).expect("planned");
    for delta in [Vec2::new(1.0e3, -2.0e3), Vec2::new(1.0e6, 1.0e6), Vec2::new(-1.0e6, 5.0e5)] {
        let moved = plan.moved_by(delta).expect("moved");
        let tail = moved.tail.as_ref().expect("a tail");
        assert!(tail.is_attached_to_outline(&moved.frame), "a move of {delta:?} left the tail behind");
        assert!(tail.is_attached_to(moved.bounds, BalloonShape::Ellipse), "a move of {delta:?} left the tail behind");
        assert!(moved.bounds.width().is_finite() && moved.bounds.width() > 0.0);
    }
}

/// A plan's style, rebuilt from the plan itself, so F1's second half measures with the same style
/// the plan was made with.
fn plan_style(plan: &LetteringPlan) -> FitTextStyle {
    let mut style = FitTextStyle::new(plan.text_layout.font.clone().unwrap_or_default(), "Regular", plan.text_layout.size.unwrap_or_default());
    style.leading = plan.text_layout.leading;
    style.tracking = plan.text_layout.tracking.unwrap_or_default();
    style.align = plan.text_layout.align.unwrap_or(TextAlign::Center);
    style.padding = plan.text_layout.padding.unwrap_or_default();
    style
}

/// The reviewer's other checks that are about geometry the plan hands on, spelled out so a later
/// edit cannot quietly change them.
#[test]
fn the_plan_hands_on_an_editable_closed_outline_and_a_translated_frame_path() {
    let db = FontDb::global();
    for kind in KINDS {
        let plan = new_balloon(db, &LetteringRequest::new(kind, source_for(kind, "Hi!")).placed_at(Point::new(200.0, 150.0))).expect("planned");
        assert!(plan.frame.is_closed(), "{kind:?}: the frame is not a closed editable path");
        assert!(plan.frame.anchor_count() >= 3, "{kind:?}");
        assert_eq!(plan.text_frame_path().bounds(), Some(plan.text_frame), "{kind:?}: the text object's frame is not the plan's");
        let delta = Vec2::new(-30.0, 45.0);
        let moved = plan.moved_by(delta).expect("moved");
        assert_eq!(moved.frame, plan.frame.transformed(Affine::translate(delta)), "{kind:?}: the outline was rebuilt, not moved");
        assert_eq!(moved.bounds, Rect::new(plan.bounds.x0 + delta.x, plan.bounds.y0 + delta.y, plan.bounds.x1 + delta.x, plan.bounds.y1 + delta.y));
        assert_eq!(moved.text_frame_path().bounds(), Some(moved.text_frame), "{kind:?}");
    }
}

/// `Measured` is the one place the read-back fields meet the frame, so it is where a wrong field
/// choice would show. Three of the four decisions the module makes are read-back fields; this pins
/// that the frame a measurement quotes is the frame it was measured in, and that `fits` is
/// `overflow` and nothing else.
#[test]
fn a_measurement_quotes_the_frame_it_was_measured_in_and_fits_is_overflow() {
    let db = FontDb::global();
    let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let frame = rect(400.0, 300.0, 90.0, 40.0);
    let measured = measure(db, "The harbour at dawn", &style, &frame, AreaFit::None).expect("measurable");
    assert_eq!(measured.frame, frame);
    assert_eq!(measured.fits(), !measured.layout.overflow);
    assert_eq!(measured.text_bounds(), measured.layout.bounds);
    // The bottom the height is derived from is the last line's baseline plus its descent, not the
    // ink bottom: the engine tests its own fit against exactly that.
    let last = measured.layout.lines.last().expect("a line");
    assert!((measured.bottom() - (last.baseline + last.descent)).abs() < 1e-12);
    assert!((measured.needed_height() - ((measured.bottom() - (frame.y0 + style.padding)) + 2.0 * style.padding)).abs() < 1e-12);
    let _: &Measured = &measured;
}

// ---------------------------------------------------------------------------------------------
// Round-2 checks: the round-1 fixes as new code, and the edges they introduced
// ---------------------------------------------------------------------------------------------

/// **F1, walked.** Every path that returns a `FitDecision` — `Fits`, `Warned`, `Shrunk` (fits and
/// does not), `Resized`, `Blocked` and `Unfittable` — must hand back a measurement taken in the
/// frame of the box it hands back, which is the field's own doc and what makes the plan's
/// `text_bounds` say where the text actually landed.
#[test]
fn r2_every_fit_decision_path_measures_in_the_frame_it_returns() {
    let db = FontDb::global();
    let plain = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let mut small = plain.clone();
    small.shrink_percent = 10.0;
    let mut high = plain.clone();
    high.shrink_percent = 90.0;
    let long = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";
    let tight = rect(180.0, 140.0, 48.0, 32.0);
    let covering = [rect(0.0, 0.0, 1000.0, 1000.0)];
    let mut unfittable = fit_request("Incomprehensibilities", &plain, tight, OverflowPolicy::ResizeFrame, &[]);
    unfittable.limits = limits_of((48.0, 32.0), (60.0, 40.0));
    let cases: [(FitRequest<'_>, &str); 7] = [
        (fit_request(long, &plain, rect(100.0, 100.0, 320.0, 260.0), OverflowPolicy::Warn, &[]), "fits"),
        (fit_request(long, &plain, tight, OverflowPolicy::Warn, &[]), "warned"),
        (fit_request(long, &small, tight, OverflowPolicy::Shrink, &[]), "shrunk and fitted"),
        (fit_request("Incomprehensibilities", &high, tight, OverflowPolicy::Shrink, &[]), "shrunk at the floor"),
        (fit_request(long, &plain, tight, OverflowPolicy::ResizeFrame, &[]), "resized"),
        (fit_request(long, &plain, tight, OverflowPolicy::ResizeFrame, &covering), "blocked"),
        (unfittable, "unfittable"),
    ];
    let mut wrong = vec![];
    for (request, name) in &cases {
        let decision = apply(db, request).expect("applied");
        let expected = text_frame(decision.bounds, request.inner_scale, request.style.padding).expect("frame");
        if decision.measured.frame != expected {
            wrong.push(format!("{name}: {:?} returns {:?} but measured in {:?}", decision.outcome, decision.bounds, decision.measured.frame));
        }
    }
    assert!(wrong.is_empty(), "a decision measures in a box it does not return — {wrong:#?}");
}

/// **The F4 fix's third path.** `ruler_findings` reports the two ways a spoken balloon loses its
/// tail — the style has none, or no target was given — and its comment says that is all of them.
/// `assemble` drops the tail a third way: a target that is *present* but not a finite point is
/// neither refused (unlike `request.at`, which `validate_point` refuses) nor reported. The plan is
/// a `speech` with `tail: null`, which schema §6.4 does not let the command layer write.
#[test]
fn r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported() {
    let db = FontDb::global();
    for target in [Point::new(f64::NAN, 10.0), Point::new(10.0, f64::INFINITY)] {
        for entry in ["new_balloon", "refit_balloon"] {
            let request = ballpark("Hi!").with_tail(target);
            let plan = match entry {
                "new_balloon" => new_balloon(db, &request).expect("planned"),
                _ => refit_balloon(db, &request, rect(180.0, 140.0, 120.0, 80.0)).expect("refitted"),
            };
            assert_eq!(plan.kind, LetteringKind::Speech);
            assert!(plan.tail.is_none(), "{entry} {target:?}: a non-finite target still made a tail");
            assert!(
                !not_font(&plan).is_empty(),
                "{entry} {target:?}: a spoken plan with no tail and no finding — the command layer writes a speech with no tail_node_id"
            );
        }
    }
}

/// The five `FitOutcome` predicates, checked against the plan for the cases the implementer's tests
/// do not reach: a `Shrunk` that only just fits, and a `Resized` that is reported as `Resized`.
#[test]
fn fit_outcomes_agree_with_the_engine_and_with_the_box_that_comes_back() {
    let db = FontDb::global();
    let mut style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
    let long = "The storm broke over the harbour at dawn, and nobody spoke for a long while.";
    style.shrink_percent = 10.0;
    let shrunk = apply(db, &fit_request(long, &style, rect(180.0, 140.0, 48.0, 32.0), OverflowPolicy::Shrink, &[])).expect("applied");
    let FitOutcome::Shrunk { fit_scale, fits } = shrunk.outcome else {
        panic!("{:?}", shrunk.outcome);
    };
    assert!(fits && shrunk.measured.fits(), "the engine fitted it at {fit_scale}: {:?}", shrunk.measured.layout.overflow);
    assert!((shrunk.measured.layout.fit_scale - fit_scale).abs() < 1e-12, "the outcome quotes a scale the engine did not report");
    assert_eq!(shrunk.bounds, rect(180.0, 140.0, 48.0, 32.0), "shrink never moves the frame");

    let resized = apply(db, &fit_request(long, &style, rect(180.0, 140.0, 48.0, 32.0), OverflowPolicy::ResizeFrame, &[])).expect("applied");
    assert_eq!(resized.outcome, FitOutcome::Resized { bounds: resized.bounds }, "{:?}", resized.outcome);
    assert!(resized.measured.fits(), "a grown frame holds the text: {:?}", resized.measured.layout.overflow);
    assert!(resized.findings.is_empty(), "{:?}", resized.findings);
}

// ---------------------------------------------------------------------------------------------
// Round-3 checks: the round-2 fixes as new code
// ---------------------------------------------------------------------------------------------

/// **R2-F2.** The **kind** decides whether there is a tail; the style decides only which form. A
/// `narration` or a `sound_effect` wearing a *spoken* kind's style — the case round 2 found the old
/// comment denying while the code allowed it — gets `tail: None` even when a target is given, through
/// both entry points, and is reported for nothing: a caption has nobody to point at, so nothing is
/// missing. `every_kind_gets_a_tail_from_a_plan_exactly_when_the_schema_requires_one` above plans
/// each kind in its **own** style, where a caption's style already carries no tail, so it cannot tell
/// the kind's gate from the style's; this is the test that can.
#[test]
fn r3_a_kind_with_nobody_to_point_at_gets_no_tail_whatever_its_style_says() {
    let db = FontDb::global();
    for kind in [LetteringKind::Narration, LetteringKind::SoundEffect] {
        let text = if kind == LetteringKind::SoundEffect { "KRAK" } else { "Later…" };
        for entry in ["new_balloon", "refit_balloon"] {
            let request = LetteringRequest::new(kind, TextSource::Own(text))
                .placed_at(Point::new(230.0, 60.0))
                .with_style(style_of(LetteringKind::Speech))
                .with_tail(Point::new(160.0, 300.0));
            assert!(request.style.tail.is_some(), "{kind:?}: the style being worn does name a tail");
            let plan = match entry {
                "new_balloon" => new_balloon(db, &request).expect("planned"),
                _ => refit_balloon(db, &request, rect(180.0, 140.0, 240.0, 120.0)).expect("refitted"),
            };
            assert!(plan.tail.is_none(), "{entry} {kind:?}: a kind with nobody to point at was given a tail");
            assert!(not_font(&plan).is_empty(), "{entry} {kind:?}: nothing is missing, so nothing is reported: {:?}", not_font(&plan));
        }
    }
    // The gate is the kind and not the style: the mirror image — a *spoken* kind wearing a caption's
    // style, which has no tail to give — is reported rather than silently tailless.
    let spoken = LetteringRequest::new(LetteringKind::Speech, TextSource::Dialogue("Hi!"))
        .placed_at(Point::new(200.0, 150.0))
        .with_style(style_of(LetteringKind::Narration))
        .with_tail(Point::new(160.0, 300.0));
    let plan = new_balloon(db, &spoken).expect("planned");
    assert!(plan.tail.is_none());
    assert!(not_font(&plan).iter().any(|message| message.contains("tail")), "{:?}", not_font(&plan));
    // And the *form* still comes from the style for a spoken kind that keeps its tail: a thought
    // balloon planned in its own style carries bubbles, and the same balloon wearing a shout's style
    // carries that style's wedge — the kind gates presence, the style the form.
    let thought = LetteringRequest::new(LetteringKind::Thought, TextSource::Dialogue("Maybe…"))
        .placed_at(Point::new(200.0, 150.0))
        .with_tail(Point::new(160.0, 300.0));
    assert_eq!(new_balloon(db, &thought).expect("planned").tail.map(|tail| tail.kind), Some(TailKind::Bubble));
    let wearing_a_shout = thought.with_style(style_of(LetteringKind::Shout));
    assert_eq!(new_balloon(db, &wearing_a_shout).expect("planned").tail.map(|tail| tail.kind), Some(TailKind::Wedge));
}

/// **R2-F1's message.** Three ways a spoken balloon ends up with no tail, each named so a creator
/// knows what to change: the style has no tail to give, no target was given, or the target is not a
/// finite point. The finding is gated on the tail the plan **has** — the same `tail_spec` `assemble`
/// builds it from — so the three arms of the message are the three states, and each says which one.
/// The two round-2 halves (a non-finite target is reported, not refused) are asserted by
/// `r2_a_tail_target_that_is_not_a_point_is_neither_refused_nor_reported` above; this is the half
/// that says *which reason*, and that the three do not share one message.
#[test]
fn r3_the_missing_tail_finding_names_the_reason_it_is_missing_for() {
    let db = FontDb::global();
    let cases: [(LetteringRequest<'_>, &str); 3] = [
        // The style has no tail to give a spoken lettering.
        (ballpark("Hi!").with_style(style_of(LetteringKind::Narration)), "no tail to give"),
        // No target at all.
        (LetteringRequest::new(LetteringKind::Speech, TextSource::Dialogue("Hi!")).placed_at(Point::new(200.0, 150.0)), "no tail target was given"),
        // A target that is present but is not a point.
        (ballpark("Hi!").with_tail(Point::new(f64::NAN, 10.0)), "not a finite point"),
    ];
    for (request, expected) in &cases {
        for entry in ["new_balloon", "refit_balloon"] {
            let plan = match entry {
                "new_balloon" => new_balloon(db, request).expect("planned"),
                _ => refit_balloon(db, request, rect(180.0, 140.0, 240.0, 120.0)).expect("refitted"),
            };
            let findings = not_font(&plan);
            let about_the_tail: Vec<&String> = findings.iter().filter(|message| message.contains("tail")).collect();
            assert!(plan.tail.is_none(), "{entry}: a spoken kind planned with no usable tail");
            assert_eq!(about_the_tail.len(), 1, "{entry} {expected}: expected one tail finding, got {findings:?}");
            assert!(about_the_tail[0].contains(*expected), "{entry}: the finding does not name the reason {expected:?}: {}", about_the_tail[0]);
            // The other two reasons are not named: one reason per plan, or the message is not
            // telling the creator what to fix.
            for (_, other) in cases.iter().filter(|(_, reason)| reason != expected) {
                assert!(!about_the_tail[0].contains(*other), "{entry}: {expected:?} also names {other:?}: {}", about_the_tail[0]);
            }
            // And it says what the plan costs the record, not only what is missing.
            assert!(about_the_tail[0].contains("tail_node_id"), "{entry}: {}", about_the_tail[0]);
        }
    }
    // The control: a spoken kind planned with a usable tail has no tail finding, so the three above
    // are not a finding that is always there.
    let plan = new_balloon(db, &ballpark("Hi!")).expect("planned");
    assert!(plan.tail.is_some());
    assert!(!not_font(&plan).iter().any(|message| message.contains("tail")), "{:?}", not_font(&plan));
}

/// **The F6 pin's other half.** `attach_point`'s guard is `tip.x.is_finite() && tip.y.is_finite()` —
/// an `&&`, not an `||` — so a target with *one* unusable coordinate takes the downward probe too.
/// The implementer's pin uses `(NaN, NaN)`, which a `||` guard would send to the probe as well (both
/// coordinates non-finite is `false || false`), so it cannot tell the two operators apart; this one
/// can, and it fails under an `||` mutation.
#[test]
fn r3_a_target_with_one_unusable_coordinate_takes_the_same_downward_probe() {
    let bounds = rect(100.0, 100.0, 120.0, 80.0);
    let probe = downward_probe(bounds);
    for target in [Point::new(f64::NAN, 10.0), Point::new(10.0, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NAN), Point::new(160.0, f64::NAN)]
    {
        for shape in SHAPES {
            let outline = shape.outline(bounds);
            let expected = outline.nearest(probe).map(|(_, _, _, point, _)| point).expect("an outline has segments");
            let attach = attach_point(&outline, target, bounds);
            assert_eq!(attach, expected, "{shape:?} {target:?}: a target with one unusable coordinate did not become the downward probe");
            assert!(
                outline.nearest(attach).is_some_and(|(_, _, _, _, distance)| distance < 1e-6),
                "{shape:?} {target:?}: the base is off the outline"
            );
        }
    }
    // Through the constructor a caller actually uses, for all four shapes and both kinds.
    for shape in SHAPES {
        for kind in [TailKind::Wedge, TailKind::Bubble] {
            let tail = Tail::new(bounds, shape, kind, Point::new(f64::NAN, 10.0));
            assert!(tail.is_attached_to(bounds, shape), "{shape:?} {kind:?}: one unusable coordinate detached the tail");
            assert_ne!(tail.attach, bounds.center(), "{shape:?} {kind:?}: the base is the box's centre");
        }
    }
}

/// **R2-F3.** The face report's filter is the exporter's, **term for term**
/// (`crates/engine/src/cmd/fontfiles.rs:111`): a blank *family* is skipped, a CSS generic family is
/// skipped, and a family or a style over 256 bytes is skipped — and nothing else. In particular the
/// exporter does **not** test the style for emptiness, so a face with a family and a blank style is
/// one `text.missingFonts` reports, and a report that claims to mirror it must report it too. The
/// first F7 fix filtered a blank style and disagreed with the exporter on exactly the input the
/// mirroring claim is about; `finding_7_...` above covers the generics and the length caps, and this
/// covers the term that was dropped.
#[test]
fn r3_the_face_report_filters_on_the_exporters_terms_and_no_more() {
    let db = FontDb::global();
    // A blank family is not a font, whatever the style says.
    for style in [fonts::REGULAR, "", "  "] {
        let faces = [fonts::Face::new("", style), fonts::Face::new("   ", style)];
        assert!(fonts::missing_faces(db, &faces).is_empty(), "an empty family is not a font (style {style:?})");
    }
    // A blank style is a style no face has, so the family's own default stands in — and the exporter
    // reports it, so this does too, naming the face exactly as written.
    for family in ["No Such Lettering Face", FALLBACK_FAMILY] {
        let reported = fonts::missing_faces(db, &[fonts::Face::new(family, "")]);
        assert_eq!(reported.len(), 1, "{family} with a blank style is what the exporter reports: {reported:?}");
        assert_eq!((reported[0].family.as_str(), reported[0].style.as_str()), (family, ""));
        assert_ne!(reported[0].status, FontMatch::Exact);
        assert!(reported[0].resolved.is_some(), "the face that stands in is named");
    }
    // Whitespace is not emptiness: the exporter tests `style.len()`, not `style.trim()`, so a
    // whitespace-only style is reported as written and never silently dropped here.
    let spaced = fonts::missing_faces(db, &[fonts::Face::new("No Such Lettering Face", "  ")]);
    assert_eq!(spaced.len(), 1, "{spaced:?}");
    assert_eq!(spaced[0].style, "  ");
}
