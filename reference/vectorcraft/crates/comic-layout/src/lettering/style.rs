//! What each lettering kind is drawn and set in.
//!
//! One record with a `kind` needs one place that says what the kind *means* visually, or the
//! meaning scatters across the command layer, the UI and the exporter. This is that place:
//! [`LetteringStyle`] gives each kind its shape, its tail, its face, its size, its padding and the
//! limits a balloon may be grown to.
//!
//! # The faces (ADR-017)
//!
//! | Kind | Face | Why |
//! |---|---|---|
//! | `speech` | Comic Neue Regular | dialogue, the face comics are lettered in |
//! | `thought` | Comic Neue Regular | the same words, in a cloud |
//! | `narration` | Comic Neue Italic | a caption is the narrator's voice, not a character's |
//! | `shout` | Comic Neue Bold | louder dialogue |
//! | `whisper` | Comic Neue Regular | quieter, drawn with a broken outline |
//! | `sound_effect` | Bangers Regular | display lettering, which is what Bangers is for |
//!
//! They are **defaults**, not law: a project may set different faces in
//! `ProjectSettings.default_lettering_style` (schema §5.3), which is why [`LetteringStyle`] holds
//! names and not constants. A build without them still lays out and reports (see
//! [`super::fonts`]).
//!
//! # The defaults
//!
//! Sizes, paddings and limits are in points and chosen for a comic page of about 468×720 pt — a
//! 6.5×10 inch page at 72 dpi. They are the numbers M1 needs to produce a readable page; a
//! template can override every one of them through the request, and none of them is baked into a
//! path or a measurement.

use vectorcraft_comic_model::TextAlign;

use super::fonts::{BOLD, DIALOGUE_FAMILY, DISPLAY_FAMILY, Face, ITALIC, REGULAR};
use super::outline::BalloonShape;
use super::tail::TailKind;
use crate::fit::FitTextStyle;
use crate::fit::sizing::{Limits, ResizeAnchor};

/// Dialogue's size, in points.
pub const DIALOGUE_SIZE: f64 = 12.0;
/// A caption's size, in points — smaller, as a caption box is read at a glance.
pub const NARRATION_SIZE: f64 = 10.5;
/// A whisper's size, in points.
pub const WHISPER_SIZE: f64 = 11.0;
/// A shout's size, in points.
pub const SHOUT_SIZE: f64 = 14.0;
/// A sound effect's size, in points — display lettering, so the largest of the six.
pub const SOUND_EFFECT_SIZE: f64 = 20.0;

/// Everything one kind of lettering is drawn and set in.
#[derive(Clone, Debug, PartialEq)]
pub struct LetteringStyle {
    /// The balloon's or caption's outer form.
    pub shape: BalloonShape,
    /// Which form of tail this kind is drawn with, when it has one.
    ///
    /// The **kind** decides whether there is a tail at all — only `speech`, `thought`, `shout` and
    /// `whisper` have someone to point at — and this field decides *which* form it takes: a wedge, or
    /// a chain of bubbles. A `narration` or `sound_effect` gets no tail whatever this says, and a
    /// spoken kind planned with `None` here is reported, because schema §6.4 requires
    /// `tail_node_id` for the kinds that have a tail (round 2, R2-F2).
    pub tail: Option<TailKind>,
    /// The face, size, alignment and padding the text is set in.
    pub text: FitTextStyle,
    /// The smallest and largest box this kind may be, in points.
    ///
    /// Each minimum is a box that holds **one line** of its kind's own type: a smaller balloon would
    /// be a balloon nothing can be written in, and a creator who drags one down to the minimum
    /// should still be able to read it. The search starts here, so a minimum the text cannot fit in
    /// would make every plan start by growing.
    pub limits: Limits,
    /// Which edge stays put when the box grows.
    pub anchor: ResizeAnchor,
    /// Whether the outline is drawn as a broken line — how a whisper is told from a speech balloon
    /// at a glance. A stroke property, not a second path.
    pub dashed: bool,
}

impl LetteringStyle {
    /// The face this style sets its type in, for the missing-font report.
    #[must_use]
    pub fn face(&self) -> Face {
        Face::new(self.text.family.clone(), self.text.style.clone())
    }
}

