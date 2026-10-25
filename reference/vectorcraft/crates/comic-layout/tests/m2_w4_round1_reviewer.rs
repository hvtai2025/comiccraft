//! M2 W4, reviewer round 1 — the lettering face contract and the balloon move, attacked.
//!
//! These tests are **not** copies of the implementer's. Each one is aimed at a way their suite
//! could pass while the code is wrong:
//!
//! * [`the_message_never_rules_out_craft_fonts_for_a_family_craft_fonts_carries`] builds the case the
//!   new `is_comic_family` branch gets wrong: a family **craft-fonts carries** that is not one of
//!   ADR-017's two comic faces. The message rules craft-fonts out for it, and in a build that has
//!   craft-fonts the face standing in came *from* craft-fonts — so the sentence contradicts the state
//!   it is read in. This is the one test here that fails, and it fails on the defect; it goes green
//!   when the clause stops being asserted without being checked. It is unconditional: the family is
//!   absent in the gate's configuration (`missing`) and present in a craft-fonts build (`substitute`),
//!   and the clause is a function of the family in both.
//! * [`the_clause_that_says_where_the_face_belongs_is_a_function_of_the_family_and_not_of_the_status`]
//!   pins the record's claim that nothing is inferred from the status, in the direction a mutant
//!   would move: a version that sent a `substitute` of *any* family to craft-fonts reds it.
//! * [`a_move_re_bases_the_tail_on_the_translated_edited_path_and_never_on_the_box`] is a second
//!   discriminating move test, on a different kind (`thought`) and a different edit direction, and it
//!   asserts the base by an **independent derivation** — the pre-move base translated — rather than by
//!   re-running the implementation's own formula.
//! * [`the_front_half_of_the_message_is_the_exporters_four_fields_in_both_branches`] pins the half
//!   the M2-L3 fix was supposed to leave alone, so "surgical" is evidence rather than a claim.

// `clippy.toml` allows `expect` in tests, which covers `#[test]` functions but not the helpers
// this file uses to build cases. The convention for a review suite with helpers is the file-level
// allow, as in `crates/comic-model/tests/m2_w1_migration_review.rs`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use vectorcraft_comic_layout::lettering::fonts::{MissingFace, missing_face_finding, missing_faces};
use vectorcraft_comic_layout::lettering::outline::BalloonShape;
use vectorcraft_comic_layout::lettering::plan::{LetteringRequest, TextSource, new_balloon};
use vectorcraft_comic_layout::lettering::style::style_of;
use vectorcraft_comic_layout::lettering::tail::attach_point;
use vectorcraft_comic_layout::lettering::{BOLD, DIALOGUE_FAMILY, DISPLAY_FAMILY, Face, REGULAR};
use vectorcraft_comic_model::LetteringKind;
use vectorcraft_geom::{Point, Vec2};
use vectorcraft_text::{CRAFT_FONTS, FALLBACK_FAMILY, FontDb, FontMatch};

/// A style no file in any configuration carries, so every family below is reported **in both**
/// configurations: `missing` without craft-fonts, `substitute` with it. The clause the message
/// appends is a function of the family alone, so building the case with an absent style is the
/// configuration-independent way to ask what the message says.
const ABSENT_STYLE: &str = "No Such Weight 4f2a";

/// The families **craft-fonts carries** that are not ADR-017's two comic faces, read from the
/// companion checkout's `fonts/manifest.txt` (`storytold/craft-fonts`).
///
/// Hard-coded so the case can be built in the gate's configuration, where `CRAFT_FONTS` is empty and
/// no family can be asked of the build. When the build *does* have craft-fonts the list is checked
/// against it, so a name craft-fonts no longer carries cannot stay here unnoticed — and a family that
/// had become a comic face could not either.
const OTHER_CRAFT_FONTS_FAMILIES: [&str; 5] = ["BIZ UDMincho", "BIZ UDPGothic", "Noto Sans Arabic", "Noto Sans CJK SC", "Shippori Mincho"];

/// The finding a plan reports about the face it asked for — the missing-face report, told apart from
/// the findings about the text, the box and the tail.
fn face_reports(plan: &vectorcraft_comic_layout::lettering::plan::LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

fn spoken_request<'a>(kind: LetteringKind, text: &'a str) -> LetteringRequest<'a> {
    LetteringRequest::new(kind, TextSource::Dialogue(text)).placed_at(Point::new(200.0, 150.0)).with_tail(Point::new(160.0, 320.0))
}

