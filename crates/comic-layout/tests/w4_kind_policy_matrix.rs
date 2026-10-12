//! M2 W4, criterion 2 — *"every lettering kind renders, wraps, detects overflow and applies its
//! policy"* — checked at the criterion's own wording, over **every kind × every policy**.
//!
//! # What "renders" can mean here, said rather than implied
//!
//! `comic-layout` computes a plan and paints nothing, so **every cell of this matrix is verified at
//! record level**: the box, the closed editable frame path, the tail rule, the frame the text flows
//! in, the `AreaFit`, the `OverflowPolicy` and the findings — with the words actually laid out by
//! `vectorcraft_text` and that engine's own answers read back (`TextLayout.overflow`, `.lines`,
//! `.fit_scale`, `.bounds`). The wrapping and overflow halves are therefore checked against the real
//! layout engine rather than restated from this crate's arithmetic.
//!
//! What is **not** verified here is a single pixel. Whether the faces draw the right shapes is
//! criterion 6's golden render, and it is deliberately font-dependent (M2 §4.1) — so no cell in this
//! file claims more than record level, and no kind has a cell that claims less.
//!
//! `CRAFT_FONTS` is absent in the gate's configuration, so the same plans also carry the missing-face
//! report criterion 5 is about; the helpers here separate it from every other finding so a test
//! about overflow is not accidentally a test about fonts.

use vectorcraft_comic_layout::fit::{FitError, FitOutcome, area_fit, measure, text_frame};
use vectorcraft_comic_layout::lettering::LETTERING_KINDS;
use vectorcraft_comic_layout::lettering::plan::{LetteringPlan, LetteringRequest, TextSource, refit_balloon};
use vectorcraft_comic_layout::lettering::style::style_of;
use vectorcraft_comic_model::{LetteringKind, OverflowPolicy};
use vectorcraft_geom::{Point, Rect};
use vectorcraft_text::{AreaFit, FontDb};

const POLICIES: [OverflowPolicy; 3] = [OverflowPolicy::Warn, OverflowPolicy::Shrink, OverflowPolicy::ResizeFrame];

/// A sentence that cannot be set on one line of any of the six kinds.
const SENTENCE: &str = "Wait — did you hear that? It came from the cellar, I am sure of it.";

/// One short line: the text every kind's own minimum box is sized for.
const SHORT: &str = "Hi!";

fn source_for(kind: LetteringKind, text: &str) -> TextSource<'_> {
    if kind.is_spoken() { TextSource::Dialogue(text) } else { TextSource::Own(text) }
}

/// The kind's own smallest box, at a point on the page.
fn min_box_of(kind: LetteringKind) -> Rect {
    let min = style_of(kind).limits.min;
    Rect::new(120.0, 90.0, 120.0 + min.width, 90.0 + min.height)
}

/// The kind's own largest box, at a point on the page.
fn max_box_of(kind: LetteringKind) -> Rect {
    let max = style_of(kind).limits.max;
    Rect::new(120.0, 90.0, 120.0 + max.width, 90.0 + max.height)
}

fn request_for(kind: LetteringKind, text: &str, policy: OverflowPolicy) -> LetteringRequest<'_> {
    let request = LetteringRequest::new(kind, source_for(kind, text)).placed_at(Point::new(200.0, 150.0)).with_policy(policy);
    // A spoken kind must point at someone; a caption and a sound effect have nobody to point at.
    if kind.is_spoken() { request.with_tail(Point::new(160.0, 320.0)) } else { request }
}

