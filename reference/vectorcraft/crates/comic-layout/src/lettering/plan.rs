//! The lettering **plan**: what the command layer writes, computed but not written here.
//!
//! `new_balloon` and `refit_balloon` return a [`LetteringPlan`] — geometry, the settings the record
//! carries, and the findings — and take no `&mut Document` (design §8). That is what makes a
//! lettering operation one undoable step and what lets this crate be tested with plain rectangles
//! and no document at all.
//!
//! # The objects a plan describes
//!
//! | Plan field | The object it describes | The record field |
//! |---|---|---|
//! | `frame` | the balloon outline or caption box | `Lettering.frame_node_id` |
//! | `tail.path` | the tail | `Lettering.tail_node_id` |
//! | `text_frame_path()` | the area text object's frame | `Lettering.text_node_id` |
//!
//! A plan therefore names three objects and one setting block, and it is the command layer that
//! creates the nodes and stores their ids — this crate never mints a `NodeId` (design §9, §5.2.1).
//!
//! # Who owns the text
//!
//! [`TextSource`] makes the schema's rule 8 structural: a **spoken** kind takes its words from the
//! linked `Dialogue` and a **narration** or **sound_effect** carries its own. A request that says
//! otherwise is rejected, because the alternative — accepting either — is how a dialogue's text
//! quietly becomes two authorities (design §6.1). The plan echoes which owner it was told about in
//! `text_owner`, and holds no text itself.

use vectorcraft_comic_model::{Finding, LetteringKind, OverflowPolicy, TextLayoutSettings};
use vectorcraft_doc::AreaFit;
use vectorcraft_geom::{Affine, PathData, Point, Rect, Vec2, shapes};
use vectorcraft_text::FontDb;

use super::LetteringKindExt;
use super::outline::BalloonShape;
use super::style::LetteringStyle;
use super::tail::{Tail, TailKind};
use crate::fit::policy::{FitRequest, apply};
use crate::fit::sizing::{Placement, SizeRequest, size_to_text};
use crate::fit::{FitError, FitOutcome, Measured, area_fit, layout_settings, overflow_finding, text_frame};

/// Which record owns a piece of lettering's words (schema §6.4, rule 8, design §6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOwner {
    /// The linked `Dialogue` record: what is said, by whom.
    Dialogue,
    /// The `Lettering` record itself: `narration` and `sound_effect` payloads.
    Lettering,
}

/// The words a lettering is planned with, and the record that owns them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSource<'a> {
    /// A spoken kind: these are the linked `Dialogue`'s words.
    Dialogue(&'a str),
    /// A caption or a sound effect: these are the `Lettering`'s own words.
    Own(&'a str),
}

impl<'a> TextSource<'a> {
    /// The words themselves. Borrowed: the plan never keeps a copy (design §6.1).
    #[must_use]
    pub fn words(&self) -> &'a str {
        match self {
            Self::Dialogue(text) | Self::Own(text) => text,
        }
    }

    /// Which record owns them.
    #[must_use]
    pub fn owner(&self) -> TextOwner {
        match self {
            Self::Dialogue(_) => TextOwner::Dialogue,
            Self::Own(_) => TextOwner::Lettering,
        }
    }
}

/// Everything a lettering is planned from.
///
/// Built with [`LetteringRequest::new`], which takes the kind's default style, and adjusted with
/// the `with_…` methods; the fields are public for a caller that assembles one directly (for
/// instance from a project's `default_lettering_style`).
#[derive(Clone, Debug)]
pub struct LetteringRequest<'a> {
    /// What is being lettered.
    pub kind: LetteringKind,
    /// The words, and the record that owns them.
    pub text: TextSource<'a>,
    /// Shape, tail, face, limits and padding for the kind.
    pub style: LetteringStyle,
    /// Where a **new** balloon sits: the anchor point of its box (its centre, or a caption's top
    /// centre). A refit ignores it and keeps the frame it is given.
    pub at: Point,
    /// What to do when the text does not fit.
    pub overflow_policy: OverflowPolicy,
    /// Where the tail points — read back from the document, since tail geometry lives there.
    ///
    /// It must be a finite point to be used. One that is not is **reported** rather than refused
    /// (unlike [`at`](Self::at), which a balloon cannot do without), so a caller whose target came
    /// out of a damaged document still gets a usable balloon and a finding naming the reason.
    pub tail_target: Option<Point>,
    /// Rectangles the box must not overlap: the other panels and the other lettering on the page.
    pub obstacles: &'a [Rect],
}

impl<'a> LetteringRequest<'a> {
    /// A request for `kind` holding `text`, in the kind's default style, at the origin.
    #[must_use]
    pub fn new(kind: LetteringKind, text: TextSource<'a>) -> Self {
        Self {
            kind,
            text,
            style: super::style::style_of(kind),
            at: Point::ZERO,
            overflow_policy: OverflowPolicy::default(),
            tail_target: None,
            obstacles: &[],
        }
    }

    /// The same request with the box's anchor point at `at`.
    #[must_use]
    pub fn placed_at(mut self, at: Point) -> Self {
        self.at = at;
        self
    }

    /// The same request with `policy` from the record.
    #[must_use]
    pub fn with_policy(mut self, policy: OverflowPolicy) -> Self {
        self.overflow_policy = policy;
        self
    }

    /// The same request whose tail points at `target`.
    ///
    /// A target that is not a finite point is not a target: the plan comes back without a tail and
    /// with a finding saying so, because a spoken lettering must not be written without one
    /// (schema §6.4).
    #[must_use]
    pub fn with_tail(mut self, target: Point) -> Self {
        self.tail_target = Some(target);
        self
    }

    /// The same request with `obstacles` the box must not overlap.
    #[must_use]
    pub fn with_obstacles(mut self, obstacles: &'a [Rect]) -> Self {
        self.obstacles = obstacles;
        self
    }

