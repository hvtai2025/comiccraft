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

/// Whether `family` is one of the two comic families ADR-017 ships in `craft-fonts`.
///
/// The same pair, and the same comparison, as `vectorcraft_text::craft_fonts::comic_fonts` selects
/// them with, and as [`is_generic_family`] compares (`craft-fonts` carries no other comic family, so
/// there is no third case to ask about).
///
/// This is the condition [`missing_face_finding`] reports on, and the only one it reports on: it is
/// what decides whether naming `craft-fonts` is the creator's fix or a wrong turn.
#[must_use]
pub fn is_comic_family(family: &str) -> bool {
    let family = family.trim();
    family.eq_ignore_ascii_case(DIALOGUE_FAMILY) || family.eq_ignore_ascii_case(DISPLAY_FAMILY)
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

/// The **`CRAFT_FONTS`-empty contract**, as one named call (design §8.3, M2 §4.3).
///
/// The lettering faces a project asks for, each one reported when it is not available exactly as
/// named — while every plan that asks for one goes on to lay its text out in the face that stands
/// in. Two guarantees, and neither is worth anything alone:
///
/// * **The report is produced.** A face that is not there is named, with its status and the face
///   shown instead, in the exporter's own terms ([`MissingFace`] is field for field
///   `text.missingFonts`'s record).
/// * **The layout still runs.** A missing face is neither an error nor a refusal: the words are
///   still wrapped, measured and placed, so the creator gets a page *and* a message.
///
/// **A fallback that renders and says nothing is the failure this contract rules out**, because all
/// the creator sees is a page that looks wrong with no way to learn why. This is therefore the one
/// name the face half of a plan's report is produced through — `plan::ruler_findings` calls this and
/// nothing else for it — so a substitute face that skipped the report would have to skip the
/// contract by name, and a test that cites the name reds.
#[must_use]
pub fn lettering_face_report(db: &FontDb, faces: &[Face], subject: &str) -> Vec<Finding> {
    missing_face_findings(&missing_faces(db, faces), subject)
}

/// One missing face's finding.
///
/// The message reports **what was observed** — the family and style the project asked for, the
/// status, and the face that stands in — and says where the face comes from **only when this module
/// knows it** ([`is_comic_family`], the one condition it branches on):
///
/// * A comic family: `craft-fonts` is named, because ADR-017 ships those faces there and that is
///   why a build without `CRAFT_FONTS` has none of them.
/// * Any other family: the message says what it knows — the face is **not** one of ADR-017's comic
///   faces — and then gives both ways in **without ruling either out**: a face `craft-fonts` carries
///   reaches a build through `CRAFT_FONTS_DIR`, and anything else must be installed or replaced.
///
/// **The second bullet is why the else-branch changed (M2-L4).** It used to end *"so craft-fonts is
/// not where it belongs"*, which is false for most of what `craft-fonts` carries: its manifest lists
/// seven families and only two are ADR-017 comic faces, so for the other five the message sent the
/// creator away from the very checkout holding their font — and in a `craft-fonts` build it
/// contradicted the state it was read in, because the substitute face standing in came *from* that
/// checkout. [`is_comic_family`] is therefore **correct as a predicate and was wrong as a premise**:
/// knowing which families are *comic* faces does not establish where a non-comic face lives, so the
/// message no longer claims it does. The branch still keys on the family and not the status, which is
/// the other half of its contract.
///
/// The second half is why this function has a branch at all. The previous wording ended *"the comic
/// faces belong in craft-fonts, and CRAFT_FONTS being empty is why they are looked for there"* for
/// **every** missing face, which asserts a cause nothing checked: for a project's own face — one
/// that was never in `craft-fonts` — it is false in a build that did embed craft-fonts and in one
/// that did not, and it sends the creator to a checkout that never carried the face (M2 §7.4, W4's
/// D2). The status already distinguishes the two shapes of absence the exporter distinguishes
/// (`missing` is the absent family, `substitute` the absent style of a family that *is* here), so
/// nothing is lost by stating no cause (M2-W4-N1 keeps the `resolved: None` case honest).
#[must_use]
pub fn missing_face_finding(face: &MissingFace, subject: &str) -> Finding {
    let shown = match &face.resolved {
        Some((family, style)) => format!("{family} {style}"),
        None => FALLBACK_FAMILY.to_string(),
    };
    let home = if is_comic_family(&face.family) {
        format!("{} is one of ADR-017's comic faces, which are shipped in craft-fonts and reach a build through CRAFT_FONTS_DIR", face.family)
    } else {
        format!(
            "{} is not one of ADR-017's comic faces ({DIALOGUE_FAMILY}, {DISPLAY_FAMILY}): a face craft-fonts carries reaches a build through CRAFT_FONTS_DIR, and any other has to be installed, or another face chosen",
            face.family
        )
    };
    Finding::warning(
        None,
        subject.to_string(),
        format!("the lettering face {} {} is not available ({}) and shows in {shown}; {home}", face.family, face.style, face.status.as_str()),
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
    fn a_comic_family_is_the_comic_families_and_a_near_miss_is_not() {
        // The condition the message branches on, on its own: ADR-017's two families, compared the
        // way the exporter resolves a family (any ASCII case) and the way `is_generic_family`
        // compares (trimmed). A near miss is not one of them — `Comic Neue Pro` and `Bangers
        // Display` are other faces, and a generic family is a family the platform resolves.
        for comic in [DIALOGUE_FAMILY, DISPLAY_FAMILY] {
            assert!(is_comic_family(comic), "{comic}");
            assert!(is_comic_family(&comic.to_uppercase()), "{comic} in another case");
            assert!(is_comic_family(&format!("  {comic}  ")), "{comic} with whitespace");
        }
        for not_comic in ["", "  ", "Comic Neue Pro", "Bangers Display", "Comic Sans MS", FALLBACK_FAMILY, "sans-serif"] {
            assert!(!is_comic_family(not_comic), "{not_comic:?} is not an ADR-017 comic face");
        }
    }

    #[test]
    fn the_families_craft_fonts_calls_comic_are_the_ones_this_condition_accepts() {
        // ADR-017 names the two families here and `vectorcraft_text::craft_fonts::comic_fonts` selects
        // the same two out of the build input, so the condition the message branches on is the same
        // condition the fonts arrive by. The constants are this module's own, so that half always
        // runs; the cross-check needs a craft-fonts build and is the half that would catch a third
        // comic family added upstream without being added here.
        for comic in [DIALOGUE_FAMILY, DISPLAY_FAMILY] {
            assert!(is_comic_family(comic), "{comic}");
        }
        let from_the_build_input = vectorcraft_text::craft_fonts::comic_fonts();
        if from_the_build_input.is_empty() {
            eprintln!("skipped: built without craft-fonts (set CRAFT_FONTS_DIR to a craft-fonts checkout to run the cross-check)");
        }
        for font in from_the_build_input {
            assert!(is_comic_family(font.family), "{} reaches a build as a comic face, so the message must treat it as one", font.family);
        }
    }

    #[test]
    fn the_comic_branch_names_craft_fonts_as_the_home_and_the_other_branch_claims_none() {
        // The message is a function of the face record, so it is built here rather than resolved:
        // both branches then hold in a build with the comic faces embedded *and* in one without
        // them, which is the pair of configurations the contract is stated over. `Comic Neue` is
        // ADR-017's, so the comic branch names craft-fonts as the home. `No Such Lettering Face` is
        // a project's own — and the non-comic branch may **not** conclude from that alone that
        // craft-fonts is the wrong place, because `CRAFT_FONTS` is empty in this build and most of
        // what craft-fonts lists is not a comic face (M2-L4).
        let message = |family: &str| {
            let face = MissingFace {
                family: family.to_string(),
                style: BOLD.to_string(),
                status: FontMatch::Missing,
                resolved: Some((FALLBACK_FAMILY.to_string(), BOLD.to_string())),
            };
            missing_face_finding(&face, "lettering speech").message
        };
        let observed = |family: &str| format!("the lettering face {family} {BOLD} is not available (missing) and shows in {FALLBACK_FAMILY} {BOLD};");
        for comic in [DIALOGUE_FAMILY, DISPLAY_FAMILY] {
            let named = message(comic);
            // Unchanged in the front half: the face, the status and what stands in.
            assert!(named.starts_with(&observed(comic)), "{named}");
            assert!(named.contains("shipped in craft-fonts"), "a comic face is one craft-fonts is where it belongs: {named}");
        }
        let own = message("No Such Lettering Face");
        assert!(own.starts_with(&observed("No Such Lettering Face")), "{own}");
        assert!(
            own.contains(&format!("not one of ADR-017's comic faces ({DIALOGUE_FAMILY}, {DISPLAY_FAMILY})")),
            "the condition is reported, not a cause: {own}"
        );
        // This used to assert `!own.contains("CRAFT_FONTS")` on the reasoning that a build input must
        // not be blamed for a face it never carried. **The reasoning was the M2-L4 defect**: the
        // premise "not a comic face, so craft-fonts never carried it" is false for five of the seven
        // families `craft-fonts` lists, and `CRAFT_FONTS` is empty in this build, so the module cannot
        // know which those are. A cause is not avoided by omitting a name; it is avoided by not
        // asserting one. So the assertion is the contract: the message must not **rule craft-fonts
        // out**, and it must still give the creator both ways in.
        assert!(!own.contains("craft-fonts is not where it belongs"), "the message must not rule craft-fonts out for a family it may carry: {own}");
        assert!(own.contains("CRAFT_FONTS_DIR"), "and it still tells the creator where a craft-fonts face comes from: {own}");
        assert!(own.contains("installed"), "and it still gives the route for a face craft-fonts does not carry: {own}");
        assert_ne!(message(DIALOGUE_FAMILY), own, "the two cases are two messages, not one");
    }

    #[test]
    fn the_message_names_no_cause_but_the_one_it_checked() {
        // The old wording — "CRAFT_FONTS being empty is why they are looked for there" — was false
        // for every face that is not one of the comic faces, and a creator follows it to the wrong
        // fix. Neither branch asserts why a face is absent: the status carries the exporter's two
        // shapes of absence (`missing`, `substitute`), and where the face comes from is said only
        // where this module knows it.
        for (family, belongs_in_craft_fonts) in [(DIALOGUE_FAMILY, true), ("No Such Lettering Face", false), (FALLBACK_FAMILY, false)] {
            let face = MissingFace {
                family: family.to_string(),
                style: REGULAR.to_string(),
                status: FontMatch::Style,
                resolved: Some((family.to_string(), BOLD.to_string())),
            };
            let message = missing_face_finding(&face, "lettering speech").message;
            assert!(!message.contains("empty"), "a build input's state is asserted, and it was never read: {message}");
            assert!(!message.contains("is why"), "no cause is claimed without being checked: {message}");
            assert_eq!(message.contains("shipped in craft-fonts"), belongs_in_craft_fonts, "{message}");
            assert!(!message.contains("missing"), "the status is the family present and the style absent: {message}");
        }
    }

    #[test]
    fn the_contract_reports_the_face_that_was_asked_for_and_never_the_one_that_stands_in() {
        // The contract at the level it is named. A face that cannot exist in any build — with the
        // comic faces embedded or without them — is reported, and the report names the *requested*
        // family and style. Those are exactly the two things a silent substitution replaces, so
        // asserting on them is what makes a report about the substitute face red rather than merely
        // different: `Comic Neue Bold` showing as `Source Sans 3 Regular` is the state this
        // contract describes, and the state it must never describe as what the project asked for.
        let db = FontDb::global();
        let requested = Face::new("No Such Lettering Face", BOLD);
        let findings = lettering_face_report(db, std::slice::from_ref(&requested), "lettering speech");
        assert_eq!(findings.len(), 1, "{findings:?}");
        let message = &findings[0].message;
        assert!(message.contains("No Such Lettering Face Bold"), "{message}");
        assert!(message.contains("missing"), "{message}");
        assert!(message.contains(FALLBACK_FAMILY), "{message}");
        assert_eq!(findings[0].subject, "lettering speech");
        assert_eq!(findings[0].severity, vectorcraft_comic_model::Severity::Warning);
        // The other half: a face that *is* available exactly reports nothing. The contract is about
        // absent faces, not about reporting faces, so a report that fired on every face would be as
        // useless as one that fired on none.
        assert!(lettering_face_report(db, &[Face::new(FALLBACK_FAMILY, REGULAR)], "lettering speech").is_empty());
    }

    #[test]
    fn the_family_present_style_absent_half_of_the_contract_is_reported_as_a_substitute_and_not_as_absence() {
        // The exporter distinguishes "no such family" from "the family, but not this style" —
        // `FontMatch::Missing` (`"missing"`) against `FontMatch::Style` (`"substitute"`), which the
        // finding prints as the status — and a creator's fix differs: install the face, or ask for a
        // style the family has. A bundled family with a style no file carries is the one way to pin
        // that distinction in every build, `CRAFT_FONTS` present or absent.
        let db = FontDb::global();
        let requested = Face::new(FALLBACK_FAMILY, "No Such Weight 4f2a");
        let missing = missing_faces(db, std::slice::from_ref(&requested));
        assert_eq!(missing.len(), 1, "{missing:?}");
        assert_eq!(missing[0].status, FontMatch::Style, "the family is present, so this is a style and not an absence");
        let findings = lettering_face_report(db, std::slice::from_ref(&requested), "lettering speech");
        assert_eq!(findings.len(), 1, "{findings:?}");
        let message = &findings[0].message;
        assert!(message.contains(FALLBACK_FAMILY), "{message}");
        assert!(message.contains("No Such Weight 4f2a"), "{message}");
        assert!(message.contains("substitute"), "the status printed is not the exporter's own string: {message}");
        assert!(!message.contains("missing"), "the family is present, so this is a style and not an absence: {message}");
    }

    #[test]
    fn the_report_is_capped_so_a_pasted_name_cannot_produce_an_endless_menu() {
        let db = FontDb::global();
        let faces: Vec<Face> = (0..MAX_FACES + 5).map(|i| Face::new(format!("No Such Face {i}"), REGULAR)).collect();
        assert_eq!(missing_faces(db, &faces).len(), MAX_FACES);
    }
}