/// The missing-face report, which the `CRAFT_FONTS`-empty configuration is *expected* to produce.
fn face_reports(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

/// Every finding that is not the missing-face report.
fn other_findings(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| !finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

/// The plan for `kind`, reported to the caller: the `#[test]` bodies are where a failure is allowed
/// to be a panic (`clippy.toml`'s `allow-panic-in-tests`), not the helpers.
fn plan_of(kind: LetteringKind, text: &str, policy: OverflowPolicy, bounds: Rect) -> Result<LetteringPlan, FitError> {
    refit_balloon(FontDb::global(), &request_for(kind, text, policy), bounds)
}

/// Every kind, in every policy, plans a closed editable frame, a text frame inside it, a laid-out
/// block and the tail its kind is entitled to.
///
/// The text fits here (one short line in the box the kind's own minimum is measured against), so
/// the three policies agree on the outcome and the differences this test is not about cannot hide
/// one kind behind another.
#[test]
fn every_kind_renders_a_closed_frame_and_a_text_block_under_every_policy() {
    let db = FontDb::global();
    for kind in LETTERING_KINDS {
        let style = style_of(kind);
        for policy in POLICIES {
            let plan = plan_of(kind, SHORT, policy, min_box_of(kind)).expect("planned");
            let cell = format!("{kind:?} {policy:?}");
            assert_eq!((plan.kind, plan.overflow_policy), (kind, policy), "{cell}");
            // The record's own setting decides the fit mechanism, and the plan carries it: `shrink`
            // is the one policy that asks the engine to scale the type.
            assert_eq!(plan.fit, area_fit(policy, &style.text), "{cell}");
            assert_eq!(plan.text_layout.fit_to_frame, matches!(policy, OverflowPolicy::Shrink), "{cell}");
            assert!(plan.fits(), "{cell}: {:?}", plan.outcome);
            assert_eq!(plan.outcome, FitOutcome::Fits { fit_scale: 1.0 }, "{cell}");
            // The editable outline a creator works with, and the area text object's own frame.
            assert!(plan.frame.is_closed(), "{cell}: the balloon is not a closed outline");
            assert!(plan.frame.anchor_count() >= 4, "{cell}: {} anchors", plan.frame.anchor_count());
            assert!(
                plan.text_frame.x0 >= plan.bounds.x0
                    && plan.text_frame.y0 >= plan.bounds.y0
                    && plan.text_frame.x1 <= plan.bounds.x1
                    && plan.text_frame.y1 <= plan.bounds.y1,
                "{cell}: the text frame is not inside the balloon: {:?} vs {:?}",
                plan.text_frame,
                plan.bounds
            );
            // The words are laid out: a finite, positive block from the engine, not a placeholder.
            assert!(plan.text_bounds.width() > 0.0 && plan.text_bounds.height() > 0.0, "{cell}: {:?}", plan.text_bounds);
            assert!(plan.text_bounds.width().is_finite() && plan.text_bounds.height().is_finite(), "{cell}");
            // The kind's own tail rule: exactly the kinds with someone to point at.
            assert_eq!(plan.tail.is_some(), kind.is_spoken(), "{cell}: the tail rule is not the schema's");
            assert!(other_findings(&plan).is_empty(), "{cell}: nothing else to report: {:?}", other_findings(&plan));
            assert_eq!(face_reports(&plan).len(), usize::from(face_is_absent(db, &style.face())), "{cell}");
        }
    }
}

/// Whether `face` is absent exactly as named in this build — read from the font database rather than
/// from the build's configuration, so the assertion above is a biconditional and not a restatement.
fn face_is_absent(db: &FontDb, face: &vectorcraft_comic_layout::lettering::Face) -> bool {
    vectorcraft_comic_layout::lettering::missing_faces(db, std::slice::from_ref(face)).len() == 1
}

/// Every kind wraps a sentence it is given into lines, and reports the overflow when the frame is
/// narrower than one of its own words.
///
/// Two regimes, both measured rather than assumed, because the criterion says "wraps, detects
/// overflow" and the engine's answer to them is what a policy is a decision about:
///
/// * **A frame that holds a word** (here, twelve of the kind's own type sizes wide) wraps the
///   sentence into lines, and every byte of the text is on one of them, in order, ending at the last
///   byte. No line reaches past the frame by more than one character — the allowance the engine's own
///   behaviour needs, because a word too long for the frame is broken **between characters**
///   (`fit::Measured::fits`) rather than overflowing it.
/// * **A frame narrower than one of its words** — the kind's own minimum box is exactly that — sets
///   the characters that fit on **one** line and sets `overflow`, dropping the rest. That is the state
///   `warn` reports, `shrink` scales the type out of, and `resize_frame` grows the frame out of; it is
///   not a silent truncation, and the plan-level half of it is
///   `every_kind_detects_the_overflow_and_applies_the_policy_it_was_given`.
#[test]
fn every_kind_wraps_a_sentence_into_lines_and_reports_what_a_too_narrow_frame_cannot_hold() {
    let db = FontDb::global();
    for kind in LETTERING_KINDS {
        let style = style_of(kind);
        // A square box twenty of the kind's own type sizes on a side. Not a constant of the layout:
        // it is chosen to be big enough for every one of the six to place the whole sentence and
        // small enough that none of them places it on one line, and both of those are asserted below
        // rather than assumed — the shape's own inner scale makes the frame the text sees much
        // smaller than this box (0.51 for a burst, 0.99 for a caption).
        let wide_box = Rect::new(120.0, 90.0, 120.0 + style.text.size * 20.0, 90.0 + style.text.size * 20.0);
        let wide = text_frame(wide_box, style.shape.inner_scale(), style.text.padding).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let measured = measure(db, SENTENCE, &style.text, &wide, AreaFit::None).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let lines = &measured.layout.lines;
        assert!(lines.len() > 1, "{kind:?}: a sentence set on one line in a {}pt frame", wide.width());
        assert_eq!(lines.first().map(|line| line.start), Some(0), "{kind:?}: the first line does not start at the first byte");
        assert_eq!(lines.last().map(|line| line.end), Some(SENTENCE.len()), "{kind:?}: the last line does not end at the last byte");
        assert!(!measured.layout.overflow, "{kind:?}: a frame that holds several words reported an overflow");
        for (index, line) in lines.iter().enumerate() {
            assert!(
                line.x1 - line.x0 <= wide.width() + style.text.size,
                "{kind:?}: line {index} is {}pt in a {}pt frame",
                line.x1 - line.x0,
                wide.width()
            );
            // Every line, and the line after it, leave no byte of the sentence unaccounted for.
            if let Some(next) = lines.get(index + 1) {
                assert!(next.start <= line.end + 1, "{kind:?}: line {index} ends at {} and the next begins at {}", line.end, next.start);
                assert!(next.end > line.end, "{kind:?}: line {index} is empty or out of order");
            }
        }

        // The kind's own minimum box is narrower than one word of its own type, so the engine places
        // one line of what fits and says so — never text beyond the frame.
        let narrow_box = min_box_of(kind);
        let narrow = text_frame(narrow_box, style.shape.inner_scale(), style.text.padding).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let too_narrow = measure(db, SENTENCE, &style.text, &narrow, AreaFit::None).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert_eq!(too_narrow.layout.lines.len(), 1, "{kind:?}: a one-word frame placed {} lines", too_narrow.layout.lines.len());
        assert!(too_narrow.layout.overflow, "{kind:?}: the sentence did not fit and nothing reported it");
        assert!(too_narrow.layout.bounds.width() <= narrow.width() + style.text.size, "{kind:?}: text was placed outside the frame");
        for line in &too_narrow.layout.lines {
            assert!(line.x1 - line.x0 <= narrow.width() + style.text.size, "{kind:?}");
        }

        // And through a plan, on the field a creator sees: in the same frame the sentence is several
        // lines tall where one short line is one line tall, and the plan's `text_bounds` is exactly
        // the block the engine measured in the plan's own text frame.
        let one = plan_of(kind, SHORT, OverflowPolicy::Warn, wide_box).expect("planned");
        let many = plan_of(kind, SENTENCE, OverflowPolicy::Warn, wide_box).expect("planned");
        assert!(one.fits() && many.fits(), "{kind:?}: {:?} / {:?}", one.outcome, many.outcome);
        assert_eq!(many.text_bounds, measured.layout.bounds, "{kind:?}: the plan reports a block the engine did not measure");
        assert!(
            many.text_bounds.height() >= one.text_bounds.height() * 1.5,
            "{kind:?}: {}pt tall for a sentence and {}pt for one line",
            many.text_bounds.height(),
            one.text_bounds.height()
        );
        assert!(one.text_bounds.height() <= many.text_bounds.height(), "{kind:?}");
    }
}

/// Every kind reports the overflow and applies the policy the record carries.
///
/// This is the criterion's second half, cell by cell. `warn` changes nothing and says so; `shrink`
/// asks the engine to scale the type and reports only when even the floor was not enough;
/// `resize_frame` grows the frame until the text fits, inside the kind's own limits.
#[test]
fn every_kind_detects_the_overflow_and_applies_the_policy_it_was_given() {
    for kind in LETTERING_KINDS {
        let smallest = min_box_of(kind);
        let style = style_of(kind);

        let warned = plan_of(kind, SENTENCE, OverflowPolicy::Warn, smallest).expect("planned");
        assert_eq!(warned.outcome, FitOutcome::Warned, "{kind:?}: a sentence in a one-line box is not an overflow?");
        assert!(!warned.fits(), "{kind:?}");
        assert_eq!(warned.bounds, smallest, "{kind:?}: `warn` changed the frame it was given");
        assert_eq!(warned.fit, AreaFit::None, "{kind:?}: `warn` asked the engine to fit the text");
        assert_eq!(other_findings(&warned).len(), 1, "{kind:?}: {:?}", other_findings(&warned));
        assert!(other_findings(&warned)[0].contains("overflow"), "{kind:?}: {:?}", other_findings(&warned));

        let shrunk = plan_of(kind, SENTENCE, OverflowPolicy::Shrink, smallest).expect("planned");
        let FitOutcome::Shrunk { fit_scale, fits } = shrunk.outcome else {
            panic!("{kind:?}: `shrink` resolved as {:?}", shrunk.outcome);
        };
        assert!(fit_scale > 0.0 && fit_scale <= 1.0, "{kind:?}: the engine scaled the type by {fit_scale}");
        assert_eq!(shrunk.fit, AreaFit::ShrinkText { min_percent: style.text.shrink_percent }, "{kind:?}");
        // The engine's own answer decides whether there is anything left to report, and the plan
        // takes that answer rather than its own guess.
        assert_eq!(shrunk.fits(), fits, "{kind:?}");
        assert_eq!(other_findings(&shrunk).is_empty(), fits, "{kind:?}: {:?}", other_findings(&shrunk));
        assert_eq!(shrunk.bounds, smallest, "{kind:?}: `shrink` grows the frame, which is `resize_frame`'s job");

        let grown = plan_of(kind, SENTENCE, OverflowPolicy::ResizeFrame, smallest).expect("planned");
        let FitOutcome::Resized { bounds } = grown.outcome else {
            panic!("{kind:?}: `resize_frame` resolved as {:?}", grown.outcome);
        };
        assert_eq!(bounds, grown.bounds, "{kind:?}: the outcome names a box the plan does not carry");
        assert!(grown.fits(), "{kind:?}: a grown frame that still overflows: {:?}", grown.outcome);
        assert!(other_findings(&grown).is_empty(), "{kind:?}: {:?}", other_findings(&grown));
        assert_eq!(grown.fit, AreaFit::None, "{kind:?}");
        // The frame grew around what it already was, and inside the kind's own limits.
        assert!(grown.bounds.width() >= smallest.width() && grown.bounds.height() >= smallest.height(), "{kind:?}");
        assert!(
            grown.bounds.x0 <= smallest.x0 && grown.bounds.y0 <= smallest.y0,
            "{kind:?}: the box left its anchor: {:?} vs {smallest:?}",
            grown.bounds
        );
        assert!(
            grown.bounds.x1 >= smallest.x1 && grown.bounds.y1 >= smallest.y1,
            "{kind:?}: the box left its anchor: {:?} vs {smallest:?}",
            grown.bounds
        );
        assert!(
            grown.bounds.width() <= style.limits.max.width && grown.bounds.height() <= style.limits.max.height,
            "{kind:?}: past the kind's maximum"
        );
        // A sentence in the largest box the kind allows fits without wrapping *past* the frame.
        assert!(
            plan_of(kind, SENTENCE, OverflowPolicy::Warn, max_box_of(kind)).expect("planned").fits(),
            "{kind:?}: the largest box cannot hold one sentence"
        );
    }
}

/// Every kind detects the overflow and the collision, and applies **nothing**: a grow that would
/// overlap another object is reported, not applied (design §8.3).
#[test]
fn every_kind_reports_a_blocked_grow_and_applies_nothing_at_all() {
    let db = FontDb::global();
    for kind in LETTERING_KINDS {
        let smallest = min_box_of(kind);
        // A frame of four objects touching the box: the box may grow in no direction at all.
        let obstacles = [
            Rect::new(smallest.x0, smallest.y0 - 40.0, smallest.x0 + smallest.width(), smallest.y0),
            Rect::new(smallest.x0, smallest.y1, smallest.x0 + smallest.width(), smallest.y1 + 40.0),
            Rect::new(smallest.x0 - 40.0, smallest.y0, smallest.x0, smallest.y0 + smallest.height()),
            Rect::new(smallest.x1, smallest.y0, smallest.x1 + 40.0, smallest.y0 + smallest.height()),
        ];
        let request = request_for(kind, SENTENCE, OverflowPolicy::ResizeFrame).with_obstacles(&obstacles);
        let plan = refit_balloon(db, &request, smallest).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert!(matches!(plan.outcome, FitOutcome::Blocked { obstacle } if obstacle < obstacles.len()), "{kind:?}: {:?}", plan.outcome);
        assert_eq!(plan.bounds, smallest, "{kind:?}: a blocked grow moved the frame");
        assert!(!plan.fits(), "{kind:?}");
        assert!(plan.frame.is_closed(), "{kind:?}");
        let findings = other_findings(&plan);
        assert_eq!(findings.len(), 1, "{kind:?}: {:?}", findings);
        assert!(findings[0].contains("another object"), "{kind:?}: {:?}", findings);
        // The plan still describes the box it hands back, measured in that box's frame (round 1, F1).
        let style = style_of(kind);
        let frame = text_frame(smallest, style.shape.inner_scale(), style.text.padding).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let measured = measure(db, SENTENCE, &style.text, &frame, AreaFit::None).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert_eq!(plan.text_bounds, measured.layout.bounds, "{kind:?}: the report is not the box that comes back");
    }
}
