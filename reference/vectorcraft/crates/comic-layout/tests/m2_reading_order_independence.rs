//! M2 W5, criterion 4 — **"reading order is explicit, editable and validated independently of panel
//! coordinates"** — as the tests that fail when a coordinate leaks in, and the converse.
//!
//! M1 derived the order from the template's **row structure** and the project direction (`rtl`
//! reverses each row, never the page) and validated it by comparing ids and indices. So the feature
//! exists. What M1 did *not* have is evidence for the half of the sentence that is a **claim about
//! the absent input**: a test that only checks the resulting order passes happily while the
//! derivation secretly consults geometry, and every order this crate's own templates produce agrees
//! with its own geometry — so on template output the two implementations are indistinguishable.
//!
//! This file makes them distinguishable, in both directions:
//!
//! * **Same rectangles, different groupings, different orders**
//!   (`the_same_rectangles_grouped_into_different_rows…`). The narrative structure is the *rows*; the
//!   rectangles are only placed by them. Four rectangles in identical positions, grouped three ways,
//!   must read in three different orders — so the index cannot be a function of the coordinates. A
//!   `place` that sorted by `(y, x)` answers the same thing three times and is rejected here; that
//!   mutant is built and measured in `docs/reviews/M2-layout-engineer-hard-cases.md`.
//! * **A moved page keeps its order and still validates** (`a_page_whose_panels_were_moved…`). The
//!   frames are assigned so that the page's *geometric* order — computed by this file, independently
//!   of the crate — is the reverse of the order it declares, and the page must still report nothing.
//!   A validator that required the order to agree with the geometry is rejected here; that is the
//!   other half of the mutant table, and it is the plausible-wrong shape a reader would write
//!   ("the reading order has to match what the page looks like").
//! * **An order no template produces is legal, and an invalid one is reported while the geometry reads
//!   perfectly** — the two converse cases, which stop the pair above from being satisfied by a
//!   validator that has simply stopped checking.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::panel::{PlannedPanel, place};
use vectorcraft_comic_layout::reading_order;
use vectorcraft_comic_layout::validate;
use vectorcraft_comic_model::{ArtboardFacts, DocumentFacts, NodeId, PageFile, ReadingDirection, SCHEMA_VERSION};
use vectorcraft_geom::Rect;

fn letter() -> Rect {
    Rect::new(0.0, 0.0, 612.0, 792.0)
}

fn cell(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a legal panel")
}

/// The four cells of a 2x2 template on US Letter, in placement order: top-left, top-right,
/// bottom-left, bottom-right.
fn cells() -> [PanelRect; 4] {
    [cell(18.0, 18.0, 282.0, 372.0), cell(312.0, 18.0, 282.0, 372.0), cell(18.0, 402.0, 282.0, 372.0), cell(312.0, 402.0, 282.0, 372.0)]
}

/// The order a page's **geometry** reads in, computed here rather than by the crate: top to bottom,
/// left to right, on the frame rectangles a caller read out of the document.
///
/// This is the wrong answer to criterion 4, written down so the tests can state their own premise —
/// a test that claims the geometry disagrees with the order has to show it.
fn order_by_geometry(frames: &[(u64, PanelRect)]) -> Vec<u64> {
    let mut sorted = frames.to_vec();
    sorted.sort_by(|a, b| {
        let key = |panel: &(u64, PanelRect)| (panel.1.y(), panel.1.x());
        key(a).partial_cmp(&key(b)).expect("the test's coordinates are finite")
    });
    sorted.into_iter().map(|(id, _)| id).collect()
}

fn indices(placed: &[PlannedPanel]) -> Vec<u16> {
    placed.iter().map(|panel| panel.reading_order_index).collect()
}

/// A page file with the given panels — `(panel id, frame node id, reading_order_index)` — and the
/// given reading order, from JSON so a page's record shape stays `comic-model`'s business.
fn page_file(panels: &[(u64, u64, u16)], order: &[u64]) -> PageFile {
    let records: Vec<String> = panels
        .iter()
        .map(|(id, frame, index)| {
            format!(
                r#"{{ "id": {id}, "page_id": 8, "artboard_id": 1, "scene_id": 4, "frame_node_id": {frame},
                      "artwork_refs": [], "shot_description": "", "dialogue_ids": [], "lettering_ids": [],
                      "reading_order_index": {index},
                      "continuity_constraints": {{ "characters": [], "wardrobe": null, "time_of_day": null,
                                                   "callbacks": [], "exceptions": [] }},
                      "generation_status": "none" }}"#
            )
        })
        .collect();
    let numbers = |values: &[u64]| values.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
    let ids: Vec<u64> = panels.iter().map(|(id, _, _)| *id).collect();
    serde_json::from_str(&format!(
        r#"{{ "schema_version": {SCHEMA_VERSION},
             "page": {{ "id": 8, "chapter_id": 3, "scene_ids": [4], "artboard_id": 1, "page_number": 1,
                        "panel_ids": [{}], "reading_order": [{}], "status": "draft" }},
             "panels": [{}], "dialogue": [], "lettering": [] }}"#,
        numbers(&ids),
        numbers(order),
        records.join(", ")
    ))
    .expect("a test page file")
}

