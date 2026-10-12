//! M2 W4, criterion 5 — *"lettering tests pass with `CRAFT_FONTS` empty"* — as a **tested contract**
//! (M2 §4.3) rather than a habit that happens to be true on this machine.
//!
//! The distinctions this file exists to draw:
//!
//! * **The report is named, not implied.** `lettering::fonts::lettering_face_report` is the one call
//!   the face half of a plan's report is produced through, so a test can cite it and a reader can
//!   find it. What it promises is design §8.3's: a project whose lettering font is missing sees
//!   **exactly the missing-font report the exporter already produces** — the family, the style, the
//!   status and the face shown instead, the four fields `text.missingFonts` reports.
//! * **A fallback that renders is the failure mode.** A substitute face that draws the words and
//!   says nothing is *worse* than a refusal: the creator gets a page that looks wrong and no way to
//!   learn why. Every test here therefore asserts the report **and** that the layout still ran, and
//!   the reports are asserted to name the family and style the project *asked for* — the two things
//!   a silent substitution replaces.
//! * **Not conditional.** The comic faces are absent in the gate's configuration and present in
//!   others, so a test that only asks "was `Comic Neue` reported?" says nothing in one of the two
//!   builds. Where a build matters the assertion is a **biconditional** against the font database
//!   (report ⟺ not available exactly), and the core contract is pinned with faces that cannot exist
//!   in *either* configuration, so no test here can pass by being skipped.

use vectorcraft_comic_layout::fit::FitError;
use vectorcraft_comic_layout::lettering::fonts::MAX_NAME;
use vectorcraft_comic_layout::lettering::plan::{LetteringPlan, LetteringRequest, TextSource, new_balloon, refit_balloon};
use vectorcraft_comic_layout::lettering::style::style_of;
use vectorcraft_comic_layout::lettering::{
    BOLD, DIALOGUE_FAMILY, DISPLAY_FAMILY, Face, ITALIC, LETTERING_KINDS, LetteringKindExt, REGULAR, lettering_face_report, missing_face_findings,
    missing_faces,
};
use vectorcraft_comic_model::{LetteringKind, Severity};
use vectorcraft_geom::{Point, Vec2};
use vectorcraft_text::{FontDb, FontMatch};

/// A family no build can have, so the contract is testable with the comic faces present *and*
/// absent.
const ABSENT_FAMILY: &str = "No Such Lettering Face";

/// A style no face carries, used to pin the family-present/style-absent half.
const ABSENT_STYLE: &str = "No Such Weight 4f2a";

fn source_for(kind: LetteringKind, text: &str) -> TextSource<'_> {
    if kind.is_spoken() { TextSource::Dialogue(text) } else { TextSource::Own(text) }
}

fn request_for(kind: LetteringKind, text: &str) -> LetteringRequest<'_> {
    let request = LetteringRequest::new(kind, source_for(kind, text)).placed_at(Point::new(200.0, 150.0));
    if kind.is_spoken() { request.with_tail(Point::new(160.0, 320.0)) } else { request }
}

/// The kind's request, wearing the face `family`/`style` instead of the kind's own default.
fn request_wearing<'a>(kind: LetteringKind, text: &'a str, family: &str, style: &str) -> LetteringRequest<'a> {
    let mut request = request_for(kind, text);
    request.style.text.family = family.to_string();
    request.style.text.style = style.to_string();
    request
}

/// The plan for `kind` wearing `family`/`style`, reported to the caller: a `#[test]` body may panic
/// on it (`clippy.toml`'s `allow-panic-in-tests`), a helper may not.
fn plan_wearing(kind: LetteringKind, text: &str, family: &str, style: &str) -> Result<LetteringPlan, FitError> {
    new_balloon(FontDb::global(), &request_wearing(kind, text, family, style))
}

