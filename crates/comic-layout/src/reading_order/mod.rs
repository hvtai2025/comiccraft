//! Reading order: **explicit**, and validated independently of canvas coordinates (design §8.1).
//!
//! The order a page is read in is narrative data, not geometry. A page whose panels were moved on
//! the canvas keeps its reading order, and a page laid out right to left is ordered by the project's
//! `reading_direction` (schema §5.2) rather than by comparing `x` values. Nothing in this module
//! reads a rectangle.
//!
//! Two things live here:
//!
//! * the order a **template** produces, from its row structure and the direction, as positions into
//!   placement order ([`narrative_order`]);
//! * the check of an order a **page** already carries, against the panels it claims to order
//!   ([`check`], schema rule 6).

use std::collections::{BTreeMap, BTreeSet};

use vectorcraft_comic_model::{Finding, Page, Panel, ReadingDirection};

/// The narrative order of a template's panels, as **positions into placement order**.
///
/// Placement order is top to bottom, and left to right within a row. `ltr` reads it as placed; `rtl`
/// reads each row **right to left**, which is the manga order and the reason the direction is a
/// template input rather than a rendering flag.
///
/// `panels_per_row` is the shape's row structure, so the reversal is per row and never a global
/// mirror: a three-tier page reads top to bottom either way, while a two-panel row swaps. The result
/// is a permutation of `0..panels_per_row.iter().sum()`, which `the_order_is_a_permutation_of_the_placement_slots`
/// checks for every template.
///
/// A page with more than 65535 panels cannot be indexed by `Panel.reading_order_index`, which is a
/// `u16` (schema §6.2), so a slot past that saturates at `u16::MAX` rather than wrapping. No template
/// in this crate reaches it — [`MAX_GRID_SIDE`](crate::template::MAX_GRID_SIDE) bounds a page at 4096
/// panels — so the saturation is a boundary of the type, not a case `layout::plan` can produce.
#[must_use]
pub fn narrative_order(panels_per_row: &[u32], direction: ReadingDirection) -> Vec<u16> {
    let mut order = Vec::new();
    let mut first = 0usize;
    for &panels in panels_per_row {
        let panels = panels as usize;
        let row = first..first + panels;
        match direction {
            ReadingDirection::Ltr => order.extend(row),
            ReadingDirection::Rtl => order.extend(row.rev()),
        }
        first += panels;
    }
    order.into_iter().map(|slot| u16::try_from(slot).unwrap_or(u16::MAX)).collect()
}

