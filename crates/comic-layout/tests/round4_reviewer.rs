//! Round-4 reviewer tests (`code-reviewer`, M1) for `crates/comic-layout`'s panel trees.
//!
//! Round 3 came back `approved` with four preferences, and round 4 folds all four in. This file
//! verifies the *fixes*, independently of the implementer's `round4_layout_engineer.rs`, and answers
//! the three questions the round-3 record left open:
//!
//! * **the residual on finding 4** — `faults` no longer lets a non-finite coordinate switch the checks
//!   off, and `validate::page` now *skips* a non-finite artboard the way it skips a degenerate one.
//!   Is the skip the same skip, or a new silence? And is the artboard named anywhere end-to-end (the
//!   model's rule 15 compares the artboard rect with `ProjectSettings.default_page_size`)?
//! * **finding 1** — one scale per call was kept, and the doc now states the cost (`1.8e-3`pt at
//!   `1e12`pt, ~`5.6e12`pt to mask a printer-visible defect). Are those numbers right, measured through
//!   `faults` rather than read off the formula?
//! * **findings 2 and 3** — the term is "between 8 and 16 ULP", measured 9.3 … 15.3 over `1e6` … `1e10`;
//!   is that right at both mantissa extremes, and is the stated window the one that yields it?
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::validate::{Fault, Limits, PageInput as ValidatePageInput, PanelFrames, TOLERANCE_ULPS, faults, tolerance_at};
use vectorcraft_comic_model::{
    ArtboardFacts, ComicBundle, DocumentFacts, NodeId, PageFile, PageId, ValidationInput, load, validate as validate_model,
};
use vectorcraft_geom::Rect;

/// One ULP of `x` — the unit the relative term is measured in.
fn ulp(x: f64) -> f64 {
    let a = x.abs();
    let exponent = ((a.to_bits() >> 52) & 0x7ff) as i64;
    assert!(exponent > 52, "the helper needs a normal number, not {x:e}");
    f64::from_bits(((exponent - 52) as u64) << 52)
}

/// How many ULP of `scale` the relative term is, when the floor is not the one in force.
fn term_in_ulp(scale: f64) -> f64 {
    (TOLERANCE_ULPS * scale * f64::EPSILON) / ulp(scale)
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a legal panel")
}

fn fixture() -> (PathBuf, ComicBundle) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../comic-model/tests/fixtures/v1/minimal.comiccraft");
    let loaded = load(&root).expect("the M1 fixture bundle must load");
    (root, loaded.bundle)
}

/// The fixture's visual document: one 612x792pt artboard (id 1) and node ids 2–13.
fn fixture_document(artboard: Rect) -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: artboard }], nodes: (2..=13u64).map(NodeId).collect() }
}

/// The fixture's page 8 with a frame for each of its three panels: 9 and 10 share 50pt on both axes,
/// which is a real overlap, and 11 sits beside them.
fn fixture_frames() -> PanelFrames {
    PanelFrames::new()
        .with(NodeId(2), rect(0.0, 0.0, 100.0, 100.0))
        .with(NodeId(3), rect(50.0, 50.0, 100.0, 100.0))
        .with(NodeId(4), rect(400.0, 400.0, 100.0, 100.0))
}

fn page_findings(pages: &BTreeMap<PageId, PageFile>, document: &DocumentFacts) -> Vec<vectorcraft_comic_model::Finding> {
    vectorcraft_comic_layout::validate::page(&ValidatePageInput { pages, document, frames: &fixture_frames(), limits: &Limits::default() })
}

fn model_rule_15(root: &Path, bundle: &ComicBundle, document: &DocumentFacts) -> Vec<vectorcraft_comic_model::Finding> {
    validate_model(bundle, &ValidationInput { root, document, archive_root: None }).into_iter().filter(|finding| finding.rule == Some(15)).collect()
}