/// **The finding.** A creator whose lettering face is a family craft-fonts carries — but not one of
/// ADR-017's two — is told *"craft-fonts is not where it belongs: it has to be installed, or another
/// face chosen"*. Reached through `Lettering.text_layout_settings.font`,
/// `ProjectSettings.default_lettering_style` (the reader in
/// `crates/engine/src/cmd/comic/lettering.rs` puts the name straight into the plan style) or
/// `comic.lettering.add`'s `style.font`, so it is what a creator reads, not a message with no
/// caller.
///
/// The claim is false, and in a build that has craft-fonts it is false against the message's own
/// other half: the face named as "the face shown instead" was loaded **from craft-fonts**, so the
/// sentence says craft-fonts is not where the family belongs while craft-fonts is the only reason it
/// resolved at all. In the gate's configuration the same sentence sends the creator past the one fix
/// that would work (`CRAFT_FONTS_DIR`).
///
/// What is asserted is deliberately the minimum: the message may say what the family is not, and may
/// send the creator to install it — it may not rule out the checkout that carries it.
#[test]
fn the_message_never_rules_out_craft_fonts_for_a_family_craft_fonts_carries() {
    let db = FontDb::global();
    for family in OTHER_CRAFT_FONTS_FAMILIES {
        if !CRAFT_FONTS.is_empty() {
            assert!(
                CRAFT_FONTS.iter().any(|font| font.family == family),
                "{family} is not in this craft-fonts build, so the list of families craft-fonts carries is stale"
            );
            assert!(
                !vectorcraft_text::craft_fonts::comic_fonts().iter().any(|font| font.family == family),
                "{family} reaches a build as a comic face, so it is not the case this test is about"
            );
        }
        let face = Face::new(family, ABSENT_STYLE);
        let missing = missing_faces(db, std::slice::from_ref(&face));
        assert_eq!(missing.len(), 1, "{family} {ABSENT_STYLE} is available in this build, so the case cannot be built: {missing:?}");
        let message = missing_face_finding(&missing[0], "lettering speech").message;
        assert!(
            !message.contains("craft-fonts is not where it belongs"),
            "{family} is a family craft-fonts carries, so the message must not rule craft-fonts out: {message}"
        );

        // And the same through the lettering API, because that is the route a creator's command
        // takes: the plan's finding is the message the command layer writes for.
        let mut request = spoken_request(LetteringKind::Speech, "Look out!");
        request.style.text.family = family.to_string();
        request.style.text.style = ABSENT_STYLE.to_string();
        let plan = new_balloon(db, &request).expect("planned");
        let reports = face_reports(&plan);
        assert_eq!(reports.len(), 1, "{family}: {reports:?}");
        assert!(
            !reports[0].contains("craft-fonts is not where it belongs"),
            "{family}: the plan carries the message a creator reads, so it must not rule craft-fonts out: {reports:?}"
        );
    }
}

/// The record's claim, in the direction a mutant would move: the clause after the semicolon is a
/// function of the **family**, and the status is not read to decide it. A version that sent the
/// `substitute` of *any* family to craft-fonts (`is_comic_family(family) || status == Style`) reds
/// this, and it is the mutation the record says would be wrong for the most likely real case.
#[test]
fn the_clause_that_says_where_the_face_belongs_is_a_function_of_the_family_and_not_of_the_status() {
    for family in [DIALOGUE_FAMILY, DISPLAY_FAMILY, "No Such Lettering Face"] {
        let clause = |status: FontMatch| {
            let face = MissingFace {
                family: family.to_string(),
                style: REGULAR.to_string(),
                status,
                resolved: Some((FALLBACK_FAMILY.to_string(), REGULAR.to_string())),
            };
            let message = missing_face_finding(&face, "lettering speech").message;
            message.split_once("; ").expect("the message is a front half and a clause").1.to_string()
        };
        assert_eq!(clause(FontMatch::Missing), clause(FontMatch::Style), "{family}: the clause changed with the status");
        assert!(clause(FontMatch::Missing).contains(family), "{family}: the clause does not name the family");
        assert_eq!(
            clause_for(family).contains("is one of ADR-017's comic faces"),
            is_comic(family),
            "{family}: the clause does not follow the family"
        );
    }
}

/// Whether the family is ADR-017's — read from the two constants rather than from the function under
/// test, so the assertion above compares two things and not one.
fn is_comic(family: &str) -> bool {
    family == DIALOGUE_FAMILY || family == DISPLAY_FAMILY
}

fn clause_for(family: &str) -> String {
    let face = MissingFace {
        family: family.to_string(),
        style: REGULAR.to_string(),
        status: FontMatch::Missing,
        resolved: Some((FALLBACK_FAMILY.to_string(), REGULAR.to_string())),
    };
    missing_face_finding(&face, "lettering speech").message.split_once("; ").expect("a clause").1.to_string()
}

