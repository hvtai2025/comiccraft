//! M2 W4, criterion 3 — *"moving a balloon preserves or updates its tail without detaching it"* —
//! tested as a **move**, which is what the criterion is about.
//!
//! # Which move
//!
//! The move a balloon gets in this layer is `LetteringPlan::moved_by`: the plan translates the frame
//! **path** (not a box regenerated from the bounds), the box, the text frame, the measured block and
//! the `FitOutcome` that names a box, and re-bases the tail on the path the balloon now has. That is
//! the drag the command layer performs on a balloon, and it is the only balloon move this crate
//! models. The other way a balloon's geometry changes — `comic.panel.move`, which translates the
//! whole panel and every object it owns — is the command layer's (`crates/engine/src/cmd/comic/
//! panel.rs`), it moves the tail's tip *with* the balloon because the speaker moves with the panel,
//! and it is not this crate's to test.
//!
//! # Preserved, and updated — with the alternatives named
//!
//! A tail is two facts, and a move answers them differently on purpose:
//!
//! | Fact | After a move | The alternative, and why it is wrong |
//! |---|---|---|
//! | the **tip** (`Tail::tip`) | **preserved**: the balloon moved, the speaker did not | moving it with the balloon makes the balloon point at the wrong place — at nothing, in the common case where the speaker is outside the balloon's new position |
//! | the **base** (`Tail::attach`) | **updated**: the point of the *moved path* nearest the tip | keeping the old base detaches the tail from the balloon (this is the failure the criterion names); regenerating it from `bounds` attaches it to a shape the balloon does not have, because the frame is an editable path a creator has dragged |
//!
//! "Still attached" is therefore asserted against the path the plan carries —
//! `Tail::is_attached_to_outline(&plan.frame)` — which is the object `Lettering.frame_node_id` names,
//! *not* against a box that merely approximates it; and every test also asserts the tail has not been
//! left behind, by checking it is **not** attached to the outline the balloon used to have.

use vectorcraft_comic_layout::fit::FitError;
use vectorcraft_comic_layout::lettering::LETTERING_KINDS;
use vectorcraft_comic_layout::lettering::outline::BalloonShape;
use vectorcraft_comic_layout::lettering::plan::{LetteringPlan, LetteringRequest, TextSource, new_balloon};
use vectorcraft_comic_layout::lettering::style::style_of;
use vectorcraft_comic_layout::lettering::tail::{TailKind, attach_point};
use vectorcraft_comic_model::LetteringKind;
use vectorcraft_geom::{Affine, Point, Vec2};
use vectorcraft_text::FontDb;

/// The four kinds with someone to point at — the ones a tail can be detached from.
fn spoken_kinds() -> Vec<LetteringKind> {
    LETTERING_KINDS.into_iter().filter(|kind| kind.is_spoken()).collect()
}

/// Farther than any of the six kinds' balloons is wide (the widest maximum is 420pt), so "the tail
/// is still on the outline the balloon used to have" is a statement about the move and not about a
/// translation so small the two outlines overlap.
const AWAY: Vec2 = Vec2::new(700.0, 500.0);

fn source_for(kind: LetteringKind, text: &str) -> TextSource<'_> {
    if kind.is_spoken() { TextSource::Dialogue(text) } else { TextSource::Own(text) }
}

fn request_for(kind: LetteringKind, text: &str) -> LetteringRequest<'_> {
    let request = LetteringRequest::new(kind, source_for(kind, text)).placed_at(Point::new(200.0, 150.0));
    if kind.is_spoken() { request.with_tail(Point::new(160.0, 320.0)) } else { request }
}

/// The plan for `kind`, reported to the caller: a `#[test]` body may panic on it (`clippy.toml`'s
/// `allow-panic-in-tests`), a helper may not.
fn plan_of(kind: LetteringKind, text: &str) -> Result<LetteringPlan, FitError> {
    new_balloon(FontDb::global(), &request_for(kind, text))
}

fn translate(rect: vectorcraft_geom::Rect, delta: Vec2) -> vectorcraft_geom::Rect {
    vectorcraft_geom::Rect::new(rect.x0 + delta.x, rect.y0 + delta.y, rect.x1 + delta.x, rect.y1 + delta.y)
}