/// Validate one page whose frame objects hold `frames` — `(panel id, rectangle)` — one node per panel
/// starting at `100`.
fn validate_page(file: PageFile, frames: &[(u64, PanelRect)]) -> Vec<vectorcraft_comic_model::Finding> {
    let mut supplied = validate::PanelFrames::new();
    for (slot, (_, rect)) in frames.iter().enumerate() {
        assert!(supplied.insert(NodeId(u64::try_from(slot).expect("four panels") + 100), *rect).is_none());
    }
    let document = DocumentFacts {
        artboards: vec![ArtboardFacts { id: 1, rect: letter() }],
        nodes: (0..frames.len()).map(|slot| NodeId(u64::try_from(slot).expect("four panels") + 100)).collect(),
    };
    let pages = BTreeMap::from([(file.page.id, file)]);
    validate::page(&validate::PageInput { pages: &pages, document: &document, frames: &supplied, limits: &validate::Limits::default() })
}

/// The index a reading order gives each placement slot — the order written in the units
/// `reading_order::narrative_order` speaks, so a hand-made order can be compared with a template's.
fn in_slots(order: &[u64], slots: &[u64]) -> Vec<u16> {
    order.iter().map(|id| u16::try_from(slots.iter().position(|slot| slot == id).expect("a known panel")).expect("four slots")).collect()
}

/// The order a **coordinate-derived** implementation would produce for a set of rows: sort the
/// rectangles top to bottom and left to right, call each run of equal `y` a row, and reverse each run
/// for `rtl`. Returns each placement slot's position in that order.
///
/// This is the plausible-but-wrong reading, written out in the test so the test can show what it
/// rejects. It agrees with the crate on the output of every template in this crate — which is why a
/// test that only ran a template would prove nothing — and disagrees as soon as the row grouping and
/// the geometry are not the same thing.
fn wrong_coordinate_order(rows: &[Vec<PanelRect>], direction: ReadingDirection) -> Vec<u16> {
    let flat: Vec<PanelRect> = rows.iter().flatten().copied().collect();
    let mut by_geometry: Vec<usize> = (0..flat.len()).collect();
    by_geometry.sort_by(|&a, &b| (flat[a].y(), flat[a].x()).partial_cmp(&(flat[b].y(), flat[b].x())).expect("the test's coordinates are finite"));
    let mut order: Vec<usize> = Vec::with_capacity(flat.len());
    for run in by_geometry.chunk_by(|a, b| flat[*a].y() == flat[*b].y()) {
        match direction {
            ReadingDirection::Ltr => order.extend(run),
            ReadingDirection::Rtl => order.extend(run.iter().rev()),
        }
    }
    let mut positions = vec![0_u16; flat.len()];
    for (position, slot) in order.into_iter().enumerate() {
        positions[slot] = u16::try_from(position).expect("four slots");
    }
    positions
}