/// The missing-face report — the findings that are not about the text, the box or the tail.
fn face_reports(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

/// Every finding that is not the missing-face report.
fn other_findings(plan: &LetteringPlan) -> Vec<String> {
    plan.findings.iter().filter(|finding| !finding.message.contains("is not available")).map(|finding| finding.message.clone()).collect()
}

/// Whether the database has `face` exactly as named — the predicate the contract is stated over,
/// read from the database rather than from the environment, so the test is not a restatement of the
/// build's configuration.
fn available_exactly(db: &FontDb, face: &Face) -> bool {
    db.resolve(&face.family, &face.style).is_some_and(|(_, matched)| matched == FontMatch::Exact)
}

/// The contract, first half: a face that is not there is **named**, and named as the one that was
/// asked for.
///
/// This is the test that goes red if a substitute face is used: a plan that fell back silently would
/// report nothing here, and a plan that reported the face that stood in — `Comic Neue Bold` showing
/// as `Source Sans 3 Regular`, written up as though that were what the project asked for — would
/// name the wrong family and style. Neither survives these assertions, and neither is a
/// configuration-dependent answer: this family exists in no build.
#[test]
fn a_missing_lettering_face_is_named_in_the_report_and_never_left_to_a_silent_substitute() {
    let db = FontDb::global();
    let requested = Face::new(ABSENT_FAMILY, BOLD);
    assert!(!available_exactly(db, &requested), "{ABSENT_FAMILY} {BOLD} is available in this build, so it cannot pin the contract");

    // The named contract itself, for the one face.
    let report = lettering_face_report(db, std::slice::from_ref(&requested), "lettering speech");
    assert_eq!(report.len(), 1, "{report:?}");
    assert_eq!(report[0].subject, "lettering speech");
    assert_eq!(report[0].severity, Severity::Warning, "a missing face is a warning, not an error: the page still renders");
    assert_eq!(report[0].rule, None);
    assert!(report[0].message.contains(&format!("{ABSENT_FAMILY} {BOLD}")), "the requested face is not named: {}", report[0].message);
    assert!(report[0].message.contains("missing"), "the exporter's own status name is not used: {}", report[0].message);
    // The four fields the exporter reports, as a report: the family, the style, the status and the
    // face shown instead. The last one is the substitute, named as the substitute.
    let missing = missing_faces(db, std::slice::from_ref(&requested));
    assert_eq!(missing.len(), 1);
    assert_eq!((missing[0].family.as_str(), missing[0].style.as_str()), (ABSENT_FAMILY, BOLD));
    assert_eq!(missing[0].status, FontMatch::Missing);
    assert!(missing[0].resolved.is_some(), "the face shown instead is not named");
    assert!(report[0].message.contains(&missing[0].resolved.clone().unwrap().0), "the substitute is not named: {}", report[0].message);
    assert_eq!(report, missing_face_findings(&missing, "lettering speech"));

    // And the same through a plan, both entry points, with the layout still running.
    let bounds = plan_wearing(LetteringKind::Speech, "Look out!", ABSENT_FAMILY, BOLD).expect("planned").bounds;
    for entry in ["new_balloon", "refit_balloon"] {
        let request = request_wearing(LetteringKind::Speech, "Look out!", ABSENT_FAMILY, BOLD);
        let plan = match entry {
            "new_balloon" => new_balloon(db, &request).expect("planned"),
            _ => refit_balloon(db, &request, bounds).expect("refitted"),
        };
        let reports = face_reports(&plan);
        assert_eq!(reports.len(), 1, "{entry}: {reports:?}");
        assert!(reports[0].contains(&format!("{ABSENT_FAMILY} {BOLD}")), "{entry}: {reports:?}");
        assert!(reports[0].contains("missing"), "{entry}: {reports:?}");
        // The page. A missing face is not an error and not a refusal.
        assert!(plan.fits(), "{entry}: {:?}", plan.outcome);
        assert!(plan.frame.is_closed(), "{entry}");
        assert!(plan.text_bounds.width() > 0.0 && plan.text_bounds.height() > 0.0, "{entry}: the text was not laid out");
        assert!(plan.tail.is_some(), "{entry}: the balloon lost its tail with its face");
        assert!(other_findings(&plan).is_empty(), "{entry}: {:?}", other_findings(&plan));
    }
}

/// The contract, for **every kind**, through both entry points: each kind's own face absent is named
/// as that kind's face, and every one of the six still plans a balloon or a caption.
#[test]
fn every_kind_names_its_own_absent_face_through_both_entry_points() {
    let db = FontDb::global();
    for kind in LETTERING_KINDS {
        let default = style_of(kind).face();
        let request = request_wearing(kind, "Look out!", ABSENT_FAMILY, &default.style);
        let planned = new_balloon(db, &request).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let refitted = refit_balloon(db, &request, planned.bounds).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        for (entry, plan) in [("new_balloon", &planned), ("refit_balloon", &refitted)] {
            let reports = face_reports(plan);
            assert_eq!(reports.len(), 1, "{kind:?} {entry}: {reports:?}");
            // The *style* the kind asks for is left alone, so this says the report is per kind and
            // not one hard-coded sentence.
            assert!(reports[0].contains(&format!("{ABSENT_FAMILY} {}", default.style)), "{kind:?} {entry}: {reports:?}");
            assert!(reports[0].contains("missing"), "{kind:?} {entry}: {reports:?}");
            assert_eq!(plan.findings[0].subject, format!("lettering {}", kind.name()), "{kind:?} {entry}");
            assert_eq!(plan.tail.is_some(), kind.is_spoken(), "{kind:?} {entry}: the tail rule");
            assert!(plan.fits(), "{kind:?} {entry}: {:?}", plan.outcome);
            assert!(plan.frame.is_closed(), "{kind:?} {entry}");
            assert!(plan.text_bounds.width() > 0.0, "{kind:?} {entry}");
            assert!(other_findings(plan).is_empty(), "{kind:?} {entry}: {:?}", other_findings(plan));
        }
    }

    // A companion check that the six kinds' *default* faces are the ones reported when they are the
    // ones missing: the same result with the kind's own family as the absent one, so the two tests
    // cannot both be satisfied by a report that names a constant instead of the request.
    for kind in LETTERING_KINDS {
        let default = style_of(kind).face();
        let request = request_wearing(kind, "Look out!", ABSENT_FAMILY, &default.style);
        let plan = new_balloon(db, &request).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert!(face_reports(&plan)[0].contains(&default.style), "{kind:?}: the kind's own style is not the one named");
    }
}

/// The contract, over the faces the six kinds ask for **by default**: a face is reported if and only
/// if it is not available exactly as named.
///
/// The biconditional is what keeps this test honest in both builds. With `CRAFT_FONTS` empty every
/// kind's face is absent, so every one of them must be named; with the comic faces embedded the same
/// call must report nothing for them. A test that asserted "four reports" would be a statement about
/// this machine; this one is a statement about the contract.
#[test]
fn every_kind_face_that_is_not_available_exactly_named_is_reported_and_no_other_is() {
    let db = FontDb::global();
    let faces: Vec<Face> = LETTERING_KINDS.iter().map(|kind| style_of(*kind).face()).collect();
    let report = lettering_face_report(db, &faces, "lettering");
    let mut absent: Vec<(String, String)> = Vec::new();
    for face in &faces {
        let named =
            report.iter().any(|finding| finding.message.contains(&format!("the lettering face {} {} is not available", face.family, face.style)));
        assert_eq!(
            named,
            !available_exactly(db, face),
            "{} {}: {:?}",
            face.family,
            face.style,
            report.iter().map(|f| f.message.clone()).collect::<Vec<_>>()
        );
        if !available_exactly(db, face) {
            absent.push((face.family.clone(), face.style.clone()));
        }
    }
    // One report per absent face, each once: the six kinds share four faces between them.
    absent.sort();
    absent.dedup();
    assert_eq!(report.len(), absent.len());
    for finding in &report {
        assert_eq!(finding.subject, "lettering");
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.rule, None);
    }
    // The faces are the schema's six kinds and not a list of one: the table ADR-017 pins, spelled as
    // the exporter spells it.
    let defined: Vec<(&str, &str)> = vec![(DIALOGUE_FAMILY, REGULAR), (DIALOGUE_FAMILY, ITALIC), (DIALOGUE_FAMILY, BOLD), (DISPLAY_FAMILY, REGULAR)];
    for (family, style) in defined {
        let face = Face::new(family, style);
        let named = report.iter().any(|finding| finding.message.contains(&format!("{family} {style}")));
        assert_eq!(named, !available_exactly(db, &face), "the kind table asks for {family} {style}: {report:?}");
    }
}

