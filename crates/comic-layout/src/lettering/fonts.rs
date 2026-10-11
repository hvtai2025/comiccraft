//! The comic lettering faces, and what happens when they are not there.
//!
//! The faces are **not in this repository**. ADR-017 puts them in `storytold/craft-fonts`
//! (`AGENTS.md` › Assets): **Comic Neue** (Regular, Bold, Italic) for dialogue, captions and
//! narration, and **Bangers** (Regular) for display — sound effects and cover text. They reach the
//! process through `CRAFT_FONTS_DIR`, and `vectorcraft_text::CRAFT_FONTS` is **empty** in every
//! build without it.
//!
//! So nothing here may assume a face exists:
//!
//! * A missing family is **never** silently substituted as if it were there. The layout still runs —
//!   `vectorcraft_text` substitutes the fallback face so the text is set, positioned and measured —
//!   and this module reports the substitution the way the exporter does: the family, the style, the
//!   status and the face shown instead.
//! * [`Face`] is a family and style pair, spelled exactly as the exporter's `MissingFont` spells
//!   them, so a project's lettering report and an export's font warning name the same thing.
//! * Lettering must lay out and report with `CRAFT_FONTS` empty; a build without the faces loses
//!   the *look* of comic lettering, never the layout (design §8.3, audit §12.5).

use vectorcraft_comic_model::Finding;
use vectorcraft_text::{FALLBACK_FAMILY, FontDb, FontMatch};

/// The family ADR-017 gives dialogue, captions and narration.
pub const DIALOGUE_FAMILY: &str = "Comic Neue";

/// The family ADR-017 gives display lettering: sound effects, titles and cover text.
pub const DISPLAY_FAMILY: &str = "Bangers";

/// The regular style within a family.
pub const REGULAR: &str = "Regular";
/// The bold style within a family.
pub const BOLD: &str = "Bold";
/// The italic style within a family.
pub const ITALIC: &str = "Italic";

/// At most this many faces are reported, matching the exporter's own cap.
pub const MAX_FACES: usize = 1000;

/// The longest family or style name reported, matching the exporter's own cap (bytes).
pub const MAX_NAME: usize = 256;

/// The CSS generic font families (CSS Fonts Module Level 4), which a document names in place of a
/// font and no font file provides.
///
/// The same list, with the same exclusion, as `text.missingFonts` (`crates/engine/src/cmd/
/// fontfiles.rs`): `fangsong` is left out because it is also the family name of a Chinese font. A
/// project whose lettering font is `sans-serif` is not missing a face — it asked for a family the
/// platform resolves — and reporting it would be the one thing the mirroring claim rules out.
pub const GENERIC_FAMILIES: &[&str] = &[
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
];

/// Whether `family` is a CSS generic family, compared in any ASCII case as the exporter compares it.
#[must_use]
pub fn is_generic_family(family: &str) -> bool {
    let family = family.trim();
    GENERIC_FAMILIES.iter().any(|generic| generic.eq_ignore_ascii_case(family))
}

/// A family and a style within it.
///
/// The pair, not the family alone: `Comic Neue` present with only `Regular` is a real and different
/// situation from the family being absent, and the exporter distinguishes them with
/// [`FontMatch::Style`] against [`FontMatch::Missing`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// The family name, resolved through the font database.
    pub family: String,
    /// The style within it.
    pub style: String,
}

impl Face {
    /// A face from a family and a style.
    #[must_use]
    pub fn new(family: impl Into<String>, style: impl Into<String>) -> Self {
        Self { family: family.into(), style: style.into() }
    }
}

/// A face a project names that is not available as named.
///
/// Field for field what `text.missingFonts` reports (`crates/engine/src/cmd/fontfiles.rs`), so a
/// creator sees one kind of missing-font report across the application.
#[derive(Clone, Debug, PartialEq)]
pub struct MissingFace {
    /// The family the project asked for.
    pub family: String,
    /// The style it asked for.
    pub style: String,
    /// [`FontMatch::Missing`] when the family is unknown, [`FontMatch::Style`] when only the style
    /// is.
    pub status: FontMatch,
    /// The family and style of the face shown instead.
    pub resolved: Option<(String, String)>,
}

