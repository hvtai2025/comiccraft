//! Fitting lettering text into its frame — the measurement half of lettering.
//!
//! **This module does not lay text out.** Every measurement is
//! [`vectorcraft_text::layout_with`], and every answer comes from reading [`TextLayout`] back
//! (design §8.3, audit §8):
//!
//! | Question | Field read |
//! |---|---|
//! | Does the text overflow? | `TextLayout::overflow` — the mechanism design §12.1 names |
//! | Where is the text? | `TextLayout::bounds`, and each line's `baseline` / `descent` |
//! | How much did shrink-to-fit scale it? | `TextLayout::fit_scale` |
//!
//! If a case ever appears that these three cannot answer, the right move is a failing test and a
//! conversation with the lead — not a wrapping loop here. Two behaviours of the engine were
//! measured before anything was built on them, and both are pinned by tests in this module: a word
//! too long for its frame is broken **between characters** rather than overflowed, and a frame too
//! narrow for even one character drops the line and sets `overflow`.
//!
//! The text is handed in as `&str` and never stored: the `Dialogue` record owns the words and the
//! visual text object is a *rendering* of them (design §6.1). A plan built from here carries
//! geometry and [`vectorcraft_comic_model::TextLayoutSettings`], and no text.
//!
//! # The two jobs
//!
//! * [`sizing::size_to_text`] — a balloon being created: the smallest box that holds the text at
//!   the style's own size, inside the kind's minimum and maximum.
//! * [`policy::apply`] — a balloon that already exists: the text no longer fits, so apply the
//!   record's [`vectorcraft_comic_model::OverflowPolicy`].
//!
//! Both are deterministic (no AI, no randomness, no clock) and both return what the command layer
//! should write instead of writing it.

pub mod policy;
pub mod sizing;

use vectorcraft_comic_model::{Finding, TextAlign, TextLayoutSettings};
use vectorcraft_doc::{AreaFit, CharStyle, Justify, ParaStyle, TextKind, TextObject};
use vectorcraft_geom::{Point, Rect, Size, shapes};
use vectorcraft_text::{FontDb, LayoutOptions, TextLayout, layout_with};

pub use policy::{FitDecision, FitRequest, apply, collision};
pub use sizing::{Limits, Placement, ResizeAnchor, SizeRequest, Sized, size_to_text};

/// What applying a policy, or sizing a new box, resolved to.
///
/// One vocabulary for both jobs, because the caller's next step is the same either way: write the
/// box and the [`AreaFit`] the plan carries, and show the findings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FitOutcome {
    /// The text fits its frame as it stands.
    Fits {
        /// The engine's scale factor, `1.0` when nothing was scaled.
        fit_scale: f64,
    },
    /// It did not, and the policy is `warn`: reported, nothing changed.
    Warned,
    /// It did not, and the policy is `shrink`: `vectorcraft-text` scaled the type by `fit_scale`.
    /// `fits` is false when even the smallest size the policy allows still overflows.
    Shrunk {
        /// `TextLayout::fit_scale`: what the engine scaled the type by.
        fit_scale: f64,
        /// Whether the scaled text is inside the frame.
        fits: bool,
    },
    /// It did not, and the policy is `resize_frame`: the frame grew to `bounds`.
    Resized {
        /// The frame after growing.
        bounds: Rect,
    },
    /// It did not, and growing the frame would have overlapped the obstacle at this index in the
    /// caller's list. Reported, not applied.
    Blocked {
        /// The index of the offending obstacle in the caller's slice.
        obstacle: usize,
    },
    /// It did not fit any frame within the kind's limits.
    Unfittable,
}

impl FitOutcome {
    /// Whether the plan's box holds its text as it stands.
    #[must_use]
    pub fn fits(self) -> bool {
        matches!(self, Self::Fits { .. } | Self::Resized { .. } | Self::Shrunk { fits: true, .. })
    }
}