/// The other half of the exporter's distinction: the family is there and the style is not, which a
/// creator fixes differently.
#[test]
fn a_family_that_is_present_without_the_style_asked_for_is_a_substitute_and_not_an_absence() {
    let db = FontDb::global();
    // The bundled body family, in a style no file carries: the family resolves, the style does not.
    let substituted = Face::new(vectorcraft_text::FALLBACK_FAMILY, ABSENT_STYLE);
    let missing = missing_faces(db, std::slice::from_ref(&substituted));
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].status, FontMatch::Style, "the family is present, so this is a substitute and not an absence");
    let report = lettering_face_report(db, std::slice::from_ref(&substituted), "lettering speech");
    assert_eq!(report.len(), 1);
    assert!(report[0].message.contains(&format!("{} {ABSENT_STYLE}", vectorcraft_text::FALLBACK_FAMILY)), "{}", report[0].message);
    assert!(report[0].message.contains("substitute"), "the exporter's own status for this is `substitute`: {}", report[0].message);
    assert!(!report[0].message.contains("missing"), "the family is not missing: {}", report[0].message);
    // And the plan still lays the text out in that family's nearest style.
    let plan = plan_wearing(LetteringKind::Speech, "Look out!", vectorcraft_text::FALLBACK_FAMILY, ABSENT_STYLE).expect("planned");
    assert!(plan.fits());
    assert_eq!(face_reports(&plan).len(), 1);
}