/// The faces in `faces` that are not available exactly as named, each once, sorted by family and
/// style.
///
/// The filter is the exporter's, **term for term** (round 1, F7): an empty *family* is skipped (it
/// is not a font), a CSS generic family is skipped ([`is_generic_family`] — the platform resolves
/// it, no file provides it), and a family or style longer than [`MAX_NAME`] bytes is skipped.
///
/// A blank *style* is deliberately **not** filtered, because the exporter does not filter it either:
/// a hand-authored document can name style `""`, and `text.missingFonts` reports that face. Dropping
/// it here would make the two reports disagree on exactly the input the mirroring claim is about
/// (round 2, R2-F3).
#[must_use]
pub fn missing_faces(db: &FontDb, faces: &[Face]) -> Vec<MissingFace> {
    let mut out: Vec<MissingFace> = faces
        .iter()
        .filter(|face| !face.family.trim().is_empty())
        .filter(|face| !is_generic_family(&face.family) && face.family.len() <= MAX_NAME && face.style.len() <= MAX_NAME)
        .filter_map(|face| {
            let resolved = db.resolve(&face.family, &face.style);
            let status = resolved.as_ref().map_or(FontMatch::Missing, |(_, matched)| *matched);
            (status != FontMatch::Exact).then(|| MissingFace {
                family: face.family.clone(),
                style: face.style.clone(),
                status,
                resolved: resolved.map(|(found, _)| (found.family.clone(), found.style.clone())),
            })
        })
        .take(MAX_FACES)
        .collect();
    out.sort_by(|a, b| a.family.cmp(&b.family).then_with(|| a.style.cmp(&b.style)));
    out.dedup_by(|a, b| a.family == b.family && a.style == b.style);
    out
}

/// One `Finding` per missing face, worded the way the exporter words its font warning.
///
/// `Warning`: the page still renders, in the fallback face, and a creator fixes it by installing
/// the face or choosing another — which is exactly what the exporter's own warning says.
#[must_use]
pub fn missing_face_findings(missing: &[MissingFace], subject: &str) -> Vec<Finding> {
    missing.iter().map(|face| missing_face_finding(face, subject)).collect()
}

