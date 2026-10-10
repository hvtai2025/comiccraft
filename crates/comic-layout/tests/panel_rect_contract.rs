//! Reviewer tests (`code-reviewer`, M0 round 1) for [`PanelRect`].
//!
//! Round 1 found that the implementer exercised the shared-edge overlap case on **one** axis
//! only (a shared vertical edge) and the containment failure on **one** edge only (the right
//! edge), so a single-axis `<`/`<=` slip survived their suite. These tests cover both axes and
//! all four edges.
//!
//! Round 1 also found finding **R1**: `PanelRect::new` documented an invariant it could not
//! enforce, because the four fields were `pub` and `Deserialize` was derived — two paths that
//! built a `PanelRect` without consulting the constructor. The reproduction test written for
//! R1 asserted the invariant and was expected to fail. **The owner fixed the defect** by making
//! the fields private, removing `Deserialize` (a `PanelRect` is a layout primitive, not a wire
//! type — schema §6.2 gives `Panel` no geometry), and returning a typed error from `new`.
//!
//! The original reproduction is therefore no longer *expressible*: a struct literal and
//! `serde_json::from_str::<PanelRect>` are now compile errors, which is the strongest form of
//! the assertion it was making. What remains testable at runtime is the invariant itself, which
//! [`every_accepted_rectangle_holds_the_invariant`] covers, plus the typed errors that replaced
//! the single `None`.

// `clippy.toml` sets `allow-panic-in-tests`, but clippy only applies it to `#[cfg(test)]`
// modules, not to an integration-test crate. The repository's policy (clippy.toml header) is
// that tests may panic, so this is the file-level equivalent for an integration test.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use vectorcraft_comic_layout::{PanelRect, PanelRectError};

fn rect(x: f64, y: f64, w: f64, h: f64) -> PanelRect {
    PanelRect::new(x, y, w, h).expect("valid test rectangle")
}

#[test]
fn touching_edges_do_not_overlap_on_either_axis() {
    // Shared vertical edge — the case the implementer covered.
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let b = rect(10.0, 0.0, 10.0, 10.0);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));

    // Shared horizontal edge — NOT covered by the implementer's suite. Mutating the y-axis
    // comparison from `<` to `<=` left their suite green while reporting every vertically
    // stacked pair of panels as overlapping.
    let top = rect(0.0, 0.0, 10.0, 10.0);
    let bottom = rect(0.0, 10.0, 10.0, 10.0);
    assert!(!top.overlaps(&bottom), "a shared horizontal edge is a gutter, not an overlap");
    assert!(!bottom.overlaps(&top));

    // Shared corner — both edges at once.
    let diagonal = rect(10.0, 10.0, 10.0, 10.0);
    assert!(!a.overlaps(&diagonal), "a shared corner point is not an overlap");
}

#[test]
fn an_overlap_of_one_unit_is_still_an_overlap() {
    // The complement of the touching-edge test: the boundary between "touching" and
    // "overlapping" must sit strictly between them on both axes.
    let a = rect(0.0, 0.0, 10.0, 10.0);
    assert!(a.overlaps(&rect(9.0, 0.0, 10.0, 10.0)), "a one-unit horizontal overlap is an overlap");
    assert!(a.overlaps(&rect(0.0, 9.0, 10.0, 10.0)), "a one-unit vertical overlap is an overlap");
    assert!(!a.overlaps(&rect(10.0, 0.0, 10.0, 10.0)), "exactly touching horizontally is not");
    assert!(!a.overlaps(&rect(0.0, 10.0, 10.0, 10.0)), "exactly touching vertically is not");
}

#[test]
fn containment_fails_when_straddling_any_single_edge() {
    // The implementer checked only the right edge. A `<` vs `<=` slip on one axis, or a
    // swapped argument in `other.x() >= self.x()`, would survive their suite.
    let page = rect(0.0, 0.0, 100.0, 100.0);
    // Flush on every edge: contained.
    assert!(page.contains(&rect(0.0, 0.0, 100.0, 100.0)));
    // Straddling one edge by a single unit each way: not contained.
    assert!(!page.contains(&rect(-1.0, 0.0, 10.0, 10.0)), "straddling the left edge");
    assert!(!page.contains(&rect(0.0, -1.0, 10.0, 10.0)), "straddling the top edge");
    assert!(!page.contains(&rect(0.0, 0.0, 101.0, 10.0)), "straddling the right edge");
    assert!(!page.contains(&rect(0.0, 0.0, 10.0, 101.0)), "straddling the bottom edge");
}

