//! Round-1 reviewer tests for W2 — `layout`, `panel`, `template`, `reading_order`, `validate`.
//!
//! Written by `code-reviewer` independently of `layout-engineer`'s inline tests, aimed at the
//! boundaries those tests do not reach: the untrusted template name, the floating-point edge of the
//! row/gutter arithmetic, the containment boundary, and the `(rule, subject)` dedupe claim against
//! `comic-model`.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy applies it to
//! `#[cfg(test)]` modules but not to an integration-test crate, so this is the file-level equivalent.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::layout::{PageInput as PlanPage, PagePlan, PlanRequest, plan};
use vectorcraft_comic_layout::reading_order::narrative_order;
use vectorcraft_comic_layout::template::error::TemplateError;
use vectorcraft_comic_layout::template::{DEFAULT_GUTTER, DEFAULT_MARGIN, LayoutSettings, MAX_GRID_SIDE, Shape, TemplateName, content_box, rows};
use vectorcraft_comic_layout::validate::{self, Fault, Limits, PageInput as ValidatePageInput, PanelFrames};
use vectorcraft_comic_model::{ArtboardFacts, DocumentFacts, NodeId, PageId, ReadingDirection, ValidationInput, load, validate as validate_model};
use vectorcraft_geom::{Rect, Size};

fn us_letter() -> Rect {
    Rect::new(0.0, 0.0, 612.0, 792.0)
}

fn shape(name: TemplateName) -> Shape {
    name.shape().expect("a template name has a shape")
}

fn planned(rect: Rect, name: TemplateName, direction: ReadingDirection, settings: LayoutSettings) -> PagePlan {
    let count = shape(name).panel_count();
    plan(PlanPage::Artboard(rect), &PlanRequest::new(name, count, direction), &settings).expect("the page must fit the template")
}

// ---------------------------------------------------------------------------------------------
// Untrusted input: `ProjectSettings.default_template` is a free-form string in a project file
// (schema §5.3), and `TemplateName::from_name` is the only thing standing between it and a
// `Vec` sized by two `u32`s parsed out of that string.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_grid_side_past_the_bound_is_refused_by_name_and_by_shape() {
    // Both halves of the guard: the parser (untrusted string) and `shape` (a public `Grid` variant
    // a caller can build with any two `u32`s). If only the parser were guarded, `shape` would
    // allocate `columns * rows` before refusing.
    assert_eq!(TemplateName::from_name("grid-64x64"), Ok(TemplateName::Grid { rows: 64, columns: 64 }));
    assert_eq!(shape(TemplateName::Grid { rows: MAX_GRID_SIDE, columns: MAX_GRID_SIDE }).panel_count(), 4096);

    for (name, rows_wanted, columns) in [
        ("grid-0x0", 0, 0),
        ("grid-0x2", 0, 2),
        ("grid-2x0", 2, 0),
        ("grid-65x1", 65, 1),
        ("grid-1x65", 1, 65),
        ("grid-1x99999999", 1, 99_999_999),
        ("grid-4294967295x4294967295", u32::MAX, u32::MAX),
    ] {
        assert_eq!(
            TemplateName::from_name(name),
            Err(TemplateError::UnusableGrid { rows: rows_wanted, columns, max: MAX_GRID_SIDE }),
            "`{name}` must be refused, not allocated for"
        );
        assert!(
            TemplateName::Grid { rows: rows_wanted, columns }.shape().is_err(),
            "Grid {{ {rows_wanted}, {columns} }} must be refused by `shape` as well as by the parser"
        );
    }
    // The largest accepted grid is a real shape, so the bound is a bound and not a refusal of
    // everything.
    let biggest = shape(TemplateName::Grid { rows: 64, columns: 64 });
    assert_eq!(biggest.rows(), 64);
    assert_eq!(biggest.panels_per_row().len(), 64);
}

#[test]
fn a_template_name_is_normalized_or_refused_but_never_guessed() {
    // The spellings a project file may legally carry.
    for spelling in ["three-tier", "THREE_TIER", "Three-Tier", "  three-tier\t", "three_tier"] {
        assert_eq!(TemplateName::from_name(spelling), Ok(TemplateName::ThreeTier), "`{spelling}`");
    }
    assert_eq!(TemplateName::from_name("grid_2x3"), Ok(TemplateName::Grid { rows: 2, columns: 3 }));
    assert_eq!(TemplateName::from_name("GRID-2X3"), Ok(TemplateName::Grid { rows: 2, columns: 3 }));

    // Rust's `u32::from_str` accepts a leading `+`, so a `grid-+2x3` name resolves. Harmless (a
    // project file has no reason to spell it) but it is leniency the parser adds on top of the
    // schema's free-form string, and it is recorded here rather than left to be discovered.
    assert_eq!(TemplateName::from_name("grid-+2x3"), Ok(TemplateName::Grid { rows: 2, columns: 3 }));

    // Everything else must be *reported*. A silent fallback would lay out a page the project did
    // not ask for, and the caller would never know.
    let refused = [
        "",
        " ",
        "\t",
        "three",
        "three-tiers",
        "tiers-3",
        "grid",
        "grid-",
        "grid-2",
        "grid-2x",
        "grid-2x3x4",
        "grid-twoxthree",
        "grid- 2x3",  // interior space is not a legal u32
        "grid-2.0x3", // a float is not a u32
        "grid--2x3",  // a negative side is not a u32
        "three-tier\0",
        "three-tier\u{200b}",       // zero-width space
        "thr\u{0435}\u{0435}-tier", // Cyrillic `е` — `to_ascii_lowercase` must not fold it
        "three\u{2010}tier",        // U+2010 hyphen, not ASCII `-`
    ];
    for name in refused {
        assert!(
            matches!(TemplateName::from_name(name), Err(TemplateError::UnknownTemplate { .. })),
            "`{name}` must be reported as unknown: {:?}",
            TemplateName::from_name(name)
        );
    }
    // And the one that *is* accepted, so the list above is not a blanket refusal.
    assert_eq!(TemplateName::from_name("grid-2x3 "), Ok(TemplateName::Grid { rows: 2, columns: 3 }));
}