#[test]
fn the_model_names_a_non_finite_artboard_when_the_project_has_a_default_size() {
    // Finding 4's residual, end-to-end: `validate::page` skips a non-finite artboard (nothing to measure
    // a panel against), so the question is whether *anything* in the pipeline names it. `comic-model`'s
    // rule 15 compares the artboard's rect with `ProjectSettings.default_page_size`, and this asserts
    // that it fires for an infinite artboard and stays silent for a legal one — with the real fixture,
    // through the real entry point, and not only by reading the rule.
    let (root, bundle) = fixture();
    assert_eq!(
        bundle.project.settings.default_page_size.map(|size| (size.width, size.height)),
        Some((612.0, 792.0)),
        "the fixture must have a default to compare against"
    );
    let pages = bundle.pages.clone();

    let legal = Rect::new(0.0, 0.0, 612.0, 792.0);
    let infinite = Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY);
    let legal_findings = page_findings(&pages, &fixture_document(legal));
    let infinite_findings = page_findings(&pages, &fixture_document(infinite));
    assert_eq!(legal_findings.len(), 1, "the fixture's overlapping frames are one finding on a legal artboard: {legal_findings:?}");
    assert_eq!(infinite_findings, Vec::new(), "and none on an artboard the geometry check is skipped for: {infinite_findings:?}");

    // The artboard itself, named by the model: exactly one rule-15 finding, with the artboard's own size
    // in the message and the model's subject spelling.
    let named = model_rule_15(&root, &bundle, &fixture_document(infinite));
    assert_eq!(named.len(), 1, "an infinite artboard must be named by rule 15: {named:?}");
    assert_eq!(named[0].subject, "Page(8).artboard_id");
    assert!(named[0].message.contains("inf"), "{}", named[0].message);
    assert!(!named[0].is_error(), "rule 15 is a warning: a page may deliberately differ from the default");
    assert_eq!(model_rule_15(&root, &bundle, &fixture_document(legal)), Vec::new(), "an artboard the size of the default is not a rule-15 finding");

    // And the *condition* on that coverage, stated rather than left to be discovered: rule 15 returns
    // early when the project names no default page size, so with `None` nothing in the pipeline names
    // the artboard. That is rule 15's own precondition (there is no expected size to differ from) and
    // not a hole this crate can close: design §12.1 gives "unsupported page dimensions" to `comic-model`.
    let mut without_default = bundle.clone();
    without_default.project.settings.default_page_size = None;
    assert_eq!(model_rule_15(&root, &without_default, &fixture_document(infinite)), Vec::new(), "rule 15 needs a default to compare with");
    assert_eq!(
        page_findings(&without_default.pages, &fixture_document(infinite)),
        Vec::new(),
        "and the layout half still skips it — so with no default, nothing names it"
    );
}