/// The move, on a kind the implementer's discriminating test does not use, with the base checked
/// against an **analytic** answer rather than against the implementation's own formula.
///
/// A tail is two facts, and the criterion is about both: the tip is **preserved** (the speaker did
/// not move) and the base is **updated** onto the outline the balloon now has. What separates the two
/// "updated" implementations is *which* outline: the editable path the plan carries, or the shape the
/// kind is drawn as. The plan's frame is a rectangle here while the kind's shape is a cloud — what a
/// creator dragging an anchor produces — so the two candidates are different points.
///
/// The implementer's test checks the base against `attach_point(&moved.frame, tip, moved.bounds)`,
/// which is the implementation's own formula. This one derives the answer independently: the edited
/// outline **is a rectangle** (`BalloonShape::Box.outline`), and the nearest point of a rectangle to a
/// point outside it is that point clamped to the rectangle — arithmetic that does not run
/// `attach_point` at all. (A first attempt at this derivation asserted the base equals the pre-move
/// base translated, which is wrong: the *tip* does not move with the balloon, so the nearest point of
/// the moved path is a different relative point. Recorded because that mistake is the tempting one.)
#[test]
fn a_move_re_bases_the_tail_on_the_translated_edited_path_and_never_on_the_kinds_own_shape() {
    let db = FontDb::global();
    let before = new_balloon(db, &spoken_request(LetteringKind::Thought, "Hmm, I wonder about that.")).expect("planned");
    let tip = before.tail.as_ref().expect("a thought balloon has a tail").tip;
    let base_before_the_edit = before.tail.as_ref().expect("tail").attach;

    // The creator's edit: the cloud's editable outline replaced by a rectangle, off its default
    // shape. `BalloonShape::Box.outline` is the same constructor the plan itself uses.
    let mut edited = before.clone();
    edited.frame = BalloonShape::Box.outline(before.bounds);
    assert_ne!(edited.frame, before.frame, "the edit did not change the path");

    // A move up and to the left, and a large one: the balloon leaves the tip behind, so "on the
    // moved path" is a statement about the move and not about a translation too small to tell apart.
    let delta = Vec2::new(-360.0, 420.0);
    let moved = edited.moved_by(delta).expect("moved");
    let tail = moved.tail.as_ref().expect("the tail survived the move");

    let outside = tip.x < moved.bounds.x0 || tip.x > moved.bounds.x1 || tip.y < moved.bounds.y0 || tip.y > moved.bounds.y1;
    assert!(outside, "the tip is inside the moved balloon, so the analytic form below does not apply: tip {tip:?}, bounds {:?}", moved.bounds);
    let analytic = Point::new(tip.x.clamp(moved.bounds.x0, moved.bounds.x1), tip.y.clamp(moved.bounds.y0, moved.bounds.y1));
    assert!(
        (tail.attach - analytic).hypot() < 1e-6,
        "the base is {:?}, and the nearest point of the rectangle the path is sits at {analytic:?}",
        tail.attach
    );

    // The shape the kind is drawn as is *not* what the tail was re-based on, and the two answers are
    // far enough apart that the assertion is about the implementation and not about a rounding.
    let on_the_shape = attach_point(&style_of(LetteringKind::Thought).shape.outline(moved.bounds), tip, moved.bounds);
    assert!((on_the_shape - analytic).hypot() > 1.0, "this balloon and target make the two candidate bases agree, so they cannot be told apart");
    assert_eq!(tail.attach, analytic, "the base was re-based on the kind's shape rather than on the path the balloon has");

    // Preserved, and attached to *this* balloon: the plan's own path, not the one it left.
    assert_eq!(tail.tip, tip, "the tip moved with the balloon");
    assert_ne!(tail.attach, base_before_the_edit, "the base did not follow the balloon");
    assert!(tail.is_attached_to_outline(&moved.frame), "the tail is not on the path the balloon has");
    assert!(!tail.is_attached_to_outline(&before.frame), "the tail is still on the outline the balloon left");
    assert_eq!(moved.frame, edited.frame.transformed(vectorcraft_geom::Affine::translate(delta)), "the edited path was rebuilt, not translated");
    assert!(moved.frame.is_closed());
}

/// The half the M2-L3 fix must not have touched: the family, the style, the exporter's status name
/// and the face that stands in — the four fields `text.missingFonts` reports — in **both** branches
/// and for both statuses.
#[test]
fn the_front_half_of_the_message_is_the_exporters_four_fields_in_both_branches() {
    for family in [DIALOGUE_FAMILY, DISPLAY_FAMILY, "No Such Lettering Face"] {
        for status in [FontMatch::Missing, FontMatch::Style] {
            let shown = (FALLBACK_FAMILY.to_string(), REGULAR.to_string());
            let face = MissingFace { family: family.to_string(), style: ABSENT_STYLE.to_string(), status, resolved: Some(shown.clone()) };
            let message = missing_face_finding(&face, "lettering speech").message;
            let front =
                format!("the lettering face {family} {ABSENT_STYLE} is not available ({}) and shows in {} {}", status.as_str(), shown.0, shown.1);
            assert!(message.starts_with(&front), "the four fields the exporter reports are not the first thing said: {message}");
        }
    }
    // And the face that stands in is the one the record names, not the requested one written twice.
    let face = MissingFace {
        family: "No Such Lettering Face".to_string(),
        style: BOLD.to_string(),
        status: FontMatch::Missing,
        resolved: Some((FALLBACK_FAMILY.to_string(), "Bold".to_string())),
    };
    let message = missing_face_finding(&face, "lettering speech").message;
    assert!(message.contains(&format!("and shows in {FALLBACK_FAMILY} Bold")), "{message}");
}