    /// The same request in an explicit style.
    #[must_use]
    pub fn with_style(mut self, style: LetteringStyle) -> Self {
        self.style = style;
        self
    }
}

/// What the command layer applies, once.
///
/// There is no text in here. The words belong to the `Dialogue` (or, for a caption, to the record
/// the plan is for), and a plan holding a second copy is exactly the duplication design §6.1
/// exists to prevent.
#[derive(Clone, Debug, PartialEq)]
pub struct LetteringPlan {
    /// What is being lettered.
    pub kind: LetteringKind,
    /// The record that owns the words this plan was measured against.
    pub text_owner: TextOwner,
    /// The box: the balloon or caption as the reader sees it, in document points.
    pub bounds: Rect,
    /// The editable outline — `Lettering.frame_node_id` (schema §6.4).
    pub frame: PathData,
    /// The tail, for the kinds that have one — `Lettering.tail_node_id`.
    pub tail: Option<Tail>,
    /// The frame the area text flows inside, in document points.
    pub text_frame: Rect,
    /// The `AreaFit` the text object carries (design §8.3).
    pub fit: AreaFit,
    /// `Lettering.text_layout_settings`: the inputs to `vectorcraft_text` (schema §6.6).
    pub text_layout: TextLayoutSettings,
    /// `Lettering.overflow_policy`, as it was applied.
    pub overflow_policy: OverflowPolicy,
    /// How it resolved.
    pub outcome: FitOutcome,
    /// Where the text actually landed (`TextLayout.bounds`). Measured, never persisted
    /// (design §8.3).
    pub text_bounds: Rect,
    /// What to report: overflow, a blocked grow, a missing face, a missing tail target.
    pub findings: Vec<Finding>,
}

impl LetteringPlan {
    /// The area text object's frame — `TextKind::Area { frame }` (design §8.3).
    #[must_use]
    pub fn text_frame_path(&self) -> PathData {
        shapes::rectangle(self.text_frame)
    }

    /// The tail's path, when the kind has one.
    #[must_use]
    pub fn tail_path(&self) -> Option<&PathData> {
        self.tail.as_ref().map(|tail| &tail.path)
    }

    /// Whether the text is inside its frame as this plan stands.
    #[must_use]
    pub fn fits(&self) -> bool {
        self.outcome.fits()
    }

    /// The finding subject for this plan: `lettering speech`, `lettering narration`.
    #[must_use]
    pub fn subject(&self) -> String {
        subject_of(self.kind)
    }

    /// The whole balloon moved by `delta`, with its tail still attached and still pointing where it
    /// pointed (design §8.3).
    ///
    /// The frame path is *translated*, not rebuilt from the box: a creator who dragged an anchor on
    /// a balloon keeps that shape, and the tail is re-based on the path that actually moved. The
    /// tip does not move — moving a balloon does not move the person it is talking to.
    pub fn moved_by(&self, delta: Vec2) -> Result<Self, FitError> {
        if !(delta.x.is_finite() && delta.y.is_finite()) {
            return Err(FitError::NotFinite { field: "move" });
        }
        let translate = Affine::translate(delta);
        let bounds = translate_rect(self.bounds, delta);
        let frame = self.frame.transformed(translate);
        Ok(Self {
            kind: self.kind,
            text_owner: self.text_owner,
            bounds,
            tail: self.tail.as_ref().map(|tail| tail.after_outline_move(&frame, bounds)),
            frame,
            text_frame: translate_rect(self.text_frame, delta),
            fit: self.fit,
            text_layout: self.text_layout.clone(),
            overflow_policy: self.overflow_policy,
            // A `Resized` outcome names the box the grow produced, so it moves with the plan: copying
            // it verbatim would leave the outcome describing where the balloon used to be (round 1,
            // F3). Every other outcome is a scale or a report and has no coordinate in it.
            outcome: match self.outcome {
                FitOutcome::Resized { bounds } => FitOutcome::Resized { bounds: translate_rect(bounds, delta) },
                other => other,
            },
            text_bounds: translate_rect(self.text_bounds, delta),
            // The findings name sizes and policies, never coordinates, so a move cannot make one
            // stale.
            findings: self.findings.clone(),
        })
    }
}

/// Plan a **new** balloon or caption: a box sized to the text it must hold.
///
/// The box is grown from the kind's minimum until the text fits at the style's own size, inside the
/// kind's maximum and clear of `obstacles`. Text that fits nowhere comes back with
/// [`FitOutcome::Unfittable`] and a finding, at the largest box allowed — never as an error, so the
/// creator still has a balloon on the page to work with.
pub fn new_balloon(db: &FontDb, request: &LetteringRequest<'_>) -> Result<LetteringPlan, FitError> {
    check_source(request.kind, request.text)?;
    validate_point("balloon position", request.at)?;
    let style = &request.style;
    let subject = subject_of(request.kind);
    let sized = size_to_text(
        db,
        &SizeRequest {
            text: request.text.words(),
            style: &style.text,
            inner_scale: style.shape.inner_scale(),
            limits: style.limits,
            place: Placement::new(request.at, style.anchor),
            obstacles: request.obstacles,
        },
    )?;
    let mut findings = ruler_findings(db, request, &subject);
    findings.extend(sizing_findings(sized.outcome, &subject, sized.bounds, style));
    assemble(request, sized.bounds, sized.measured, sized.outcome, findings)
}

/// Re-fit an **existing** balloon or caption whose text or frame changed.
///
/// The box is the caller's — this is the "edit a balloon's text and re-fit it in the frame" path
/// (M1's acceptance step 5) — and the record's [`OverflowPolicy`] decides what happens when the
/// text no longer fits. A `resize_frame` plan that would overlap another object is reported, not
/// applied (design §8.3).
pub fn refit_balloon(db: &FontDb, request: &LetteringRequest<'_>, bounds: Rect) -> Result<LetteringPlan, FitError> {
    check_source(request.kind, request.text)?;
    crate::fit::validate_rect("balloon box", bounds)?;
    let style = &request.style;
    let subject = subject_of(request.kind);
    let decision = apply(
        db,
        &FitRequest {
            text: request.text.words(),
            style: &style.text,
            bounds,
            inner_scale: style.shape.inner_scale(),
            policy: request.overflow_policy,
            limits: style.limits,
            anchor: style.anchor,
            obstacles: request.obstacles,
            subject: &subject,
        },
    )?;
    let mut findings = ruler_findings(db, request, &subject);
    findings.extend(decision.findings);
    assemble(request, decision.bounds, decision.measured, decision.outcome, findings)
}

