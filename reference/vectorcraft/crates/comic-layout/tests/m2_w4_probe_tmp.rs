//! TEMPORARY probe (reviewer round 1) — deleted before the report is written.

use vectorcraft_comic_layout::lettering::outline::BalloonShape;
use vectorcraft_comic_layout::lettering::plan::{LetteringRequest, TextSource, new_balloon};
use vectorcraft_comic_layout::lettering::tail::attach_point;
use vectorcraft_comic_model::LetteringKind;
use vectorcraft_geom::{Affine, Point, Vec2};
use vectorcraft_text::FontDb;

#[test]
fn probe_translation_equivariance() {
    let db = FontDb::global();
    let request = LetteringRequest::new(LetteringKind::Thought, TextSource::Dialogue("Hmm, I wonder about that."))
        .placed_at(Point::new(200.0, 150.0))
        .with_tail(Point::new(160.0, 320.0));
    let before = new_balloon(db, &request).expect("planned");
    let tip = before.tail.as_ref().expect("tail").tip;
    println!("tip={tip:?} attach={:?} bounds={:?}", before.tail.as_ref().unwrap().attach, before.bounds);
    println!("cloud frame bounds={:?} anchors={}", before.frame.bounds(), before.frame.anchor_count());
    let boxed = BalloonShape::Box.outline(before.bounds);
    println!("boxed frame bounds={:?} anchors={}", boxed.bounds(), boxed.anchor_count());
    println!("attach on cloud  = {:?}", attach_point(&before.frame, tip, before.bounds));
    println!("attach on boxed    = {:?}", attach_point(&boxed, tip, before.bounds));
    for delta in [Vec2::new(-360.0, 420.0), Vec2::new(60.0, -40.0), Vec2::new(1000.0, -700.0)] {
        let translated = boxed.transformed(Affine::translate(delta));
        let direct = attach_point(&translated, tip, before.bounds);
        let derived = attach_point(&boxed, tip, before.bounds) + delta;
        let moved = {
            let mut edited = before.clone();
            edited.frame = boxed.clone();
            edited.moved_by(delta).expect("moved")
        };
        println!(
            "delta={delta:?}\n  direct={direct:?} derived={derived:?} diff={}\n  moved_by base={:?} moved bounds={:?} frame bounds={:?}",
            (direct - derived).hypot(),
            moved.tail.as_ref().unwrap().attach,
            moved.bounds,
            moved.frame.bounds()
        );
    }
}