#[test]
fn the_name_that_is_refused_is_echoed_to_the_caller_unchanged() {
    // The message must quote what the project actually said, or a creator cannot find the line to
    // fix. It is also the one place an untrusted string reaches a user-visible message.
    let written = "  Thr\u{e9}e_Tier  ";
    let error = TemplateName::from_name(written).expect_err("no template answers to that");
    match &error {
        TemplateError::UnknownTemplate { name } => assert_eq!(name, written, "the name as written, before normalization"),
        other => panic!("expected UnknownTemplate, got {other:?}"),
    }
    assert!(error.to_string().starts_with(&format!("`{written}` is not a template")), "{error}");
}

#[test]
fn a_name_that_parses_but_names_a_place_the_page_cannot_hold_is_refused_by_the_plan_not_the_parser() {
    // `grid-64x64` is a legal name; it is the page that cannot hold 4096 panels. The distinction
    // matters: one is "your project says something I do not understand", the other "your page is
    // too small", and a creator acts on them differently.
    assert_eq!(TemplateName::from_name("grid-64x64"), Ok(TemplateName::Grid { rows: 64, columns: 64 }));
    let error = plan(
        PlanPage::Artboard(us_letter()),
        &PlanRequest::new(TemplateName::Grid { rows: 64, columns: 64 }, 4096, ReadingDirection::Ltr),
        &LayoutSettings::default(),
    )
    .expect_err("4096 panels do not fit a US Letter page");
    assert!(matches!(error, vectorcraft_comic_layout::layout::LayoutError::Template(TemplateError::NoRoomDown { .. })), "{error:?}");
}

// ---------------------------------------------------------------------------------------------
// Geometry, checked by hand rather than by restating the implementation.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_content_box_and_every_template_agree_with_the_arithmetic_done_by_hand() {
    let settings = LayoutSettings::default();
    assert_eq!((DEFAULT_MARGIN, DEFAULT_GUTTER), (18.0, 12.0));
    let content = content_box(us_letter(), &settings).expect("US Letter has a content area");
    // US Letter inset by 18pt: 612-36 = 576 wide, 792-36 = 756 tall, starting at (18, 18).
    assert_eq!((content.x0, content.y0, content.x1, content.y1), (18.0, 18.0, 594.0, 774.0));
    assert_eq!((content.width(), content.height()), (576.0, 756.0));

    // three-tier: 3 rows, 1 panel each. (756 - 2*12)/3 = 244, stepping 244+12.
    let three = rows(us_letter(), &shape(TemplateName::ThreeTier), &settings).expect("three tiers fit");
    let cells: Vec<(f64, f64, f64, f64)> = three.iter().flatten().map(|r| (r.x(), r.y(), r.width(), r.height())).collect();
    assert_eq!(cells, vec![(18.0, 18.0, 576.0, 244.0), (18.0, 274.0, 576.0, 244.0), (18.0, 530.0, 576.0, 244.0)]);

    // wide-top: 2 rows of 1 and 2. (756 - 12)/2 = 372; (576 - 12)/2 = 282, stepping 282+12.
    let wide = rows(us_letter(), &shape(TemplateName::WideTop), &settings).expect("a wide top fits");
    let cells: Vec<(f64, f64, f64, f64)> = wide.iter().flatten().map(|r| (r.x(), r.y(), r.width(), r.height())).collect();
    assert_eq!(
        cells,
        vec![(18.0, 18.0, 576.0, 372.0), (18.0, 402.0, 282.0, 372.0), (312.0, 402.0, 282.0, 372.0)],
        "the top panel is full width; the two below are half width each, one gutter apart"
    );
}

#[test]
fn a_single_panel_template_fills_the_content_box_exactly() {
    let settings = LayoutSettings::default();
    let one = rows(us_letter(), &shape(TemplateName::Grid { rows: 1, columns: 1 }), &settings).expect("one panel fits");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].len(), 1);
    let panel = one[0][0];
    let content = content_box(us_letter(), &settings).expect("a content box");
    assert_eq!((panel.x(), panel.y()), (content.x0, content.y0));
    assert_eq!((panel.right(), panel.bottom()), (content.x1, content.y1));
    assert_eq!((panel.width(), panel.height()), (576.0, 756.0));
}