#[test]
fn the_tolerance_is_finite_at_every_finite_scale_and_non_finite_only_for_a_non_finite_one() {
    // Round 4 recorded the *finite* ceiling as a residual: `8 * scale` itself overflowed past
    // `f64::MAX / 8`, so the same two panels were an overlap on a `1e307`pt page and nothing at all on a
    // `1e308`pt one. **That residual was round 5's defect, not a resolution limit.** The overflow was in
    // the *order of the product*: `TOLERANCE_ULPS * scale.abs() * f64::EPSILON` parses as
    // `(8.0 * scale) * EPSILON`, so `8.0 * 2.25e307` = `1.8e308` overflowed to `+inf` **before** epsilon
    // was ever multiplied in, `GEOMETRY_TOLERANCE.max(inf)` was `inf`, every threshold held, and the
    // geometry check was switched off **by a finite page** — round 4's hole reached a different way.
    // Round 5 multiplies the small constant first (`8 * EPSILON` = `1.776e-15`, times even `f64::MAX` =
    // `3.19e293`, finite everywhere), so the ceiling is gone: a page is never silently unchecked for
    // being large, only for being unrepresentable. Both orders are written out below, so a future reader
    // cannot "tidy" the reorder away without a test saying what it costs.
    let old_order = |scale: f64| vectorcraft_comic_layout::validate::GEOMETRY_TOLERANCE.max(TOLERANCE_ULPS * scale.abs() * f64::EPSILON);
    let threshold = f64::MAX / TOLERANCE_ULPS;
    assert!((threshold / 2.247e307 - 1.0).abs() < 1e-3, "the old order overflowed past {threshold:e}pt");

    // (a) Bit-identical where it must be: every ordinary page, including both sides of the crossover
    // where the floor takes over, and the value the masking test pins to the last digit.
    for scale in [0.0, 1e-3, 1.0, 612.0, 1e3, 1e5, 4.8e5, 5.6e5, 1e6, 3.7e6, 1e7, 1e8, 1e10, 1e12, 1e15, 1e100, 1e307, threshold] {
        assert_eq!(tolerance_at(scale), old_order(scale), "the two orders must agree at {scale:e}pt");
    }
    assert_eq!(tolerance_at(1e12), 0.0017763568394002505, "the round-4 masking test's pinned value is unchanged");

    // (b) And they differ only where the old one was wrong: past the threshold the old order is `inf` and
    // the delivered one is finite, all the way to `f64::MAX`.
    assert!(old_order(threshold * 1.01).is_infinite(), "the old order is infinite just past the threshold");
    assert!(tolerance_at(threshold * 1.01).is_finite(), "the reordered term is finite there");
    assert!(tolerance_at(f64::MAX).is_finite(), "and at `f64::MAX` there is no ceiling at all");
    assert!((3.1e293..3.3e293).contains(&tolerance_at(f64::MAX)), "the term at `f64::MAX` is {:e}", tolerance_at(f64::MAX));

    // (c) A non-finite *scale* still has no ceiling — which is exactly why `magnitude` must keep skipping
    // a coordinate that is not finite, and why that half of round 4 is still load-bearing.
    assert!(tolerance_at(f64::INFINITY).is_infinite());
    assert!(tolerance_at(f64::NEG_INFINITY).is_infinite());
    assert_eq!(tolerance_at(f64::NAN), vectorcraft_comic_layout::validate::GEOMETRY_TOLERANCE);

    // (d) The behaviour, which is the whole point: the same two panels now report on every page size,
    // including the one that used to be silent.
    let expected = vec![Fault::Overlap { first: 0, second: 1 }];
    let limits = Limits::default();
    let panels = [rect(0.0, 0.0, 1e300, 1e300), rect(5e299, 0.0, 1e300, 1e300)];
    for (scale, why) in [
        (f64::INFINITY, "an infinite page: the *magnitude* is finite (the panels are), so the check is made"),
        (1e307, "finite and below the old overflow"),
        (2.25e307, "finite and past the old overflow: the page round 4 recorded as silent"),
        (1e308, "finite and far past it"),
        (f64::MAX, "the largest finite page there is"),
    ] {
        let page = Rect::new(0.0, 0.0, scale, scale);
        assert_eq!(vectorcraft_comic_layout::validate::faults(page, None, &panels, &limits), expected, "{why}");
    }
    // One ULP at `1e308`pt, for the record — the reason round 3 mistook the ceiling for a resolution
    // limit: the geometry *is* huge there, and it is still a comparison the arithmetic represents.
    assert!((1.9e292..2.0e292).contains(&ulp(1e308)), "one ULP at 1e308pt is {:e}", ulp(1e308));
}