/// Why a fit could not be attempted at all.
///
/// Never returned for text that does not fit — that is a [`FitOutcome`] and a `Finding`, because a
/// creator can act on it and a balloon that reports is better than a command that fails.
#[derive(Clone, Debug, PartialEq)]
pub enum FitError {
    /// A coordinate or size was NaN or infinite.
    NotFinite { field: &'static str },
    /// A size was zero or negative where a positive one is required.
    NonPositive { field: &'static str },
    /// A frame's minimum exceeded its maximum.
    LimitsInverted,
    /// The rectangle a shape leaves inside a box is not big enough for the padding, so no text
    /// could ever be set in it. Reported rather than silently clamped: it means the style's
    /// padding and the kind's minimum disagree, which is a style bug worth fixing.
    PaddingTooLarge { padding: f64, inner: f64 },
    /// The text source does not match the lettering kind (schema §6.4, rule 8): a `speech` balloon
    /// is given the linked `Dialogue`'s words, a `narration` its own.
    TextSourceMismatch { kind: &'static str, spoken: bool },
}

impl std::fmt::Display for FitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite { field } => write!(f, "lettering {field} is not a finite number"),
            Self::NonPositive { field } => write!(f, "lettering {field} must be greater than zero"),
            Self::LimitsInverted => write!(f, "the lettering style's minimum frame size is larger than its maximum"),
            Self::PaddingTooLarge { padding, inner } => write!(f, "lettering padding {padding} leaves no room inside a {inner}-point frame"),
            Self::TextSourceMismatch { kind, spoken } => write!(
                f,
                "a {kind} lettering takes its text {} (schema §6.4 rule 8)",
                if *spoken { "from the linked Dialogue" } else { "in its own text field" }
            ),
        }
    }
}

impl std::error::Error for FitError {}

/// The text to fit and the type to fit it in.
///
/// Every length is in points, matching [`TextLayoutSettings`] — except `tracking`, which is in
/// 1/1000 em exactly as [`vectorcraft_doc::CharStyle::tracking`] is, because this value is written
/// into that field.
#[derive(Clone, Debug, PartialEq)]
pub struct FitTextStyle {
    /// A family name resolved through the font database.
    pub family: String,
    /// The face style within it: `Regular`, `Bold`, `Italic`.
    pub style: String,
    /// Size in points.
    pub size: f64,
    /// Leading in points; `None` is the engine's Auto (120% of the size).
    pub leading: Option<f64>,
    /// Tracking in 1/1000 em.
    pub tracking: f64,
    /// Alignment inside the frame.
    pub align: TextAlign,
    /// Padding between the frame and the text, in points (`LayoutOptions::inset`).
    pub padding: f64,
    /// Shrink Text to Fit's floor as a percentage, 10..100.
    pub shrink_percent: f64,
}

impl FitTextStyle {
    /// A style with the given face, size and sensible comic defaults.
    #[must_use]
    pub fn new(family: impl Into<String>, style: impl Into<String>, size: f64) -> Self {
        Self {
            family: family.into(),
            style: style.into(),
            size,
            leading: Some(size * 1.2),
            tracking: 0.0,
            align: TextAlign::Center,
            padding: 7.0,
            shrink_percent: 50.0,
        }
    }

    /// Checks the numbers a layout call needs, so a bad style is an error rather than a NaN that
    /// spreads into a path.
    pub(crate) fn validate(&self) -> Result<(), FitError> {
        for (value, field) in [(self.size, "text size"), (self.tracking, "tracking"), (self.padding, "padding")] {
            if !value.is_finite() {
                return Err(FitError::NotFinite { field });
            }
        }
        if self.size <= 0.0 {
            return Err(FitError::NonPositive { field: "text size" });
        }
        if self.padding < 0.0 {
            return Err(FitError::NonPositive { field: "padding" });
        }
        if let Some(leading) = self.leading
            && !(leading.is_finite() && leading > 0.0)
        {
            return Err(FitError::NonPositive { field: "leading" });
        }
        // `shrink_percent` is fed to `AreaFit::parse`, which replaces a non-finite value with its
        // own default (50) and clamps the rest into 10..=100. Replacing a NaN silently would hide a
        // data bug behind a plausible number, so a non-finite percent is refused here; an
        // out-of-range one is *not* refused, because the engine clamps it deliberately and the plan
        // and the finding both quote the clamped value (round 1, F2).
        if !self.shrink_percent.is_finite() {
            return Err(FitError::NotFinite { field: "shrink percent" });
        }
        Ok(())
    }
}