#[test]
fn containment_holds_for_a_panel_inside_a_negative_origin_page() {
    // Pages and panels are in document points and may sit at a negative origin; the
    // comparisons are not written assuming a non-negative origin.
    let page = rect(-50.0, -50.0, 100.0, 100.0);
    assert!(page.contains(&rect(-50.0, -50.0, 100.0, 100.0)));
    assert!(page.contains(&rect(-10.0, -10.0, 20.0, 20.0)));
    assert!(!page.contains(&rect(-60.0, -10.0, 20.0, 20.0)));
    assert!(!page.contains(&rect(-10.0, -10.0, 200.0, 20.0)), "a panel wider than the page");
}

#[test]
fn right_and_bottom_are_the_far_edges() {
    // Every predicate above is built on these two, and neither was tested directly.
    assert_eq!(rect(10.0, 20.0, 30.0, 40.0).right(), 40.0);
    assert_eq!(rect(10.0, 20.0, 30.0, 40.0).bottom(), 60.0);
    assert_eq!(rect(-50.0, -50.0, 100.0, 100.0).right(), 50.0);
    assert_eq!(rect(0.0, 0.0, 0.5, 0.25).right(), 0.5, "sub-unit pages must not round away");
}

#[test]
fn new_rejects_every_reason_the_documentation_names() {
    for (x, y, w, h) in [
        (0.0, 0.0, 0.0, 0.0),                // both zero
        (0.0, 0.0, 0.0, 10.0),               // zero width
        (0.0, 0.0, 10.0, 0.0),               // zero height
        (0.0, 0.0, -0.0, 10.0),              // negative zero width
        (0.0, 0.0, 10.0, -1.0),              // negative height
        (f64::NAN, 0.0, 10.0, 10.0),         // NaN origin x
        (0.0, f64::NAN, 10.0, 10.0),         // NaN origin y
        (0.0, 0.0, f64::NAN, 10.0),          // NaN width
        (0.0, 0.0, 10.0, f64::NAN),          // NaN height
        (f64::INFINITY, 0.0, 10.0, 10.0),    // infinite origin x
        (0.0, 0.0, f64::INFINITY, 10.0),     // infinite width
        (0.0, 0.0, f64::NEG_INFINITY, 10.0), // infinite height
        (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
    ] {
        assert!(PanelRect::new(x, y, w, h).is_err(), "PanelRect::new({x}, {y}, {w}, {h}) must be rejected");
    }
    // The smallest legal rectangle is accepted — the rejection is on the sign, not the magnitude.
    assert!(PanelRect::new(0.0, 0.0, f64::MIN_POSITIVE, f64::MIN_POSITIVE).is_ok());
}

#[test]
fn new_names_the_field_and_the_reason() {
    // Round 1 R1's second half: the caller could not tell a caller bug from a data bug,
    // because three causes collapsed into one `None` (design §12.1 wants an actionable
    // message). The typed error is what makes that possible.
    assert_eq!(PanelRect::new(0.0, 0.0, -1.0, 10.0), Err(PanelRectError::NonPositive { field: "width" }));
    assert_eq!(PanelRect::new(0.0, 0.0, 10.0, -1.0), Err(PanelRectError::NonPositive { field: "height" }));
    assert_eq!(PanelRect::new(0.0, f64::NAN, 10.0, 10.0), Err(PanelRectError::NotFinite { field: "y" }));
    // The messages are distinct and name the field, so a validation report can quote them.
    let width = PanelRectError::NonPositive { field: "width" }.to_string();
    let nan = PanelRectError::NotFinite { field: "y" }.to_string();
    assert!(width.contains("width"), "{width}");
    assert!(nan.contains("y"), "{nan}");
    assert_ne!(width, nan);
}

#[test]
fn every_accepted_rectangle_holds_the_invariant() {
    // The runtime half of R1. With private fields and no `Deserialize`, `new` is the only
    // construction path, so this is a property of the type rather than of one call site:
    // every accepted rectangle is finite, strictly positive in both dimensions, has a far edge
    // strictly beyond its origin, and therefore overlaps itself.
    for (x, y, w, h) in [
        (0.0, 0.0, 1.0, 1.0),
        (-50.0, -50.0, 100.0, 100.0),
        (0.0, 0.0, f64::MIN_POSITIVE, f64::MIN_POSITIVE),
        (1e-9, 1e-9, 1e-9, 1e-9),
        (1e15, -1e15, 1e9, 1e9),
        (f64::MAX / 4.0, f64::MAX / 4.0, f64::MAX / 4.0, f64::MAX / 4.0),
    ] {
        let r = rect(x, y, w, h);
        assert!(r.width() > 0.0 && r.width().is_finite(), "width of {r:?}");
        assert!(r.height() > 0.0 && r.height().is_finite(), "height of {r:?}");
        assert!(r.x().is_finite() && r.y().is_finite(), "origin of {r:?}");
        assert!(r.right() > r.x(), "right edge must be past the origin for {r:?}");
        assert!(r.bottom() > r.y(), "bottom edge must be past the origin for {r:?}");
        assert!(r.contains(&r), "a rectangle contains itself: {r:?}");
        assert!(r.overlaps(&r), "a rectangle with area overlaps itself: {r:?}");
    }
}

#[test]
fn a_rectangle_too_far_out_to_have_area_is_refused() {
    // The case that produced finding R1's follow-up: at this magnitude `x + 1.0 == x`, so the
    // rectangle would have zero area and would fail to overlap itself, which would make the
    // validator's overlap check unsound. `new` refuses it rather than returning a trap.
    let huge = f64::MAX / 4.0;
    assert!(PanelRect::new(huge, 0.0, 1.0, 10.0).is_err());
    assert!(PanelRect::new(0.0, 0.0, 10.0, 1.0).is_err() || PanelRect::new(0.0, huge, 10.0, 1.0).is_err());
}

// -------------------------------------------------------------------------------------------------
// Round 2 — the post-fix revision (private fields, typed errors, and the new `NotRepresentable`
// guard that round 1 never saw). Focus: is the new behaviour correct and complete, and does the
// invariant survive at the edges of `f64` arithmetic on both axes and both signs?
// -------------------------------------------------------------------------------------------------

/// Every magnitude that changes what `f64` can represent, plus both signs of each.
#[rustfmt::skip]
const MAGNITUDES: [f64; 18] = [
    0.0,                    // exact zero: every positive size advances the edge
    f64::from_bits(1),      // 5e-324, the smallest subnormal
    1e-320,                 // subnormal
    -1e-320,
    1e-308,                 // just below the smallest normal
    -1e-308,
    f64::MIN_POSITIVE,      // 2.2e-308, the smallest normal
    1.0,
    -1.0,
    1e15,                   // ULP 0.125 — sub-unit sizes still land
    1e16,                   // ULP 2.0 — a one-point gutter rounds away
    -1e16,
    1e300,                  // ULP 1.5e284
    -1e300,
    f64::MAX / 4.0,
    -f64::MAX / 4.0,
    f64::MAX,
    -f64::MAX,
];

/// Sizes spanning the whole range, including one below every ULP and one that overflows.
#[rustfmt::skip]
const SIZES: [f64; 10] = [
    f64::from_bits(1),      // 5e-324
    f64::MIN_POSITIVE,      // 2.2e-308
    1e-320,
    1e-9,
    0.5,
    1.0,
    2.0,                    // exactly one ULP at 1e16
    1e15,
    1e300,
    f64::MAX,
];

#[test]
fn the_representability_guard_accepts_exactly_the_rectangles_that_advance_their_far_edge() {
    // The guard `new` applies is `x + width <= x` → reject. This walks every combination of a
    // magnitude and a size on **both axes** and asserts the guard's verdict matches the
    // arithmetic it is guarding — no combination may be rejected whose far edge is
    // representable, and none accepted whose far edge is not.
    for &origin in &MAGNITUDES {
        for &size in &SIZES {
            let far_edge_advances = origin + size > origin;
            // Every input here is finite and strictly positive, so `NotFinite`/`NonPositive`
            // cannot be the right answer to either call; any other error means the guard
            // misfired and the coordinate's magnitude, not the size, was the problem.
            for (label, verdict, far_edge) in [
                ("x/width", PanelRect::new(origin, 0.0, size, 1.0), origin + size),
                ("y/height", PanelRect::new(0.0, origin, 1.0, size), origin + size),
            ] {
                let field = if label.starts_with("x") { "width" } else { "height" };
                match (verdict, far_edge_advances) {
                    (Ok(_), true) => {}
                    (Ok(_), false) => panic!("accepted {label} {origin:e} + {size:e} = {far_edge:e}, which does not advance the far edge"),
                    (Err(PanelRectError::NotRepresentable { field: f }), false) if f == field => {}
                    (Err(other), false) => {
                        panic!("{label} {origin:e} + {size:e} gave {other:?}, expected NotRepresentable {{ field: {field:?} }}")
                    }
                    (Err(other), true) => panic!("{label} {origin:e} + {size:e} gave {other:?}, but the far edge advances to {far_edge:e}"),
                }
            }
        }
    }
}

#[test]
fn the_guard_is_symmetric_on_both_axes_and_both_signs() {
    // A far edge that advances on the x axis must advance on the y axis at the same magnitude,
    // and a negative origin must be judged by the same rule as a positive one. Round 2 focus:
    // `f64` ULP behaviour is not symmetric about zero, so this is checked rather than assumed.
    for &m in &[1e15f64, 1e16, 1e17, 1e300, f64::MAX / 4.0, f64::MAX] {
        let below_ulp = PanelRect::new(m, 0.0, 1.0, 1.0);
        let below_ulp_y = PanelRect::new(0.0, m, 1.0, 1.0);
        assert_eq!(below_ulp.is_err(), below_ulp_y.is_err(), "axes disagree at {m:e}");

        let at_ulp = PanelRect::new(m, 0.0, m.abs().min(1e300), 1e300);
        let at_ulp_y = PanelRect::new(0.0, m, 1e300, m.abs().min(1e300));
        assert_eq!(at_ulp.is_ok(), at_ulp_y.is_ok(), "axes disagree at {m:e}");

        // Same magnitude, opposite sign: the rule is about the ULP of the coordinate, and the
        // ULP of `x` equals the ULP of `-x`, so the verdicts must match.
        let positive = PanelRect::new(m, 0.0, 1.0, 1.0).is_err();
        let negative = PanelRect::new(-m, 0.0, 1.0, 1.0).is_err();
        assert_eq!(positive, negative, "±{m:e} disagree");
    }
}

#[test]
fn the_predicates_hold_for_every_rectangle_the_guard_accepts() {
    // The guard exists so that "a rectangle with area overlaps itself" is a property the layout
    // validator can rely on. This is that property, checked over the same magnitude × size grid
    // rather than on the six hand-picked cases round 1 used.
    for &x in &MAGNITUDES {
        for &y in &MAGNITUDES {
            for &w in &SIZES {
                for &h in &SIZES {
                    let Ok(rect) = PanelRect::new(x, y, w, h) else { continue };
                    assert!(rect.width() > 0.0 && rect.height() > 0.0, "{rect:?}");
                    assert!(rect.right() > rect.x(), "far edge must advance for {rect:?}");
                    assert!(rect.bottom() > rect.y(), "far edge must advance for {rect:?}");
                    assert!(rect.overlaps(&rect), "a rectangle with area overlaps itself: {rect:?}");
                    assert!(rect.contains(&rect), "a rectangle contains itself: {rect:?}");
                }
            }
        }
    }
}

#[test]
fn a_one_point_gutter_at_a_magnitude_where_it_rounds_away_is_refused_not_silently_lost() {
    // ULP at 1e16 is 2.0, so a one-point panel there is invisible in `f64`. Refusing it is the
    // correct behaviour: a validator that divides 1e16 by two panels plus a gutter would
    // otherwise silently produce a zero-width gutter and two identical frames.
    assert_eq!(PanelRect::new(1e16, 0.0, 1.0, 10.0), Err(PanelRectError::NotRepresentable { field: "width" }));
    // One ULP is accepted, and the far edge is exactly the neighbouring coordinate.
    let left = PanelRect::new(1e16, 0.0, 2.0, 10.0).expect("2.0 is one ULP at 1e16");
    assert_eq!(left.right(), 1e16 + 2.0);
    // A panel starting exactly at that far edge is a neighbour, not an overlap.
    let right = PanelRect::new(left.right(), 0.0, 2.0, 10.0).expect("valid");
    assert!(!left.overlaps(&right), "a shared edge at 1e16 is still a gutter");
    // Sharing real area at the same magnitude still reports the overlap, in both directions.
    let overlapping = PanelRect::new(1e16 + 1.0, 0.0, 4.0, 10.0).expect("valid");
    assert!(left.overlaps(&overlapping) && overlapping.overlaps(&left), "an overlap at 1e16 must not round away");
}

#[test]
fn an_overflowing_far_edge_is_accepted_and_keeps_the_predicates_sound() {
    // Round 2 finding R2-2: `new` rejects a far edge that fails to advance but **accepts** one
    // that overflows to `+∞`, because `x + width <= x` is false when the sum is infinite. The
    // doc comment promises "four finite, strictly positive dimensions … a positive
    // representable area"; the four stored fields are indeed finite, but `right()` is not, and
    // nothing checks it. The predicates still behave — this test pins that — so the guard's
    // purpose is intact; the gap is in the documented invariant, not in the arithmetic.
    let rect = PanelRect::new(f64::MAX, 0.0, f64::MAX, 10.0).expect("x + width overflows to +inf, which is past x");
    assert!(rect.right().is_infinite(), "right() really is +inf today: {}", rect.right());
    assert!(rect.x().is_finite() && rect.width().is_finite(), "the four stored fields are still finite");
    assert!(rect.overlaps(&rect) && rect.contains(&rect), "the guard's purpose — self-overlap — still holds");

    // A page whose far edge is finite must not contain a panel whose far edge overflows: the
    // infinite edge must not make `other.right() <= self.right()` compare loosely.
    let page = PanelRect::new(0.0, 0.0, f64::MAX, 100.0).expect("valid page");
    let beyond = PanelRect::new(f64::MAX, 0.0, f64::MAX, 100.0).expect("valid panel");
    assert!(page.right().is_finite(), "this page's far edge is finite: {}", page.right());
    assert!(beyond.right().is_infinite(), "the panel's far edge overflowed: {}", beyond.right());
    assert!(!page.contains(&beyond), "a panel starting past the page is not contained, infinite far edge or not");
    assert!(!page.overlaps(&beyond), "and they share no area");
    // Touching exactly at the page's far edge is a neighbour, not an overlap.
    let flush = PanelRect::new(page.right(), 0.0, 1.0, 100.0);
    assert!(flush.is_err(), "a panel 1 point wide at f64::MAX is not representable: {flush:?}");
    let flush = PanelRect::new(page.right(), 0.0, 1e300, 100.0).expect("a representable neighbour at the edge");
    assert!(!page.overlaps(&flush), "flush at the far edge is still not an overlap");
}

#[test]
fn a_sub_unit_rectangle_is_accepted_even_though_its_area_underflows() {
    // Round 2 finding R2-3: the doc says "a positive representable area", but the smallest
    // legal rectangle's area is exactly 0.0 in `f64`. The rectangle is still perfectly usable
    // — every predicate is built on edges, not on the product — so the rejection would be
    // wrong; only the wording overstates what is guaranteed. This pins the real behaviour.
    let rect = PanelRect::new(0.0, 0.0, f64::MIN_POSITIVE, f64::MIN_POSITIVE).expect("the smallest legal rectangle");
    assert!(rect.right() > rect.x() && rect.bottom() > rect.y(), "its far edges do advance");
    assert_eq!(rect.width() * rect.height(), 0.0, "but its area underflows, so 'positive representable area' is not what is guaranteed");
    assert!(rect.overlaps(&rect) && rect.contains(&rect), "and the predicates that matter still hold");

    // Sub-unit sizes at a normal origin must not round away either.
    let small = PanelRect::new(-0.25, 0.0, 0.5, 0.25).expect("valid");
    assert_eq!(small.right(), 0.25);
    assert!(small.overlaps(&PanelRect::new(0.0, 0.0, 0.25, 0.25).expect("valid")), "a quarter-point overlap is an overlap");
    assert!(!small.overlaps(&PanelRect::new(0.25, 0.0, 0.5, 0.25).expect("valid")), "and a shared quarter-point edge is not");
}

#[test]
fn a_negative_origin_page_places_its_far_edge_where_the_arithmetic_says_it_is() {
    // Negative origins are normal in document points and are the case where a naive
    // `right() = max(x, x + width)` normalisation would hide a bug. The guard and the
    // predicates must both work on the negative side of zero.
    for &m in &[1e16f64, 1e300, f64::MAX / 4.0] {
        let page = PanelRect::new(-m, -m, m, m).expect("a page from -m to 0");
        assert_eq!(page.right(), 0.0, "−{m:e} + {m:e} is exactly zero");
        assert_eq!(page.bottom(), 0.0);
        assert!(page.contains(&PanelRect::new(-m / 2.0, -m / 2.0, m / 2.0, m / 2.0).expect("valid")));
        assert!(
            !page.contains(&PanelRect::new(0.0, 0.0, m / 2.0, m / 2.0).expect("valid")),
            "a panel starting at 0 sticks out past the page's right edge"
        );
        assert!(!page.overlaps(&PanelRect::new(0.0, 0.0, m / 2.0, m / 2.0).expect("valid")));
    }
}

#[test]
fn every_error_variant_has_its_own_quotable_message() {
    // Round 2: `NotRepresentable` is a variant round 1 never saw, and no test asserted its
    // message. Design §12.1 wants a deterministic check that *reports*, so each cause must be
    // distinguishable in text a validator can quote back — three distinct causes collapsing
    // into one message would put R1 back in a different shape.
    let messages = [
        PanelRectError::NotFinite { field: "x" }.to_string(),
        PanelRectError::NonPositive { field: "width" }.to_string(),
        PanelRectError::NotRepresentable { field: "height" }.to_string(),
    ];
    assert_eq!(messages.len(), 3);
    for (error, field) in [
        (PanelRectError::NotFinite { field: "x" }, "x"),
        (PanelRectError::NonPositive { field: "width" }, "width"),
        (PanelRectError::NotRepresentable { field: "height" }, "height"),
    ] {
        let message = error.to_string();
        assert!(message.contains(field), "{message} must name the offending field");
        assert!(!message.is_empty(), "an error with no text cannot be reported");
        // `std::error::Error` is what a command layer wraps it in; the Display text must
        // survive that, so it is checked through the trait object too.
        let as_dyn: &dyn std::error::Error = &error;
        assert_eq!(as_dyn.to_string(), message);
    }
    // The distinguishing content must be the *cause*, not the field name: two causes on the
    // same field must not produce the same report, and the same cause on two fields must be
    // distinguishable too. (Comparing messages built from different fields alone would pass
    // even if two variants rendered the same sentence — that mutation was tried and was caught
    // only by the same-field comparison below.)
    for field in ["x", "y", "width", "height"] {
        let finite = PanelRectError::NotFinite { field }.to_string();
        let non_positive = PanelRectError::NonPositive { field }.to_string();
        let not_representable = PanelRectError::NotRepresentable { field }.to_string();
        assert_ne!(finite, non_positive, "NotFinite and NonPositive must not report alike for `{field}`");
        assert_ne!(finite, not_representable, "NotFinite and NotRepresentable must not report alike for `{field}`");
        assert_ne!(non_positive, not_representable, "NonPositive and NotRepresentable must not report alike for `{field}`");
    }
    // And the same cause on two different fields must name which one, so a validator can say
    // *which* dimension was wrong.
    assert_ne!(
        PanelRectError::NonPositive { field: "width" }.to_string(),
        PanelRectError::NonPositive { field: "height" }.to_string(),
        "a validator must be able to say which dimension was wrong"
    );
    assert_ne!(
        PanelRectError::NotRepresentable { field: "width" }.to_string(),
        PanelRectError::NotRepresentable { field: "height" }.to_string(),
        "and likewise for an unrepresentable extent"
    );
}