#[test]
fn the_last_row_of_every_page_height_lands_exactly_on_the_content_box() {
    // `rows` documents that "the last row's bottom edge is the content box's, so nothing drifts by
    // rounding", and the implementer's three-tier test asserts it for 756/3 = 252 — a division that
    // happens to be exact. It is not exact in general, and the caller that consumes the plan
    // (`validate::plan`, and the command that applies it) compares the panel against the very
    // boundary the arithmetic misses by.
    let settings = LayoutSettings::default();
    let mut drifted = Vec::new();
    let mut checked = 0usize;
    for tenth in 6000..=20000 {
        let height = f64::from(tenth) / 10.0;
        for row_count in [3u32, 5, 6, 7, 8] {
            let name = TemplateName::Grid { rows: row_count, columns: 1 };
            let page = Rect::new(0.0, 0.0, 612.0, height);
            let content = content_box(page, &settings).expect("a content box");
            let laid = rows(page, &shape(name), &settings).expect("the page has room");
            let last = laid.last().and_then(|row| row.last()).expect("a row of a panel");
            checked += 1;
            if last.bottom() != content.y1 {
                drifted.push((height, row_count, last.bottom() - content.y1));
            }
        }
    }
    assert!(checked >= 1000, "the sweep must actually run: {checked}");
    assert!(
        drifted.is_empty(),
        "{} of {checked} page heights put the last row's bottom edge past (or short of) the content box; first five: {:?}",
        drifted.len(),
        drifted.iter().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn a_correctly_planned_page_is_never_reported_against_itself() {
    // The plan is checked against the margin it was laid out with (validate::plan passes
    // `Some(plan.margin())`), so a plan the template itself produced must report nothing: a warning
    // here tells a creator to "confirm" a bleed that does not exist, and an error here stops a
    // command that was about to write a correct page.
    let settings = LayoutSettings::default();
    let limits = Limits::default();
    let mut offenders: Vec<(f64, u32, Vec<vectorcraft_comic_model::Finding>)> = Vec::new();
    let mut checked = 0usize;
    for tenth in 6000..=20000 {
        let height = f64::from(tenth) / 10.0;
        for row_count in [3u32, 5, 6, 7, 8] {
            let name = TemplateName::Grid { rows: row_count, columns: 1 };
            let plan = planned(Rect::new(0.0, 0.0, 612.0, height), name, ReadingDirection::Ltr, settings);
            checked += 1;
            let findings = validate::plan(&plan, &limits);
            if !findings.is_empty() {
                offenders.push((height, row_count, findings));
            }
        }
    }
    assert!(checked >= 1000, "the sweep must actually run: {checked}");
    assert!(
        offenders.is_empty(),
        "{} of {checked} correctly laid-out pages are reported against themselves; first: {:?}",
        offenders.len(),
        offenders.first().map(|(height, rows, findings)| (*height, *rows, findings.iter().map(ToString::to_string).collect::<Vec<_>>()))
    );
}

#[test]
fn a_full_bleed_plan_is_not_an_error_against_the_page_it_fills() {
    // The same defect with the margin at zero, where the drift is past the *page* rather than past
    // the safe margin — an error, not a warning, on a page that is exactly as the caller asked.
    let settings = LayoutSettings::new(0.0, 12.0).expect("zero is a legal margin");
    let limits = Limits::default();
    let mut offenders = Vec::new();
    for tenth in 6000..=20000 {
        let height = f64::from(tenth) / 10.0;
        for row_count in [3u32, 5, 6, 7, 8] {
            let name = TemplateName::Grid { rows: row_count, columns: 1 };
            let plan = planned(Rect::new(0.0, 0.0, 612.0, height), name, ReadingDirection::Ltr, settings);
            let findings = validate::plan(&plan, &limits);
            if !findings.is_empty() {
                offenders.push((height, row_count, findings.iter().map(ToString::to_string).collect::<Vec<_>>()));
            }
        }
    }
    assert!(offenders.is_empty(), "{} full-bleed pages are reported as broken; first: {:?}", offenders.len(), offenders.first());
}

#[test]
fn every_panel_a_template_places_is_strictly_inside_the_page_and_keeps_the_invariant() {
    // The PanelRect invariant (finite, strictly positive, far edge strictly beyond the origin) must
    // hold for *everything* the templates produce, and every panel must be inside the page — that is
    // the containment half of design §12.1, before any validator sees it.
    let mut checked = 0usize;
    for &(width, height) in &[(612.0, 792.0), (1000.0, 1000.0), (200.0, 200.0), (595.276, 841.89), (1.0, 1.0), (0.5, 100.0)] {
        // A margin of at least half a point, so the page-containment assertion below is about the
        // template and not about the 1e-13 drift `the_last_row_of_every_page_height_lands_exactly_
        // on_the_content_box` reports.
        for settings in [LayoutSettings::default(), LayoutSettings::new(1.0, 0.5).expect("legal"), LayoutSettings::new(0.5, 0.5).expect("legal")] {
            for name in [
                TemplateName::TwoTier,
                TemplateName::ThreeTier,
                TemplateName::FourTier,
                TemplateName::WideTop,
                TemplateName::Grid { rows: 2, columns: 3 },
                TemplateName::Grid { rows: 1, columns: 1 },
            ] {
                let page = Rect::new(0.0, 0.0, width, height);
                let Ok(laid) = rows(page, &shape(name), &settings) else { continue };
                let content = content_box(page, &settings).expect("laid out, so there is a content box");
                for (row_index, row) in laid.iter().enumerate() {
                    for (column, panel) in row.iter().enumerate() {
                        checked += 1;
                        assert!(panel.width().is_finite() && panel.width() > 0.0, "{name:?} {width}x{height} row {row_index} col {column}");
                        assert!(panel.height().is_finite() && panel.height() > 0.0, "{name:?} {width}x{height} row {row_index} col {column}");
                        assert!(panel.right() > panel.x() && panel.bottom() > panel.y(), "{name:?} must advance its far edge");
                        assert!(panel.x() >= page.x0 && panel.y() >= page.y0, "{name:?} must not start before the page");
                        assert!(panel.right() <= page.x1 && panel.bottom() <= page.y1, "{name:?} must not end past the page");
                        // And exactly the content box, not merely somewhere inside the page.
                        assert!(panel.x() >= content.x0 && panel.y() >= content.y0, "{name:?} must be inside the margin");
                        assert!(panel.right() <= content.x1 && panel.bottom() <= content.y1, "{name:?} must be inside the margin");
                    }
                }
            }
        }
    }
    assert!(checked > 50, "the sweep must actually run: {checked}");
}

#[test]
fn a_page_whose_rectangle_is_inverted_or_not_a_number_is_refused_cleanly() {
    let settings = LayoutSettings::default();
    let request = PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr);
    for page in [
        Rect::new(0.0, 0.0, f64::NAN, 792.0),
        Rect::new(f64::NAN, 0.0, 612.0, 792.0),
        Rect::new(0.0, 0.0, 612.0, f64::NAN),
        Rect::new(0.0, 0.0, 612.0, f64::INFINITY),
        Rect::new(0.0, 0.0, f64::NEG_INFINITY, 792.0),
        Rect::new(0.0, 0.0, 0.0, 792.0),
        Rect::new(0.0, 0.0, 612.0, 0.0),
        Rect::new(0.0, 0.0, -612.0, 792.0),
        Rect::new(0.0, 792.0, 612.0, 0.0), // inverted on the y axis
        Rect::new(612.0, 0.0, 0.0, 792.0), // inverted on the x axis
    ] {
        let error = plan(PlanPage::Artboard(page), &request, &settings).expect_err("that page cannot be laid out");
        assert!(matches!(error, vectorcraft_comic_layout::layout::LayoutError::Artboard(_)), "{page:?} gave {error:?}");
    }
    // A `NewPage` carries a `PageSize`, which is not a rectangle, so it cannot be inverted; but a
    // `NaN` size must be refused the same way rather than producing `NaN` rectangles.
    let error = plan(PlanPage::NewPage(Size::new(f64::NAN, 792.0)), &request, &settings).expect_err("a NaN page size is not a page");
    assert!(matches!(error, vectorcraft_comic_layout::layout::LayoutError::Artboard(_)), "{error:?}");
}

#[test]
fn a_page_count_that_is_not_the_templates_is_refused_for_every_template() {
    // Not padded and not truncated: a template that placed a different number of panels than the
    // caller asked for would be laying out a page nobody requested.
    let settings = LayoutSettings::default();
    for name in
        [TemplateName::TwoTier, TemplateName::ThreeTier, TemplateName::FourTier, TemplateName::WideTop, TemplateName::Grid { rows: 2, columns: 3 }]
    {
        let count = shape(name).panel_count();
        for requested in [0, count.saturating_sub(1), count + 1, count + 7, u32::MAX] {
            let error = plan(PlanPage::Artboard(us_letter()), &PlanRequest::new(name, requested, ReadingDirection::Ltr), &settings)
                .expect_err("a count the template does not place must be refused");
            match error {
                vectorcraft_comic_layout::layout::LayoutError::PanelCountMismatch { template, requested: r, template_count } => {
                    assert_eq!(template, name.as_name());
                    assert_eq!(r, requested);
                    assert_eq!(template_count, count);
                }
                other => panic!("{name:?} with {requested} gave {other:?}"),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading order: from the shape and the direction, never from a coordinate.
// ---------------------------------------------------------------------------------------------

#[test]
fn rtl_reverses_each_row_and_never_the_page() {
    // A manga page reads top to bottom, right to left *within a row*. A global mirror would read
    // the last row first, which is wrong for every comic page in existence.
    assert_eq!(narrative_order(&[1, 2], ReadingDirection::Rtl), vec![0, 2, 1], "wide-top");
    assert_eq!(narrative_order(&[1, 1, 1], ReadingDirection::Rtl), vec![0, 1, 2], "three-tier: nothing to reverse");
    assert_eq!(narrative_order(&[3, 2], ReadingDirection::Rtl), vec![2, 1, 0, 4, 3]);
    assert_eq!(narrative_order(&[2, 3], ReadingDirection::Rtl), vec![1, 0, 4, 3, 2]);
    assert_eq!(narrative_order(&[2, 3], ReadingDirection::Ltr), vec![0, 1, 2, 3, 4]);
    // A single row of three: read right to left, so the third placement slot is read first.
    assert_eq!(narrative_order(&[3], ReadingDirection::Rtl), vec![2, 1, 0]);

    // The implementer deleted `narrative_indices` (F4): every order `narrative_order` can build is a
    // product of per-row reversals, so it is an involution and a public inverse could not be told
    // apart from its own identity by any input. The inverse now lives in `panel::place`, where it is
    // observable, and is pinned there with the non-involution order `[1,2,0]` by
    // `a_slots_position_is_looked_up_in_the_order_not_read_off_it`.
}

#[test]
fn a_wide_top_page_gives_the_panels_the_indices_the_order_names() {
    // The whole path: template -> rows -> plan -> `Panel.reading_order_index`. Right to left, the
    // right-hand panel of the bottom row is read second.
    let plan = planned(us_letter(), TemplateName::WideTop, ReadingDirection::Rtl, LayoutSettings::default());
    assert_eq!(plan.reading_order, vec![0, 2, 1]);
    assert_eq!(plan.panels.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![0, 2, 1]);
    // The rectangles are placement order and did not move for the direction.
    assert_eq!(plan.panels[1].rect.x(), 18.0, "the left panel of the bottom row");
    assert_eq!(plan.panels[2].rect.x(), 312.0, "the right panel of the bottom row, read second");
    // And a left-to-right page of the same shape is read in placement order.
    let ltr = planned(us_letter(), TemplateName::WideTop, ReadingDirection::Ltr, LayoutSettings::default());
    assert_eq!(ltr.reading_order, vec![0, 1, 2]);
    assert_eq!(
        ltr.panels.iter().map(|panel| (panel.rect.x(), panel.rect.y())).collect::<Vec<_>>(),
        plan.panels.iter().map(|panel| (panel.rect.x(), panel.rect.y())).collect::<Vec<_>>(),
        "the direction orders the page; it does not move it"
    );
}

#[test]
fn a_two_by_three_grid_reads_right_to_left_in_every_row() {
    let plan = planned(us_letter(), TemplateName::Grid { rows: 2, columns: 3 }, ReadingDirection::Rtl, LayoutSettings::default());
    assert_eq!(plan.panels.len(), 6);
    assert_eq!(plan.reading_order, vec![2, 1, 0, 5, 4, 3]);
    assert_eq!(plan.panels.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![2, 1, 0, 5, 4, 3]);
    // The panel with index 0 is the top-right one, not the top-left one.
    let first = plan.panels.iter().find(|panel| panel.reading_order_index == 0).expect("a first panel");
    assert!(first.rect.x() > 300.0, "the first panel read on an rtl page is the right-most of the top row");
}

// ---------------------------------------------------------------------------------------------
// Validation: the error/warning split, the touch boundary, one defect one finding.
// ---------------------------------------------------------------------------------------------

fn rect(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a test rectangle")
}

#[test]
fn a_panel_flush_with_the_page_edge_is_inside_the_page() {
    // Containment is `>`, not `>=`: a panel the template laid out to the page edge is inside it. With
    // `>=` every full-bleed page would report as broken.
    let page = Rect::new(0.0, 0.0, 100.0, 100.0);
    assert_eq!(validate::faults(page, None, &[rect(0.0, 0.0, 100.0, 100.0)], &Limits::default()), Vec::new());
    assert_eq!(validate::faults(page, None, &[rect(50.0, 0.0, 50.0, 100.0)], &Limits::default()), Vec::new());
    assert_eq!(validate::faults(page, None, &[rect(0.0, 50.0, 100.0, 50.0)], &Limits::default()), Vec::new());
    // A panel flush with the page edge and one panel only is *inside the margin too* when the margin
    // is zero, which is the only margin a plan of a zero-margin page has.
    assert_eq!(validate::faults(page, Some(0.0), &[rect(0.0, 0.0, 100.0, 100.0)], &Limits::default()), Vec::new());
}

#[test]
fn a_panel_a_whisker_past_an_edge_is_outside_on_each_of_the_four_edges() {
    let page = Rect::new(0.0, 0.0, 100.0, 100.0);
    let past = f64::from(f32::EPSILON);
    // Sized above the minimum so the only fault is the containment one, one panel so there is no
    // pair to measure a gutter on.
    for off_page in [rect(-past, 0.0, 200.0, 200.0), rect(0.0, -past, 200.0, 200.0), rect(100.0, 0.0, 200.0, 200.0), rect(0.0, 100.0, 200.0, 200.0)] {
        assert_eq!(validate::faults(page, None, &[off_page], &Limits::default()), vec![Fault::OutsidePage { panel: 0 }], "{off_page:?}");
    }
}

#[test]
fn touching_panels_are_a_zero_gap_and_one_defect_is_one_finding() {
    // Touching is 0pt, not an overlap (PanelRect::overlaps is strict), and the pair is reported once
    // — as a thin-gutter warning, plus the too-small error each panel earns for itself.
    let page = Rect::new(0.0, 0.0, 612.0, 792.0);
    let touching = [rect(0.0, 0.0, 100.0, 100.0), rect(100.0, 0.0, 100.0, 100.0)];
    assert_eq!(
        validate::faults(page, None, &touching, &Limits::default()),
        vec![Fault::GutterTooThin { first: 0, second: 1, gap: 0.0, minimum: 6.0 }]
    );
    // The same pair with the gutter limit switched off reports nothing at all.
    assert_eq!(validate::faults(page, None, &touching, &Limits { min_gutter: 0.0, ..Limits::default() }), Vec::new());

    // An overlapping pair is one finding: the overlap, not the overlap plus a negative gap.
    let overlapping = [rect(0.0, 0.0, 100.0, 100.0), rect(99.0, 0.0, 100.0, 100.0)];
    assert_eq!(validate::faults(page, None, &overlapping, &Limits::default()), vec![Fault::Overlap { first: 0, second: 1 }]);

    // Three panels in a column: the two adjacent pairs, and never the diagonal pair.
    let column = [rect(0.0, 0.0, 100.0, 100.0), rect(0.0, 100.0, 100.0, 100.0), rect(0.0, 200.0, 100.0, 100.0)];
    assert_eq!(
        validate::faults(page, None, &column, &Limits::default()),
        vec![
            Fault::GutterTooThin { first: 0, second: 1, gap: 0.0, minimum: 6.0 },
            Fault::GutterTooThin { first: 1, second: 2, gap: 0.0, minimum: 6.0 },
        ]
    );
}

#[test]
fn the_fault_severities_are_the_ones_design_12_1_needs() {
    // error: something is broken and will not render as intended. warning: it renders, a creator
    // may have meant it.
    let errors = [
        Fault::Overlap { first: 0, second: 1 },
        Fault::TooSmall { panel: 0, width: 1.0, height: 1.0, min_width: 48.0, min_height: 48.0 },
        Fault::OutsidePage { panel: 0 },
    ];
    let warnings = [Fault::GutterTooThin { first: 0, second: 1, gap: 1.0, minimum: 6.0 }, Fault::InSafeMargin { panel: 0 }];
    for fault in errors {
        assert!(fault.describe(&|index: usize| format!("P{index}")).is_error(), "{fault:?}");
    }
    for fault in warnings {
        assert!(!fault.describe(&|index: usize| format!("P{index}")).is_error(), "{fault:?}");
    }
    // The layout checks are design §12.1's, not one of schema §13's eighteen referential rules, so
    // the finding carries no rule number — a caller must not be able to mistake it for rule 6.
    for fault in errors.into_iter().chain(warnings) {
        assert_eq!(fault.describe(&|_index: usize| String::new()).rule, None, "{fault:?}");
    }
}

#[test]
fn the_two_fault_messages_the_implementation_never_pinned_say_what_is_wrong() {
    // Only Overlap and GutterTooThin are pinned by the module's own test; the other three messages
    // could be swapped or emptied and the suite would stay green.
    let label = |index: usize| format!("Panel({})", index + 9);
    let small = Fault::TooSmall { panel: 0, width: 10.0, height: 20.0, min_width: 48.0, min_height: 48.0 }.describe(&label);
    assert_eq!(small.subject, "Panel(9)");
    assert_eq!(small.message, "is 10x20pt, below the 48x48pt minimum panel size");
    let outside = Fault::OutsidePage { panel: 2 }.describe(&label);
    assert_eq!(outside.subject, "Panel(11)");
    assert_eq!(outside.message, "is not inside the page");
    let margin = Fault::InSafeMargin { panel: 1 }.describe(&label);
    assert_eq!(margin.subject, "Panel(10)");
    assert!(margin.message.contains("safe margin"), "{}", margin.message);
}

#[test]
fn a_page_is_checked_against_its_artboard_and_the_labels_are_the_pages_own() {
    // The geometry check over a loaded page uses the caller's frames, labels each panel by its id,
    // and skips what it has no facts for.
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
    let pages: std::collections::BTreeMap<PageId, vectorcraft_comic_model::PageFile> = [(PageId(8), file)].into_iter().collect();
    let document = DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: us_letter() }], nodes: [NodeId(2), NodeId(3)].into_iter().collect() };
    // Panel 9's frame is flush with the page's right edge and 10's is beside it: with the artboard
    // as the page, none of them is off-page, so the only thing wrong is the 0pt gutter between them.
    let frames = PanelFrames::new().with(NodeId(2), rect(0.0, 0.0, 306.0, 792.0)).with(NodeId(3), rect(306.0, 0.0, 306.0, 792.0));
    let findings = validate::page(&ValidatePageInput { pages: &pages, document: &document, frames: &frames, limits: &Limits::default() });
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].subject, "Panel(9)");
    assert!(findings[0].message.starts_with("sits 0pt from Panel(10)"), "{}", findings[0].message);
    assert!(!findings[0].is_error(), "a 0pt gutter renders; it is a warning");

    // A frame the caller did not supply is skipped, not guessed: no second authority for a rectangle.
    let empty = validate::page(&ValidatePageInput { pages: &pages, document: &document, frames: &PanelFrames::new(), limits: &Limits::default() });
    assert_eq!(empty, Vec::new(), "no frames means no geometry to check, not invented geometry");
}