/// The subject a finding about this kind carries: `lettering speech`, `lettering sound_effect`.
///
/// The kind's name comes from [`LetteringKindExt::name`], which is the schema's own string, so a
/// finding and the record's JSON name the same thing.
#[must_use]
pub fn subject_of(kind: LetteringKind) -> String {
    format!("lettering {}", kind.name())
}

/// Reject a request whose text source does not match its kind (schema §6.4, rule 8).
fn check_source(kind: LetteringKind, source: TextSource<'_>) -> Result<(), FitError> {
    let spoken = kind.is_spoken();
    let from_dialogue = matches!(source, TextSource::Dialogue(_));
    if spoken == from_dialogue {
        return Ok(());
    }
    Err(FitError::TextSourceMismatch { kind: kind.name(), spoken })
}

/// Whether `p` is a point a document can hold: both coordinates finite.
#[must_use]
fn is_point(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Reject a placement that is not a point.
///
/// A placement is *required* — a balloon with no position is not a balloon — so an unusable one is
/// refused. A tail target is *optional*, so an unusable one is reported instead, and the balloon is
/// still planned (round 2, R2-F1).
fn validate_point(field: &'static str, at: Point) -> Result<(), FitError> {
    if is_point(at) {
        return Ok(());
    }
    Err(FitError::NotFinite { field })
}

/// The findings that are about the project's rulers rather than about the text: the face the kind
/// asks for, and the tail a spoken balloon needs.
fn ruler_findings(db: &FontDb, request: &LetteringRequest<'_>, subject: &str) -> Vec<Finding> {
    let mut findings = super::fonts::lettering_face_report(db, &[request.style.face()], subject);
    // A spoken kind must come back with a tail: the record cannot be written legally without
    // `tail_node_id` for a kind that has one (schema §6.4), and the module head promises "a tail
    // exactly for the kinds that have someone to point at".
    //
    // The finding is gated on the tail the plan **actually has** — the same [`tail_spec`] `assemble`
    // builds it from — rather than on one field at a time, so the three ways to end up without one
    // cannot each need their own guard and drift apart. They are: the style has no tail to give
    // (a project's `default_lettering_style`, or a caller's `with_style`); no target was given; and a
    // target that is present but not a finite point. The last one is the limb round 2 found missing,
    // where the target was neither refused nor reported and the plan was silently tailless
    // (round 2, R2-F1).
    if request.kind.is_spoken()
        && let None = tail_spec(request)
    {
        findings.push(Finding::warning(None, subject.to_string(), missing_tail_message(request)));
    }
    findings
}

/// Why a spoken lettering has no tail, in terms a creator can act on.
///
/// Only called when [`tail_spec`] found none, so the match is exhaustive over the reasons rather
/// than over the values: a style with a tail and a target that is a point is a tail.
fn missing_tail_message(request: &LetteringRequest<'_>) -> String {
    let reason = match (request.style.tail, request.tail_target) {
        (None, _) => "this style has no tail to give a spoken lettering",
        (Some(_), None) => "no tail target was given",
        (Some(_), Some(_)) => "the tail target is not a finite point",
    };
    format!(
        "{reason}, so the balloon has no tail: schema §6.4 requires tail_node_id for the kinds that have a tail, and a spoken balloon points at whoever is speaking (design §8.3)"
    )
}

/// The tail a request asks for: the form from the style and the point to aim it at.
///
/// **The kind decides whether there is a tail** and the style decides *which* form it takes. Only
/// [`LetteringKind::is_spoken`] has anyone to point at — a caption and a sound effect have nobody —
/// so a `narration` wearing a style that carries a wedge still gets no tail, which is what the
/// module head means by "a tail exactly for the kinds that have someone to point at" (round 2,
/// R2-F2). A target that is not a finite point is not a target.
///
/// `assemble` builds the tail from this and `ruler_findings` reports when it is `None`, so the
/// geometry and the finding are one decision, not two (round 2, R2-F1).
fn tail_spec(request: &LetteringRequest<'_>) -> Option<(TailKind, Point)> {
    let kind = request.style.tail?;
    let target = request.tail_target?;
    (request.kind.is_spoken() && is_point(target)).then_some((kind, target))
}

/// The findings sizing leaves behind: a box that could not hold the text, or a clear fit that
/// collided with something.
fn sizing_findings(outcome: FitOutcome, subject: &str, bounds: Rect, style: &LetteringStyle) -> Vec<Finding> {
    let message = match outcome {
        FitOutcome::Fits { .. } | FitOutcome::Warned | FitOutcome::Shrunk { .. } | FitOutcome::Resized { .. } => return Vec::new(),
        FitOutcome::Unfittable => format!(
            "the text cannot be set in any frame up to {}x{} points, so the balloon is at its maximum size",
            round(style.limits.max.width),
            round(style.limits.max.height)
        ),
        FitOutcome::Blocked { obstacle } => format!(
            "the balloon would have to grow to {}x{} points to hold the text, which overlaps obstacle {obstacle}; it was left at its maximum size",
            round(bounds.width()),
            round(bounds.height())
        ),
    };
    vec![overflow_finding(subject, message)]
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Put a plan together around a decided box.
fn assemble(
    request: &LetteringRequest<'_>,
    bounds: Rect,
    measured: Measured,
    outcome: FitOutcome,
    findings: Vec<Finding>,
) -> Result<LetteringPlan, FitError> {
    let style = &request.style;
    let text_frame = text_frame(bounds, style.shape.inner_scale(), style.text.padding)?;
    let tail = tail_spec(request).map(|(kind, target)| Tail::new(bounds, style.shape, kind, target));
    Ok(LetteringPlan {
        kind: request.kind,
        text_owner: request.text.owner(),
        bounds,
        frame: style.shape.outline(bounds),
        tail,
        text_frame,
        fit: area_fit(request.overflow_policy, &style.text),
        text_layout: layout_settings(&style.text, matches!(request.overflow_policy, OverflowPolicy::Shrink)),
        overflow_policy: request.overflow_policy,
        outcome,
        text_bounds: measured.text_bounds(),
        findings,
    })
}

/// `Rect` translated by a vector, spelled out rather than reached for, so a NaN cannot arrive
/// through `kurbo`'s own arithmetic unnoticed.
fn translate_rect(rect: Rect, delta: Vec2) -> Rect {
    Rect::new(rect.x0 + delta.x, rect.y0 + delta.y, rect.x1 + delta.x, rect.y1 + delta.y)
}

/// The shape a plan's kind draws, for a caller that has a plan and not the style.
#[must_use]
pub fn shape_of(kind: LetteringKind) -> BalloonShape {
    super::style::style_of(kind).shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Size;

    fn target() -> Point {
        Point::new(160.0, 300.0)
    }

    /// The findings that are not the missing-face report.
    ///
    /// A build without `CRAFT_FONTS` reports the absent comic faces on every plan (that is the
    /// contract), so a test about overflow or a collision has to say which findings it means.
    fn not_font(plan: &LetteringPlan) -> Vec<String> {
        plan.findings.iter().filter(|finding| !finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
    }

    /// The missing-face findings, which the CRAFT_FONTS-empty run is *expected* to produce.
    fn font_reports(plan: &LetteringPlan) -> Vec<String> {
        plan.findings.iter().filter(|finding| finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
    }

    /// A sentence that never fits a balloon built for `Hi!`, used by the re-fit tests.
    const LONG: &str = "Wait — did you hear that? It came from the cellar, I am sure of it.";

    fn speech(text: &str) -> LetteringRequest<'_> {
        LetteringRequest::new(LetteringKind::Speech, TextSource::Dialogue(text)).placed_at(Point::new(200.0, 150.0)).with_tail(target())
    }

    #[test]
    fn a_new_speech_balloon_plans_a_frame_a_tail_and_a_text_object() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Look out!")).expect("planned");
        assert!(plan.fits());
        assert_eq!(plan.text_owner, TextOwner::Dialogue);
        assert!(plan.frame.is_closed(), "the outline is a closed, editable path");
        assert_eq!(plan.frame.anchor_count(), 4, "an ellipse: four anchors");
        assert!(plan.tail.is_some(), "a speech balloon points at someone");
        assert!(plan.tail.as_ref().is_some_and(|tail| tail.is_attached_to(plan.bounds, BalloonShape::Ellipse)));
        assert_eq!(plan.tail_path().map(PathData::is_closed), Some(true));
        assert!(plan.text_frame_path().bounds().is_some());
        // The text frame is inside the balloon, and the balloon holds the text.
        assert!(plan.bounds.x0 <= plan.text_frame.x0 && plan.text_frame.x1 <= plan.bounds.x1);
        assert!(plan.bounds.y0 <= plan.text_frame.y0 && plan.text_frame.y1 <= plan.bounds.y1);
        assert_eq!(plan.text_layout.font.as_deref(), Some("Comic Neue"));
        assert_eq!(plan.text_layout.align, Some(vectorcraft_comic_model::TextAlign::Center));
        assert_eq!(plan.overflow_policy, OverflowPolicy::default());
    }

    #[test]
    fn a_caption_is_a_box_with_no_tail_and_its_own_text() {
        // M1's third object: a `narration` lettering, whose text is the record's own (rule 8).
        let db = FontDb::global();
        let request = LetteringRequest::new(LetteringKind::Narration, TextSource::Own("Three hours earlier…")).placed_at(Point::new(230.0, 60.0));
        let plan = new_balloon(db, &request).expect("planned");
        assert_eq!(plan.kind, LetteringKind::Narration);
        assert_eq!(plan.text_owner, TextOwner::Lettering);
        assert!(plan.tail.is_none(), "a caption points at nobody");
        assert_eq!(plan.frame.anchor_count(), 4, "a caption box is a rectangle");
        assert!(plan.bounds.width() > plan.bounds.height(), "a caption is wide, not tall");
        assert!((plan.bounds.y0 - 60.0).abs() < 1e-9, "a caption is placed by its top centre");
        assert_eq!(plan.text_layout.align, Some(vectorcraft_comic_model::TextAlign::Left));
        assert!(not_font(&plan).is_empty(), "{:?}", not_font(&plan));
    }

    #[test]
    fn a_sound_effect_is_display_lettering_with_no_tail() {
        let db = FontDb::global();
        let request = LetteringRequest::new(LetteringKind::SoundEffect, TextSource::Own("KRAK")).placed_at(Point::new(200.0, 150.0));
        let plan = new_balloon(db, &request).expect("planned");
        assert_eq!(plan.text_layout.font.as_deref(), Some("Bangers"));
        assert!(plan.tail.is_none());
        assert_eq!(plan.frame.anchor_count() % 2, 0, "a burst is spikes and valleys");
    }

    #[test]
    fn a_thought_balloon_gets_a_bubble_tail() {
        let db = FontDb::global();
        let request =
            LetteringRequest::new(LetteringKind::Thought, TextSource::Dialogue("Maybe…")).placed_at(Point::new(200.0, 150.0)).with_tail(target());
        let plan = new_balloon(db, &request).expect("planned");
        let tail = plan.tail.as_ref().expect("a thought balloon has a tail");
        assert_eq!(tail.kind, super::super::tail::TailKind::Bubble);
        assert_eq!(tail.path.subpaths.len(), 2);
    }

    #[test]
    fn a_whisper_plans_a_broken_outline_but_an_ordinary_path() {
        let db = FontDb::global();
        let request =
            LetteringRequest::new(LetteringKind::Whisper, TextSource::Dialogue("psst")).placed_at(Point::new(200.0, 150.0)).with_tail(target());
        let plan = new_balloon(db, &request).expect("planned");
        assert!(super::super::style::style_of(LetteringKind::Whisper).dashed);
        assert!(plan.frame.is_closed(), "the dash is a stroke property, not a broken path");
    }

    #[test]
    fn the_text_source_must_match_the_kind() {
        // Rule 8, as a compile-time-shaped error rather than a comment: a speech balloon takes the
        // dialogue's words, a caption carries its own.
        let db = FontDb::global();
        let spoken_kind_carrying_its_own = LetteringRequest::new(LetteringKind::Speech, TextSource::Own("hi"));
        assert_eq!(new_balloon(db, &spoken_kind_carrying_its_own).err(), Some(FitError::TextSourceMismatch { kind: "speech", spoken: true }));
        let caption_taking_the_dialogues = LetteringRequest::new(LetteringKind::Narration, TextSource::Dialogue("hi"));
        assert_eq!(new_balloon(db, &caption_taking_the_dialogues).err(), Some(FitError::TextSourceMismatch { kind: "narration", spoken: false }));
    }

    #[test]
    fn editing_the_text_refits_it_in_the_frame_it_already_has() {
        // M1's step 5, and the case the frame is *not* allowed to move for: `warn` reports.
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        let before = plan.bounds;
        let longer = speech("Wait — did you hear that? It came from the cellar, I am sure of it.").with_policy(OverflowPolicy::Warn);
        let refit = refit_balloon(db, &longer, before).expect("refitted");
        assert_eq!(refit.bounds, before, "warn does not move the frame");
        assert_eq!(refit.outcome, FitOutcome::Warned);
        assert_eq!(not_font(&refit).len(), 1, "{:?}", not_font(&refit));
        assert_eq!(refit.findings[0].subject, "lettering speech", "the finding names what it is about");
        assert!(refit.frame.is_closed(), "the balloon is still an editable path");
    }

    #[test]
    fn a_shrinking_refit_keeps_the_frame_and_reports_the_scale() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        // The floor is 10% for this one, so the engine has room to find a scale that fits: at the
        // style's default 50% this much text does not fit a balloon built for "Hi!" and the plan
        // says so instead (see the test below).
        let mut longer = speech(LONG).with_policy(OverflowPolicy::Shrink);
        longer.style.text.shrink_percent = 10.0;
        let refit = refit_balloon(db, &longer, plan.bounds).expect("refitted");
        assert_eq!(refit.bounds, plan.bounds, "shrink never moves the frame");
        assert!(matches!(refit.outcome, FitOutcome::Shrunk { fits: true, .. }), "{:?}", refit.outcome);
        assert!(refit.text_layout.fit_to_frame, "the record says the type is scaled to fit");
        assert!(matches!(refit.fit, AreaFit::ShrinkText { .. }));
        assert!(not_font(&refit).is_empty(), "{:?}", not_font(&refit));
    }

    #[test]
    fn a_shrink_that_hits_its_floor_reports_instead_of_shrinking_the_type_away() {
        // The style's default floor is 50%: at that size this much text still overflows a balloon
        // built for "Hi!", and the honest answer is to report it rather than scale the type further.
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        let longer = speech(LONG).with_policy(OverflowPolicy::Shrink);
        let refit = refit_balloon(db, &longer, plan.bounds).expect("refitted");
        let FitOutcome::Shrunk { fit_scale, fits } = refit.outcome else {
            panic!("shrink must report a scale, got {:?}", refit.outcome);
        };
        assert!(!fits);
        assert!((fit_scale - 0.5).abs() < 1e-9, "the reported scale is the style's floor: {fit_scale}");
        assert_eq!(not_font(&refit).len(), 1, "{:?}", not_font(&refit));
    }

    #[test]
    fn a_resizing_refit_grows_the_balloon_and_keeps_the_tail_attached() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        let before = plan.bounds;
        let longer = speech("Wait — did you hear that? It came from the cellar, I am sure of it.").with_policy(OverflowPolicy::ResizeFrame);
        let refit = refit_balloon(db, &longer, before).expect("refitted");
        assert!(matches!(refit.outcome, FitOutcome::Resized { .. }), "{:?}", refit.outcome);
        assert!(refit.bounds.width() > before.width() && refit.bounds.height() > before.height());
        let tail = refit.tail.as_ref().expect("still a tail");
        assert!((tail.tip - target()).x.abs() < 1e-9 && (tail.tip - target()).y.abs() < 1e-9, "the target did not move");
        assert!(tail.is_attached_to(refit.bounds, BalloonShape::Ellipse), "the tail grew with the balloon");
        assert!(refit.fits());
    }

    #[test]
    fn a_refit_that_would_overlap_another_balloon_is_reported_not_applied() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        let neighbour = Rect::new(plan.bounds.x1, plan.bounds.y0, plan.bounds.x1 + 60.0, plan.bounds.y1);
        let longer = speech(LONG).with_policy(OverflowPolicy::ResizeFrame).with_obstacles(std::slice::from_ref(&neighbour));
        let refit = refit_balloon(db, &longer, plan.bounds).expect("refitted");
        assert!(matches!(refit.outcome, FitOutcome::Blocked { .. }), "{:?}", refit.outcome);
        assert_eq!(refit.bounds, plan.bounds, "blocked means the frame is exactly as it was");
        assert!(not_font(&refit).iter().any(|message| message.contains("another object")), "{:?}", not_font(&refit));
        // The plan describes the box it hands back: measuring the text in the plan's own text frame
        // reproduces the plan's `text_bounds`. Measuring it in the largest box the search tried
        // would report text at coordinates no box in the plan has (round 1, F1).
        let style = super::super::style::style_of(LetteringKind::Speech).text;
        let measured = crate::fit::measure(db, LONG, &style, &refit.text_frame, AreaFit::None).expect("measurable");
        assert_eq!(refit.text_bounds, measured.layout.bounds, "the plan reports text the box it describes does not hold");
    }

    #[test]
    fn moving_a_balloon_moves_its_frame_and_keeps_its_tail_on_it() {
        // Design §8.3, and the reason `Tail` derives its path from the outline: a balloon can be
        // moved as far as a creator likes and the tail is still joined to it.
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Look out!")).expect("planned");
        let delta = Vec2::new(120.0, -80.0);
        let moved = plan.moved_by(delta).expect("moved");
        assert!((moved.bounds.x0 - (plan.bounds.x0 + delta.x)).abs() < 1e-12);
        assert_eq!(
            moved.text_frame,
            Rect::new(plan.text_frame.x0 + delta.x, plan.text_frame.y0 + delta.y, plan.text_frame.x1 + delta.x, plan.text_frame.y1 + delta.y)
        );
        assert_eq!(moved.frame, plan.frame.transformed(Affine::translate(delta)), "the anchors moved, the shape did not change");
        let tail = moved.tail.as_ref().expect("still a tail");
        assert_eq!(tail.tip, plan.tail.as_ref().expect("tail").tip, "the speaker did not move with the balloon");
        assert!(tail.is_attached_to_outline(&moved.frame), "the tail is still joined to the frame");
        assert!(tail.is_attached_to(moved.bounds, BalloonShape::Ellipse));
        assert_eq!(moved.findings, plan.findings);
    }

    #[test]
    fn moving_a_grown_balloon_moves_the_outcome_that_names_its_box() {
        // `FitOutcome::Resized` carries the box the grow produced, so a move has to move it too:
        // copying it verbatim leaves the outcome naming where the balloon used to be, and a caller
        // that re-derives the frame from the outcome writes it there (round 1, F3).
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        let longer = speech(LONG).with_policy(OverflowPolicy::ResizeFrame);
        let grown = refit_balloon(db, &longer, plan.bounds).expect("refitted");
        assert!(matches!(grown.outcome, FitOutcome::Resized { .. }), "{:?}", grown.outcome);
        let delta = Vec2::new(120.0, -80.0);
        let moved = grown.moved_by(delta).expect("moved");
        assert_eq!(
            moved.outcome,
            FitOutcome::Resized { bounds: moved.bounds },
            "the outcome names {:?}; the plan is at {:?}",
            moved.outcome,
            moved.bounds
        );
        // The other outcomes have no coordinate in them, and a move leaves them alone.
        let warned = refit_balloon(db, &speech(LONG).with_policy(OverflowPolicy::Warn), plan.bounds).expect("refitted");
        assert_eq!(warned.moved_by(delta).expect("moved").outcome, FitOutcome::Warned);
    }

    #[test]
    fn moving_a_balloon_onto_its_target_leaves_a_path_behind_not_a_detached_tail() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Look out!")).expect("planned");
        let delta = target() - plan.bounds.center();
        let moved = plan.moved_by(delta).expect("moved");
        let tail = moved.tail.as_ref().expect("still a tail");
        assert!(tail.path.bounds().is_some());
        assert!(tail.is_attached_to_outline(&moved.frame), "a degenerate tail is still an attached tail");
        assert!(moved.frame.is_closed());
    }

    #[test]
    fn a_move_by_a_non_finite_vector_is_refused() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Hi!")).expect("planned");
        assert_eq!(plan.moved_by(Vec2::new(f64::NAN, 0.0)).err(), Some(FitError::NotFinite { field: "move" }));
    }

    #[test]
    fn a_balloon_whose_text_cannot_fit_comes_back_at_its_maximum_with_a_finding() {
        let db = FontDb::global();
        let mut request = speech("Incomprehensibilities");
        request.style.limits = crate::fit::sizing::Limits::new(Size::new(48.0, 32.0), Size::new(60.0, 40.0)).expect("valid limits");
        let plan = new_balloon(db, &request).expect("a balloon is still planned");
        assert_eq!(plan.outcome, FitOutcome::Unfittable);
        assert_eq!(plan.bounds.width(), 60.0);
        assert!(not_font(&plan).iter().any(|message| message.contains("cannot be set")), "{:?}", not_font(&plan));
        assert!(plan.tail.is_some(), "the balloon still has its tail");
    }

    #[test]
    fn a_spoken_kind_wearing_a_tailless_style_is_reported() {
        // Two ways to end up with a balloon nobody points at, and both are the same defect: the
        // record cannot legally be written without `tail_node_id` for a kind that has a tail
        // (schema §6.4). The style is the caller's (`with_style` exists for a project's
        // `default_lettering_style`), so it cannot be refused — it has to be reported (round 1, F4).
        let db = FontDb::global();
        let mut request = speech("Look out!");
        request.style.tail = None;
        let plan = new_balloon(db, &request).expect("planned");
        assert!(plan.tail.is_none());
        assert!(
            not_font(&plan).iter().any(|message| message.to_lowercase().contains("tail")),
            "a spoken lettering planned without a tail is reported by nothing: {:?}",
            not_font(&plan)
        );
        // A caption wearing a balloon's style is not reported for it: a caption has no tail by
        // design, so there is nothing missing.
        let caption = LetteringRequest::new(LetteringKind::Narration, TextSource::Own("Later…"))
            .placed_at(Point::new(230.0, 60.0))
            .with_style(super::super::style::style_of(LetteringKind::Speech));
        let plan = new_balloon(db, &caption).expect("planned");
        assert!(plan.tail.is_none());
        assert!(not_font(&plan).is_empty(), "{:?}", not_font(&plan));
    }

    #[test]
    fn a_tail_target_that_is_not_a_point_is_reported_and_never_silently_dropped() {
        // The third way a spoken balloon loses its tail, and the one round 2 found: the target is
        // *present* but is not a point, so it is neither a target nor a missing one — and the plan
        // came back tailless with nothing said. A tail target is optional, so it is reported rather
        // than refused (a placement, which is required, is refused), and the balloon is still
        // planned (round 2, R2-F1).
        let db = FontDb::global();
        // A box that holds the text, so the refit path is about the tail and not about overflow.
        let bounds = new_balloon(db, &speech("Look out!")).expect("planned").bounds;
        for target in [Point::new(f64::NAN, 10.0), Point::new(10.0, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NAN)] {
            for entry in ["new_balloon", "refit_balloon"] {
                let request = speech("Look out!").with_tail(target);
                let plan = match entry {
                    "new_balloon" => new_balloon(db, &request).expect("planned"),
                    _ => refit_balloon(db, &request, bounds).expect("refitted"),
                };
                assert_eq!(plan.kind, LetteringKind::Speech);
                assert!(plan.tail.is_none(), "{entry} {target:?}: a non-point still made a tail");
                assert!(
                    not_font(&plan).iter().any(|message| message.contains("not a finite point")),
                    "{entry} {target:?}: a tailless spoken plan is reported by nothing: {:?}",
                    not_font(&plan)
                );
                assert!(plan.frame.is_closed(), "{entry} {target:?}: the balloon is still a balloon");
                assert!(plan.fits(), "{entry} {target:?}: the text still fits");
            }
        }
        // A caption with the same unusable target is not reported: it has no tail to lose.
        let caption = LetteringRequest::new(LetteringKind::Narration, TextSource::Own("Later…"))
            .placed_at(Point::new(230.0, 60.0))
            .with_tail(Point::new(f64::NAN, f64::NAN));
        let plan = new_balloon(db, &caption).expect("planned");
        assert!(plan.tail.is_none());
        assert!(not_font(&plan).is_empty(), "{:?}", not_font(&plan));
    }

    #[test]
    fn the_kind_decides_whether_there_is_a_tail_and_the_style_only_which_form() {
        // The rule the module head states, made true of the geometry and not only of the finding: a
        // `narration` wearing a balloon's style — with a target, which is what round 2's probe found
        // — gets no tail, because a caption has nobody to point at (round 2, R2-F2). The style
        // chooses the *form* of a tail a spoken kind does have.
        let db = FontDb::global();
        let narration_with_a_wedge = LetteringRequest::new(LetteringKind::Narration, TextSource::Own("Later…"))
            .placed_at(Point::new(230.0, 60.0))
            .with_style(super::super::style::style_of(LetteringKind::Speech))
            .with_tail(target());
        let plan = new_balloon(db, &narration_with_a_wedge).expect("planned");
        assert!(plan.tail.is_none(), "a caption was given a tail: {:?}", plan.tail);
        assert!(not_font(&plan).is_empty(), "{:?}", not_font(&plan));

        // The same for a sound effect, the other kind with nobody to point at.
        let effect_with_a_wedge = LetteringRequest::new(LetteringKind::SoundEffect, TextSource::Own("KRAK"))
            .placed_at(Point::new(200.0, 150.0))
            .with_style(super::super::style::style_of(LetteringKind::Speech))
            .with_tail(target());
        assert!(new_balloon(db, &effect_with_a_wedge).expect("planned").tail.is_none());

        // A spoken kind keeps the form its style names: a thought balloon's bubbles, not a wedge.
        let thought =
            LetteringRequest::new(LetteringKind::Thought, TextSource::Dialogue("Maybe…")).placed_at(Point::new(200.0, 150.0)).with_tail(target());
        let plan = new_balloon(db, &thought).expect("planned");
        assert_eq!(plan.tail.as_ref().map(|tail| tail.kind), Some(super::super::tail::TailKind::Bubble));
    }

    #[test]
    fn a_spoken_balloon_without_a_tail_target_says_so() {
        let db = FontDb::global();
        let request = LetteringRequest::new(LetteringKind::Speech, TextSource::Dialogue("Hi!")).placed_at(Point::new(200.0, 150.0));
        let plan = new_balloon(db, &request).expect("planned");
        assert!(plan.tail.is_none());
        assert!(not_font(&plan).iter().any(|message| message.contains("no tail target")), "{:?}", not_font(&plan));
    }

    #[test]
    fn an_empty_dialogue_is_a_balloon_the_size_of_its_kinds_minimum() {
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("")).expect("planned");
        assert!(plan.fits());
        assert_eq!(plan.bounds.width(), 48.0);
        assert_eq!(plan.bounds.height(), 44.0, "the minimum holds the line the caret will sit on");
        assert!(plan.frame.is_closed(), "even an unwritten balloon is a real path");
    }

    #[test]
    fn the_plan_never_carries_the_text() {
        // The structural half of design §6.1: the record owns the words and the plan is geometry. A
        // plan that held them would be a second authority the moment a creator edited one.
        //
        // Asserted on the **type**: this pattern names every field of `LetteringPlan` without `..`,
        // so a second copy of the words could not be added to the struct without this failing to
        // compile. (Reading `format!("{plan:?}")` for the words, which is what an earlier version of
        // this test did, tests the formatter rather than the type.) The annotations are the field
        // types: a rectangle, a path, an enum, the settings, the `AreaFit`, a tail and the findings —
        // none of them a `String` that could hold the dialogue.
        let db = FontDb::global();
        let words = "A sentence nobody else could have written.";
        let plan = new_balloon(db, &speech(words)).expect("planned");
        let LetteringPlan { kind, text_owner, bounds, frame, tail, text_frame, fit, text_layout, overflow_policy, outcome, text_bounds, findings } =
            plan;
        let _: LetteringKind = kind;
        let _: TextOwner = text_owner;
        let _: Rect = bounds;
        let _: PathData = frame;
        let _: Option<Tail> = tail;
        let _: Rect = text_frame;
        let _: AreaFit = fit;
        let _: TextLayoutSettings = text_layout;
        let _: OverflowPolicy = overflow_policy;
        let _: FitOutcome = outcome;
        let _: Rect = text_bounds;
        let _: Vec<Finding> = findings;
        assert_eq!(text_owner, TextOwner::Dialogue, "and the words are the dialogue's");
    }

    #[test]
    fn two_speech_balloons_and_one_caption_plan_as_three_letterings_that_do_not_overlap() {
        // M1's acceptance bar at this layer's level: the three objects the milestone asks for, each
        // with a closed editable frame, its own tail rule and the record that owns its words. The
        // second balloon is planned with the first as an obstacle, which is how two balloons on one
        // panel are kept apart — the same mechanism that stops an automatic resize.
        let db = FontDb::global();
        let first = new_balloon(db, &speech("Look out!")).expect("the first balloon");
        let second = new_balloon(
            db,
            &speech("Behind you — it is still coming!").placed_at(Point::new(200.0, 420.0)).with_obstacles(std::slice::from_ref(&first.bounds)),
        )
        .expect("the second balloon");
        let caption = new_balloon(
            db,
            &LetteringRequest::new(LetteringKind::Narration, TextSource::Own("Three hours earlier…"))
                .placed_at(Point::new(230.0, 60.0))
                .with_obstacles(&[first.bounds, second.bounds]),
        )
        .expect("the caption");

        for plan in [&first, &second, &caption] {
            assert!(plan.fits(), "{:?}: {:?}", plan.kind, plan.outcome);
            assert!(plan.frame.is_closed(), "{:?}", plan.kind);
            assert!(plan.bounds.width() >= plan.text_frame.width(), "{:?}", plan.kind);
        }
        assert_eq!((first.kind, second.kind, caption.kind), (LetteringKind::Speech, LetteringKind::Speech, LetteringKind::Narration));
        assert_eq!((first.text_owner, second.text_owner, caption.text_owner), (TextOwner::Dialogue, TextOwner::Dialogue, TextOwner::Lettering));
        assert!(first.tail.is_some() && second.tail.is_some(), "a speech balloon points at whoever is speaking");
        assert!(caption.tail.is_none(), "a caption has nobody to point at");
        assert!(!crate::fit::overlaps(first.bounds, second.bounds), "the second balloon was kept clear of the first");
        assert!(!crate::fit::overlaps(first.bounds, caption.bounds));
        assert!(!crate::fit::overlaps(second.bounds, caption.bounds));
    }

    #[test]
    fn the_same_request_always_plans_the_same_balloon() {
        let db = FontDb::global();
        let a = new_balloon(db, &speech("Look out!")).expect("planned");
        let b = new_balloon(db, &speech("Look out!")).expect("planned");
        assert_eq!(a, b, "no randomness, no clock: a plan is a function of its request");
    }

    #[test]
    fn a_missing_lettering_face_is_reported_without_stopping_the_plan() {
        // The CRAFT_FONTS-empty contract: the layout runs in the fallback face and the finding says
        // which face is not there. When the faces *are* embedded the same call reports nothing, so
        // the assertion is conditional on the face really being absent.
        let db = FontDb::global();
        let plan = new_balloon(db, &speech("Look out!")).expect("planned");
        let comic_neue_available = super::super::fonts::missing_faces(db, &[super::super::fonts::Face::new("Comic Neue", "Regular")]).is_empty();
        let reported = !font_reports(&plan).is_empty();
        assert_eq!(reported, !comic_neue_available, "{:?}", font_reports(&plan));
        assert!(!plan.text_bounds.width().is_nan(), "the text was laid out either way");
        assert!(plan.text_bounds.width() > 0.0);
        assert!(plan.frame.is_closed());
    }

    #[test]
    fn the_face_contract_holds_for_a_face_no_build_can_have() {
        // The test above is *conditional* on the comic faces being absent, so in a build that embeds
        // them it exercises nothing — M2 §4.3's point is that the empty case has to be **tested**
        // rather than merely true of this machine. This one is not conditional: the family cannot
        // exist in either configuration, so the same assertions hold whichever way `CRAFT_FONTS` is
        // built, and they are the ones that go red if the plan ever names the face that stands in
        // instead of the one the project asked for, or stops reporting at all.
        let db = FontDb::global();
        let mut request = speech("Look out!");
        request.style.text.family = "No Such Lettering Face".to_string();
        request.style.text.style = super::super::fonts::BOLD.to_string();
        let bounds = new_balloon(db, &request).expect("planned").bounds;
        for entry in ["new_balloon", "refit_balloon"] {
            let plan = match entry {
                "new_balloon" => new_balloon(db, &request).expect("planned"),
                _ => refit_balloon(db, &request, bounds).expect("refitted"),
            };
            let reports = font_reports(&plan);
            assert_eq!(reports.len(), 1, "{entry}: exactly one report, for the one face asked for: {reports:?}");
            assert!(reports[0].contains("No Such Lettering Face Bold"), "{entry}: the requested face is named: {reports:?}");
            assert!(reports[0].contains("missing"), "{entry}: with the exporter's own status: {reports:?}");
            assert_eq!(plan.findings[0].subject, "lettering speech", "{entry}");
            // And the layout ran anyway. A missing face is neither an error nor a refusal, so the
            // creator gets a page *and* the message — never a page alone.
            assert!(plan.fits(), "{entry}: {:?}", plan.outcome);
            assert!(plan.frame.is_closed(), "{entry}");
            assert!(plan.text_bounds.width() > 0.0 && plan.text_bounds.height() > 0.0, "{entry}: {:?}", plan.text_bounds);
            assert_eq!(plan.tail.as_ref().map(|tail| tail.kind), Some(TailKind::Wedge), "{entry}: the balloon kept its tail");
        }
    }
}
