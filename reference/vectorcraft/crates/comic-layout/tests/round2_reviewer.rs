//! Round-2 reviewer tests (`code-reviewer`, M1) for `crates/comic-layout`'s panel trees.
//!
//! Round 1 found F1 (the last row's far edge drifted past the content box) and the implementer
//! replaced the index-multiplied placement with **edge-to-edge placement plus a remainder for the
//! last row and the last panel**. Round 2 reviews that arithmetic as new code:
//!
//! * is the remainder exact for every template, every axis, and pages whose content box is not a
//!   round number of points;
//! * does a zero gutter still make neighbours meet *exactly*;
//! * is a plan the template produced ever reported against itself, over a much wider space than the
//!   two sweeps that already claim to cover it.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use vectorcraft_comic_layout::layout::{PageInput as PlanPage, PagePlan, PlanRequest, plan};
use vectorcraft_comic_layout::template::{LayoutSettings, TemplateName, content_box, rows};
use vectorcraft_comic_layout::validate::{Fault, Limits, faults};
use vectorcraft_comic_model::ReadingDirection;
use vectorcraft_geom::Rect;

fn shape(name: TemplateName) -> vectorcraft_comic_layout::template::Shape {
    name.shape().expect("a template name has a shape")
}

fn planned(rect: Rect, name: TemplateName, settings: LayoutSettings) -> PagePlan {
    plan(PlanPage::Artboard(rect), &PlanRequest::new(name, shape(name).panel_count(), ReadingDirection::Ltr), &settings)
        .expect("the page has room for its template")
}

/// The safe-area check the templates' own output must survive: the gutter they were laid out with
/// is the minimum, and no panel-size floor, so a finding can only be a real defect.
fn self_check_limits(gutter: f64) -> Limits {
    Limits { min_panel_width: 0.0, min_panel_height: 0.0, min_gutter: gutter }
}

