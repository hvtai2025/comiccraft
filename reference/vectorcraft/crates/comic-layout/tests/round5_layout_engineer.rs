//! Round-5 `layout-engineer` test for `crates/comic-layout`'s `validate` tree.
//!
//! Round 4 left a recorded hole: `tolerance_at` evaluated `TOLERANCE_ULPS * scale.abs() * f64::EPSILON`
//! left to right, so `(8.0 * scale)` overflowed to `+inf` for a *finite* `scale` past `f64::MAX / 8`
//! (~`2.247e307`pt) **before** the multiply by epsilon. The whole term was then `inf`, every threshold
//! held, and the geometry check was silently switched off — for a finite page, the same failure mode
//! round 4 closed for the non-finite one. Round 3's justification for leaving it ("one ULP there is
//! `~2e292`pt, so no defect is representable") was wrong: a `5e299`pt overlap on a `2.25e307`pt page is
//! perfectly representable and was **silently unreported**. Round 5 multiplies the small constant first,
//! which keeps the term finite at every finite magnitude. This test is that defect: it fails on the
//! left-to-right order and passes on the reordered one.
//!
//! The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
//! `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use vectorcraft_comic_layout::PanelRect;
use vectorcraft_comic_layout::validate::{Fault, Limits, TOLERANCE_ULPS, faults, tolerance_at};
use vectorcraft_geom::Rect;

fn rect(x: f64, y: f64, width: f64, height: f64) -> PanelRect {
    PanelRect::new(x, y, width, height).expect("a legal panel")
}

#[test]
fn a_representable_overlap_on_a_2_25e307pt_page_is_reported_not_silently_clean() {
    // The finite ceiling, stated as the arithmetic rather than as a claim: `8.0 * 2.25e307` is `1.8e308`,
    // past `f64::MAX`, so on the old order the term is `inf` before epsilon is ever multiplied in.
    assert!((8.0f64 * 2.25e307).is_infinite(), "the left-to-right order really does overflow at a finite scale");
    let overflow_starts_at = f64::MAX / TOLERANCE_ULPS;
    assert!(2.25e307 > overflow_starts_at, "and 2.25e307pt is past where that overflow starts ({overflow_starts_at:e}pt)");

    // On the reordered order the term is finite here — and at *every* finite scale, which is the point:
    // there is no magnitude at which a page silently stops being checked.
    let tolerance = tolerance_at(2.25e307);
    assert!(tolerance.is_finite(), "the tolerance at 2.25e307pt must be finite, not {tolerance:e}");
    assert_eq!(tolerance, TOLERANCE_ULPS * f64::EPSILON * 2.25e307, "the small constant is multiplied first");
    assert!(tolerance_at(f64::MAX).is_finite(), "and no finite scale has a ceiling");
    // The reorder must not move any ordinary page: bit-identical at the 1e12pt the round-4 test pins, and
    // that pinned value is the same literal to the last digit.
    assert_eq!(tolerance_at(1e12), TOLERANCE_ULPS * 1e12 * f64::EPSILON, "the two orders agree at 1e12pt");
    assert_eq!(tolerance_at(1e12), 0.0017763568394002505, "and the pinned value is unchanged by the reorder");

    // The defect itself. A `2.25e307`pt page is finite, so `magnitude` does not skip it; the panels share
    // `5e299`pt on both axes, seven orders of magnitude past the `4.0e292`pt tolerance, so this is a
    // comparison the arithmetic represents — and one a page that size could never make printable.
    let page = Rect::new(0.0, 0.0, 2.25e307, 2.25e307);
    let panels = [rect(0.0, 0.0, 1e300, 1e300), rect(5e299, 5e299, 1e300, 1e300)];
    let limits = Limits { min_gutter: 0.0, ..Limits::default() };
    assert!(5e299 > tolerance, "the shared depth is far past the tolerance, so it is representable");
    assert_eq!(
        faults(page, None, &panels, &limits),
        vec![Fault::Overlap { first: 0, second: 1 }],
        "a 5e299pt overlap on a 2.25e307pt page must be reported, not silently clean"
    );
}
