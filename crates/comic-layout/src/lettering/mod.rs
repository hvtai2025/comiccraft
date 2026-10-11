//! Lettering geometry: the balloon a dialogue is held in, the caption box a narration sits in, the
//! tail that points at the speaker, and the frame the text flows into.
//!
//! **This tree computes and returns a plan. It never takes `&mut Document`.** The command layer
//! creates the nodes and writes the record from the plan (design §8), which is what makes a
//! lettering operation one undoable step.
//!
//! # The objects each kind plans
//!
//! `Lettering` is one record with a `kind` (schema §6.4), and each kind needs its own set of
//! objects. `frame` is always a closed, editable [`PathData`]; `tail` is present exactly for the
//! kinds that have someone to point at; `text_frame` is the area text frame.
//!
//! | Kind | Shape | Tail | Type | Record fields it fills |
//! |---|---|---|---|---|
//! | `speech` | ellipse | wedge | Comic Neue Regular | `frame_node_id`, `tail_node_id`, `text_node_id` |
//! | `thought` | cloud | bubble chain | Comic Neue Regular | the same three |
//! | `narration` | caption box | none | Comic Neue Italic | `frame_node_id`, `text_node_id` |
//! | `shout` | burst | wedge | Comic Neue Bold | the same three |
//! | `whisper` | ellipse, broken outline | wedge | Comic Neue Regular | the same three |
//! | `sound_effect` | burst | none | Bangers Regular | `frame_node_id`, `text_node_id` |
//!
//! The typography is the schema's own `LetteringKind` — there is no second enum here — and the
//! objects are described by geometry only. A [`NodeId`](vectorcraft_comic_model::NodeId) is minted
//! by the engine, never here (design §5.2.1, §9).
//!
//! # The text belongs to the record
//!
//! The visual text object is a *rendering* of the words, never a second source of truth
//! (design §6.1). [`TextSource`] names which record owns them — the linked `Dialogue` for a spoken
//! kind, the `Lettering` itself for a caption — and a plan holds no text at all.
//!
//! # Entry points
//!
//! * [`new_balloon`] — a new balloon or caption, sized to the text it must hold.
//! * [`refit_balloon`] — an existing one, after its text or frame changed, with the record's
//!   [`OverflowPolicy`](vectorcraft_comic_model::OverflowPolicy) applied as a plan.
//! * [`LetteringPlan::moved_by`] — a balloon moved by a drag, with its tail still attached.

pub mod fonts;
pub mod outline;
pub mod plan;
pub mod style;
pub mod tail;

use vectorcraft_comic_model::LetteringKind;

pub use fonts::{BOLD, DIALOGUE_FAMILY, DISPLAY_FAMILY, Face, ITALIC, MAX_FACES, MissingFace, REGULAR, missing_face_findings, missing_faces};
pub use outline::BalloonShape;
pub use plan::{LetteringPlan, LetteringRequest, TextOwner, TextSource, new_balloon, refit_balloon};
pub use style::{LetteringStyle, style_of};
pub use tail::{Tail, TailKind};

/// What each schema `LetteringKind` is drawn as.
///
/// An extension trait rather than an inherent `impl`, because `LetteringKind` belongs to
/// `comic-model` and is frozen there (schema §5.5). `is_spoken` and `carries_own_text` are the
/// model's own methods and are deliberately **not** repeated here.
pub trait LetteringKindExt {
    /// The balloon or caption outline this kind is drawn as.
    fn shape(self) -> BalloonShape;

    /// The tail this kind has, or `None` when it has nobody to point at.
    fn tail_kind(self) -> Option<TailKind>;

    /// The schema's own name for the kind, as `LetteringKind` serialises it.
    fn name(self) -> &'static str;

    /// The default style: shape, tail, face, size, padding and limits.
    fn lettering_style(self) -> LetteringStyle;
}

impl LetteringKindExt for LetteringKind {
    fn shape(self) -> BalloonShape {
        style_of(self).shape
    }

    fn tail_kind(self) -> Option<TailKind> {
        style_of(self).tail
    }

    fn name(self) -> &'static str {
        match self {
            Self::Speech => "speech",
            Self::Thought => "thought",
            Self::Narration => "narration",
            Self::Shout => "shout",
            Self::Whisper => "whisper",
            Self::SoundEffect => "sound_effect",
        }
    }

    fn lettering_style(self) -> LetteringStyle {
        style_of(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [LetteringKind; 6] = [
        LetteringKind::Speech,
        LetteringKind::Thought,
        LetteringKind::Narration,
        LetteringKind::Shout,
        LetteringKind::Whisper,
        LetteringKind::SoundEffect,
    ];

    #[test]
    fn a_kind_has_a_tail_exactly_when_it_is_spoken() {
        // The two facts agree, and they are derived from different places — the model's rule 8 and
        // this module's drawing table — so asserting they agree is asserting the table is right.
        for kind in KINDS {
            assert_eq!(kind.tail_kind().is_some(), kind.is_spoken(), "{kind:?}");
        }
    }

    #[test]
    fn a_kind_that_carries_its_own_text_is_a_box_or_a_burst_with_no_tail() {
        for kind in KINDS {
            if kind.carries_own_text() {
                assert!(kind.tail_kind().is_none(), "{kind:?}");
                assert!(matches!(kind.shape(), BalloonShape::Box | BalloonShape::Burst), "{kind:?}");
            }
        }
    }

    #[test]
    fn the_kind_names_are_the_schemas_own_strings() {
        // The subject of a finding and the record's JSON must name the same thing.
        let json = serde_json::to_string(&LetteringKind::SoundEffect).expect("serialisable");
        assert_eq!(json, "\"sound_effect\"");
        assert_eq!(LetteringKind::SoundEffect.name(), "sound_effect");
        for kind in KINDS {
            assert_eq!(format!("\"{}\"", kind.name()), serde_json::to_string(&kind).expect("serialisable"), "{kind:?}");
        }
    }

    #[test]
    fn the_extension_trait_agrees_with_the_style_table() {
        for kind in KINDS {
            assert_eq!(kind.shape(), style_of(kind).shape);
            assert_eq!(kind.tail_kind(), style_of(kind).tail);
            assert_eq!(kind.lettering_style(), style_of(kind));
        }
    }
}