#[test]
fn the_remainder_is_exact_for_every_template_and_axis_on_a_page_at_the_origin() {
    // The implementer's sweep covers `611x<h>pt` with `grid-<r>x<c>`; this covers the *fixed*
    // templates too, both axes, a fractional page and a fractional content box.
    let mut checked = 0;
    let mut off = Vec::new();
    for (width, height) in [(612.0, 792.0), (612.3, 964.3), (595.7, 841.9), (100.0, 100.0), (50.5, 33.3), (1000.1, 1999.9), (7.0, 7.0)] {
        for margin in [0.0, 0.1, 3.3, 18.0] {
            for gutter in [0.0, 0.1, 3.3, 12.0] {
                let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                let names = [
                    TemplateName::TwoTier,
                    TemplateName::ThreeTier,
                    TemplateName::FourTier,
                    TemplateName::WideTop,
                    TemplateName::Grid { rows: 1, columns: 1 },
                    TemplateName::Grid { rows: 1, columns: 3 },
                    TemplateName::Grid { rows: 3, columns: 1 },
                    TemplateName::Grid { rows: 2, columns: 3 },
                ];
                for name in names {
                    let page = Rect::new(0.0, 0.0, width, height);
                    let Some(content) = content_box(page, &settings).ok() else { continue };
                    let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                    let case = format!("{name:?} {width}x{height} margin {margin} gutter {gutter}");
                    for (index, panels) in laid.iter().enumerate() {
                        let last = panels.last().expect("a row holds a panel");
                        checked += 1;
                        if last.right() != content.x1 {
                            off.push(format!("{case}: row {index} right {:.17} vs content {:.17}", last.right(), content.x1));
                        }
                        if index + 1 == laid.len() && last.bottom() != content.y1 {
                            off.push(format!("{case}: last row bottom {:.17} vs content {:.17}", last.bottom(), content.y1));
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 1000, "the sweep must run: {checked}");
    assert!(off.is_empty(), "{} far edges are not the content box's:\n{}", off.len(), off.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn a_page_the_template_produced_is_never_reported_against_itself_wherever_it_sits() {
    // Round 1's F1 was "a correct plan is reported against itself", measured at the origin. The fix
    // claims the class is gone, so the contract has to hold for a page *away* from the origin too —
    // design §5.1.1 makes a page an artboard "at whatever place the document put it".
    let mut checked = 0;
    let mut offenders = Vec::new();
    for origin in [(0.0, 0.0), (100.0, 50.0), (0.1, 0.3), (-100.0, 100.0), (-1000.5, -2000.25), (500_000.25, 300_000.75)] {
        for (width, height) in [(612.0, 792.0), (611.0, 1000.0), (50.5, 33.3), (595.7, 841.9)] {
            for margin in [0.0, 3.3, 18.0] {
                for gutter in [0.0, 3.3, 12.0] {
                    let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                    let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                    for name in [
                        TemplateName::TwoTier,
                        TemplateName::ThreeTier,
                        TemplateName::WideTop,
                        TemplateName::Grid { rows: 1, columns: 3 },
                        TemplateName::Grid { rows: 2, columns: 3 },
                        TemplateName::Grid { rows: 7, columns: 1 },
                    ] {
                        let Ok(plan) =
                            plan(PlanPage::Artboard(page), &PlanRequest::new(name, shape(name).panel_count(), ReadingDirection::Ltr), &settings)
                        else {
                            continue;
                        };
                        checked += 1;
                        let findings = vectorcraft_comic_layout::validate::plan(&plan, &self_check_limits(gutter));
                        if !findings.is_empty() {
                            offenders.push(format!("{name:?} at {origin:?} {width}x{height} margin {margin} gutter {gutter}: {findings:?}"));
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 1000, "the sweep must run: {checked}");
    assert!(
        offenders.is_empty(),
        "{} correctly planned pages are reported against themselves:\n{}",
        offenders.len(),
        offenders.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn a_zero_gutter_makes_neighbours_meet_exactly_on_both_axes_wherever_the_page_sits() {
    // "A zero gutter must make rows meet exactly" — on both axes, for every template, and not only at
    // the origin. `left += width + gutter` is `left + width` at a zero gutter, which is exactly
    // `right()`, so the meeting is by construction rather than by luck of the division.
    let mut checked = 0;
    let mut off = Vec::new();
    for origin in [(0.0, 0.0), (0.1, 0.3), (-1000.5, -2000.25), (7777.75, 1234.5)] {
        for (width, height) in [(612.0, 792.0), (612.3, 964.3), (50.5, 33.3)] {
            for margin in [0.0, 3.3, 18.0] {
                let settings = LayoutSettings::new(margin, 0.0).expect("a zero gutter is legal");
                let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                for name in [TemplateName::WideTop, TemplateName::Grid { rows: 1, columns: 3 }, TemplateName::Grid { rows: 3, columns: 3 }] {
                    let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                    for (index, panels) in laid.iter().enumerate() {
                        for pair in panels.windows(2) {
                            checked += 1;
                            if pair[1].x() != pair[0].right() {
                                off.push(format!(
                                    "{name:?} at {origin:?} {width}x{height} margin {margin}: row {index} column gap {:.17}",
                                    pair[1].x() - pair[0].right()
                                ));
                            }
                        }
                        if index > 0 {
                            checked += 1;
                            let above = laid[index - 1][0].bottom();
                            if panels[0].y() != above {
                                off.push(format!(
                                    "{name:?} at {origin:?} {width}x{height} margin {margin}: row {index} gap {:.17}",
                                    panels[0].y() - above
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 100, "the sweep must run: {checked}");
    assert!(
        off.is_empty(),
        "{} neighbours do not meet exactly at a zero gutter:\n{}",
        off.len(),
        off.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn no_template_overlaps_itself_or_leaves_the_content_box_on_any_axis() {
    // The semantic half of the fix: a template whose panels now *overlap*, or a panel outside the
    // content box, would be worse than the drift it replaced. Overlap is `PanelRect::overlaps`, the
    // strict predicate, so a shared sliver of any size counts.
    let mut checked = 0;
    let mut bad = Vec::new();
    for (width, height) in [(612.0, 792.0), (612.3, 964.3), (100.0, 100.0), (1000.1, 1999.9), (50.5, 33.3)] {
        for margin in [0.0, 3.3, 18.0] {
            for gutter in [0.0, 0.1, 3.3, 12.0] {
                let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                let page = Rect::new(0.0, 0.0, width, height);
                let Some(content) = content_box(page, &settings).ok() else { continue };
                for name in [
                    TemplateName::TwoTier,
                    TemplateName::ThreeTier,
                    TemplateName::FourTier,
                    TemplateName::WideTop,
                    TemplateName::Grid { rows: 1, columns: 1 },
                    TemplateName::Grid { rows: 1, columns: 3 },
                    TemplateName::Grid { rows: 3, columns: 1 },
                    TemplateName::Grid { rows: 2, columns: 3 },
                    TemplateName::Grid { rows: 9, columns: 3 },
                ] {
                    let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                    let panels: Vec<_> = laid.iter().flatten().copied().collect();
                    let case = format!("{name:?} {width}x{height} margin {margin} gutter {gutter}");
                    for (index, panel) in panels.iter().enumerate() {
                        checked += 1;
                        if panel.x() < content.x0 || panel.y() < content.y0 || panel.right() > content.x1 || panel.bottom() > content.y1 {
                            bad.push(format!("{case}: panel {index} escapes the content box"));
                        }
                    }
                    for first in 0..panels.len() {
                        for second in (first + 1)..panels.len() {
                            if panels[first].overlaps(&panels[second]) {
                                bad.push(format!("{case}: panels {first} and {second} overlap"));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 1000, "the sweep must run: {checked}");
    assert!(bad.is_empty(), "{} placements are wrong:\n{}", bad.len(), bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn the_templates_the_round_one_review_did_not_name_by_hand_are_right() {
    // Round 1 checked three-tier, wide-top, grid 2x3 and a single panel by hand. The remainder was
    // written for the equal-row case; these are the shapes that exercise unequal rows and the two
    // single-panel edges, by hand and independently of the implementer's tests.
    let settings = LayoutSettings::default();
    let letter = Rect::new(0.0, 0.0, 612.0, 792.0);
    let content = content_box(letter, &settings).expect("a content box");
    assert_eq!((content.x0, content.y0, content.x1, content.y1), (18.0, 18.0, 594.0, 774.0));

    // A single row: the row's height is the whole content height, its panels divided across.
    let one_row = rows(letter, &shape(TemplateName::Grid { rows: 1, columns: 3 }), &settings).expect("a single row");
    assert_eq!(one_row.len(), 1);
    assert_eq!(one_row[0].len(), 3);
    assert_eq!((one_row[0][0].x(), one_row[0][0].y(), one_row[0][0].width(), one_row[0][0].height()), (18.0, 18.0, 184.0, 756.0));
    assert_eq!((one_row[0][2].x(), one_row[0][2].width()), (18.0 + 2.0 * 196.0, 184.0));
    assert_eq!(one_row[0][2].right(), content.x1);
    assert_eq!(one_row[0][2].bottom(), content.y1);

    // A single column: every row is the full content width.
    let one_column = rows(letter, &shape(TemplateName::Grid { rows: 3, columns: 1 }), &settings).expect("a single column");
    assert_eq!(one_column.iter().map(|row| row.len()).collect::<Vec<_>>(), vec![1, 1, 1]);
    assert_eq!(one_column[0][0].width(), 576.0);
    assert_eq!(one_column[2][0].bottom(), content.y1);

    // `wide-top` — unequal rows: a full-width row of 372 over two half-width rows of 372.
    let wide = rows(letter, &shape(TemplateName::WideTop), &settings).expect("a wide top");
    assert_eq!((wide[0][0].height(), wide[1][0].height()), (372.0, 372.0));
    assert_eq!((wide[0][0].width(), wide[1][0].width(), wide[1][1].width()), (576.0, 282.0, 282.0));
    assert_eq!(wide[1][1].right(), content.x1);
    assert_eq!(wide[1][0].bottom(), content.y1);

    // `grid-2x3` — the same cells as two `wide-top` rows three wide.
    let grid = rows(letter, &shape(TemplateName::Grid { rows: 2, columns: 3 }), &settings).expect("a 2x3 grid");
    assert_eq!(grid.iter().map(Vec::len).collect::<Vec<_>>(), vec![3, 3]);
    assert_eq!((grid[0][0].width(), grid[1][0].height()), (184.0, 372.0));
    assert_eq!((grid[0][2].right(), grid[1][2].right()), (content.x1, content.x1));
    assert_eq!(grid[1][1].bottom(), content.y1);
}

#[test]
fn a_page_whose_content_box_is_not_a_whole_number_of_points_keeps_the_far_edges_inside_it() {
    // The remainder is `content.y1 - top`, and `top` for the last row is a sum, so the guarantee the
    // module doc states — "the far edges are the content box's to within one ULP of the page's
    // coordinate magnitude" — has to be checked on a page whose coordinates are not round numbers.
    // The contract that matters to a caller is that no far edge lands *outside* the content box;
    // `r1_the_remainder_lands_on_the_content_box_wherever_the_page_is` below holds the two-sided
    // version, and this one is deliberately one-sided so it cannot agree with it by construction.
    let mut checked = 0;
    let mut escapes = Vec::new();
    for (origin, width, height) in [
        ((0.1, 0.3), 50.5, 33.3),
        ((0.1, 0.3), 612.3, 964.3),
        ((0.7, 0.7), 595.7, 841.9),
        ((0.0, 0.0), 611.0, 1000.0),
        ((1.0 / 3.0, 1.0 / 7.0), 612.0, 792.0),
    ] {
        for margin in [0.0, 0.1, 3.3, 18.0] {
            for gutter in [0.0, 0.1, 3.3, 12.0] {
                let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                let Some(content) = content_box(page, &settings).ok() else { continue };
                for name in [TemplateName::WideTop, TemplateName::Grid { rows: 7, columns: 1 }, TemplateName::Grid { rows: 2, columns: 3 }] {
                    let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                    for (index, panels) in laid.iter().enumerate() {
                        let last = panels.last().expect("a row holds a panel");
                        checked += 1;
                        if last.right() > content.x1 || last.bottom() > content.y1 {
                            escapes.push(format!(
                                "{name:?} {origin:?} {width}x{height} margin {margin} gutter {gutter}: row {index} right {:.17} bottom {:.17} vs content {:.17}/{:.17}",
                                last.right(),
                                last.bottom(),
                                content.x1,
                                content.y1
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 100, "the sweep must run: {checked}");
    assert!(
        escapes.is_empty(),
        "{} far edges escape the content box:\n{}",
        escapes.len(),
        escapes.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn the_tolerance_is_reachable_from_geometry_the_crate_did_not_compute() {
    // `GEOMETRY_TOLERANCE`'s doc claims its remaining purpose is geometry read out of a document.
    // `faults` is public and takes rectangles, so a frame the creator sized to exactly the minimum,
    // read back one ULP low, must not be reported: the tolerance is what the doc says it is.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let minimum = 100.0_f64;
    let one_ulp_low = f64::from_bits(minimum.to_bits() - 1);
    assert!(one_ulp_low < minimum, "the test needs a value really one ULP under the minimum");
    let equal = vectorcraft_comic_layout::PanelRect::new(0.0, 0.0, minimum, minimum).expect("a legal panel");
    assert_eq!(faults(page, None, &[equal], &Limits { min_panel_width: minimum, min_panel_height: minimum, ..Limits::default() }), Vec::new());
    let short = vectorcraft_comic_layout::PanelRect::new(0.0, 0.0, one_ulp_low, minimum).expect("a legal panel");
    assert_eq!(faults(page, None, &[short], &Limits { min_panel_width: minimum, min_panel_height: minimum, ..Limits::default() }), Vec::new());
    // A hundredth of a point under is a real defect, and the tolerance is far below it.
    let thin = vectorcraft_comic_layout::PanelRect::new(0.0, 0.0, minimum - 0.01, minimum).expect("a legal panel");
    assert_eq!(faults(page, None, &[thin], &Limits { min_panel_width: minimum, min_panel_height: minimum, ..Limits::default() }).len(), 1);
}

#[test]
fn a_panel_exactly_on_the_safe_margin_is_inside_it() {
    // The margin branch is live for a hand-built plan (F5). A panel whose far edge is *exactly* the
    // margin boundary is inside, and one a whisker past it is not — the boundary the fix moved.
    let settings = LayoutSettings::default();
    let mut hand_built = planned(Rect::new(0.0, 0.0, 612.0, 792.0), TemplateName::ThreeTier, settings);
    let content = content_box(hand_built.artboard, &settings).expect("a content box");
    hand_built.panels[0].rect = vectorcraft_comic_layout::PanelRect::new(content.x0, content.y0, 100.0, 100.0).expect("a legal panel");
    assert_eq!(vectorcraft_comic_layout::validate::plan(&hand_built, &Limits::default()), Vec::new(), "a panel on the boundary is inside the margin");
    hand_built.panels[0].rect = vectorcraft_comic_layout::PanelRect::new(content.x0 - 0.5, content.y0, 100.0, 100.0).expect("a legal panel");
    let findings = vectorcraft_comic_layout::validate::plan(&hand_built, &Limits::default());
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(matches!(findings[0].message.as_str(), message if message.contains("safe margin")));
    assert!(!findings[0].is_error());
}

#[test]
fn the_gate_check_limits_do_not_hide_a_real_overlap() {
    // The tolerance is 1e-9pt; a shared area past it is a real overlap and must be reported, so the
    // self-check sweeps above are not passing because the validator went blind.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let a = vectorcraft_comic_layout::PanelRect::new(0.0, 0.0, 100.0, 100.0).expect("a legal panel");
    let b = vectorcraft_comic_layout::PanelRect::new(50.0, 50.0, 100.0, 100.0).expect("a legal panel");
    assert_eq!(faults(page, None, &[a, b], &self_check_limits(0.0)), vec![Fault::Overlap { first: 0, second: 1 }]);
}

// ---------------------------------------------------------------------------------------------
// Findings. Each of these asserts the behaviour the crate documents; each fails on this revision.
// ---------------------------------------------------------------------------------------------

#[test]
fn r1_the_remainder_lands_on_the_content_box_wherever_the_page_is() {
    // R1 (bug). `template::rows` documents that "the far edges **are** the content box's ... by
    // construction", with an error "at most half an ULP of content.y1". It is not so: the remainder
    // is `content.y1 - top`, and `last_top + (content.y1 - last_top)` is **not** `content.y1` in
    // f64 for every legal page. The drifted pages are the ones away from the document origin, whose
    // content box is not a round number of points — none of which either sweep probes.
    let mut checked = 0;
    let mut past = Vec::new();
    for (origin, width, height) in [
        ((0.1, 0.3), 50.5, 33.3),
        ((0.1, 0.3), 612.0, 792.0),
        ((-1000.5, -2000.25), 612.0, 792.0),
        ((-100.5, 250.25), 611.0, 1000.0),
        ((1000.5, 2000.25), 612.3, 964.3),
        ((0.0, 0.0), 612.0, 792.0),
    ] {
        for margin in [0.0, 3.3, 18.0] {
            for gutter in [0.0, 3.3, 12.0] {
                let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                let Some(content) = content_box(page, &settings).ok() else { continue };
                for name in [
                    TemplateName::TwoTier,
                    TemplateName::WideTop,
                    TemplateName::Grid { rows: 1, columns: 3 },
                    TemplateName::Grid { rows: 7, columns: 1 },
                ] {
                    let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                    for (index, panels) in laid.iter().enumerate() {
                        let last = panels.last().expect("a row holds a panel");
                        checked += 1;
                        // R1's assertion, retracted by the lead and re-stated at the strength f64
                        // delivers: exact when the page is anchored at the document origin, and
                        // within one ULP of the coordinate magnitude elsewhere. Exactness is not
                        // achievable in general — `a + (b - a) == b` is not an f64 identity, and a
                        // `PanelRect` is a position and a size, so no rearrangement avoids it.
                        //
                        // The ULP is the crate's own convention, `scale * EPSILON` (see
                        // `template`'s `ulps` and `validate::tolerance_at`), and the scale is the
                        // **content box's** largest coordinate rather than the page's. That is
                        // deliberately the stricter reading of the module doc: the content box is
                        // inside the page, so its magnitude is the smaller of the two, and the bound
                        // still holds (round 3 measured the worst far edge at 0.25 ULP of the page's
                        // magnitude, with margins up to 49.9% of the page). Not a weakening: the old
                        // assertion demanded what f64 cannot deliver, and the implementation's doc
                        // has been corrected to match.
                        let at_origin = origin.0 == 0.0 && origin.1 == 0.0;
                        let magnitude = content.x1.abs().max(content.y1.abs()).max(content.x0.abs()).max(content.y0.abs());
                        let within_ulp = |edge: f64, want: f64| (edge - want).abs() <= magnitude * f64::EPSILON;
                        if if at_origin { last.right() != content.x1 } else { !within_ulp(last.right(), content.x1) } {
                            past.push(format!(
                                "{name:?} page {page:?} margin {margin} gutter {gutter}: row {index} right {:.17e} is content.x1 {:.17e} + {:.3e}",
                                last.right(),
                                content.x1,
                                last.right() - content.x1
                            ));
                        }
                        if index + 1 == laid.len() && (if at_origin { last.bottom() != content.y1 } else { !within_ulp(last.bottom(), content.y1) }) {
                            past.push(format!(
                                "{name:?} page {page:?} margin {margin} gutter {gutter}: last row bottom {:.17e} is content.y1 {:.17e} + {:.3e}",
                                last.bottom(),
                                content.y1,
                                last.bottom() - content.y1
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(checked >= 500, "the sweep must run: {checked}");
    assert!(
        past.is_empty(),
        "{} far edges do not land on the content box:\n{}",
        past.len(),
        past.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn r2_a_page_far_from_the_origin_is_not_reported_against_itself() {
    // R2 (bug). Round 1's F1 class — "a plan the template produced is reported against itself" —
    // survives at a coordinate magnitude the absolute 1e-9 tolerance cannot cover. `plan()` is a
    // document-point coordinate and `faults` judges a gap against the creator's own minimum, so once
    // `ulp(coordinate) > GEOMETRY_TOLERANCE` (~4.5e6pt) the template's own gutter measures short of
    // the gutter it was asked for.
    let mut reported = Vec::new();
    for magnitude in [1.0e6_f64, 1.0e7, 1.0e8] {
        for sign in [1.0_f64, -1.0] {
            let origin = (sign * magnitude, sign * magnitude);
            for (width, height) in [(612.0, 792.0), (611.0, 1000.0)] {
                for margin in [0.0, 3.3, 18.0] {
                    for gutter in [3.3, 12.0] {
                        let settings = LayoutSettings::new(margin, gutter).expect("legal settings");
                        let page = Rect::new(origin.0, origin.1, origin.0 + width, origin.1 + height);
                        for name in [
                            TemplateName::ThreeTier,
                            TemplateName::WideTop,
                            TemplateName::Grid { rows: 7, columns: 1 },
                            TemplateName::Grid { rows: 2, columns: 3 },
                        ] {
                            let Ok(plan) =
                                plan(PlanPage::Artboard(page), &PlanRequest::new(name, shape(name).panel_count(), ReadingDirection::Ltr), &settings)
                            else {
                                continue;
                            };
                            let findings = vectorcraft_comic_layout::validate::plan(&plan, &self_check_limits(gutter));
                            if !findings.is_empty() {
                                reported.push(format!(
                                    "{name:?} at {origin:?} {width}x{height} margin {margin} gutter {gutter}: {} — {}",
                                    findings.len(),
                                    findings[0].message
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        reported.is_empty(),
        "{} correctly planned pages far from the origin are reported against themselves:\n{}",
        reported.len(),
        reported.iter().take(6).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn r3_the_room_guard_covers_the_quantity_the_last_row_actually_uses() {
    // R3 (preference). The guard is `nominal_row_height <= 0.0` — `NoRoomDown` — but the last row is
    // built with a **different** quantity, the remainder `content.y1 - top`, which the guard never
    // sees. The fix introduced that quantity, so the page the guard lets through now refuses with
    // `TemplateError::Rect`, a rectangle error that reads as a caller bug rather than a page with no
    // room. The width axis has the same shape.
    for (page, rows_wanted) in [(Rect::new(0.0, 0.0, 100.0, 10.05), 64u32), (Rect::new(0.0, 0.0, 100.0, 10.06), 64)] {
        let gutter = (10.05_f64 / 63.0) * (1.0 - 1e-14);
        let settings = LayoutSettings::new(0.0, gutter).expect("a legal gutter");
        let result = rows(page, &shape(TemplateName::Grid { rows: rows_wanted, columns: 1 }), &settings);
        assert!(
            matches!(result, Err(vectorcraft_comic_layout::template::error::TemplateError::NoRoomDown { .. })) || result.is_ok(),
            "a page the room guard let through must lay out or be refused as NoRoomDown, not as {result:?}"
        );
    }
}