#[test]
fn every_unusable_artboard_gets_one_skip_not_one_silence_per_cause() {
    // Round 3's question: is the non-finite skip *the same skip* a degenerate artboard gets, or a new
    // silence under a new name? `validate::page` cannot say "skipped" — a skip is an empty list — so the
    // only honest test is that every cause produces the *same* list, and that the list still carries the
    // findings that need no artboard. The rule-7 finding is here to prove the skip is scoped to geometry
    // and does not take the rest of the page's checks with it.
    let (_, bundle) = fixture();
    let mut pages = bundle.pages.clone();
    {
        // Panel 11's frame is gone from the document: rule 7, which needs no artboard at all.
        pages.get_mut(&PageId(8)).expect("the fixture has page 8").panels[2].frame_node_id = NodeId(99);
    }
    let causes = [
        ("zero area", Rect::new(0.0, 0.0, 0.0, 792.0)),
        ("inverted", Rect::new(612.0, 0.0, 0.0, 792.0)),
        ("NaN", Rect::new(0.0, 0.0, f64::NAN, 792.0)),
        ("+inf", Rect::new(0.0, 0.0, f64::INFINITY, 792.0)),
        ("-inf", Rect::new(0.0, 0.0, f64::NEG_INFINITY, 792.0)),
    ];
    let mut seen: Option<(String, Vec<vectorcraft_comic_model::Finding>)> = None;
    for (name, artboard) in causes {
        let findings = page_findings(&pages, &fixture_document(artboard));
        assert_eq!(findings.len(), 1, "{name}: the skip must not take the rule-7 finding with it: {findings:?}");
        assert_eq!(findings[0].rule, Some(7), "{name}: {findings:?}");
        match &seen {
            Some((first, expected)) => assert_eq!(&findings, expected, "{name} must be skipped exactly as {first} is"),
            None => seen = Some((name.to_owned(), findings)),
        }
    }
    // A *finite* artboard of the same shape reports the geometry as well, so the skip is a decision about
    // the artboard and not a check that stopped working: two findings, the link and the overlap.
    let legal = page_findings(&pages, &fixture_document(Rect::new(0.0, 0.0, 612.0, 792.0)));
    assert_eq!(legal.len(), 2, "a legal artboard reports both: {legal:?}");
    // And the residual is distinguishable from the skip only by its cause, which the caller cannot see: a
    // *finite* artboard past the overflow (1e308pt) is not skipped and is silent for a different reason.
    // Recorded, not asserted as correct — see the round-4 record.
    let past_the_ceiling = page_findings(&pages, &fixture_document(Rect::new(0.0, 0.0, 1e308, 1e308)));
    assert_eq!(past_the_ceiling.len(), 1, "1e308pt is finite, so it is not skipped, and the overflow silences the geometry: {past_the_ceiling:?}");
    assert_eq!(past_the_ceiling[0].rule, Some(7));

    // **The guard's stated reason is no longer the reason** (round 4's finding 1). Its comment says
    // "`faults` cannot size a tolerance from an infinite magnitude, and handing it one would satisfy
    // every threshold — the geometry check would be silently off". The same round-4 change that added
    // the `is_finite()` arm also fixed `magnitude`, so the primitive the guard is protecting the caller
    // *from* answers correctly for the very page the guard skips — and `page()` skips it anyway. The
    // contrast is the finding: an overlap the primitive reports, on the page whose geometry findings
    // `page()` discards.
    let infinite_page = Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY);
    assert_eq!(
        faults(infinite_page, None, &[rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 50.0, 100.0, 100.0)], &Limits::default()),
        vec![Fault::Overlap { first: 0, second: 1 }],
        "the primitive makes the overlap finding for the page `page()` will not hand it"
    );
    let discarded = page_findings(&pages, &fixture_document(infinite_page));
    assert_eq!(discarded.len(), 1, "and `page()` discards it, leaving only the link finding: {discarded:?}");
    assert!(discarded.iter().all(|finding| finding.rule == Some(7)), "no geometry finding survives the skip: {discarded:?}");
}

