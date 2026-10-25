//! Round-3 reviewer tests (`code-reviewer`, M1) for `crates/comic-layout`'s panel trees.
//!
//! Round 2 raised 5 findings (2 bug, 3 preference) and all five are addressed. Round 3 verifies the
//! *fixes* rather than re-reading them: the guarantee `template::rows` now states is the strength
//! `f64` delivers (≤ 1 ULP of the page's coordinate magnitude, exact at the document origin), the
//! validator's tolerance is now relative, the room guard now measures the quantity the last row
//! actually gets, and the zero-gutter column property has a test a mutation cannot survive.
//!
//! The tolerance is new code, so it gets its own arithmetic here: the shape, the constant, one scale
//! per call, the floor, negatives, and zero.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::layout::{PageInput as PlanPage, PlanRequest, plan};
use vectorcraft_comic_layout::template::error::TemplateError;
use vectorcraft_comic_layout::template::{LayoutSettings, TemplateName, content_box, rows};
use vectorcraft_comic_layout::validate::{GEOMETRY_TOLERANCE, Limits, TOLERANCE_ULPS, faults, tolerance_at};
use vectorcraft_comic_model::ReadingDirection;
use vectorcraft_geom::Rect;

/// One ULP of `x` — the unit `template::rows` and `validate::tolerance_at` are both stated in.
fn ulp(x: f64) -> f64 {
    let a = x.abs();
    let exponent = ((a.to_bits() >> 52) & 0x7ff) as i64;
    assert!(exponent > 52, "the helper needs a normal number, not {x:e}");
    f64::from_bits(((exponent - 52) as u64) << 52)
}

/// The largest absolute coordinate of a page — the magnitude the crate's guarantee is relative to.
fn magnitude(page: Rect) -> f64 {
    page.x0.abs().max(page.y0.abs()).max(page.x1.abs()).max(page.y1.abs())
}

fn shape(name: TemplateName) -> vectorcraft_comic_layout::template::Shape {
    name.shape().expect("a template name has a shape")
}

/// A page the template produced must survive its own gutter as the minimum: a finding here can only
/// be the crate reporting its own arithmetic (round 1's F1, round 2's R1/R2).
fn self_check_limits(gutter: f64) -> Limits {
    Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: gutter }
}