// ---------------------------------------------------------------------------------------------
// The dedupe claim: `validate::page` re-reports rules 6 and 7 with `comic-model`'s subjects.
// ---------------------------------------------------------------------------------------------

fn fixture() -> (PathBuf, vectorcraft_comic_model::ComicBundle) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../comic-model/tests/fixtures/v1/minimal.comiccraft");
    let loaded = load(&root).expect("the M1 fixture bundle must load");
    (root, loaded.bundle)
}

/// The fixture's visual document, as the sibling review verified it: one 612x792pt artboard (id 1)
/// and node ids 2–13.
fn fixture_document() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: us_letter() }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn layout_findings(
    pages: &std::collections::BTreeMap<PageId, vectorcraft_comic_model::PageFile>,
    document: &DocumentFacts,
) -> Vec<vectorcraft_comic_model::Finding> {
    validate::page(&ValidatePageInput { pages, document, frames: &PanelFrames::new(), limits: &Limits::default() })
}

fn model_subjects(root: &Path, bundle: &vectorcraft_comic_model::ComicBundle, document: &DocumentFacts) -> std::collections::BTreeSet<(u8, String)> {
    validate_model(bundle, &ValidationInput { root, document, archive_root: None })
        .into_iter()
        .filter_map(|finding| finding.rule.map(|rule| (rule, finding.subject)))
        .collect()
}