/// Both directions of the move, for every spoken kind: the tip is **preserved**, the base is
/// **updated** onto the moved path, and the tail is neither left behind nor detached.
#[test]
fn a_move_keeps_each_spoken_kinds_tail_on_the_balloon_and_its_tip_on_the_speaker() {
    for kind in spoken_kinds() {
        let before = plan_of(kind, "Look out!").expect("planned");
        let tip = before.tail.as_ref().expect("a spoken balloon has a tail").tip;
        let base = before.tail.as_ref().expect("a spoken balloon has a tail").attach;
        let tail_kind = before.tail.as_ref().expect("a spoken balloon has a tail").kind;
        let moved = before.moved_by(AWAY).unwrap_or_else(|e| panic!("{kind:?}: {e}"));

        // Still *that balloon's* tail: carried by the moved plan, of the kind its own style names.
        let tail = moved.tail.as_ref().unwrap_or_else(|| panic!("{kind:?}: the move detached the tail"));
        assert_eq!(tail.kind, tail_kind, "{kind:?}: the tail changed form");
        assert_eq!(tail_kind, style_of(kind).tail.expect("a spoken kind's style has a tail"), "{kind:?}");

        // Preserved: the tip did not move. The speaker is not part of the balloon.
        assert_eq!(tail.tip, tip, "{kind:?}: the tip moved with the balloon");

        // Updated: the base is the point of the moved path nearest the tip. Asserted as the *value*
        // the geometry gives, so an implementation that kept the old base, or that re-based the tail
        // on a box rather than on the path, is a different value rather than a different story.
        assert_eq!(tail.attach, attach_point(&moved.frame, tip, moved.bounds), "{kind:?}: the base was not re-derived from the moved path");
        assert_ne!(tail.attach, base, "{kind:?}: the base did not follow the balloon ({AWAY:?})");

        // Still attached — to the path the plan carries, which is what the record's frame names.
        assert!(tail.is_attached_to_outline(&moved.frame), "{kind:?}: the tail is not on the moved frame");
        assert!(tail.is_attached_to(moved.bounds, style_of(kind).shape), "{kind:?}: the tail is not on the moved balloon");
        assert!(!tail.is_attached_to_outline(&before.frame), "{kind:?}: the tail is still on the outline the balloon left");
        assert!(tail.path.bounds().is_some(), "{kind:?}: the tail is not a path");

        // The frame moved, and *only* moved: the same anchors, the same shape, translated. A move
        // that regenerated the outline would lose the edits a creator made to it.
        assert_eq!(moved.frame, before.frame.transformed(Affine::translate(AWAY)), "{kind:?}");
        assert_eq!(moved.frame.anchor_count(), before.frame.anchor_count(), "{kind:?}");
        assert!(moved.frame.is_closed(), "{kind:?}");
        assert_eq!(moved.bounds, translate(before.bounds, AWAY), "{kind:?}");
        assert_eq!(moved.text_frame, translate(before.text_frame, AWAY), "{kind:?}");
        assert_eq!(moved.text_bounds, translate(before.text_bounds, AWAY), "{kind:?}");
        // And nothing else changed: a move is a move.
        assert_eq!(moved.kind, before.kind);
        assert_eq!(moved.text_owner, before.text_owner);
        assert_eq!(moved.overflow_policy, before.overflow_policy);
        assert_eq!(moved.fit, before.fit);
        assert_eq!(moved.text_layout, before.text_layout);
        assert_eq!(moved.outcome, before.outcome);
        assert_eq!(moved.findings, before.findings, "{kind:?}: a move cannot make a finding stale");
    }
}

/// The base is re-derived from the **editable path**, not from the box: a creator who has dragged the
/// outline off its default shape keeps that shape, and the tail follows the path the balloon actually
/// has.
///
/// This is the assertion that separates the two "updated" implementations. A plan whose frame is a
/// box while its kind's shape is an ellipse is exactly the state a dragged anchor produces (the frame
/// path is the editable object), and the two candidate bases are different points — so a move that
/// rebuilt the outline from `bounds` would put the base on a shape the balloon does not have.
#[test]
fn a_move_re_bases_the_tail_on_the_path_the_balloon_has_and_not_on_its_box() {
    let before = plan_of(LetteringKind::Speech, "Look out!").expect("planned");
    let tip = before.tail.as_ref().expect("a spoken balloon has a tail").tip;
    // The creator's edit: the outline is no longer the ellipse the bounds would produce.
    let mut edited = before.clone();
    edited.frame = BalloonShape::Box.outline(before.bounds);
    assert_ne!(edited.frame, before.frame, "the edit did not change the path");

    let delta = Vec2::new(60.0, -40.0);
    let moved = edited.moved_by(delta).expect("moved");
    let tail = moved.tail.as_ref().expect("the tail survived the move");
    let on_the_path = attach_point(&moved.frame, tip, moved.bounds);
    let on_the_box = attach_point(&BalloonShape::Ellipse.outline(moved.bounds), tip, moved.bounds);
    assert_ne!(on_the_path, on_the_box, "this balloon and target make the two answers agree, so they cannot be told apart");

    assert_eq!(tail.attach, on_the_path, "the tail was re-based on the box rather than on the path the balloon has");
    assert!(tail.is_attached_to_outline(&moved.frame), "the tail is not on the path the plan carries");
    // The stated consequence: the path is what "attached" is about. The balloon is not that ellipse
    // any more — the creator's outline is a box — and the tail is on the box.
    assert!(!tail.is_attached_to(moved.bounds, BalloonShape::Ellipse), "the plan's frame is not the shape's outline");
    assert_eq!(tail.tip, tip, "the tip is preserved through the edit as well");
    assert_eq!(moved.frame, edited.frame.transformed(Affine::translate(delta)), "the edited path was not translated but rebuilt");
}