/// Check a page's explicit order against the panels it claims to order (schema rule 6).
///
/// Independent of canvas coordinates: it compares ids and indices only, so a page whose panels were
/// moved still validates. `panels` is the page's panel set as its file holds it; a panel the order
/// names but the file does not hold is a **dangling reference**, which rule 1 owns, so it is skipped
/// here rather than reported twice.
///
/// The findings carry `rule: Some(6)`, because this *is* schema rule 6: `comic-model` reports the
/// same predicate from the same facts, and this crate reports it because design §12.1 assigns
/// "invalid reading order" to `comic-layout`. Both subjects are spelled as `comic-model` spells them
/// — `Page(<id>).reading_order` and `Panel(<id>).reading_order_index` — so a caller that runs both
/// validators can deduplicate them.
///
/// **The dedupe key is the pair `(rule, subject)`, not a whole [`Finding`].** The messages differ on
/// purpose — this crate's message names the layout consequence, `comic-model`'s names the schema
/// rule — so comparing a `Finding` field by field would report the same defect twice. A caller that
/// wants one finding per defect keys on the pair, which is unique per defect in both validators.
/// The inverse of [`narrative_order`] is deliberately **not** a public function.
///
/// It existed as one and round 1 was right to strike it: every order this module can produce is a
/// product of per-row reversals, so it is an **involution** and its inverse is the order itself —
/// a public function no test could tell apart from its own inverse. The inverse is now computed
/// where it is actually needed, in [`crate::panel::place`], by looking each slot's position up in
/// the order; that is correct for *any* permutation, so a future shape whose order is not an
/// involution cannot silently get the wrong index.
#[must_use]
pub fn check(page: &Page, panels: &[Panel]) -> Vec<Finding> {
    let mut out = Vec::new();
    let membership: BTreeSet<u64> = page.panel_ids.iter().map(|id| id.0).collect();
    let ordered: BTreeSet<u64> = page.reading_order.iter().map(|id| id.0).collect();
    let is_permutation = membership == ordered && page.panel_ids.len() == page.reading_order.len();
    if !is_permutation {
        out.push(Finding::error(
            Some(6),
            format!("Page({}).reading_order", page.id.0),
            format!(
                "`reading_order` ({:?}) is not a permutation of `panel_ids` ({:?}); every panel must appear exactly once",
                page.reading_order.iter().map(|id| id.0).collect::<Vec<_>>(),
                page.panel_ids.iter().map(|id| id.0).collect::<Vec<_>>()
            ),
        ));
        // One defect, one finding: while the order is not a permutation a panel can be at two
        // positions at once, so comparing each index with "its" position would report the same
        // broken order a second time, with a message that reads as if the panel were the problem.
        return out;
    }
    let declared: BTreeMap<u64, u16> = panels.iter().map(|panel| (panel.id.0, panel.reading_order_index)).collect();
    for (position, id) in page.reading_order.iter().enumerate() {
        let Some(&index) = declared.get(&id.0) else { continue };
        let position = u16::try_from(position).unwrap_or(u16::MAX);
        if index != position {
            out.push(Finding::error(
                Some(6),
                format!("Panel({}).reading_order_index", id.0),
                format!("is {index} but the panel is at position {position} in `Page.reading_order`"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_comic_model::{ChapterId, ContinuityConstraints, GenerationStatus, NodeId, PageId, PageStatus, PanelId, SceneId, Unknown};

    fn page_record(panel_ids: &[u64], reading_order: &[u64]) -> Page {
        Page {
            id: PageId(8),
            chapter_id: ChapterId(3),
            scene_ids: vec![SceneId(4)],
            artboard_id: 1,
            page_number: 1,
            panel_ids: panel_ids.iter().copied().map(PanelId).collect(),
            reading_order: reading_order.iter().copied().map(PanelId).collect(),
            status: PageStatus::Draft,
            unknown: Unknown::default(),
        }
    }

    fn panel(id: u64, reading_order_index: u16) -> Panel {
        Panel {
            id: PanelId(id),
            page_id: PageId(8),
            artboard_id: 1,
            scene_id: SceneId(4),
            frame_node_id: NodeId(id),
            artwork_refs: Vec::new(),
            shot_description: String::new(),
            dialogue_ids: Vec::new(),
            lettering_ids: Vec::new(),
            reading_order_index,
            continuity_constraints: ContinuityConstraints {
                characters: Vec::new(),
                wardrobe: None,
                time_of_day: None,
                callbacks: Vec::new(),
                exceptions: Vec::new(),
            },
            generation_status: GenerationStatus::None,
            // v2's one field (§6.2), at its default: this test builds a panel record to feed
            // `narrative_order`/`reading_order` checks, and none of them reads a lock.
            locked: false,
            unknown: Unknown::default(),
        }
    }

    #[test]
    fn ltr_reads_the_placement_order() {
        assert_eq!(narrative_order(&[1, 2, 2], ReadingDirection::Ltr), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn rtl_reverses_each_row_rather_than_the_whole_page() {
        // Row 0 has one panel, so it stays first; rows 1 and 2 reverse within themselves. A global
        // mirror would put the last panel of the page first, which is wrong for every manga page.
        assert_eq!(narrative_order(&[1, 2, 2], ReadingDirection::Rtl), vec![0, 2, 1, 4, 3]);
        // A page of tiers is read top to bottom either way: direction changes nothing here.
        assert_eq!(narrative_order(&[1, 1, 1], ReadingDirection::Rtl), vec![0, 1, 2]);
        assert_eq!(narrative_order(&[1, 1, 1], ReadingDirection::Rtl), narrative_order(&[1, 1, 1], ReadingDirection::Ltr));
    }

    #[test]
    fn a_row_of_three_reverses_wholesale() {
        assert_eq!(narrative_order(&[3], ReadingDirection::Rtl), vec![2, 1, 0]);
        assert_eq!(narrative_order(&[3], ReadingDirection::Ltr), vec![0, 1, 2]);
    }

    #[test]
    fn the_order_is_a_permutation_of_the_placement_slots() {
        // Every template and both directions: each slot appears exactly once. The inverse is not
        // asserted here because it is not a function of this module — `panel::place` looks each
        // slot's position up in the order, which is what `place_*` tests below check.
        let shapes: [&[u32]; 4] = [&[1, 1], &[1, 1, 1], &[1, 2], &[1, 2, 2]];
        for panels_per_row in shapes {
            for direction in [ReadingDirection::Ltr, ReadingDirection::Rtl] {
                let order = narrative_order(panels_per_row, direction);
                let expected: Vec<u16> = (0..panels_per_row.iter().sum::<u32>()).map(|slot| slot as u16).collect();
                let mut sorted = order.clone();
                sorted.sort_unstable();
                assert_eq!(sorted, expected, "{panels_per_row:?} {direction:?} must be a permutation of the slots");
            }
        }
    }

    #[test]
    fn an_empty_shape_has_an_empty_order() {
        assert!(narrative_order(&[], ReadingDirection::Rtl).is_empty());
    }

    #[test]
    fn a_page_whose_order_agrees_with_its_panels_reports_nothing() {
        let page = page_record(&[9, 10, 11], &[9, 10, 11]);
        let panels = vec![panel(9, 0), panel(10, 1), panel(11, 2)];
        assert_eq!(check(&page, &panels), Vec::new());
        // A right-to-left page is just a different permutation: 9 is read last, and its index says so.
        let rtl = page_record(&[9, 10, 11], &[11, 10, 9]);
        let rtl_panels = vec![panel(9, 2), panel(10, 1), panel(11, 0)];
        assert_eq!(check(&rtl, &rtl_panels), Vec::new());
    }

    #[test]
    fn a_panel_the_order_forgets_is_reported_once() {
        let page = page_record(&[9, 10, 11], &[9, 10]);
        let findings = check(&page, &[panel(9, 0), panel(10, 1), panel(11, 2)]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, Some(6));
        assert!(findings[0].is_error());
        assert_eq!(findings[0].subject, "Page(8).reading_order");
        assert!(findings[0].message.contains("not a permutation"), "{}", findings[0].message);
    }

    #[test]
    fn a_repeated_panel_is_reported_even_when_the_lengths_match() {
        // `[9, 9]` for `panel_ids [9, 10]`: same length, different set. A length-only check misses
        // it, which is why the check compares the sets as well. One finding, not two: while the order
        // is not a permutation, comparing an index with "its" position would report the same defect
        // again — panel 9 is at both positions.
        let findings = check(&page_record(&[9, 10], &[9, 9]), &[panel(9, 0), panel(10, 1)]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].subject, "Page(8).reading_order");
        assert!(findings[0].message.contains("not a permutation"), "{}", findings[0].message);
    }

    #[test]
    fn a_panel_whose_index_disagrees_with_its_position_is_reported() {
        // The permutation is right; the panel's own index is not. This is the half a set comparison
        // cannot see, and it is what `Page.reading_order` and `Panel.reading_order_index` disagreeing
        // looks like in a hand-edited bundle.
        let findings = check(&page_record(&[9, 10], &[9, 10]), &[panel(9, 1), panel(10, 0)]);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].subject, "Panel(9).reading_order_index");
        assert_eq!(findings[0].message, "is 1 but the panel is at position 0 in `Page.reading_order`");
        assert_eq!(findings[1].subject, "Panel(10).reading_order_index");
    }

    #[test]
    fn an_order_naming_a_panel_the_file_does_not_hold_is_left_to_rule_1() {
        // A dangling reference is rule 1's finding, not rule 6's: reporting it here would return the
        // same defect twice with a different rule number.
        let findings = check(&page_record(&[9, 12], &[9, 12]), &[panel(9, 0)]);
        assert_eq!(findings.len(), 0, "{findings:?}");
    }
}