#[test]
fn every_rule_six_and_seven_finding_carries_the_subject_comic_model_uses_too() {
    let (root, bundle) = fixture();
    let document = fixture_document();

    // Case A: the order is a permutation, but a panel's own index disagrees with its position.
    let mut disagreed = bundle.clone();
    {
        let file = disagreed.pages.get_mut(&PageId(8)).expect("the fixture has page 8");
        file.panels[0].reading_order_index = 7;
    }
    let mine = layout_findings(&disagreed.pages, &document);
    let theirs = model_subjects(&root, &disagreed, &document);
    let mine_6_7: Vec<(u8, String)> = mine.iter().filter_map(|f| f.rule.map(|r| (r, f.subject.clone()))).collect();
    assert!(mine_6_7.contains(&(6, "Panel(9).reading_order_index".to_string())), "the index half of rule 6 must be reported: {mine:?}");
    for pair in &mine_6_7 {
        assert!(theirs.contains(pair), "{pair:?} is not reported by comic-model, so a caller cannot dedupe it away");
    }

    // Case B: the order is not a permutation at all.
    let mut broken = bundle.clone();
    {
        let file = broken.pages.get_mut(&PageId(8)).expect("the fixture has page 8");
        file.page.reading_order.pop();
    }
    let mine = layout_findings(&broken.pages, &document);
    let theirs = model_subjects(&root, &broken, &document);
    let mine_6_7: Vec<(u8, String)> = mine.iter().filter_map(|f| f.rule.map(|r| (r, f.subject.clone()))).collect();
    assert!(mine_6_7.contains(&(6, "Page(8).reading_order".to_string())), "the permutation half of rule 6 must be reported: {mine:?}");
    for pair in &mine_6_7 {
        assert!(theirs.contains(pair), "{pair:?} is not reported by comic-model, so a caller cannot dedupe it away");
    }

    // Case C: a panel's frame node is gone.
    let mut dangling = bundle.clone();
    {
        let file = dangling.pages.get_mut(&PageId(8)).expect("the fixture has page 8");
        file.panels[0].frame_node_id = NodeId(404);
    }
    let mine = layout_findings(&dangling.pages, &document);
    let theirs = model_subjects(&root, &dangling, &document);
    let mine_6_7: Vec<(u8, String)> = mine.iter().filter_map(|f| f.rule.map(|r| (r, f.subject.clone()))).collect();
    assert!(mine_6_7.contains(&(7, "Panel(9).frame_node_id".to_string())), "rule 7 must be reported: {mine:?}");
    for pair in &mine_6_7 {
        assert!(theirs.contains(pair), "{pair:?} is not reported by comic-model, so a caller cannot dedupe it away");
    }
}