#[test]
fn the_same_rectangles_grouped_into_different_rows_get_different_orders() {
    // The narrative structure **is** the row grouping: the rectangles are placed by the rows, and the
    // order is read off the rows. So the same four rectangles, in the same four places, grouped three
    // ways, must read in three different orders — as one row of four (reversed wholesale), as two rows
    // of two (each row reversed on its own), and as four rows of one (nothing to reverse) — because
    // `rtl` reverses each row and never the page.
    //
    // The four cells are one band across the page, so the grouping and the geometry are *not* the same
    // thing; that is what lets this input tell the two readings apart, and it is the only kind of input
    // that can. On a template's own output the coordinate reading and the row reading agree exactly
    // (asserted at the end), so a test that only ran a template would pass either way.
    let band = [cell(18.0, 18.0, 135.0, 372.0), cell(165.0, 18.0, 135.0, 372.0), cell(312.0, 18.0, 135.0, 372.0), cell(459.0, 18.0, 135.0, 372.0)];
    let one_row = vec![band.to_vec()];
    let two_rows = vec![band[..2].to_vec(), band[2..].to_vec()];
    let four_rows = band.iter().map(|rect| vec![*rect]).collect::<Vec<_>>();
    for grouping in [&one_row, &two_rows, &four_rows] {
        assert_eq!(
            grouping.iter().flatten().copied().collect::<Vec<_>>(),
            band.to_vec(),
            "the premise: every call gets the same four rectangles, in the same four places"
        );
    }

    assert_eq!(indices(&place(&one_row, ReadingDirection::Rtl)), vec![3, 2, 1, 0], "one row of four reverses wholesale");
    assert_eq!(indices(&place(&two_rows, ReadingDirection::Rtl)), vec![1, 0, 3, 2], "two rows of two reverse one row each");
    assert_eq!(indices(&place(&four_rows, ReadingDirection::Rtl)), vec![0, 1, 2, 3], "four rows of one have nothing to reverse");

    // And what the coordinate reading says about the very same rectangles: one geometric row of four,
    // so `[3, 2, 1, 0]` for all three groupings. It gets the first right and cannot tell the other two
    // from it — which is the leak this test exists to catch.
    assert_eq!(wrong_coordinate_order(&one_row, ReadingDirection::Rtl), vec![3, 2, 1, 0]);
    assert_eq!(wrong_coordinate_order(&two_rows, ReadingDirection::Rtl), vec![3, 2, 1, 0], "the wrong reading cannot see the grouping");
    assert_eq!(wrong_coordinate_order(&four_rows, ReadingDirection::Rtl), vec![3, 2, 1, 0]);
    assert_ne!(indices(&place(&two_rows, ReadingDirection::Rtl)), wrong_coordinate_order(&two_rows, ReadingDirection::Rtl));
    assert_ne!(indices(&place(&four_rows, ReadingDirection::Rtl)), wrong_coordinate_order(&four_rows, ReadingDirection::Rtl));

    // The direction moves the order and no rectangle, and so does the grouping.
    assert_eq!(indices(&place(&two_rows, ReadingDirection::Ltr)), vec![0, 1, 2, 3]);
    assert_eq!(wrong_coordinate_order(&two_rows, ReadingDirection::Ltr), vec![0, 1, 2, 3]);
    assert_eq!(
        place(&one_row, ReadingDirection::Rtl).iter().map(|panel| panel.rect).collect::<Vec<_>>(),
        band.to_vec(),
        "place returns the rectangles it was given, in the order it was given them"
    );

    // Where the two readings **cannot** be told apart: the geometry a template produces. A 2x2 grid and
    // a wide-top, both directions, agree exactly — the sentence that makes the constructions above
    // necessary rather than decorative.
    let grid = vec![cells()[..2].to_vec(), cells()[2..].to_vec()];
    let wide_top = vec![vec![cell(18.0, 18.0, 576.0, 372.0)], vec![cells()[2], cells()[3]]];
    for rows in [&grid, &wide_top] {
        for direction in [ReadingDirection::Ltr, ReadingDirection::Rtl] {
            assert_eq!(
                indices(&place(rows, direction)),
                wrong_coordinate_order(rows, direction),
                "{direction:?}: on a template's own output the two readings agree"
            );
        }
    }
}

#[test]
fn a_page_whose_panels_were_moved_keeps_its_order_and_still_validates() {
    let four = cells();
    let slots = [9_u64, 10, 11, 12];

    // Left to right. Panel 9 is read first and its frame has been dragged to the bottom-right cell, 12
    // to the top-left, and so on: the page's **geometry** now reads 12, 10, 11, 9 while the page still
    // declares 9, 10, 11, 12. Nothing about the frames is illegal — the same four cells in a different
    // assignment — so the only thing that could report is a check that consulted the frames.
    let moved = [(9_u64, four[3]), (10, four[1]), (11, four[2]), (12, four[0])];
    let geometric = order_by_geometry(&moved);
    assert_eq!(geometric, vec![12, 10, 11, 9], "the premise: this page's geometry reads in a different order");
    assert_ne!(geometric, slots, "and not the declared one, or the test proves nothing");
    let file = page_file(&[(9, 100, 0), (10, 101, 1), (11, 102, 2), (12, 103, 3)], &slots);
    assert_eq!(reading_order::check(&file.page, &file.panels), Vec::new(), "the order is a permutation and every index is in step");
    assert_eq!(validate_page(file, &moved), Vec::new(), "a moved page reports nothing: the order is not read off the rectangles");

    // Right to left, and the converse assignment: an `rtl` page declares 10, 9, 12, 11 — the manga
    // order, each row reversed — while its frames sit in the plain left-to-right sequence. Again the
    // geometry disagrees with the order, in the opposite direction, and again nothing is reported.
    let ltr_frames = [(9_u64, four[0]), (10, four[1]), (11, four[2]), (12, four[3])];
    assert_eq!(order_by_geometry(&ltr_frames), vec![9, 10, 11, 12], "the premise: the frames read left to right");
    assert_eq!(
        reading_order::narrative_order(&[2, 2], ReadingDirection::Rtl).iter().map(|slot| slots[usize::from(*slot)]).collect::<Vec<_>>(),
        vec![10, 9, 12, 11],
        "and the rtl order a 2x2 page is read in"
    );
    let rtl = page_file(&[(9, 100, 1), (10, 101, 0), (11, 102, 3), (12, 103, 2)], &[10, 9, 12, 11]);
    assert_eq!(reading_order::check(&rtl.page, &rtl.panels), Vec::new());
    assert_eq!(validate_page(rtl, &ltr_frames), Vec::new(), "an rtl page whose frames read left to right is still read right to left");
}