/// One missing face's finding.
#[must_use]
pub fn missing_face_finding(face: &MissingFace, subject: &str) -> Finding {
    let shown = match &face.resolved {
        Some((family, style)) => format!("{family} {style}"),
        None => FALLBACK_FAMILY.to_string(),
    };
    Finding::warning(
        None,
        subject.to_string(),
        format!(
            "the lettering face {} {} is not available ({}) and shows in {shown}; the comic faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there",
            face.family,
            face.style,
            face.status.as_str(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_faces_this_module_names_are_the_ones_adr_017_pins() {
        // A rename in craft-fonts that missed these constants would silently fall back at runtime.
        assert_eq!(DIALOGUE_FAMILY, "Comic Neue");
        assert_eq!(DISPLAY_FAMILY, "Bangers");
    }

    #[test]
    fn a_family_that_cannot_exist_is_reported_as_missing_and_not_styled_as_present() {
        let db = FontDb::global();
        let faces = [Face::new("Comic Neue", REGULAR), Face::new("No Such Lettering Face", REGULAR)];
        let missing = missing_faces(db, &faces);
        let invented = missing.iter().find(|face| face.family == "No Such Lettering Face").expect("an unknown family is reported");
        assert_eq!(invented.status, FontMatch::Missing);
        assert!(invented.resolved.is_some(), "the fallback face is named, as the exporter names it");
        assert!(missing.iter().all(|face| face.status != FontMatch::Exact));
        assert_eq!(missing_faces(db, &faces), missing, "sorted and deduplicated, so the report is stable");
    }

    #[test]
    fn a_face_that_is_available_exactly_is_not_reported() {
        // The bundled body faces are always present, so this is the one case with a known answer in
        // every build — with or without CRAFT_FONTS.
        let db = FontDb::global();
        let missing = missing_faces(db, &[Face::new(FALLBACK_FAMILY, REGULAR)]);
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn a_css_generic_family_and_an_over_long_name_are_not_reported() {
        // The exporter leaves both out (`crates/engine/src/cmd/fontfiles.rs`), and the point of
        // mirroring its report is that a creator sees one answer: a project whose lettering font is
        // `sans-serif` asked for a family the platform resolves, and a 300-byte "family" is not a
        // name any file carries (round 1, F7).
        let db = FontDb::global();
        for generic in GENERIC_FAMILIES {
            assert!(is_generic_family(generic), "{generic} is not recognised as generic");
            assert!(is_generic_family(&generic.to_uppercase()), "{generic} in another case");
            assert!(is_generic_family(&format!("  {generic}  ")), "{generic} with whitespace, as the exporter trims");
            assert!(missing_faces(db, &[Face::new(*generic, REGULAR)]).is_empty(), "{generic} is not a missing font");
        }
        // Not a generic family: a real family whose name merely contains one.
        assert!(!is_generic_family("Sans Serif Pro"));
        assert!(!is_generic_family(""));
        let long = "x".repeat(MAX_NAME + 1);
        assert!(missing_faces(db, &[Face::new(long.clone(), REGULAR)]).is_empty(), "a family name over {MAX_NAME} bytes");
        assert!(missing_faces(db, &[Face::new("No Such Face", long.clone())]).is_empty(), "a style name over {MAX_NAME} bytes");
        assert!(missing_faces(db, &[Face::new("x".repeat(MAX_NAME), REGULAR)]).len() == 1, "a name at the cap is still reported");
    }

    #[test]
    fn a_blank_family_is_not_a_missing_font_and_a_blank_style_is_reported_as_the_exporter_reports_it() {
        // The exporter's filter tests the family for emptiness and not the style
        // (`crates/engine/src/cmd/fontfiles.rs:111`), so a face with a family and no style is one it
        // reports — and this report mirrors it term for term rather than being quietly stricter
        // (round 2, R2-F3).
        let db = FontDb::global();
        // A blank family is not a font, in either field.
        assert!(missing_faces(db, &[Face::new("", ""), Face::new("   ", REGULAR), Face::new("", " ")]).is_empty());
        // A blank style is a style no face has, so the family's own default stands in — reported for
        // an unknown family and for an installed one alike, exactly as the exporter reports it.
        for family in ["No Such Face", FALLBACK_FAMILY] {
            let reported = missing_faces(db, &[Face::new(family, "")]);
            assert_eq!(reported.len(), 1, "{family}: {reported:?}");
            assert_eq!((reported[0].family.as_str(), reported[0].style.as_str()), (family, ""));
            assert_ne!(reported[0].status, FontMatch::Exact);
            assert!(reported[0].resolved.is_some(), "the face that stands in is named");
        }
    }

    #[test]
    fn the_finding_names_the_face_the_status_and_what_stands_in() {
        let db = FontDb::global();
        let missing = missing_faces(db, &[Face::new("No Such Lettering Face", BOLD)]);
        let findings = missing_face_findings(&missing, "lettering speech");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, vectorcraft_comic_model::Severity::Warning);
        let message = &findings[0].message;
        assert!(message.contains("No Such Lettering Face Bold"), "{message}");
        assert!(message.contains("missing"), "{message}");
        assert!(message.contains(FALLBACK_FAMILY), "{message}");
        assert_eq!(findings[0].subject, "lettering speech");
    }

    #[test]
    fn the_report_is_capped_so_a_pasted_name_cannot_produce_an_endless_menu() {
        let db = FontDb::global();
        let faces: Vec<Face> = (0..MAX_FACES + 5).map(|i| Face::new(format!("No Such Face {i}"), REGULAR)).collect();
        assert_eq!(missing_faces(db, &faces).len(), MAX_FACES);
    }
}