#[test]
fn the_clean_fixture_page_reports_nothing_here_so_the_check_is_not_noise() {
    // The control for the test above: nothing in the fixture is broken, so this crate must add
    // nothing of its own. A `(rule, subject)` dedupe is only useful if the two crates agree on a
    // *clean* bundle as well.
    let (root, bundle) = fixture();
    let document = fixture_document();
    let mine = layout_findings(&bundle.pages, &document);
    assert_eq!(mine, Vec::new(), "the fixture page is clean: {mine:?}");
    let theirs = model_subjects(&root, &bundle, &document);
    for rule in [6u8, 7] {
        assert!(!theirs.iter().any(|(r, _)| *r == rule), "the control is only meaningful if comic-model is clean too: {theirs:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The contract: a plan, plain data, no document.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_plan_is_plain_data_the_caller_owns() {
    // No handle, no interior mutability, no borrow of a document: the plan can be sent, shared and
    // compared, and applying it is the caller's business (design §8, §11.2).
    fn plain<T: Send + Sync + Clone + std::fmt::Debug + PartialEq + 'static>() {}
    plain::<PagePlan>();
    plain::<vectorcraft_comic_layout::panel::PlannedPanel>();
    plain::<vectorcraft_comic_layout::layout::PageInput>();
    plain::<vectorcraft_comic_layout::layout::PlanRequest>();
    plain::<vectorcraft_comic_layout::validate::Fault>();

    // And it carries the rectangle, not a reference into the document.
    let existing = planned(us_letter(), TemplateName::ThreeTier, ReadingDirection::Ltr, LayoutSettings::default());
    assert_eq!(existing.artboard_size, None, "an existing artboard is the page's size and the plan must not report a second one");
    let new_page = plan(
        PlanPage::NewPage(Size::new(612.0, 792.0)),
        &PlanRequest::new(TemplateName::ThreeTier, 3, ReadingDirection::Ltr),
        &LayoutSettings::default(),
    )
    .expect("a 612x792 page fits three tiers");
    assert_eq!(new_page.artboard_size, Some(Size::new(612.0, 792.0)));
    assert_eq!(new_page.artboard, Rect::new(0.0, 0.0, 612.0, 792.0), "a new page is created at the document origin");
}

#[test]
fn reading_a_page_never_writes_to_it() {
    // `reading_order::check` and `validate::page` take records by `&`, so the compiler is the proof;
    // this pins the observable half — the same records in gives the same findings and the records
    // are still the ones that went in.
    let page_json = r#"{ "schema_version": 1,
      "page": { "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                "panel_ids": [9], "reading_order": [9], "status": "draft" },
      "panels": [ { "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 2,
                    "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                    "reading_order_index": 5,
                    "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                    "generation_status": "none" } ],
      "dialogue": [], "lettering": [] }"#;
    let file: vectorcraft_comic_model::PageFile = serde_json::from_str(page_json).expect("a test page file");
    let before = file.clone();
    let pages: std::collections::BTreeMap<PageId, vectorcraft_comic_model::PageFile> = [(PageId(8), file)].into_iter().collect();
    let document = DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: us_letter() }], nodes: [NodeId(2)].into_iter().collect() };
    let first = validate::page(&ValidatePageInput { pages: &pages, document: &document, frames: &PanelFrames::new(), limits: &Limits::default() });
    let second = validate::page(&ValidatePageInput { pages: &pages, document: &document, frames: &PanelFrames::new(), limits: &Limits::default() });
    assert_eq!(first, second, "the check is read-only and deterministic");
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].message, "is 5 but the panel is at position 0 in `Page.reading_order`");
    assert_eq!(pages[&PageId(8)], before, "the page file is exactly as it went in");
}