/// A measurement: the engine's layout plus the frame it was laid out in.
///
/// Holding the frame and the inset here, rather than re-deriving them, is what makes
/// [`Measured::fits`] and [`Measured::needed_height`] answers about *this* layout.
#[derive(Clone, Debug)]
pub struct Measured {
    /// What `layout_with` returned.
    pub layout: TextLayout,
    /// The frame it was laid out in, in document points.
    pub frame: Rect,
    /// The padding it was laid out with (`LayoutOptions::inset`).
    pub inset: f64,
}

impl Measured {
    /// Whether the engine placed all of the text inside the frame.
    ///
    /// This is [`TextLayout::overflow`] and nothing else — the mechanism design §12.1 names for the
    /// overflow check, so a balloon's answer and a validation report's answer are the same answer.
    ///
    /// Two things about the engine's own behaviour are worth stating, because both were measured
    /// rather than assumed (see this module's tests):
    ///
    /// * A word too long for the frame is **broken between characters**, so `Unforgettable` is set
    ///   in a narrow frame rather than overflowing it. Widening a balloon for a long word is
    ///   therefore a matter of *taste* (see [`crate::fit::sizing`]), not of avoiding a false report.
    /// * A frame too narrow for even one character drops the line and sets `overflow`, with no lines
    ///   at all in the result — which is what makes the flag sufficient here.
    #[must_use]
    pub fn fits(&self) -> bool {
        !self.layout.overflow
    }

    /// The lowest point of the block: the last line's baseline plus its descent.
    ///
    /// The descent, not the ink bottom, because that is the value the engine tests its own fit
    /// against (`baseline + descent > bottom`); the ink would call a line placed that the engine
    /// drops.
    ///
    /// **With no lines at all** — a frame too narrow to place one character, which is the state
    /// [`Measured::fits`] documents — the block has nothing below the cell's top, so the bottom *is*
    /// the cell's top and [`Measured::needed_height`] comes out as the two paddings: a height, not
    /// the negative number `TextLayout.bounds.y1` (zero in text space) would produce (round 1, F5).
    #[must_use]
    pub fn bottom(&self) -> f64 {
        self.layout.lines.last().map_or(self.frame.y0 + self.inset, |line| line.baseline + line.descent)
    }

    /// The frame height this block needs, insets included.
    ///
    /// The layout starts the block at the frame's inner top (`FirstBaseline::Ascent`), so the height
    /// it needs is the distance from that top to its lowest descent, plus the padding above and
    /// below. The frame's own top is where the cell top comes from, which is why the frame is stored
    /// with the measurement.
    #[must_use]
    pub fn needed_height(&self) -> f64 {
        let top = self.frame.y0 + self.inset;
        (self.bottom() - top) + 2.0 * self.inset
    }

    /// The ink extent `TextLayout::bounds` reports, in document points.
    #[must_use]
    pub fn text_bounds(&self) -> Rect {
        self.layout.bounds
    }
}