/// The two halves in one assertion: a missing face changes what the creator is **told**, not what is
/// laid out.
///
/// Two plans that ask for two different faces, both absent, resolve to the same face that stands in,
/// so every piece of geometry is identical and the only difference between them is the finding that
/// names the family each one asked for. That is the contract entire: the layout is the fallback's
/// layout either way, and the report is the only thing carrying the difference. A silent
/// substitution collapses this test to nothing — the two reports would both be empty.
#[test]
fn the_layout_is_the_fallback_face_is_layout_and_the_report_is_the_only_difference() {
    let a = plan_wearing(LetteringKind::Speech, "Look out!", "No Such Lettering Face A", REGULAR).expect("planned");
    let b = plan_wearing(LetteringKind::Speech, "Look out!", "No Such Lettering Face B", REGULAR).expect("planned");
    assert_eq!(a.bounds, b.bounds);
    assert_eq!(a.frame, b.frame);
    assert_eq!(a.text_frame, b.text_frame);
    assert_eq!(a.text_bounds, b.text_bounds);
    assert_eq!(a.outcome, b.outcome);
    assert_eq!(a.tail.as_ref().map(|tail| tail.path.clone()), b.tail.as_ref().map(|tail| tail.path.clone()));
    assert_eq!(face_reports(&a).len(), 1, "{:?}", face_reports(&a));
    assert_eq!(face_reports(&b).len(), 1, "{:?}", face_reports(&b));
    assert_ne!(face_reports(&a), face_reports(&b), "the two plans asked for different faces and must say so");
    assert!(face_reports(&a)[0].contains("No Such Lettering Face A Regular"));
    assert!(face_reports(&b)[0].contains("No Such Lettering Face B Regular"));
    assert!(!face_reports(&a)[0].contains("No Such Lettering Face B"), "one plan reported the other's face");
}

/// A move carries the report with it: a creator who drags a balloon does not lose the message about
/// its face (criteria 5 and 3 together).
#[test]
fn a_moved_balloon_still_says_which_face_is_missing() {
    let plan = plan_wearing(LetteringKind::Speech, "Look out!", ABSENT_FAMILY, BOLD).expect("planned");
    let moved = plan.moved_by(Vec2::new(120.0, -80.0)).expect("moved");
    assert_eq!(moved.findings, plan.findings, "the move invented or dropped a finding");
    let reports = face_reports(&moved);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].contains(&format!("{ABSENT_FAMILY} {BOLD}")), "{reports:?}");
    assert!(moved.fits());
}