#[test]
fn a_page_whose_artboard_has_no_area_is_skipped_rather_than_flagged() {
    // A present-but-degenerate artboard has no rectangle to measure against. Reporting every panel
    // of it as off-page would be a pageful of noise over the one real finding rule 4 owns (the
    // missing artboard) — the same reasoning the "artboard is absent" case already states, and the
    // same skip. Removing that skip from `validate::page` survives the module's own suite.
    let file: vectorcraft_comic_model::PageFile = serde_json::from_str(
        r#"{ "schema_version": 1,
          "page": { "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                    "panel_ids": [9], "reading_order": [9], "status": "draft" },
          "panels": [ { "id": 9, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": 2,
                        "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                        "reading_order_index": 0,
                        "continuity_constraints": { "characters": [], "wardrobe": null, "time_of_day": null, "callbacks": [], "exceptions": [] },
                        "generation_status": "none" } ],
          "dialogue": [], "lettering": [] }"#,
    )
    .expect("a test page file");
    let pages: std::collections::BTreeMap<PageId, vectorcraft_comic_model::PageFile> = [(PageId(8), file)].into_iter().collect();
    let frames = PanelFrames::new().with(NodeId(2), rect(0.0, 0.0, 100.0, 100.0));
    for artboard in [
        Rect::new(0.0, 0.0, 0.0, 792.0),
        Rect::new(0.0, 0.0, 612.0, 0.0),
        Rect::new(0.0, 0.0, -612.0, -792.0),
        Rect::new(0.0, 0.0, f64::NAN, 792.0),
        Rect::new(0.0, 0.0, 612.0, f64::NAN),
    ] {
        let document = DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: artboard }], nodes: [NodeId(2)].into_iter().collect() };
        let findings = validate::page(&ValidatePageInput { pages: &pages, document: &document, frames: &frames, limits: &Limits::default() });
        assert_eq!(findings, Vec::new(), "an artboard of {artboard:?} is not a page rectangle to measure against");
    }
    // The control: the same panel against a real artboard *is* checked, so the skip above is a skip
    // and not the function doing nothing.
    let small =
        DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 50.0, 50.0) }], nodes: [NodeId(2)].into_iter().collect() };
    let findings = validate::page(&ValidatePageInput { pages: &pages, document: &small, frames: &frames, limits: &Limits::default() });
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].subject, "Panel(9)");
    assert_eq!(findings[0].message, "is not inside the page");
}

#[test]
fn a_seven_row_page_is_not_reported_against_the_margin_it_was_laid_out_with() {
    // The concrete case the sweep above finds: a 612x1000pt page laid out with seven rows at the
    // crate's default margin and gutter reports its own last row as crossing the safe margin.
    let page = Rect::new(0.0, 0.0, 612.0, 1000.0);
    let plan = planned(page, TemplateName::Grid { rows: 7, columns: 1 }, ReadingDirection::Ltr, LayoutSettings::default());
    let findings = validate::plan(&plan, &Limits::default());
    assert!(findings.is_empty(), "a correctly laid-out plan must report nothing, but got {findings:?}");

    // And with the margin at zero the same page is reported as an *error*, not a warning.
    let bleed = planned(page, TemplateName::Grid { rows: 7, columns: 1 }, ReadingDirection::Ltr, LayoutSettings::new(0.0, 12.0).expect("legal"));
    let findings = validate::plan(&bleed, &Limits::default());
    assert!(findings.is_empty(), "a correctly laid-out full-bleed plan must report nothing, but got {findings:?}");
}