/// Lay `text` out inside `frame` and return everything read back from the result.
///
/// The caller supplies the [`AreaFit`], because the fit *is* the policy: `None` asks the text to
/// overflow and report it, `ShrinkText` asks the engine to scale it down (audit §8).
///
/// The text object is built with an identity `xf`, so text space and document space coincide and
/// [`TextLayout::bounds`] is directly comparable with `frame`. That is what lets a measurement be
/// treated as geometry without transforming anything.
pub fn measure(db: &FontDb, text: &str, style: &FitTextStyle, frame: &Rect, fit: AreaFit) -> Result<Measured, FitError> {
    style.validate()?;
    validate_rect("frame", *frame)?;

    let mut char_style = CharStyle::default();
    char_style.font_family.clone_from(&style.family);
    char_style.font_style.clone_from(&style.style);
    char_style.size = style.size;
    char_style.leading = style.leading;
    char_style.tracking = style.tracking;

    let mut object = TextObject::point(Point::ZERO, text, char_style);
    object.kind = TextKind::Area { frame: shapes::rectangle(*frame) };
    object.para = ParaStyle { justify: justify(style.align), ..ParaStyle::default() };
    object.area.inset = style.padding;
    object.area.fit = fit;

    let options = LayoutOptions { inset: style.padding, fit, ..LayoutOptions::default() };
    let layout = layout_with(db, &object, &options);
    Ok(Measured { layout, frame: *frame, inset: style.padding })
}

/// The rectangle a shape leaves inside its box: the largest rectangle the outline guarantees, with
/// the same centre and aspect.
///
/// A balloon's text frame is a **rectangle**, not the outline. The inscribed rectangle is the
/// region every point of which is inside the outline, so no line of text can spill into a cloud's
/// lobes or a burst's spikes; an outline frame would also leave the layout engine's region cells
/// disagreeing with the shape the reader sees.
#[must_use]
pub fn inner_rect(bounds: Rect, inner_scale: f64) -> Rect {
    Rect::from_center_size(bounds.center(), Size::new(bounds.width() * inner_scale, bounds.height() * inner_scale))
}

/// The frame the text flows inside, for a shape's inner scale and a style's padding.
///
/// The padding is **not** taken off here. It is applied once, by the layout itself
/// (`LayoutOptions::inset`, written from `TextLayoutSettings.padding`); subtracting it here as well
/// would leave a frame whose text is inset twice — half the room the box promises, and a balloon
/// that grows to its maximum for a caption that fits in a third of it. `padding` is a parameter
/// only so that a frame too small to hold its own padding is reported as
/// [`FitError::PaddingTooLarge`] rather than handed to the layout to produce nothing.
pub fn text_frame(bounds: Rect, inner_scale: f64, padding: f64) -> Result<Rect, FitError> {
    validate_rect("bounds", bounds)?;
    if !(inner_scale.is_finite() && inner_scale > 0.0 && inner_scale <= 1.0) {
        return Err(FitError::NonPositive { field: "inner scale" });
    }
    if !(padding.is_finite() && padding >= 0.0) {
        return Err(FitError::NonPositive { field: "padding" });
    }
    let inner = inner_rect(bounds, inner_scale);
    let usable = Size::new(inner.width() - 2.0 * padding, inner.height() - 2.0 * padding);
    if usable.width <= 0.0 || usable.height <= 0.0 {
        return Err(FitError::PaddingTooLarge { padding, inner: inner.width().min(inner.height()) });
    }
    Ok(inner)
}

/// The `TextLayoutSettings` a plan writes into the record (schema §6.6).
///
/// Every field is an **input** to `vectorcraft_text`; the measured result (`TextLayout::bounds`,
/// `.overflow`, `.fit_scale`) is never persisted (design §8.3), which is why this takes a style and
/// not a measurement.
#[must_use]
pub fn layout_settings(style: &FitTextStyle, fit_to_frame: bool) -> TextLayoutSettings {
    TextLayoutSettings {
        font: Some(style.family.clone()),
        size: Some(style.size),
        leading: style.leading,
        tracking: Some(style.tracking),
        align: Some(style.align),
        padding: Some(style.padding),
        fit_to_frame,
    }
}

