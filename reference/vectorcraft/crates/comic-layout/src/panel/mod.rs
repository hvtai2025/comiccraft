//! The panel as a plan places it: a rectangle, and its position in the page's reading order.
//!
//! This is the handover between the two owners design §6.1 separates: the **rectangle** is what the
//! command turns into the frame object, and the **index** is what `Panel.reading_order_index` and
//! `Page.reading_order` hold. Neither is stored in a comic record by this crate — the document
//! object owns the geometry, the record owns the id.

use vectorcraft_comic_model::ReadingDirection;

use crate::PanelRect;
use crate::reading_order;

/// One panel a plan places.
///
/// The rectangle is in document points, ready for the command that creates the frame object; the
/// index is the panel's position in the page's reading order, which is narrative data and never
/// derived from the rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedPanel {
    /// The frame rectangle, in document points.
    pub rect: PanelRect,
    /// The panel's position in the page's reading order.
    pub reading_order_index: u16,
}

/// Build a plan's panels from a template's rows.
///
/// `rows` is in placement order — top to bottom, left to right within a row — as
/// [`crate::template::rows`] returns it, and the panels come out in that same order. The narrative
/// positions come from [`reading_order::narrative_order`], so they are direction-aware and are never
/// inferred from a coordinate.
///
/// Each slot's position is found **by looking it up in the order** rather than by assuming the order
/// is its own inverse. Every order M1's templates produce *is* an involution — each row is reversed —
/// so the two readings agree today; the lookup is used anyway because it is correct for any
/// permutation, and a future shape whose order is not an involution would otherwise get the wrong
/// index silently.
#[must_use]
pub fn place(rows: &[Vec<PanelRect>], direction: ReadingDirection) -> Vec<PlannedPanel> {
    let panels_per_row: Vec<u32> = rows.iter().map(|row| u32::try_from(row.len()).unwrap_or(u32::MAX)).collect();
    let order = reading_order::narrative_order(&panels_per_row, direction);
    rows.iter().flatten().enumerate().map(|(slot, rect)| PlannedPanel { rect: *rect, reading_order_index: position_of(&order, slot) }).collect()
}

/// The narrative position of a placement slot.
///
/// `u16::MAX` when the order does not mention the slot, which cannot happen for an order built by
/// [`reading_order::narrative_order`] — it is answered rather than panicked on so that a hand-made
/// order gives a wrong index instead of a crash.
fn position_of(order: &[u16], slot: usize) -> u16 {
    order.iter().position(|&ordered| usize::from(ordered) == slot).map_or(u16::MAX, |position| u16::try_from(position).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 grid of 100x100 cells at the origin, in placement order.
    fn cells() -> Vec<Vec<PanelRect>> {
        let rect = |x, y| PanelRect::new(x, y, 100.0, 100.0).expect("a test rectangle");
        vec![vec![rect(0.0, 0.0), rect(100.0, 0.0)], vec![rect(0.0, 100.0), rect(100.0, 100.0)]]
    }

    #[test]
    fn place_keeps_placement_order_and_adds_the_narrative_index() {
        let placed = place(&cells(), ReadingDirection::Ltr);
        assert_eq!(placed.len(), 4);
        assert_eq!(placed.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        // Placement order is unchanged: the first panel is still the top-left one.
        assert_eq!((placed[0].rect.x(), placed[0].rect.y()), (0.0, 0.0));
        assert_eq!((placed[3].rect.x(), placed[3].rect.y()), (100.0, 100.0));
    }

    #[test]
    fn rtl_reverses_the_index_within_each_row_and_leaves_the_rectangles_alone() {
        let placed = place(&cells(), ReadingDirection::Rtl);
        assert_eq!(placed.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![1, 0, 3, 2]);
        assert_eq!((placed[0].rect.x(), placed[0].rect.y()), (0.0, 0.0), "the geometry is the template's, not the order's");
        assert_eq!((placed[1].rect.x(), placed[1].rect.y()), (100.0, 0.0));
    }

    #[test]
    fn a_row_of_one_panel_reads_the_same_either_way() {
        let rect = |y| PanelRect::new(0.0, y, 100.0, 100.0).expect("a test rectangle");
        let rows = vec![vec![rect(0.0)], vec![rect(100.0)], vec![rect(200.0)]];
        let ltr = place(&rows, ReadingDirection::Ltr);
        let rtl = place(&rows, ReadingDirection::Rtl);
        assert_eq!(ltr, rtl);
        assert_eq!(ltr.iter().map(|panel| panel.reading_order_index).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn a_slots_position_is_looked_up_in_the_order_not_read_off_it() {
        // An order that is not its own inverse: slot 1 is read first, slot 2 second, slot 0 third.
        // Every order `narrative_order` builds is an involution, so an input like this can only come
        // from a hand-made order — and it is exactly what tells the lookup apart from `order[slot]`.
        let order = [1u16, 2, 0];
        assert_eq!(position_of(&order, 0), 2);
        assert_eq!(position_of(&order, 1), 0);
        assert_eq!(position_of(&order, 2), 1);
        assert_eq!(position_of(&order, 9), u16::MAX, "a slot the order does not mention is answered, not panicked on");
    }

    #[test]
    fn no_rows_place_no_panels() {
        assert!(place(&[], ReadingDirection::Rtl).is_empty());
    }
}