/// The exporter's own caps and exclusions, at the contract's own door: a name no file can carry is
/// not a font, and the report cannot be made to grow without limit by a pasted name.
#[test]
fn the_contract_keeps_the_exporters_own_exclusions_at_its_own_door() {
    let db = FontDb::global();
    // A CSS generic family and an over-long name are not fonts and are not reported — by the
    // exporter's own rule, verified at the named entry point rather than only in `missing_faces`.
    for not_a_font in ["sans-serif", "SERIF", "  monospace  "] {
        assert!(lettering_face_report(db, &[Face::new(not_a_font, REGULAR)], "lettering speech").is_empty(), "{not_a_font}");
    }
    let too_long = "x".repeat(MAX_NAME + 1);
    assert!(lettering_face_report(db, &[Face::new(too_long, REGULAR)], "lettering speech").is_empty());
    // But a real family asked for in a style no file carries *is* reported, blank style included,
    // which is the exporter's own choice to report it (round 2, R2-F3): the family is a font and the
    // style need not be one the exporter drops.
    assert_eq!(lettering_face_report(db, &[Face::new(DIALOGUE_FAMILY, "")], "lettering speech").len(), 1);
    assert_eq!(lettering_face_report(db, &[Face::new(ABSENT_FAMILY, "")], "lettering speech").len(), 1);
}

/// Where the message sends the creator — the one clause in it that is a claim about the world rather
/// than about the face, and so the one that has to be asserted instead of assumed.
///
/// The comic faces are absent in the gate's configuration and present in others, so this is written
/// as the suite writes the other configuration-dependent halves: a face the build *could* carry is
/// asserted conditionally, and a face no build can carry is asserted outright.
///
/// * `Comic Neue` in a style no file carries is reported in **both** configurations — `missing`
///   without the faces, `substitute` with them — so the comic half needs no condition, and no
///   configuration may send it anywhere but `craft-fonts`.
/// * A family of the project's own is reported in every configuration, and must never be sent
///   there: that was W4's D2. The old wording did exactly that, for every face alike.
#[test]
fn the_report_names_craft_fonts_for_a_comic_face_and_never_for_a_face_of_the_projects_own() {
    let db = FontDb::global();

    // A project's own face: what was observed, and the fix that works. Impossible in every build,
    // so this half is unconditional — both the absent family and the absent style of a family that
    // is not one. (The style is the family's own, which the requested name pins.)
    let own = lettering_face_report(db, &[Face::new(ABSENT_FAMILY, BOLD), Face::new(ABSENT_FAMILY, ABSENT_STYLE)], "lettering speech");
    assert_eq!(own.len(), 2, "{own:?}");
    for finding in &own {
        let message = &finding.message;
        assert!(message.contains(&format!("the lettering face {ABSENT_FAMILY}")), "{message}");
        assert!(
            message.contains(&format!("not one of ADR-017's comic faces ({DIALOGUE_FAMILY}, {DISPLAY_FAMILY})")),
            "the face is not told what it is, only what it is not: {message}"
        );
        assert!(message.contains("craft-fonts is not where it belongs"), "a face no checkout carries is sent to one: {message}");
        assert!(message.contains("installed"), "and the fix that works is not named: {message}");
    }

    // A comic face: `craft-fonts` is where it belongs and the message says so, in either
    // configuration. This is the clause W4's D2 was about, asserted as a property of the report.
    let comic = Face::new(DIALOGUE_FAMILY, ABSENT_STYLE);
    let report = lettering_face_report(db, std::slice::from_ref(&comic), "lettering speech");
    assert_eq!(report.len(), 1, "{DIALOGUE_FAMILY} {ABSENT_STYLE} must be reported whether or not this build embedded the faces: {report:?}");
    assert!(report[0].message.contains(&format!("{DIALOGUE_FAMILY} is one of ADR-017's comic faces")), "{}", report[0].message);
    assert!(report[0].message.contains("craft-fonts"), "{}", report[0].message);
    assert!(!report[0].message.contains("empty"), "and it names no cause it has not checked: {}", report[0].message);

    // The same clause through a plan, for the face the display kind asks for: the message a creator
    // actually reads does not depend on the route it was produced by.
    let plan = plan_wearing(LetteringKind::SoundEffect, "Boom!", DISPLAY_FAMILY, ABSENT_STYLE).expect("planned");
    let reports = face_reports(&plan);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].contains(&format!("{DISPLAY_FAMILY} is one of ADR-017's comic faces")), "{reports:?}");
    assert!(plan.fits(), "{:?}", plan.outcome);
}