/// The engine's [`Justify`] for a schema [`TextAlign`] (schema §5.5).
///
/// `Justify::Auto` is deliberately not used: the record's `align` is explicit, and resolving it
/// against the project's reading direction is the caller's decision, not a silent default here.
#[must_use]
pub fn justify(align: TextAlign) -> Justify {
    match align {
        TextAlign::Left => Justify::Left,
        TextAlign::Center => Justify::Center,
        TextAlign::Right => Justify::Right,
    }
}

/// The `AreaFit` an overflow policy asks `vectorcraft_text` to apply (design §8.3).
///
/// `Shrink` is the engine's own shrink-to-fit path; the other two leave the frame alone and let the
/// engine report the overflow with its overflow marker.
///
/// `shrink_percent` goes through `AreaFit::parse`, which clamps into 10..=100 exactly as the engine
/// does, and the clamped value is what the returned `AreaFit` carries — so the plan the command
/// layer writes and the finding a creator reads both state the policy that was applied. A
/// non-finite percent never gets this far: [`FitTextStyle::validate`] refuses it.
#[must_use]
pub fn area_fit(policy: vectorcraft_comic_model::OverflowPolicy, style: &FitTextStyle) -> AreaFit {
    match policy {
        vectorcraft_comic_model::OverflowPolicy::Shrink => AreaFit::parse("shrinkText", Some(style.shrink_percent)).unwrap_or(AreaFit::None),
        vectorcraft_comic_model::OverflowPolicy::Warn | vectorcraft_comic_model::OverflowPolicy::ResizeFrame => AreaFit::None,
    }
}

/// A `Finding` for text that does not fit (design §12.1, "Text overflow").
///
/// Severity is `Warning`, not `Error`: overflow is legal — the engine draws its overflow marker and
/// the page still renders — and `Severity::Error` is documented as a referential inconsistency. A
/// validation error for a balloon a creator can see and fix would make `comic.project.validate`
/// unusable on a work in progress.
#[must_use]
pub fn overflow_finding(subject: &str, message: String) -> Finding {
    Finding::warning(None, subject.to_string(), message)
}

/// Whether two rectangles share any area.
///
/// Touching along an edge does **not** overlap, exactly as [`crate::PanelRect::overlaps`] defines
/// it: a lettering frame flush with a panel's edge is a layout, not a collision, and reporting it
/// would drown the real collisions.
#[must_use]
pub fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1
}

/// The index of the first obstacle `bounds` overlaps.
///
/// An index rather than a copy of the rectangle, so the caller can name the object it must move.
#[must_use]
pub fn first_collision(bounds: Rect, obstacles: &[Rect]) -> Option<usize> {
    obstacles.iter().position(|obstacle| overlaps(bounds, *obstacle))
}