/// The style a kind is drawn in by default.
#[must_use]
pub fn style_of(kind: vectorcraft_comic_model::LetteringKind) -> LetteringStyle {
    use vectorcraft_comic_model::LetteringKind;

    match kind {
        LetteringKind::Speech => LetteringStyle {
            shape: BalloonShape::Ellipse,
            tail: Some(TailKind::Wedge),
            text: FitTextStyle::new(DIALOGUE_FAMILY, REGULAR, DIALOGUE_SIZE),
            limits: limits((48.0, 44.0), (280.0, 220.0)),
            anchor: ResizeAnchor::Centre,
            dashed: false,
        },
        LetteringKind::Thought => LetteringStyle {
            shape: BalloonShape::Cloud,
            tail: Some(TailKind::Bubble),
            text: {
                let mut text = FitTextStyle::new(DIALOGUE_FAMILY, REGULAR, DIALOGUE_SIZE);
                // A cloud's lobes bite further into the box than an ellipse does, so its text keeps
                // a little more distance from the outline.
                text.padding = 8.0;
                text
            },
            limits: limits((56.0, 60.0), (300.0, 230.0)),
            anchor: ResizeAnchor::Centre,
            dashed: false,
        },
        LetteringKind::Narration => LetteringStyle {
            shape: BalloonShape::Box,
            tail: None,
            text: {
                let mut text = FitTextStyle::new(DIALOGUE_FAMILY, ITALIC, NARRATION_SIZE);
                text.align = TextAlign::Left;
                text.padding = 5.0;
                text
            },
            // A caption is wide and shallow, and grows downward from the edge the artist placed:
            // a box that grew about its centre would climb out of the panel it captions.
            limits: limits((90.0, 26.0), (420.0, 150.0)),
            anchor: ResizeAnchor::TopCentre,
            dashed: false,
        },
        LetteringKind::Shout => LetteringStyle {
            shape: BalloonShape::Burst,
            tail: Some(TailKind::Wedge),
            text: {
                let mut text = FitTextStyle::new(DIALOGUE_FAMILY, BOLD, SHOUT_SIZE);
                text.padding = 6.0;
                text
            },
            limits: limits((64.0, 66.0), (340.0, 260.0)),
            anchor: ResizeAnchor::Centre,
            dashed: false,
        },
        LetteringKind::Whisper => LetteringStyle {
            shape: BalloonShape::Ellipse,
            tail: Some(TailKind::Wedge),
            text: FitTextStyle::new(DIALOGUE_FAMILY, REGULAR, WHISPER_SIZE),
            limits: limits((52.0, 42.0), (260.0, 200.0)),
            anchor: ResizeAnchor::Centre,
            dashed: true,
        },
        LetteringKind::SoundEffect => LetteringStyle {
            shape: BalloonShape::Burst,
            tail: None,
            text: {
                let mut text = FitTextStyle::new(DISPLAY_FAMILY, REGULAR, SOUND_EFFECT_SIZE);
                text.padding = 4.0;
                text
            },
            limits: limits((64.0, 72.0), (380.0, 260.0)),
            anchor: ResizeAnchor::Centre,
            dashed: false,
        },
    }
}