/// The degenerate move: a balloon dragged onto the speaker.
///
/// The tip must still be a tip — the tail has no direction when the two coincide, so it is
/// normalised to a short downward tail from the base (`Tail::reachable_tip`) rather than left as a
/// zero-length or NaN path. The alternative is a path with no bounds, which is not a tail.
#[test]
fn a_move_onto_the_speaker_leaves_a_tail_that_is_still_attached_rather_than_a_degenerate_path() {
    for kind in spoken_kinds() {
        let before = plan_of(kind, "Look out!").expect("planned");
        let tip = before.tail.as_ref().expect("a spoken balloon has a tail").tip;
        let delta = tip - before.bounds.center();
        let moved = before.moved_by(delta).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let tail = moved.tail.as_ref().unwrap_or_else(|| panic!("{kind:?}: the tail was dropped"));
        assert_eq!(tail.attach, attach_point(&moved.frame, tip, moved.bounds), "{kind:?}");
        assert!(tail.is_attached_to_outline(&moved.frame), "{kind:?}: a balloon on its own speaker still has a tail on it");
        assert!(tail.path.bounds().is_some(), "{kind:?}: the tail path has no bounds");
        // The tail still runs from the balloon towards the point it points at, to within the length
        // the normalisation gives it, so a creator can grab it and drag it back.
        assert!((tail.tip - tail.attach).hypot() > 0.0, "{kind:?}: a zero-length tail");
        assert!((tail.tip - tip).hypot() <= (tail.tip - tail.attach).hypot() + 1e-9, "{kind:?}");
        assert!(moved.frame.is_closed(), "{kind:?}");
        assert!((tip - moved.bounds.center()).hypot() < 1e-9, "{kind:?}: the balloon is not on the target");
    }
}

/// The kinds with nobody to point at, and a spoken kind whose style the project stripped of a tail:
/// a move has nothing to preserve and must not invent one.
#[test]
fn a_move_neither_invents_a_tail_nor_loses_one_a_balloon_never_had() {
    let db = FontDb::global();
    for kind in LETTERING_KINDS {
        if kind.is_spoken() {
            continue;
        }
        let before = plan_of(kind, "KRAK").expect("planned");
        assert!(before.tail.is_none(), "{kind:?} has nobody to point at");
        let moved = before.moved_by(AWAY).expect("moved");
        assert!(moved.tail.is_none(), "{kind:?}: the move invented a tail");
        assert_eq!(moved.frame, before.frame.transformed(Affine::translate(AWAY)), "{kind:?}");
        assert!(moved.frame.is_closed(), "{kind:?}");
    }
    // A project's own style may carry no tail for a spoken kind; the plan reports it, and the move
    // changes neither the report nor the tail's absence.
    let mut request = request_for(LetteringKind::Speech, "Look out!");
    request.style.tail = None;
    let before = new_balloon(db, &request).expect("planned");
    assert!(before.tail.is_none());
    let moved = before.moved_by(AWAY).expect("moved");
    assert!(moved.tail.is_none(), "the move invented a tail the plan was reported for not having");
    assert_eq!(moved.findings, before.findings, "the move dropped the report about the missing tail");
    assert_eq!(moved.tail.as_ref().map(|tail| tail.kind), None);
    assert_eq!(moved.tail_path(), None);
}

/// A move that is not a move is refused, and the plan is untouched: the caller keeps a balloon it can
/// still work with.
#[test]
fn a_move_by_a_non_finite_vector_is_refused_rather_than_applied_partially() {
    let before = plan_of(LetteringKind::Speech, "Look out!").expect("planned");
    for delta in [Vec2::new(f64::NAN, 0.0), Vec2::new(0.0, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NAN)] {
        assert!(before.moved_by(delta).is_err(), "{delta:?} was accepted");
    }
    assert_eq!(before.tail.as_ref().map(|tail| tail.kind), Some(TailKind::Wedge));
    assert!(before.tail.as_ref().expect("tail").is_attached_to_outline(&before.frame));
}