/// Reject a NaN or a non-positive rectangle before it reaches `kurbo`, which would otherwise carry
/// the NaN into every rectangle derived from it and make every comparison false.
pub(crate) fn validate_rect(field: &'static str, rect: Rect) -> Result<(), FitError> {
    for value in [rect.x0, rect.y0, rect.x1, rect.y1] {
        if !value.is_finite() {
            return Err(FitError::NotFinite { field });
        }
    }
    // Both coordinates were checked finite above, so a `<=` comparison is exact here.
    if rect.width() <= 0.0 {
        return Err(FitError::NonPositive { field: "bounds width" });
    }
    if rect.height() <= 0.0 {
        return Err(FitError::NonPositive { field: "bounds height" });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }

    #[test]
    fn the_inner_rect_keeps_the_centre_and_the_box_aspect() {
        let inner = inner_rect(rect(10.0, 20.0, 100.0, 50.0), std::f64::consts::FRAC_1_SQRT_2);
        let sqrt_half = std::f64::consts::FRAC_1_SQRT_2;
        assert!((inner.width() - 100.0 * sqrt_half).abs() < 1e-12);
        assert!((inner.height() - 50.0 * sqrt_half).abs() < 1e-12);
        assert!((inner.center().x - 60.0).abs() < 1e-12);
        assert!((inner.center().y - 45.0).abs() < 1e-12);
        // A corner of the inscribed rectangle lies on the ellipse of the same aspect.
        let (dx, dy) = ((inner.x1 - 60.0) / 50.0, (inner.y1 - 45.0) / 25.0);
        assert!((dx * dx + dy * dy - 1.0).abs() < 1e-12, "the corner is not on the ellipse: {dx} {dy}");
    }

    #[test]
    fn the_text_frame_is_the_inscribed_rectangle_and_the_padding_is_the_layouts_job() {
        // The frame keeps the shape's full inscribed rectangle: the padding reaches the layout once
        // (`LayoutOptions::inset`), and taking it off here as well would leave the text half the
        // room the box promises.
        let bounds = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(text_frame(bounds, 0.5, 2.0), Ok(rect(2.5, 2.5, 5.0, 5.0)));
        assert_eq!(text_frame(bounds, 1.0, 0.0), Ok(bounds));
        // Padding that leaves no room at all is still refused rather than handed to the layout.
        assert_eq!(text_frame(bounds, 0.5, 2.5), Err(FitError::PaddingTooLarge { padding: 2.5, inner: 5.0 }));
    }

    #[test]
    fn a_nan_frame_is_rejected_rather_than_carried_into_a_path() {
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let db = FontDb::global();
        let bad = Rect::new(0.0, 0.0, f64::NAN, 10.0);
        assert_eq!(measure(db, "hello", &style, &bad, AreaFit::None).err(), Some(FitError::NotFinite { field: "frame" }));
    }

    #[test]
    fn a_bad_style_is_rejected_before_a_layout_call() {
        let db = FontDb::global();
        let frame = rect(0.0, 0.0, 100.0, 100.0);
        let mut style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        style.size = 0.0;
        assert_eq!(measure(db, "hello", &style, &frame, AreaFit::None).err(), Some(FitError::NonPositive { field: "text size" }));
        style.size = 12.0;
        style.padding = f64::INFINITY;
        assert_eq!(measure(db, "hello", &style, &frame, AreaFit::None).err(), Some(FitError::NotFinite { field: "padding" }));
    }

    #[test]
    fn empty_text_is_laid_out_and_fits_any_frame() {
        // The empty string is a real input — a balloon whose dialogue has not been written yet — and
        // it must not be reported as overflow: `TextLayout.bounds` is `Rect::ZERO` for it, which is
        // outside any frame away from the origin.
        let db = FontDb::global();
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let measured = measure(db, "", &style, &rect(400.0, 300.0, 80.0, 40.0), AreaFit::None).expect("measurable");
        assert!(measured.layout.glyphs.is_empty());
        assert!(measured.fits(), "an empty balloon is not overflowing");
    }

    #[test]
    fn a_long_word_is_broken_between_characters_rather_than_overflowing() {
        // Measured, not assumed: the engine breaks an overlong word character by character, so a
        // long word is not an overflow — and this is why widening a balloon for one is a matter of
        // taste rather than of correctness.
        let db = FontDb::global();
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let frame = rect(0.0, 0.0, 40.0, 200.0);
        let measured = measure(db, "Unforgettable", &style, &frame, AreaFit::None).expect("measurable");
        assert!(measured.fits(), "the word was broken to fit");
        assert!(measured.layout.lines.len() > 1, "more than one line: {:?}", measured.layout.lines.len());
        assert!(
            measured.layout.lines.iter().all(|line| line.x1 <= line.avail.1 + 1e-9),
            "every line is inside its own span: {:?}",
            measured.layout.lines.iter().map(|line| (line.x0, line.x1, line.avail)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_frame_too_narrow_for_one_character_is_overflow_with_no_lines_at_all() {
        // The other half of the same measurement: what the engine cannot do is place a character
        // that does not fit, and there `TextLayout.overflow` is the answer `Measured::fits` reads.
        let db = FontDb::global();
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let measured = measure(db, "M", &style, &rect(0.0, 0.0, 8.0, 200.0), AreaFit::None).expect("measurable");
        assert!(measured.layout.overflow);
        assert!(measured.layout.lines.is_empty(), "the line was dropped, not placed past the frame");
        assert!(!measured.fits());
    }

    #[test]
    fn needed_height_is_the_distance_from_the_cell_top_to_the_last_descent() {
        // The bottom is the last line's baseline **plus its descent** — the value the engine tests
        // its own fit against — not the ink bottom, and not the frame's origin. Swapping it for
        // `layout.bounds.y1` would leave the relative properties below true, so the descent is
        // asserted exactly (round 1: this test named the descent and never checked it).
        let db = FontDb::global();
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let text = "The rain kept falling on the roof.";
        let frame = rect(0.0, 0.0, 60.0, 400.0);
        let a = measure(db, text, &style, &frame, AreaFit::None).expect("measurable");
        let last = a.layout.lines.last().expect("the block has lines");
        assert!((a.bottom() - (last.baseline + last.descent)).abs() < 1e-12, "the bottom is not the last descent: {:?}", a.bottom());
        assert!((a.needed_height() - ((a.bottom() - (frame.y0 + style.padding)) + 2.0 * style.padding)).abs() < 1e-12);
        assert!(a.needed_height() > 2.0 * style.padding, "a block of lines needs more than its padding");

        // Two frames with different origins must report the same need: the block starts at the
        // frame's inner top, so the number is a property of the text and the width alone.
        let b = measure(db, text, &style, &rect(300.0, 200.0, 60.0, 400.0), AreaFit::None).expect("measurable");
        assert!((a.needed_height() - b.needed_height()).abs() < 1e-9);
        // The block is taller in a narrower frame, which is what makes the sizing search grow.
        let wide = measure(db, text, &style, &rect(0.0, 0.0, 300.0, 400.0), AreaFit::None).expect("measurable");
        assert!(a.needed_height() > wide.needed_height());
    }

    #[test]
    fn a_block_with_no_lines_needs_its_padding_and_never_a_negative_height() {
        // A frame too narrow for one character places no lines at all. The bottom is then the cell's
        // top, so the height needed is the two paddings — a height, where `TextLayout.bounds.y1`
        // (zero in text space) would give a negative one (round 1, F5).
        let db = FontDb::global();
        let style = FitTextStyle::new("Comic Neue", "Regular", 12.0);
        let frame = rect(400.0, 300.0, 8.0, 200.0);
        let narrow = measure(db, "M", &style, &frame, AreaFit::None).expect("measurable");
        assert!(narrow.layout.lines.is_empty(), "the frame is not narrow enough to drop the line");
        assert!(narrow.needed_height() >= 0.0, "a block with no lines needs {:?} points", narrow.needed_height());
        assert!((narrow.needed_height() - 2.0 * style.padding).abs() < 1e-12, "{:?}", narrow.needed_height());
        assert!((narrow.bottom() - (frame.y0 + style.padding)).abs() < 1e-12);
    }

    #[test]
    fn an_edged_touch_is_not_a_collision() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        assert!(!overlaps(a, rect(10.0, 0.0, 10.0, 10.0)), "a shared edge is not an overlap");
        assert!(!overlaps(a, rect(0.0, 10.0, 10.0, 10.0)), "a shared edge is not an overlap on either axis");
        assert!(!overlaps(a, rect(10.0, 10.0, 10.0, 10.0)), "a shared corner is not an overlap");
        assert!(overlaps(a, rect(9.0, 0.0, 10.0, 10.0)));
        assert!(overlaps(a, rect(0.0, 9.0, 10.0, 10.0)));
        assert_eq!(first_collision(a, &[rect(50.0, 50.0, 10.0, 10.0), rect(1.0, 1.0, 5.0, 5.0)]), Some(1));
        assert_eq!(first_collision(a, &[]), None);
    }
}