/// Limits from a minimum and a maximum pair, as the table above writes them.
fn limits(min: (f64, f64), max: (f64, f64)) -> Limits {
    Limits::new(vectorcraft_geom::Size::new(min.0, min.1), vectorcraft_geom::Size::new(max.0, max.1))
        .unwrap_or(Limits { min: vectorcraft_geom::Size::new(min.0, min.1), max: vectorcraft_geom::Size::new(max.0, max.1) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_comic_model::LetteringKind;

    const KINDS: [LetteringKind; 6] = [
        LetteringKind::Speech,
        LetteringKind::Thought,
        LetteringKind::Narration,
        LetteringKind::Shout,
        LetteringKind::Whisper,
        LetteringKind::SoundEffect,
    ];

    #[test]
    fn every_kinds_minimum_box_holds_one_line_of_its_own_type() {
        // The minimum is not decoration: it is where every search starts, and a creator can drag a
        // balloon down to it. A minimum that cannot hold one line would be a balloon nothing can be
        // written in — measured here rather than asserted from the numbers, because it depends on
        // the face, the size, the leading, the padding and the shape all at once.
        let db = vectorcraft_text::FontDb::global();
        for kind in KINDS {
            let style = style_of(kind);
            let min = vectorcraft_geom::Rect::new(0.0, 0.0, style.limits.min.width, style.limits.min.height);
            let frame = crate::fit::text_frame(min, style.shape.inner_scale(), style.text.padding).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            let measured =
                crate::fit::measure(db, "H", &style.text, &frame, vectorcraft_doc::AreaFit::None).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            assert!(measured.fits(), "{kind:?}: the minimum box cannot hold one line of its own type");
            assert!(style.limits.max.width >= style.limits.min.width, "{kind:?}");
            assert!(style.limits.max.height >= style.limits.min.height, "{kind:?}");
            assert!(style.limits.is_ordered(), "{kind:?}");
        }
    }

    #[test]
    fn a_caption_is_a_box_with_no_tail_and_a_balloon_has_one() {
        // The tail is what makes a balloon a balloon: it points at whoever is speaking, and a
        // caption has nobody to point at (schema §6.4, `tail_node_id`).
        assert_eq!(style_of(LetteringKind::Narration).shape, BalloonShape::Box);
        assert_eq!(style_of(LetteringKind::Narration).tail, None);
        assert_eq!(style_of(LetteringKind::SoundEffect).tail, None);
        for kind in [LetteringKind::Speech, LetteringKind::Thought, LetteringKind::Shout, LetteringKind::Whisper] {
            assert!(style_of(kind).tail.is_some(), "{kind:?} has nobody to point at?");
        }
    }

    #[test]
    fn a_caption_is_anchored_at_its_top_and_a_balloon_at_its_centre() {
        assert_eq!(style_of(LetteringKind::Narration).anchor, ResizeAnchor::TopCentre);
        for kind in [LetteringKind::Speech, LetteringKind::Thought, LetteringKind::Shout] {
            assert_eq!(style_of(kind).anchor, ResizeAnchor::Centre, "{kind:?}");
        }
    }

    #[test]
    fn the_faces_are_the_ones_adr_017_names_for_each_kind() {
        assert_eq!(style_of(LetteringKind::Speech).face(), Face::new(DIALOGUE_FAMILY, REGULAR));
        assert_eq!(style_of(LetteringKind::Narration).face(), Face::new(DIALOGUE_FAMILY, ITALIC));
        assert_eq!(style_of(LetteringKind::Shout).face(), Face::new(DIALOGUE_FAMILY, BOLD));
        assert_eq!(style_of(LetteringKind::SoundEffect).face(), Face::new(DISPLAY_FAMILY, REGULAR));
    }

    #[test]
    fn a_whisper_is_the_only_broken_outline() {
        assert!(style_of(LetteringKind::Whisper).dashed);
        for kind in [LetteringKind::Speech, LetteringKind::Thought, LetteringKind::Narration, LetteringKind::Shout] {
            assert!(!style_of(kind).dashed, "{kind:?} is not drawn broken");
        }
    }

    #[test]
    fn the_kinds_that_carry_their_own_text_are_set_smaller_than_the_ones_that_are_said() {
        // A sound effect is display lettering and the largest; a caption is the smallest, because a
        // caption is read in passing. Both are checked here so a later edit to one constant cannot
        // quietly invert the scale.
        assert!(style_of(LetteringKind::SoundEffect).text.size > style_of(LetteringKind::Shout).text.size);
        assert!(style_of(LetteringKind::Shout).text.size > style_of(LetteringKind::Speech).text.size);
        assert!(style_of(LetteringKind::Narration).text.size < style_of(LetteringKind::Speech).text.size);
    }

    #[test]
    fn every_style_leads_at_120_per_cent_of_its_size() {
        for kind in KINDS {
            let text = style_of(kind).text;
            let leading = text.leading.expect("an explicit leading, not Auto");
            assert!((leading - text.size * 1.2).abs() < 1e-9, "{kind:?}: {leading} for size {}", text.size);
        }
    }
}