#[test]
fn the_one_ulp_guarantee_is_attained_and_never_exceeded() {
    // The retracted claim's *replacement*, checked as the claim it is rather than as the validator's
    // 8-ULP tolerance: "the far edges are the content box's to within one ULP of the page's coordinate
    // magnitude". A guarantee that is never attained is slack, and one that is exceeded is round 1's
    // F1 again — so this asserts both halves, over a space neither the implementer's sweep nor mine
    // above covers: negative and huge origins, page sides from 50pt to 1e6, margins up to 49.9% of the
    // page, four gutters and nine templates. Measured: 294 420 far edges in the wider sweep this test
    // is cut from (17 088 here), none past 1 ULP, worst exactly 1 ULP (a `grid-1x3` on a 612x2000.2pt
    // page anchored at (0.1, 0.3), margin 305.388) — the implementer's own "worst 0.256 ULP" is a
    // sample, not the bound, and the bound still holds.
    let mut checked = 0;
    let mut attained = 0;
    let mut worst = 0.0_f64;
    let mut worst_case = String::new();
    for origin in [(0.0, 0.0), (0.1, 0.3), (1.0 / 3.0, 1.0 / 7.0), (-1000.5, -2000.25), (1e6, 1e6), (1e8, 1e8), (-3.7e9, 5.1e8), (1e10, 1e10)] {
        for (width, height) in [(50.5, 50.5), (612.0, 792.0), (612.0, 1999.9), (1000.0, 1999.9), (1e6, 1e6)] {
            for margin_fraction in [0.0, 0.1, 0.45, 0.499] {
                for gutter in [0.0, 0.1, 3.3, 12.0] {
                    let margin = width * margin_fraction;
                    let Ok(settings) = LayoutSettings::new(margin, gutter) else { continue };
                    let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                    let Ok(content) = content_box(page, &settings) else { continue };
                    let scale = magnitude(page);
                    for name in [
                        TemplateName::TwoTier,
                        TemplateName::ThreeTier,
                        TemplateName::WideTop,
                        TemplateName::Grid { rows: 1, columns: 3 },
                        TemplateName::Grid { rows: 7, columns: 1 },
                        TemplateName::Grid { rows: 9, columns: 3 },
                    ] {
                        let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                        for (index, panels) in laid.iter().enumerate() {
                            let last = panels.last().expect("a row holds a panel");
                            let mut measure = |edge: f64, want: f64, what: &str| {
                                checked += 1;
                                let off = (edge - want).abs() / ulp(scale);
                                if off == 1.0 {
                                    attained += 1;
                                }
                                if off > worst {
                                    worst = off;
                                    worst_case = format!("{name:?} page {page:?} margin {margin} gutter {gutter}: {what} {off} ULP");
                                }
                            };
                            measure(last.right(), content.x1, &format!("row {index} right"));
                            if index + 1 == laid.len() {
                                measure(last.bottom(), content.y1, "last row bottom");
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 15_000, "the sweep must run: {checked}");
    assert!(worst <= 1.0, "the one-ULP guarantee is exceeded: {worst} ULP ({worst_case})");
    assert!(attained > 0, "no far edge is *exactly* one ULP off, so the bound is slack and the sweep is not reaching the edge cases");
}

#[test]
fn the_tolerance_grows_with_the_magnitude_and_keeps_the_floor_near_the_origin() {
    // R2's fix is `tolerance_at(scale) = max(GEOMETRY_TOLERANCE, TOLERANCE_ULPS * |scale| * EPSILON)`.
    // The floor must still be what a US Letter page gets, the growth must be linear in the magnitude,
    // and the sign of the magnitude must not matter — a page at `-1e8` is as far from the origin as
    // one at `1e8`.
    for ordinary in [0.0, 1.0, 612.0, 1e3, 1e5, 5.6e5] {
        assert_eq!(tolerance_at(ordinary), GEOMETRY_TOLERANCE, "a page at {ordinary:e}pt gets the floor");
        assert_eq!(tolerance_at(-ordinary), GEOMETRY_TOLERANCE, "and so does a page at -{ordinary:e}pt");
    }
    for scale in [1e6, 1e7, 1e8, 1e10] {
        assert_eq!(tolerance_at(scale), TOLERANCE_ULPS * scale * f64::EPSILON, "at {scale:e}pt");
        assert_eq!(tolerance_at(-scale), tolerance_at(scale), "the sign of the page's position is not a length");
        assert!(tolerance_at(scale) > GEOMETRY_TOLERANCE, "the relative term must win at {scale:e}pt");
        // The floor is a floor: no scale gets a tolerance below `GEOMETRY_TOLERANCE`.
        assert!(tolerance_at(scale) >= GEOMETRY_TOLERANCE);
    }
    // Where the term takes over, from the formula rather than from a literal: `1e-9 / (8 * EPSILON)`.
    let crossover = GEOMETRY_TOLERANCE / (TOLERANCE_ULPS * f64::EPSILON);
    assert!(tolerance_at(crossover * 1.5) > GEOMETRY_TOLERANCE, "the term wins above the crossover");
    assert!(tolerance_at(crossover * 0.99) == GEOMETRY_TOLERANCE, "and the floor holds below it");
    // The doc calls the constant "eight ULP". `8 * scale * EPSILON` is `8 * mantissa` ULP of `scale`
    // (the mantissa is in `[1, 2)`), so the statement is a *lower* bound on the constant and never an
    // over-claim — worth pinning, because the other direction would be a false promise.
    for scale in [1e6, 3.7e6, 1e7, 1e8, 1e10] {
        let in_ulp = tolerance_at(scale) / ulp(scale);
        assert!((TOLERANCE_ULPS..2.0 * TOLERANCE_ULPS).contains(&in_ulp), "at {scale:e}pt the constant is {in_ulp} ULP");
    }
}

#[test]
fn the_tolerance_covers_the_worst_drift_the_template_itself_produces() {
    // The constant is only right if it dominates the error the crate's own arithmetic makes. Measure
    // it independently: for every far edge the template places, how far is it from the content box,
    // in ULP of the page's magnitude? The answer must stay inside `TOLERANCE_ULPS`, or `plan()` would
    // report a page the template just built — round 1's F1 and round 2's R2 both.
    let mut checked = 0;
    let mut worst = 0.0_f64;
    let mut worst_case = String::new();
    let mut inexact = 0;
    for origin in [(0.0, 0.0), (100.0, 50.0), (0.1, 0.3), (-1000.5, -2000.25), (1e6, 1e6), (1e7, 1e7), (1e8, 1e8), (-1e8, 1e8)] {
        for (width, height) in [(612.0, 792.0), (611.0, 1000.0), (612.3, 964.3), (595.7, 841.9)] {
            for margin in [0.0, 3.3, 18.0] {
                for gutter in [0.0, 3.3, 12.0] {
                    let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                    let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                    let content = content_box(page, &settings).expect("a content box");
                    let scale = magnitude(page);
                    for name in [
                        TemplateName::TwoTier,
                        TemplateName::ThreeTier,
                        TemplateName::WideTop,
                        TemplateName::Grid { rows: 1, columns: 3 },
                        TemplateName::Grid { rows: 3, columns: 5 },
                        TemplateName::Grid { rows: 7, columns: 1 },
                    ] {
                        let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                        let mut measure = |edge: f64, want: f64, what: &str| {
                            checked += 1;
                            if edge != want {
                                inexact += 1;
                            }
                            let off = (edge - want).abs() / ulp(scale);
                            if off > worst {
                                worst = off;
                                worst_case = format!("{name:?} at {origin:?} {width}x{height} margin {margin} gutter {gutter}: {what} {off} ULP");
                            }
                        };
                        for (index, panels) in laid.iter().enumerate() {
                            let last = panels.last().expect("a row holds a panel");
                            measure(last.right(), content.x1, &format!("row {index} right"));
                            if index + 1 == laid.len() {
                                measure(last.bottom(), content.y1, "last row bottom");
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 5_000, "the sweep must run: {checked}");
    assert!(inexact > 0, "the sweep must reach pages where the identity `a + (b - a) == b` fails, or it is not testing f64");
    assert!(worst > 0.0, "at least one far edge must be off for the relative term to have work to do");
    assert!(
        worst <= TOLERANCE_ULPS,
        "the worst far edge is {worst} ULP off ({worst_case}), past the {TOLERANCE_ULPS}-ULP constant — `plan()` would report the template's own page"
    );
}

#[test]
fn the_documented_guarantee_is_the_strength_f64_delivers_and_not_less() {
    // `template::rows` now promises "the far edges are the content box's to within one ULP of the
    // page's coordinate magnitude ... exact when the page's rectangle starts at the document origin".
    // Both halves of that sentence have to be true, and the second has to be *worth* something: an
    // origin-anchored page is exact, and a page away from the origin is not — R1's reproduction, kept
    // as a *positive* assertion so a future regression to "exact everywhere" is caught here.
    let settings = LayoutSettings::new(3.3, 12.0).expect("legal settings");
    // R1's single row: nothing accumulates, so the identity itself is what fails.
    let page = Rect::new(0.1, 0.3, 50.6, 33.6);
    let content = content_box(page, &settings).expect("a content box");
    let laid = rows(page, &shape(TemplateName::Grid { rows: 1, columns: 3 }), &settings).expect("the page has room");
    let bottom = laid[0][0].bottom();
    assert_ne!(bottom, content.y1, "this page falsifies the identity, so a claim of exactness here would be false");
    assert!((bottom - content.y1).abs() / ulp(magnitude(page)) <= 1.0, "and it is within one ULP of the page's magnitude");
    // R1's other reproduction: a negative origin with the default margin and no gutter, where the
    // right edges came out one ULP past `content.x1`.
    let tight = LayoutSettings::new(3.3, 0.0).expect("legal settings");
    let page = Rect::new(-1000.5, -2000.25, -388.5, -1208.25);
    let content = content_box(page, &tight).expect("a content box");
    let laid = rows(page, &shape(TemplateName::TwoTier), &tight).expect("the page has room");
    let mut off = 0;
    for panels in &laid {
        if panels[0].right() != content.x1 {
            off += 1;
        }
        assert!((panels[0].right() - content.x1).abs() / ulp(magnitude(page)) <= 1.0, "still within one ULP");
    }
    assert!(off > 0, "the negative-origin page is the one R1 measured, so it must not be exact");
    // And the half that is exact, on a page where every coordinate is a small multiple of one ULP:
    // a page anchored at the origin.
    for height in [792.0, 1000.0, 964.3, 1234.5] {
        for width in [612.0, 611.0, 595.7] {
            let page = Rect::new(0.0, 0.0, width, height);
            let content = content_box(page, &settings).expect("a content box");
            let laid = rows(page, &shape(TemplateName::WideTop), &settings).expect("the page has room");
            let last = laid.last().and_then(|row| row.last()).expect("a panel");
            assert_eq!(last.right(), content.x1, "{width}x{height}pt anchored at the origin must be exact");
            assert_eq!(last.bottom(), content.y1, "{width}x{height}pt anchored at the origin must be exact");
        }
    }
}

#[test]
fn round_two_r2_is_fixed_and_a_page_far_from_the_origin_is_still_not_a_defect() {
    // R2: an absolute `1e-9` could not cover an unbounded coordinate, so a page at 1e7/1e8 was
    // reported for the gap it had just computed. Sweep both signs and three magnitudes *through the
    // public path* — plan a page, then check it against its own gutter as the minimum.
    let mut checked = 0;
    for magnitude in [1e6_f64, 1e7, 1e8] {
        for sign in [1.0_f64, -1.0] {
            let origin = (sign * magnitude, sign * magnitude);
            for (width, height) in [(612.0, 792.0), (611.0, 1000.0), (612.3, 964.3)] {
                for margin in [0.0, 3.3, 18.0] {
                    for gutter in [0.0, 3.3, 12.0] {
                        let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                        let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                        for name in [
                            TemplateName::TwoTier,
                            TemplateName::ThreeTier,
                            TemplateName::WideTop,
                            TemplateName::Grid { rows: 2, columns: 3 },
                            TemplateName::Grid { rows: 7, columns: 1 },
                        ] {
                            let Ok(planned) =
                                plan(PlanPage::Artboard(page), &PlanRequest::new(name, shape(name).panel_count(), ReadingDirection::Ltr), &settings)
                            else {
                                continue;
                            };
                            checked += 1;
                            let findings = vectorcraft_comic_layout::validate::plan(&planned, &self_check_limits(gutter));
                            assert!(
                                findings.is_empty(),
                                "{name:?} at {origin:?} {width}x{height}pt margin {margin} gutter {gutter} is reported against itself: {findings:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 400, "the sweep must run: {checked}");
}

#[test]
fn round_two_r2_is_fixed_without_going_blind_to_a_defect_a_creator_could_see() {
    // A relative tolerance is only a fix if it does not also silence real findings. At 1e8 the
    // tolerance is `8 * 1e8 * EPSILON ≈ 1.8e-7`pt — five orders below a printer's smallest mark — so a
    // hundredth of a point under a threshold must still be reported at that magnitude, on both axes.
    let scale = 1e8;
    let page = Rect::new(scale, scale, scale + 612.0, scale + 792.0);
    let tolerance = tolerance_at(scale);
    assert!(tolerance < 0.01 / 100.0, "the tolerance at 1e8pt ({tolerance:e}) must stay far below a printer's mark");
    let margins = Limits::default();
    let inside = PanelRect::new(scale, scale, margins.min_panel_width + 0.01, margins.min_panel_height).expect("a legal panel");
    let too_small = PanelRect::new(scale, scale, margins.min_panel_width - 0.01, margins.min_panel_height).expect("a legal panel");
    assert_eq!(faults(page, None, &[inside], &margins), Vec::new(), "a panel a hundredth over the minimum is fine");
    assert_eq!(faults(page, None, &[too_small], &margins).len(), 1, "a hundredth under is still a defect at 1e8pt");
    // And the gutter axis: a gap a hundredth of a point under the minimum, at the same magnitude.
    let tight = [
        PanelRect::new(scale + 100.0, scale, 100.0, 100.0).expect("a legal panel"),
        PanelRect::new(scale + 203.29, scale, 100.0, 100.0).expect("a legal panel"),
    ];
    let limits = Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: 3.3 };
    assert_eq!(faults(page, None, &tight, &limits).len(), 1, "a gutter a hundredth under the minimum is still thin at 1e8pt");
    let wide = [
        PanelRect::new(scale + 100.0, scale, 100.0, 100.0).expect("a legal panel"),
        PanelRect::new(scale + 203.31, scale, 100.0, 100.0).expect("a legal panel"),
    ];
    assert_eq!(faults(page, None, &wide, &limits), Vec::new(), "and a gutter a hundredth over it is not");
}

#[test]
fn one_scale_per_call_covers_the_pages_own_coordinates_and_every_panel_it_is_given() {
    // NOTE, not a defect (see the round-3 record). `magnitude` takes one scale per call, from the page
    // *and* every panel, and justifies it with "every panel is inside the page (that is what
    // containment checks)". That is true of a plan this crate produced, but `faults` is public and its
    // own doc says a caller hands it rectangles this crate did not compute — so a runaway rectangle
    // raises the tolerance for all of them. This test records the exact boundary, so the note is a
    // measurement rather than an impression, and pins that masking a defect a printer can *see* needs
    // a panel at ~5.6e12pt (`0.01 / (8 * EPSILON)`), not merely a large one.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let limits = Limits::default();
    let margin = 18.0;
    let visible = 0.01_f64; // a printer's smallest mark
    let sneaking_in = 0.001;
    let just_inside = PanelRect::new(margin - sneaking_in, margin, 100.0, 100.0).expect("a legal panel");
    // At the page's own magnitude the floor applies, and the panel is reported: 0.001pt past the
    // margin is well above 1e-9, so a panel that crosses it by a thousandth of a point is a finding.
    let reported = faults(page, Some(margin), &[just_inside], &limits);
    assert_eq!(reported.len(), 1, "at an ordinary magnitude a thousandth of a point past the margin is a finding: {reported:?}");
    // Add one runaway rectangle and the same panel goes quiet, because the scale is the call's.
    let runaway = PanelRect::new(1e12, 1e12, 100.0, 100.0).expect("a legal panel");
    assert!(tolerance_at(1e12) > sneaking_in, "the runaway panel must inflate the tolerance past the violation");
    let masked = faults(page, Some(margin), &[just_inside, runaway], &limits);
    assert_eq!(masked.len(), 1, "the runaway panel is itself outside the page; only *it* is reported: {masked:?}");
    assert!(matches!(masked[0], vectorcraft_comic_layout::validate::Fault::OutsidePage { panel: 1 }), "{masked:?}");
    // The scale at which a defect a printer *can* see would go quiet — the note's boundary.
    let masking_scale = visible / (TOLERANCE_ULPS * f64::EPSILON);
    assert!(masking_scale > 5.0e12 && masking_scale < 6.0e12, "the boundary is {masking_scale:e}pt");
    assert!(tolerance_at(masking_scale) >= visible, "at the boundary the tolerance reaches the smallest visible mark");
    assert!(tolerance_at(masking_scale / 10.0) < visible, "and an order of magnitude nearer it does not");
}

#[test]
fn round_two_r3_is_fixed_on_both_axes() {
    // R3: the guard measured `nominal_row_height` while the last row was built with the remainder
    // `content.y1 - top`, so a page the guard let through failed with `Rect(NonPositive)` — an error
    // about a rectangle nobody asked for. The guard now measures the quantity the last row gets, and
    // the width axis has the same shape (`content.x1 - left` for the last column).
    // Each case: the page's narrow side, the grid, and the axis the narrow side is on. The gutter is
    // derived so that the gutters alone consume all but a hundred-trillionth of a point of the side —
    // a page the nominal-size guard passes and the remainder guard must catch.
    for (side, wanted, axis) in [
        (10.05_f64, TemplateName::Grid { rows: 64, columns: 1 }, "height"),
        (10.05, TemplateName::Grid { rows: 1, columns: 64 }, "width"),
        // A thousandth of a point more room, and the *remainder* is what is left: the nominal width
        // here is `1.6e-11`, well above zero, so only the remainder guard can refuse it.
        (10.050000001, TemplateName::Grid { rows: 1, columns: 64 }, "width"),
        (10.050000001, TemplateName::Grid { rows: 64, columns: 1 }, "height"),
    ] {
        let gutter = (side / 63.0) * (1.0 - 1e-14);
        let settings = LayoutSettings::new(0.0, gutter).expect("a legal gutter");
        let page = if axis == "height" { Rect::new(0.0, 0.0, 100.0, side) } else { Rect::new(0.0, 0.0, side, 100.0) };
        let result = rows(page, &shape(wanted), &settings);
        match result {
            // The failure must name the room, not a rectangle: this is the message R3 turned on.
            Err(TemplateError::NoRoomDown { .. }) | Err(TemplateError::NoRoomAcross { .. }) => {}
            Err(TemplateError::Rect(error)) => panic!("the guard let the page through and a {axis} rectangle failed instead: {error:?}"),
            Err(other) => panic!("unexpected refusal on the {axis} axis: {other:?}"),
            Ok(laid) => {
                // If the guard lets it through, every panel must be a real panel — a page with no room
                // must not come back as a page of degenerate ones.
                for panel in laid.iter().flatten() {
                    assert!(panel.width() > 0.0 && panel.height() > 0.0, "a page the guard let through produced a degenerate panel");
                }
                panic!("{side}pt has no room for {wanted:?}, so it must be refused as no room: Ok({laid:?})");
            }
        }
    }
    // The guard's own boundary from the other side: a page with room lays out, and the last row is the
    // remainder, so it reaches the content box.
    let settings = LayoutSettings::new(0.0, 0.15952380952380796).expect("a legal gutter");
    let roomy = Rect::new(0.0, 0.0, 100.0, 10.5);
    let laid = rows(roomy, &shape(TemplateName::Grid { rows: 64, columns: 1 }), &settings).expect("10.5pt holds 64 rows");
    assert_eq!(laid.len(), 64);
    let content = content_box(roomy, &settings).expect("a content box");
    assert_eq!(laid[63][0].bottom(), content.y1, "the last row is the remainder, and it reaches the content box");
}

#[test]
fn round_two_r4_is_fixed_and_the_column_axis_is_pinned_by_an_exact_meeting() {
    // R4: the implementer's column guard could not fail for the mutation it was written for (M-C,
    // index-multiplied column lefts), because every *far* edge stays exact. The new
    // `a_zero_gutter_row_has_its_panels_meet_exactly` asserts the edge **between** panels instead,
    // and at a zero gutter `left += width` reproduces `right()` bit for bit. Reproduced here with a
    // wider sweep — widths, column counts, origins and templates M-C was not tried on — so the claim
    // is not one property's word. A far-edge check is *not* enough: this test also asserts that.
    let mut checked = 0;
    for origin in [(0.0, 0.0), (0.1, 0.3), (-1000.5, -2000.25), (7777.75, 1234.5)] {
        for width in [611.0, 612.0, 612.3, 595.7, 1000.1] {
            for margin in [0.0, 3.3, 18.0] {
                let settings = LayoutSettings::new(margin, 0.0).expect("a zero gutter is legal");
                let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + 1000.0);
                for columns in [2u32, 3, 4, 5, 7, 9] {
                    let laid = rows(page, &shape(TemplateName::Grid { rows: 3, columns }), &settings).expect("the page has room");
                    for (row, panels) in laid.iter().enumerate() {
                        for (column, pair) in panels.windows(2).enumerate() {
                            checked += 1;
                            assert_eq!(
                                pair[1].x(),
                                pair[0].right(),
                                "{width}pt in {columns} columns at {origin:?}, row {row}: panel {} must start exactly where panel {column} ends",
                                column + 1
                            );
                        }
                        if row > 0 {
                            assert_eq!(panels[0].y(), laid[row - 1][0].bottom(), "row {row} must start where row {} ends", row - 1);
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 500, "the sweep must run: {checked}");
}

#[test]
fn the_zero_gutter_property_is_not_satisfied_by_a_far_edge_alone() {
    // The reason R4 was a finding: a mutation that places each panel by `content.x0 + index * (nominal
    // + gutter)` leaves the first panel where it was and every far edge on the content box, so only
    // the *internal* edges tell the two apart. This pins the internal edges as a distinct assertion
    // from the far edges, on a page where the index form and the edge-to-edge form genuinely differ.
    let settings = LayoutSettings::new(3.3, 0.0).expect("legal settings");
    let page = Rect::new(0.0, 0.0, 612.3, 1000.0);
    let content = content_box(page, &settings).expect("a content box");
    let laid = rows(page, &shape(TemplateName::Grid { rows: 1, columns: 7 }), &settings).expect("the page has room");
    let nominal = (content.width() - 0.0) / 7.0;
    assert!(content.x0 + 6.0 * nominal != content.x1, "the shape must be one where the index form is not the far edge");
    assert_eq!(laid[0].last().expect("a panel").right(), content.x1);
    for (column, pair) in laid[0].windows(2).enumerate() {
        assert_eq!(pair[1].x(), pair[0].right(), "column {} must start at the previous panel's far edge", column + 1);
    }
}

#[test]
fn the_tolerance_documented_floor_is_still_reachable_from_the_crates_own_output() {
    // The rewritten tolerance doc says the floor is *not* enough on its own and that the tolerance is
    // used for the crate's **own** geometry too — the correction round 2 asked for (R5). Pin the part
    // of that which is measurable: a plan the template produced over a zero gutter is judged as
    // touching, and a page at a magnitude the floor cannot reach is judged by the relative term.
    let settings = LayoutSettings::new(18.0, 0.0).expect("a zero gutter is legal");
    let page = Rect::new(0.0, 0.0, 612.0, 1000.0);
    let planned = plan(PlanPage::Artboard(page), &PlanRequest::new(TemplateName::Grid { rows: 7, columns: 1 }, 7, ReadingDirection::Ltr), &settings)
        .expect("612x1000pt fits seven rows");
    // `min_gutter: 0.0` switches the thin-gutter check off; what is left is overlap, and the
    // template's own zero-gutter page must not be an overlap.
    assert_eq!(vectorcraft_comic_layout::validate::plan(&planned, &Limits { min_gutter: 0.0, ..Limits::default() }), Vec::new());
    // The same plan at a magnitude past the floor's reach is judged by the relative term, and is still
    // not an overlap — the fix R2 was.
    let far = Rect::new(1e10, 1e10, 1e10 + 612.0, 1e10 + 1000.0);
    let planned = plan(PlanPage::Artboard(far), &PlanRequest::new(TemplateName::Grid { rows: 7, columns: 1 }, 7, ReadingDirection::Ltr), &settings)
        .expect("a page far away still has room");
    assert_eq!(
        vectorcraft_comic_layout::validate::plan(&planned, &Limits { min_gutter: 0.0, ..Limits::default() }),
        Vec::new(),
        "the relative term is what keeps a far page's own zero gutter from reading as an overlap"
    );
    assert!(tolerance_at(1e10) > GEOMETRY_TOLERANCE);
}

#[test]
fn a_non_finite_artboard_is_skipped_by_page_and_no_longer_by_faults() {
    // Round 3 recorded that an infinite page gave `faults` an infinite tolerance, so every threshold
    // was satisfied and the geometry check was silently off. Round 4 fixed that at the primitive:
    // `magnitude` skips a coordinate that is not finite, so the tolerance is never non-finite, and the
    // comparison that *is* representable between two finite panels is made again — asserted below, at
    // `faults`, because that is where the switch-off was.
    //
    // **`validate::page` still reports nothing for a non-finite artboard, and that is now a deliberate
    // skip rather than an overflow.** It is the same skip a zero-area or `NaN` artboard gets (checked
    // against those in `round4_reviewer.rs`'s `every_unusable_artboard_gets_one_skip_not_one_silence_per_cause`):
    // the crate has no rectangle to measure a panel against, and a panel is *contained in its page*, so
    // a page that is not a rectangle makes the containment half meaningless and the overlap half an
    // answer about a page whose artboard is already corrupt. What makes the silence honest is that the
    // artboard is named one layer up: `comic-model`'s rule 15 compares the artboard's rect with
    // `ProjectSettings.default_page_size` and reports it
    // (`round4_reviewer.rs::the_model_names_a_non_finite_artboard_when_the_project_has_a_default_size`).
    //
    // So this test pins the *contract* — the skip is deliberate, and `faults` is not the thing that is
    // off — not a prohibition on changing it. If the `page()`-level skip is ever revisited, this is the
    // assertion to change, and it is labelled as such rather than hidden.
    let page_json = r#"{ "schema_version": 1,
      "page": { "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                "panel_ids": [9, 10], "reading_order": [9, 10], "status": "draft" },
      "panels": [ { "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 2,
                    "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                    "reading_order_index": 0,
                    "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                    "generation_status": "none" },
                  { "id": 10, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 3,
                    "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                    "reading_order_index": 1,
                    "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                    "generation_status": "none" } ],
      "dialogue": [], "lettering": [] }"#;
    let file: vectorcraft_comic_model::PageFile = serde_json::from_str(page_json).expect("a test page file");
    let pages: std::collections::BTreeMap<vectorcraft_comic_model::PageId, vectorcraft_comic_model::PageFile> =
        [(vectorcraft_comic_model::PageId(8), file)].into_iter().collect();
    // Frames that share 50pt on both axes: a real overlap, and both panels are over the size floor.
    let frames = vectorcraft_comic_layout::validate::PanelFrames::new()
        .with(vectorcraft_comic_model::NodeId(2), PanelRect::new(0.0, 0.0, 100.0, 100.0).expect("a legal panel"))
        .with(vectorcraft_comic_model::NodeId(3), PanelRect::new(50.0, 50.0, 100.0, 100.0).expect("a legal panel"));
    let limits = Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: 0.0 };
    for (artboard, expected) in [(Rect::new(0.0, 0.0, 612.0, 792.0), 1_usize), (Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY), 0)] {
        let document = vectorcraft_comic_model::DocumentFacts {
            artboards: vec![vectorcraft_comic_model::ArtboardFacts { id: 1, rect: artboard }],
            nodes: [vectorcraft_comic_model::NodeId(2), vectorcraft_comic_model::NodeId(3)].into_iter().collect(),
        };
        let findings = vectorcraft_comic_layout::validate::page(&vectorcraft_comic_layout::validate::PageInput {
            pages: &pages,
            document: &document,
            frames: &frames,
            limits: &limits,
        });
        assert_eq!(findings.len(), expected, "on a {artboard:?} artboard the overlap must be {expected} finding(s): {findings:?}");
    }
    // And the switch-off itself is gone: the same page at `faults` reports the overlap, on an infinite
    // page and on a finite one, so the `page()`-level silence above is a policy about the artboard and
    // not a tolerance that overflowed. `tolerance_at` still has no ceiling — the fix is the scale.
    let overlap = [PanelRect::new(0.0, 0.0, 100.0, 100.0).expect("a legal panel"), PanelRect::new(50.0, 50.0, 100.0, 100.0).expect("a legal panel")];
    let expected = vec![vectorcraft_comic_layout::validate::Fault::Overlap { first: 0, second: 1 }];
    assert_eq!(faults(Rect::new(0.0, 0.0, 612.0, 792.0), None, &overlap, &limits), expected, "a finite page reports the overlap");
    assert_eq!(
        faults(Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY), None, &overlap, &limits),
        expected,
        "and so does an infinite one: the geometry check is not off"
    );
    assert!(
        tolerance_at(f64::INFINITY).is_infinite(),
        "the tolerance still has no ceiling — which is why the scale must not be allowed to be infinite"
    );
}