#[test]
fn the_ulp_multiple_is_eight_at_a_power_of_two_and_stays_under_sixteen_above_it() {
    // Finding 2's wording: "`8 * mantissa` ULP of `scale`: between 8 and 16 ULP". Checked at both
    // mantissa extremes rather than in the middle of the range where it is easy to be roughly right:
    // at a power of two the mantissa is exactly 1, so the term is *exactly* 8 ULP, and approaching the
    // next binade it approaches 16 without reaching it. The floor is what a reader could be misled by —
    // where the floor wins the multiple is larger — so its regime is checked on the other side.
    let power_of_two = 2.0_f64.powi(20);
    assert_eq!(term_in_ulp(power_of_two), 8.0, "a mantissa of 1 gives exactly 8 ULP");
    assert_eq!(ulp(power_of_two), 2.0_f64.powi(-32));
    // The largest f64 below 2^21: mantissa 2 - 2^-52.
    let sixteens = f64::from_bits(0x413f_ffff_ffff_ffff);
    assert!(sixteens < 2.0_f64.powi(21), "{sixteens:e} must be below 2^21");
    assert_eq!((2.0_f64).powi(21) - sixteens, ulp(sixteens), "the largest f64 below 2^21 is one ULP of it");
    let top = term_in_ulp(sixteens);
    assert!((15.99..16.0).contains(&top), "approaching the next binade it approaches 16: {top}");
    // So the range is [8, 16) over the whole binade — never below 8 there, never reaching 16.
    for step in 0..=64 {
        let scale = power_of_two + (power_of_two * f64::from(step) / 64.0);
        let in_ulp = term_in_ulp(scale);
        assert!((8.0..16.0).contains(&in_ulp), "at {scale:e}pt the term is {in_ulp} ULP");
    }
    // The window the doc names, re-measured: 9.3 … 15.3 over 1e6 … 1e10pt, which is inside the
    // relative-term regime (the floor wins below `1e-9 / (8 * EPSILON)` ≈ 5.63e5pt).
    let window = [1e6_f64, 3.7e6, 1e7, 1e8, 1e10].map(term_in_ulp);
    assert!(window.iter().all(|value| (9.3..15.3).contains(value)), "the doc's measured window is {window:?}");
    // Below the crossover the *floor* is the tolerance and the multiple is larger than 16 — a fact about
    // `tolerance_at`'s output rather than about the constant, which is why the doc names its window.
    assert!(tolerance_at(1e5) == vectorcraft_comic_layout::validate::GEOMETRY_TOLERANCE);
    assert!(tolerance_at(1e5) / ulp(1e5) > 16.0, "the floor gives more than 16 ULP of a 1e5pt page");
}

#[test]
fn the_documented_cost_of_one_scale_per_call_is_the_measured_one() {
    // Finding 1's two numbers, checked by measuring them with `faults` rather than by re-deriving the
    // formula — the doc's claim is about behaviour, so the test is about behaviour.
    assert_eq!(tolerance_at(1e12), TOLERANCE_ULPS * 1e12 * f64::EPSILON);
    assert!((tolerance_at(1e12) - 1.8e-3).abs() < 1e-4, "the doc says 1.8e-3pt at 1e12pt, and it is {:e}", tolerance_at(1e12));
    let masking = 0.01 / (TOLERANCE_ULPS * f64::EPSILON);
    assert!((5.6e12..5.7e12).contains(&masking), "the doc says ~5.6e12pt to mask a printer-visible defect, and it is {masking:e}pt");

    // Through the call: a panel a printer could see crossing the safe margin (0.01pt) is reported on its
    // own, still reported with a `5e12`pt rectangle in the call, and masked by a `6e12`pt one. One
    // rectangle sets the tolerance for every comparison in the call, exactly as the doc now states.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let margin = 18.0;
    let limits = Limits::default();
    let breach = rect(margin - 0.01, margin, 100.0, 100.0);
    assert_eq!(
        vectorcraft_comic_layout::validate::faults(page, Some(margin), &[breach], &limits),
        vec![Fault::InSafeMargin { panel: 0 }],
        "a 0.01pt breach is a finding on its own"
    );
    for (runaway, masked) in [(5e12_f64, false), (6e12, true)] {
        let poison = rect(runaway, runaway, 100.0, 100.0);
        let findings = vectorcraft_comic_layout::validate::faults(page, Some(margin), &[breach, poison], &limits);
        if masked {
            assert_eq!(findings, vec![Fault::OutsidePage { panel: 1 }], "at {runaway:e}pt the breach a printer can see is masked");
        } else {
            assert_eq!(findings.len(), 2, "at {runaway:e}pt it is still reported: {findings:?}");
            assert_eq!(findings[0], Fault::InSafeMargin { panel: 0 }, "{findings:?}");
        }
    }
    // The page's own magnitude is the floor's regime, so the masking is a caller's inconsistency and not
    // a cost paid by a plan this crate produced — the doc no longer claims the panels are always inside.
    assert_eq!(tolerance_at(792.0), vectorcraft_comic_layout::validate::GEOMETRY_TOLERANCE);
}