#[test]
fn an_order_no_template_produces_is_legal_when_the_indices_agree() {
    // **Editable.** The creator reads this 2x2 page 11, 9, 12, 10 and every index follows. No template
    // and no direction produces that order for this shape — `narrative_order` gives `[0, 1, 2, 3]` for
    // `ltr` and `[1, 0, 3, 2]` for `rtl` — which is the point: the order is data the page carries, not
    // something re-derived from its shape or its geometry.
    let four = cells();
    let slots = [9_u64, 10, 11, 12];
    let handmade = [11_u64, 9, 12, 10];
    assert_eq!(in_slots(&handmade, &slots), vec![2, 0, 3, 1]);
    for direction in [ReadingDirection::Ltr, ReadingDirection::Rtl] {
        assert_ne!(reading_order::narrative_order(&[2, 2], direction), vec![2_u16, 0, 3, 1], "no template produces this order ({direction:?})");
    }
    // The frames are the template's own cells, so the geometry reads in placement order — which is yet
    // another order the page does not declare.
    let frames = [(9_u64, four[0]), (10, four[1]), (11, four[2]), (12, four[3])];
    assert_ne!(order_by_geometry(&frames), handmade);
    let file = page_file(&[(9, 100, 1), (10, 101, 3), (11, 102, 0), (12, 103, 2)], &handmade);
    assert_eq!(reading_order::check(&file.page, &file.panels), Vec::new());
    assert_eq!(validate_page(file, &frames), Vec::new());
}

#[test]
fn an_invalid_order_is_reported_although_the_geometry_reads_perfectly() {
    // **The converse**, and it is what stops the two tests above from being satisfied by a validator
    // that stopped checking: the geometry is a perfect left-to-right sequence and the *order* is what
    // is wrong. Every finding below is therefore the order's, and the control at the end uses the same
    // geometry to show so.
    let four = cells();
    let frames = [(9_u64, four[0]), (10, four[1]), (11, four[2]), (12, four[3])];
    assert_eq!(order_by_geometry(&frames), vec![9, 10, 11, 12], "the premise: the geometry is a perfect left-to-right sequence");

    // (a) the order forgets a panel: one finding, on the page's order, and it says why.
    let missing = validate_page(page_file(&[(9, 100, 0), (10, 101, 1), (11, 102, 2), (12, 103, 3)], &[9, 10, 11]), &frames);
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].rule, Some(6));
    assert_eq!(missing[0].subject, "Page(8).reading_order");
    assert!(missing[0].is_error(), "an order that misses a panel is not a warning");
    assert!(missing[0].message.contains("not a permutation"), "{}", missing[0].message);

    // (b) the order is a permutation, but two panels say they sit somewhere else. One finding per
    //     panel, because each panel's own index is its own defect — and still none about the geometry.
    let swapped = validate_page(page_file(&[(9, 100, 0), (10, 101, 1), (11, 102, 3), (12, 103, 2)], &[9, 10, 11, 12]), &frames);
    assert_eq!(swapped.len(), 2, "{swapped:?}");
    assert_eq!(
        swapped.iter().map(|finding| finding.subject.as_str()).collect::<Vec<_>>(),
        vec!["Panel(11).reading_order_index", "Panel(12).reading_order_index"]
    );
    assert_eq!(swapped[0].message, "is 3 but the panel is at position 2 in `Page.reading_order`");

    // (c) the control: the same geometry, the order put right, nothing reported. So (a) and (b) are
    //     findings about the order, and the frames had no part in them.
    let fixed = validate_page(page_file(&[(9, 100, 0), (10, 101, 1), (11, 102, 2), (12, 103, 3)], &[9, 10, 11, 12]), &frames);
    assert_eq!(fixed, Vec::new());
}
