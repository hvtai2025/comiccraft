//! M2 W2 — the golden-render harness, and **one representative page**.
//!
//! M2's sixth exit criterion is *"**Gate:** `cargo xtask ci`, plus golden-render tests for one
//! representative page."* This file is that golden, its harness, and the font trap's answer.
//!
//! # The trap: a golden render and an empty `CRAFT_FONTS`
//!
//! M2's fifth criterion says lettering must pass with `CRAFT_FONTS` empty, and comic faces live in
//! `storytold/craft-fonts` — a companion checkout that is **absent** in a normal build. So a page's
//! pixels depend on whether that checkout is present: a golden captured with the faces and compared
//! without them fails for a reason that is **not a defect**, and a tolerance wide enough to hide
//! that hides real regressions too. A golden that cannot fail does not verify anything; it
//! **certifies**.
//!
//! The answer here has four parts:
//!
//! 1. **The representative page renders no text at all**, so its pixels cannot depend on any face.
//!    Panels, gutters, frames, borders, safe margins and placed artwork are geometry and images;
//!    the type *inside* a balloon is where the face enters. This page therefore carries its panels
//!    (from a template), artwork in three of them, an explicit reading order, a resize and a move —
//!    and no lettering. `the_golden_page_renders_no_text…` **enforces** that, so the golden cannot
//!    silently become font-dependent the day someone adds a balloon to the fixture.
//! 2. **The lettering is asserted at record level** on the same page (as M1's acceptance suite
//!    does), and the harness has a **second golden path, keyed by the face**, for a page whose
//!    lettering pixels *are* wanted. That path **skips** when the face is not in this build, naming
//!    the face the project asked for and what stands in for it. It never passes silently and never
//!    fails as though the code were wrong.
//! 3. **The tolerance is justified with a mutation it rejects**, in-process —
//!    `the_tolerance_rejects…` builds the plausible-but-wrong renders (a panel shifted 1 pt, a
//!    frame border missing, a gutter 2 pt narrow) and requires the golden's own thresholds to
//!    red them. It is the guard against the one failure mode a golden really has: widening the
//!    numbers until nothing can fail it.
//! 4. **The golden is reviewable**: `tests/golden/README.md` and the file's own doc comment carry
//!    the regeneration command, and `UPDATE_GOLDEN=1` rewrites the PNG instead of comparing.
//!
//! # The page
//!
//! One 612×792 pt page (`comic.page.create`), a `grid-2x3` template at a 24 pt margin and a 14 pt
//! gutter (`comic.page.template.apply`), three panels carrying a linked PNG plate
//! (`comic.panel.replace_art`, roles `primary`/`primary`/`background`), one panel resized
//! (`comic.panel.resize`), one moved (`comic.panel.move`) and the reading order set to a
//! non-creation order (`comic.reading_order.set`). Every one of those is a step a creator takes in
//! M2, and every one of them reaches the pixels.
//!
//! # Regenerating the golden
//!
//! ```text
//! UPDATE_GOLDEN=1 cargo test -p vectorcraft-engine --test comic_golden_render
//! ```
//!
//! Update it only when the **rendering of this page has deliberately changed** — a template, a
//! frame, a gutter or a rasteriser behaviour that was reviewed as a fix. Never update it to make a
//! red test green without saying which production change moved the pixels; that is the difference
//! between a golden and a rubber stamp. The review that updates it should quote
//! `docs/reviews/M2-golden-render.md`'s tolerance section in the commit message.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::too_many_lines)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vectorcraft_comic_layout::lettering::{Face, style_of};
use vectorcraft_comic_model::{ApprovalStatus, Asset, AssetId, AssetType, ContentHash, MediaMetadata, StorageRef, sha256_hex};
use vectorcraft_doc::{Document, NodeId, NodeKind};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;
use vectorcraft_geom::{Affine, Rect, Vec2};
use vectorcraft_testkit::raster::{self, Image, perceptual_diff, render_region};

// ---------- the harness ----------

/// One pixel per document point. 612×792 for this page: small enough to review by eye, and a point
/// of the page is a whole pixel, which is what makes a 1 pt geometry error visible (see the
/// tolerance below).
const GOLDEN_SCALE: f64 = 1.0;

/// Per-pixel perceptual tolerance, in 0..255 luminance-weighted difference after a 1 px box blur
/// (see `testkit::raster::perceptual_diff`). Artwork edges anti-alias, so the comparison is
/// perceptual rather than exact; 8 is below the ~20-per-channel difference a half-pixel edge
/// shift makes, and far below the 255 a black stroke against white makes.
const TOL: f64 = 8.0;

/// At most this fraction of pixels may exceed [`TOL`]. 0.001 of 484 704 pixels is 485 pixels —
/// about 7% of the page's ink edge pixels, which is what a sub-pixel anti-aliasing drift between
/// rasteriser builds could disturb. It is the smallest bound measured to still reject every
/// mutation in `the_tolerance_rejects…`; the numbers it was chosen from, and that a later widening
/// has to face, are printed by that test:
///
/// | wrong renderer | pixels over [`TOL`] | mean |
/// |---|---|---|
/// | every frame shifted 1 pt | 3.278% (15 888) | 2.842 |
/// | every frame shifted 3 pt | 2.537% (12 296) | 2.146 |
/// | every frame border removed | 4.780% (23 167) | 2.996 |
/// | every panel 1 pt wider (a 2 pt gutter error) | 4.996% (24 216) | 4.044 |
///
/// So the smallest of them is 25× this bound. A re-render in the same environment scores 0 pixels
/// and a mean of 0.000: the slack is not for the renderer we measure, it is for another build of it.
const MAX_BAD_FRACTION: f64 = 0.001;

/// A bound on the *mean* absolute difference over every channel of every pixel — the second half of
/// the tolerance. A percentage-of-bad-pixels bound misses a thin difference spread over the whole
/// page (every edge a shade lighter, everything off by a fraction of a pixel); the mean does not,
/// and it is what makes "the whole page drifted a little" a failure rather than a rounding.
/// Measured: the mutations above score 2.146–4.044 and a re-render scores 0.000, so 0.5 is 4× below
/// the mildest wrong renderer measured.
const MAX_MEAN: f64 = 0.5;

/// What happened to one golden.
#[derive(Clone, Debug, PartialEq, Eq)]
enum GoldenOutcome {
    /// The render matched the checked-in golden.
    Matched,
    /// `UPDATE_GOLDEN` was set and the golden was rewritten from this render.
    Wrote,
    /// The comparison could not be made here, for the reason given: **not** a pass.
    Skipped(String),
}

/// The directory the goldens live in — beside the test, not under `docs/` (`cargo xtask brands`
/// scans non-Markdown files there, and a golden is test data, not documentation).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Whether `UPDATE_GOLDEN` asks for the goldens to be rewritten rather than compared.
fn updating() -> bool {
    std::env::var("UPDATE_GOLDEN").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The command that regenerates every golden in this file, quoted in every failure and skip message
/// so the reader of a red test never has to search for it.
const REGENERATE: &str = "UPDATE_GOLDEN=1 cargo test -p vectorcraft-engine --test comic_golden_render";

/// Compare `rendered` with the golden named `file`, or write it when `UPDATE_GOLDEN` is set.
///
/// Panics when the golden is absent and no update was asked for: a missing reference is a broken
/// test, not a skip — the *pixels* are always renderable here, and only the *face* can be missing
/// (that is [`compare_lettering_golden`]'s job).
#[track_caller]
fn compare_to_golden(file: &str, rendered: &Image, note: &str) -> GoldenOutcome {
    let path = golden_dir().join(file);
    if updating() {
        std::fs::create_dir_all(golden_dir()).expect("create the golden directory");
        rendered.save_png(&path);
        println!("UPDATE_GOLDEN: wrote {} ({note})", path.display());
        return GoldenOutcome::Wrote;
    }
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("the golden {} is not readable ({e}); regenerate it with `{REGENERATE}`", path.display()));
    let golden = Image::from_png(&bytes).unwrap_or_else(|e| panic!("the golden {} is not a PNG ({e})", path.display()));
    // The sanctioned comparison first, so this file uses the tooling the milestone names rather
    // than a private copy of it.
    raster::assert_similar(&golden, rendered, TOL, MAX_BAD_FRACTION);
    // Then the bound that catches a difference spread thinly over the whole page, which a
    // percentage-of-bad-pixels cannot see. Its message carries both numbers.
    let stats = perceptual_diff(&golden, rendered, TOL);
    assert!(
        stats.mean <= MAX_MEAN,
        "{} differs from the render by a mean of {:.3} per channel over the whole page (limit {MAX_MEAN}), with {} pixels over {TOL} ({:.4}%): the pixels moved everywhere, a little. {note}. If the change is deliberate, review it and regenerate with `{REGENERATE}`",
        path.display(),
        stats.mean,
        stats.bad_pixels,
        stats.bad_pixels as f64 / stats.pixels.max(1) as f64 * 100.0
    );
    GoldenOutcome::Matched
}

/// The decision the lettered golden path makes for `face`, as a decision rather than a comparison:
/// `Ok(file)` names the reference to compare against, `Err(reason)` says why it cannot be compared.
///
/// The reference **names the face** (`…-comic-neue-regular.png`), so a page lettered in Comic Neue
/// is never compared against one lettered in whatever stood in for it. The order is the contract:
///
/// 1. the face is **not in this build** → `Err`. Nothing is compared and nothing is written, even
///    under `UPDATE_GOLDEN`: a reference filed under a face must show that face;
/// 2. the face **is** there → `Ok` with the face-keyed file, which `compare_to_golden` then
///    compares — or writes, under `UPDATE_GOLDEN` — and reports as missing, with the command that
///    records one, if no reference has been recorded for that face yet.
fn lettering_golden_path(name: &str, face: &Face) -> Result<String, String> {
    let missing = vectorcraft_comic_layout::lettering::missing_faces(vectorcraft_text::FontDb::global(), std::slice::from_ref(face));
    let Some(gone) = missing.first() else {
        return Ok(format!("{name}-{}.png", face_slug(face)));
    };
    let shown = match &gone.resolved {
        Some((family, style)) => format!("{family} {style}"),
        None => "the fallback face".to_string(),
    };
    let held = if vectorcraft_text::craft_fonts::comic_fonts().is_empty() {
        "CRAFT_FONTS is empty, so this build has no craft-fonts comic face at all".to_string()
    } else {
        format!("this build holds {} craft-fonts comic face(s), none of them it", vectorcraft_text::craft_fonts::comic_fonts().len())
    };
    Err(format!(
        "the lettering face {} {} is not available ({}), so the page is lettered in {shown} instead of it: the balloon's pixels are a function of the text's measured widths, and that measurement comes from the face. {held}; the comic faces belong in storytold/craft-fonts, which reaches the build through CRAFT_FONTS_DIR. A golden under `{name}-{}.png` would record the wrong face, so this comparison cannot run here — it is a skip, not a pass",
        face.family,
        face.style,
        gone.status.as_str(),
        face_slug(face),
    ))
}

/// The lettered page against its golden, through [`lettering_golden_path`]: a comparison when the
/// face is here, a **skip that names the face** when it is not.
///
/// A reference for a face is deliberately not committed: the comic faces are not in this tree, so
/// with the face present and no reference recorded for it, this **skips** and quotes
/// [`REGENERATE`] rather than failing. A reference that *is* recorded must match: once the file is there, this is the
/// same strict comparison the geometry golden gets.
fn compare_lettering_golden(name: &str, rendered: &Image, face: &Face) -> GoldenOutcome {
    let file = match lettering_golden_path(name, face) {
        Ok(file) => file,
        Err(reason) => return GoldenOutcome::Skipped(reason),
    };
    let path = golden_dir().join(&file);
    if !updating() && !path.is_file() {
        return GoldenOutcome::Skipped(format!(
            "no golden is recorded for {} {} (`{}`), so there is nothing to compare this lettered page against here; record one with `{REGENERATE}` on a build that has that face",
            face.family,
            face.style,
            path.display()
        ));
    }
    compare_to_golden(&file, rendered, &format!("the lettered page, in {} {}", face.family, face.style))
}

/// A face's family and style as a file-name component: `Comic Neue` `Regular` → `comic-neue-regular`.
fn face_slug(face: &Face) -> String {
    format!("{}-{}", face.family, face.style).to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "-")
}

/// The document of an open session.
fn doc_of(s: &Session) -> std::sync::Arc<Document> {
    s.doc().expect("a document").doc.clone()
}

/// The open comic project.
fn comic_of(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn comic_of_mut(s: &mut Session) -> &mut OpenComic {
    std::sync::Arc::make_mut(s.doc_mut().expect("a document").comic.as_mut().expect("an open project"))
}

/// A scratch directory inside the workspace's `target/`: never a system temporary directory and
/// never tracked, so the `assets` and `brands` gates never see a test bundle.
fn scratch(tag: &str) -> PathBuf {
    let base =
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-golden").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn exec(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, &params).unwrap_or_else(|e| panic!("{id} rejected {params}: {e}"))
}

// ---------- the representative page ----------

/// The representative page's session and the ids a test asks the records and the pixels about.
struct Page {
    session: Session,
    page: u64,
    rect: Rect,
    panels: Vec<u64>,
}

impl Page {
    fn doc(&self) -> std::sync::Arc<Document> {
        doc_of(&self.session)
    }
    fn render(&self) -> Image {
        render_region(&self.doc(), self.rect, GOLDEN_SCALE)
    }
    fn frame_bounds(&self, panel: u64) -> Rect {
        let frame = self.frame_node(panel);
        self.doc().node(frame).and_then(|n| n.geometric_bounds()).expect("the frame has geometry")
    }
    fn frame_node(&self, panel: u64) -> NodeId {
        comic_of(&self.session).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == panel).expect("the panel record").frame_node_id
    }
}

/// Build the representative page: a template's panels, artwork in three of them, a resize, a move
/// and an explicit reading order — and **no lettering**, so that nothing on it can depend on a
/// comic face (see the file comment, and the test that enforces it).
fn representative_page(tag: &str) -> Page {
    let root = scratch(tag).join("book.comiccraft");
    let mut s = Session::new();
    exec(&mut s, "comic.project.create", json!({"path": root.to_string_lossy(), "title": "Golden render", "pageSize": [612.0, 792.0]}));
    let page = exec(&mut s, "comic.page.create", json!({"size": [612.0, 792.0]}))["page"].as_u64().expect("a page id");
    let panels: Vec<u64> =
        exec(&mut s, "comic.page.template.apply", json!({"page": page, "template": "grid-2x3", "margin": 24.0, "gutter": 14.0}))["panels"]
            .as_array()
            .expect("six template slots")
            .iter()
            .map(|p| p["panel"].as_u64().expect("a panel id"))
            .collect();
    assert_eq!(panels.len(), 6, "a grid-2x3 page has six panels");

    // Artwork in three panels, in two roles: the plates are geometry as far as a golden is concerned
    // (a decoded PNG, not a face), and they are what makes the page a comic page rather than a box
    // chart. Two `primary` plates side by side, and one `background` behind nothing.
    for (panel, role, rgb, name) in [
        (panels[0], "primary", [0xd9, 0x3a, 0x2b], "red"),
        (panels[1], "primary", [0x2b, 0x8c, 0xd9], "blue"),
        (panels[4], "background", [0x8a, 0x8a, 0x8a], "grey"),
    ] {
        let asset = add_linked_png(&mut s, &root, &format!("assets/panels/{name}.png"), rgb);
        exec(&mut s, "comic.panel.replace_art", json!({"panel": panel, "asset": asset, "role": role}));
    }

    // A resize and a move, the two geometry edits M2's first criterion names. Both move *inward*, so
    // the page still validates: the golden is of a page a creator would keep, not of a broken one.
    let bound = |panel: u64| {
        comic_of(&s)
            .bundle
            .pages
            .values()
            .flat_map(|f| f.panels.iter())
            .find(|p| p.id.0 == panel)
            .and_then(|p| doc_of(&s).node(p.frame_node_id).and_then(|n| n.geometric_bounds()))
            .expect("a panel frame rectangle")
    };
    let r = bound(panels[2]);
    exec(&mut s, "comic.panel.resize", json!({"panel": panels[2], "rect": [r.x0 + 8.0, r.y0 + 8.0, r.x1 - 8.0, r.y1 - 8.0]}));
    exec(&mut s, "comic.panel.move", json!({"panel": panels[5], "dx": -4.0, "dy": -6.0}));
    // The reading order is explicit and is *not* creation order: a record the golden does not show
    // (design §5.1.1) and the record-level half of the page asserts.
    let reading: Vec<u64> = [0usize, 3, 1, 4, 2, 5].iter().map(|i| panels[*i]).collect();
    exec(&mut s, "comic.reading_order.set", json!({"page": page, "order": reading}));

    let rect = doc_of(&s).artboards[0].rect;
    Page { session: s, page, rect, panels }
}

/// A PNG under the bundle plus the `Linked` `Asset` record naming it. There is no `comic.asset.*`
/// command (M4), so the record is added to the open model exactly as M1's acceptance suite adds it
/// and as W1's fixture does; every other step goes through `Session::execute`.
fn add_linked_png(s: &mut Session, root: &Path, relative: &str, rgb: [u8; 3]) -> u64 {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the assets directory");
    image::RgbaImage::from_pixel(64, 64, image::Rgba([rgb[0], rgb[1], rgb[2], 255])).save(&path).expect("write the png");
    let bytes = std::fs::read(&path).expect("read the png");
    let oc: &mut OpenComic = comic_of_mut(s);
    let id = oc.bundle.allocate::<AssetId>().expect("an asset id");
    oc.bundle.assets.assets.push(Asset {
        id,
        asset_type: AssetType::Source,
        storage_reference: StorageRef::Linked { relative_path: relative.to_string() },
        content_hash: ContentHash { algorithm: "sha256".into(), value: sha256_hex(&bytes) },
        media_metadata: MediaMetadata { mime: "image/png".into(), width: 64, height: 64, color_profile: None, dpi: None },
        provenance: None,
        parent_asset_ids: Vec::new(),
        approval_status: ApprovalStatus::Pending,
        unknown: Default::default(),
    });
    id.0
}

/// Every record of the open project as one comparable value, for the claim that a
/// document-only geometry change is invisible to each of them. `ComicBundle` itself is not one
/// serialised value (it is a directory of files), so the parts are compared.
fn records_digest(s: &Session) -> Value {
    let b = &comic_of(s).bundle;
    json!({
        "pages": serde_json::to_value(&b.pages).expect("the page files serialise"),
        "assets": serde_json::to_value(&b.assets).expect("the asset records serialise"),
        "project": serde_json::to_value(&b.project).expect("the project record serialises"),
        "manifest": serde_json::to_value(&b.manifest).expect("the manifest serialises"),
    })
}

/// The number of errors `comic.project.validate` reports for the open project.
fn validation_errors(s: &mut Session) -> u64 {
    exec(s, "comic.project.validate", json!({}))["errors"].as_u64().expect("an error count")
}

/// Shift every panel frame of the **session's own document** by `delta` — what a wrong renderer does
/// in place, rather than to a copy.
fn shift_every_frame_in_place(s: &mut Session, delta: Vec2) {
    for frame in frames_of(&doc_of(s)) {
        if let Some(n) = std::sync::Arc::make_mut(&mut s.doc_mut().expect("a document").doc).node_mut(frame) {
            n.transform(Affine::translate(delta), false);
        }
    }
}

// ---------- the golden ----------

/// The representative page against its checked-in golden (M2 exit criteria, criterion 6).
#[test]
fn the_representative_page_matches_the_geometry_golden() {
    let page = representative_page("golden");
    let rendered = page.render();
    assert_eq!((rendered.width, rendered.height), (612, 792), "the render is the page's size in points at {GOLDEN_SCALE} px/pt");
    assert!(rendered.ink() > 10_000, "the page renders its panels, artwork and borders: {} ink pixels", rendered.ink());
    let outcome = compare_to_golden("comic-page.png", &rendered, "the representative page: a grid-2x3 template, artwork, a resize and a move");
    assert_ne!(outcome, GoldenOutcome::Skipped(String::new()), "a page with no lettering can always be rendered: it must never skip");
}

/// **The font trap's answer, enforced.** The representative page holds nothing type can be set in,
/// so no pixel of it can depend on whether `storytold/craft-fonts` is present — which is what makes
/// this golden comparable in a build with `CRAFT_FONTS` empty **and** in one without.
///
/// If this fails, the golden has silently become font-dependent: the page gained a text object (or
/// a placed document, whose own art may hold type), and with `CRAFT_FONTS` empty it would be
/// rendered in the fallback face and compare unequal to a golden captured with the comic face — a
/// failure that is not a defect. Split the golden instead: assert the words at record level, as the
/// lettered-page test does, or move the pixels to the face-keyed path in
/// [`compare_lettering_golden`].
#[test]
fn the_golden_page_renders_no_text_so_its_pixels_cannot_depend_on_a_face() {
    let page = representative_page("no-text");
    let mut typed: Vec<(u64, Option<String>, &str)> = Vec::new();
    page.doc().walk(|n| match &n.kind {
        NodeKind::Text(_) => typed.push((n.id.0, n.name.clone(), "a text object")),
        NodeKind::PlacedDocument(_) => typed.push((n.id.0, n.name.clone(), "a placed document, whose own art may hold type")),
        _ => {}
    });
    assert!(
        typed.is_empty(),
        "the golden page must hold nothing type can be set in, or its pixels would depend on the lettering face; found {typed:?}"
    );
}

/// **The golden can fail**, and it is the *only* thing here that can see what it catches.
///
/// Every mutation below is a plausible-but-wrong renderer, and every one of them is a *geometry*
/// error: `Panel` has no geometry (schema §6.2), the document owns the frame rectangles, so a frame
/// that moved a point, lost its border or narrowed its gutter leaves every record — and the
/// project's own validation report — exactly as they were. Three parts:
///
/// 1. each mutation is built and the golden's own thresholds must **reject** it;
/// 2. the score of each is printed, so the report can quote the numbers the tolerance is chosen
///    from and a later widening has to face them;
/// 3. one of the mutations is then applied **in place**, to the session's document, and the
///    record-level evidence (every record, and `comic.project.validate`) is asserted to be
///    **unchanged** while the golden is no longer satisfied. That is the whole argument for a
///    golden: without it, this defect would be certifiable as a pass.
///
/// Without this test the tolerance could be widened until nothing failed it, and a golden that
/// cannot fail does not verify anything. It is deliberately written as an assertion *on the
/// tolerance*, so widening [`TOL`] or [`MAX_BAD_FRACTION`] to accommodate a regression has to red
/// this test first, in a test named for what it is doing.
#[test]
fn the_tolerance_rejects_a_panel_shift_a_missing_border_and_a_narrowed_gutter_no_record_check_can_see() {
    let mut page = representative_page("mutation");
    let golden = page.render();
    let records_before = records_digest(&page.session);
    let errors_before = validation_errors(&mut page.session);

    let mutations: Vec<(&str, Document)> = vec![
        ("every frame shifted 1 pt", mutate_frames(&page, |_, n| n.transform(Affine::translate(Vec2::new(1.0, 0.0)), false))),
        ("every frame shifted 3 pt", mutate_frames(&page, |_, n| n.transform(Affine::translate(Vec2::new(0.0, 3.0)), false))),
        (
            "every frame border removed",
            mutate_frames(&page, |_, n| n.appearance.items.retain(|i| !matches!(i, vectorcraft_doc::AppearanceItem::Stroke(_)))),
        ),
        ("every panel 1 pt wider on each side (a 2 pt narrow gutter)", mutate_frames(&page, |b, n| n.transform(grow(b, 1.0), false))),
    ];
    for (label, doc) in mutations {
        let rendered = render_region(&doc, page.rect, GOLDEN_SCALE);
        let stats = perceptual_diff(&golden, &rendered, TOL);
        let fraction = stats.bad_pixels as f64 / stats.pixels.max(1) as f64;
        println!("{label}: {} bad pixels ({:.4}%), mean {:.3}, max {}", stats.bad_pixels, fraction * 100.0, stats.mean, stats.max);
        assert!(
            fraction > MAX_BAD_FRACTION,
            "the golden accepts `{label}`: {:.4}% of pixels differ, the bound is {:.4}%. A wrong renderer the golden cannot red is a certification, not a test",
            fraction * 100.0,
            MAX_BAD_FRACTION * 100.0
        );
        assert!(stats.mean > MAX_MEAN, "`{label}` is caught by the bad-pixel bound but not by the mean bound ({:.3} <= {MAX_MEAN})", stats.mean);
    }

    // The same mutation, applied where a wrong implementation would apply it: to the document the
    // session holds. Every record and the validation report are blind to it.
    shift_every_frame_in_place(&mut page.session, Vec2::new(1.0, 0.0));
    let shifted = page.render();
    let stats = perceptual_diff(&golden, &shifted, TOL);
    assert!(
        stats.bad_pixels as f64 / stats.pixels as f64 > MAX_BAD_FRACTION,
        "the shifted page must not satisfy the golden ({:.4}% differ)",
        stats.bad_pixels as f64 / stats.pixels as f64 * 100.0
    );
    assert_eq!(
        records_digest(&page.session),
        records_before,
        "no record moved: schema §6.2 gives Panel no geometry, so a 1 pt shift of every frame is invisible to every record-level assertion"
    );
    assert_eq!(validation_errors(&mut page.session), errors_before, "and `comic.project.validate` reports the same errors as before the shift");
}

/// The page's document with every panel frame handed to `f`, together with the frame's bounds as
/// they were — the shape a plausible wrong renderer takes: the frames are built wrongly and nothing
/// else changes.
fn mutate_frames(page: &Page, f: impl Fn(&Rect, &mut vectorcraft_doc::Node)) -> Document {
    let mut doc = (*page.doc()).clone();
    for frame in frames_of(&doc) {
        let Some(b) = doc.node(frame).and_then(|n| n.geometric_bounds()) else { continue };
        if let Some(n) = doc.node_mut(frame) {
            f(&b, n);
        }
    }
    doc
}

/// The transform that grows a frame by `by` points on each side, in document space: a gutter
/// `2 * by` points too narrow, the classic template arithmetic error.
fn grow(b: &Rect, by: f64) -> Affine {
    let (sx, sy) = ((b.width() + 2.0 * by) / b.width(), (b.height() + 2.0 * by) / b.height());
    Affine::new([sx, 0.0, 0.0, sy, b.x0 * (1.0 - sx) - by, b.y0 * (1.0 - sy) - by])
}

/// Every panel frame node of the document — the nodes `comic.panel.add` names `Panel frame`.
fn frames_of(doc: &Document) -> Vec<NodeId> {
    let mut out = Vec::new();
    doc.walk(|n| {
        if n.name.as_deref() == Some("Panel frame") {
            out.push(n.id);
        }
    });
    out
}

// ---------- the lettering half: text at record level, pixels behind a face-keyed golden ----------

/// The representative page with lettering on it — the same page, one step further: a speech balloon
/// and a narration caption, which is what M2's page actually looks like.
fn lettered_page(tag: &str) -> (Page, Value, u64) {
    let mut page = representative_page(tag);
    let words = "Hold the line, Jen.";
    let balloon =
        exec(&mut page.session, "comic.lettering.add", json!({"page": page.page, "panel": page.panels[0], "kind": "speech", "text": words}));
    exec(
        &mut page.session,
        "comic.lettering.add",
        json!({"page": page.page, "panel": page.panels[3], "kind": "narration", "text": "Later that night."}),
    );
    exec(&mut page.session, "comic.panel.set_shot", json!({"panel": page.panels[2], "shot": "Low angle, rain"}));
    let lettering = balloon["lettering"].as_u64().expect("a lettering id");
    (page, balloon, lettering)
}

fn lettering_record(s: &Session, id: u64) -> vectorcraft_comic_model::Lettering {
    comic_of(s).bundle.pages.values().flat_map(|f| f.lettering.iter()).find(|l| l.id.0 == id).cloned().expect("the lettering record")
}

fn dialogue_record(s: &Session, id: u64) -> vectorcraft_comic_model::Dialogue {
    comic_of(s).bundle.pages.values().flat_map(|f| f.dialogue.iter()).find(|d| d.id.0 == id).cloned().expect("the dialogue record")
}

fn panel_of(s: &Session, id: u64) -> vectorcraft_comic_model::Panel {
    comic_of(s).bundle.pages.values().flat_map(|f| f.panels.iter()).find(|p| p.id.0 == id).cloned().expect("the panel record")
}

fn text_of(s: &Session, id: u64) -> String {
    match &doc_of(s).node(NodeId(id)).unwrap_or_else(|| panic!("NodeId({id}) is not in the document")).kind {
        NodeKind::Text(t) => t.plain_text(),
        other => panic!("NodeId({id}) is not a text object: {other:?}"),
    }
}

/// The lettering on the representative page is asserted **at record level** (the half M1 owns), and
/// the pixels that would depend on the face are handed to [`compare_lettering_golden`] instead.
#[test]
fn the_lettered_page_asserts_its_text_at_record_level_and_differs_from_the_geometry_golden() {
    let geometry = representative_page("lettered-geometry").render();
    let (mut page, added, lettering) = lettered_page("lettered-records");

    // **The words**: the record owns them (rule 8) and the text object shows the same words.
    let record = lettering_record(&page.session, lettering);
    let dialogue = added["dialogue"].as_u64().expect("a spoken kind makes a dialogue");
    assert_eq!(dialogue_record(&page.session, dialogue).text, "Hold the line, Jen.", "the dialogue owns the words");
    assert_eq!(
        panel_of(&page.session, page.panels[0]).dialogue_ids.iter().map(|d| d.0).collect::<Vec<u64>>(),
        vec![dialogue],
        "and the panel that owns it lists it"
    );
    assert_eq!(
        (record.kind, record.text.as_deref(), record.dialogue_id.map(|d| d.0)),
        (vectorcraft_comic_model::LetteringKind::Speech, None, Some(dialogue)),
        "a spoken kind carries no words of its own"
    );
    assert_eq!(
        text_of(&page.session, added["text_node"].as_u64().expect("the text object")),
        "Hold the line, Jen.",
        "the frame's text object shows them"
    );

    // **The geometry**: a frame path, a tail path for a spoken kind, and the text object — and the
    // balloon sits inside the panel it belongs to.
    let document = doc_of(&page.session);
    let frame = document.node(record.frame_node_id).expect("the frame object");
    assert!(matches!(frame.kind, NodeKind::Path { .. }), "a balloon's outline is an editable path");
    let tail = record.tail_node_id.expect("a speech balloon has a tail (schema §6.4)");
    assert!(matches!(document.node(tail).expect("the tail object").kind, NodeKind::Path { .. }));
    let panel_bounds = page.frame_bounds(page.panels[0]);
    let balloon_bounds = frame.geometric_bounds().expect("the frame has geometry");
    assert!(panel_bounds.contains_rect(balloon_bounds), "the balloon is inside its panel: {balloon_bounds:?} vs {panel_bounds:?}");

    // **The missing face is reported**, which is the same report the golden's skip reason is built
    // from: when the comic faces are not in the build, the command's findings name the face the
    // project asked for rather than substituting it silently.
    let face = style_of(record.kind).face();
    if vectorcraft_text::craft_fonts::comic_fonts().is_empty() {
        let report = format!("{} {}", added, exec(&mut page.session, "comic.project.validate", json!({})));
        assert!(
            report.contains(&face.family),
            "with CRAFT_FONTS empty the report must name {} rather than fall back silently; report was {report}",
            face.family
        );
    }

    // **And the pixels show it**: the lettered page is not the geometry page, so the geometry golden
    // cannot stand in for it — which is exactly why the lettering has its own, face-keyed path.
    let lettered = page.render();
    let stats = perceptual_diff(&geometry, &lettered, TOL);
    assert!(stats.bad_pixels > 0, "the lettering is in the pixels, so the lettered page is a different golden");
    println!("the lettering changes {} pixels ({:.3}%) of the page", stats.bad_pixels, stats.bad_pixels as f64 / stats.pixels as f64 * 100.0);
}

/// **The skip contract, both branches.** A golden whose pixels depend on a comic face cannot be
/// compared when the face is not in the build: `CRAFT_FONTS` is empty here, and the comic faces live
/// in `storytold/craft-fonts`. What must happen is a skip that **names the face**, the status and
/// what stands in for it — never a pass, never a failure that reads as a defect in the renderer.
///
/// The other branch is asserted through the *decision*, not by writing a file, so that this test
/// says the same thing in a build **with** the faces: a face that is present resolves to a reference
/// **keyed by the face that is present**, and the two paths are different files. That is the part
/// that must hold when `CRAFT_FONTS` is set — the gate can never compare one face's pixels against
/// another face's reference, and it can never fail because the faces are there.
#[test]
fn the_lettered_golden_path_is_keyed_by_the_face_and_skips_naming_the_missing_comic_face() {
    let face = style_of(vectorcraft_comic_model::LetteringKind::Speech).face();
    // The bundled body face is in every build (`testkit::fonts` renames it; `comic-layout`
    // `lettering::fonts`'s own tests assert it resolves exactly), so this is the "face is here"
    // branch in any environment.
    let present = Face::new(vectorcraft_text::FALLBACK_FAMILY, "Regular");

    match lettering_golden_path("comic-page-lettered", &face) {
        Err(reason) => {
            assert!(reason.contains(&face.family), "a skip must name the family the project asked for: {reason}");
            assert!(reason.contains(&face.style), "and the style: {reason}");
            assert!(reason.contains("craft-fonts"), "and where the face belongs: {reason}");
            assert!(reason.contains("skip, not a pass"), "the report must be explicit that this comparison did not run: {reason}");
            println!("SKIPPED (not passed): {reason}");
        }
        Ok(file) => println!("{} {} is in this build, so the lettered golden is {file}", face.family, face.style),
    }

    let here = lettering_golden_path("comic-page-lettered", &present).expect("a bundled face is in every build");
    assert!(here.starts_with("comic-page-lettered-source-sans-3-regular"), "the reference is keyed by the face that is present: {here}");
    assert_ne!(lettering_golden_path("comic-page-lettered", &face).ok(), Some(here.clone()), "and two different faces never share one reference");

    // The end-to-end path, so the decision above is not the only thing exercised. Which branch it
    // takes depends on whether this build has the comic faces, so the test says what it is entitled
    // to say in each — the point being that neither branch can be a quiet pass or a false failure.
    let face_here = vectorcraft_comic_layout::lettering::missing_faces(vectorcraft_text::FontDb::global(), std::slice::from_ref(&face)).is_empty();
    let (page, _added, _lettering) = lettered_page("lettered-golden");
    let outcome = compare_lettering_golden("comic-page-lettered", &page.render(), &face);
    match (&outcome, face_here) {
        (GoldenOutcome::Skipped(reason), false) => {
            assert!(reason.contains(&face.family) && reason.contains(&face.style), "the skip must name the face: {reason}");
            assert!(
                reason.contains("craft-fonts") && reason.contains("skip, not a pass"),
                "and say where the face belongs and that it did not run: {reason}"
            );
            println!("SKIPPED (not passed): {reason}");
        }
        (GoldenOutcome::Skipped(reason), true) => {
            assert!(
                reason.contains("no golden is recorded") && reason.contains("UPDATE_GOLDEN"),
                "with the face present and no reference recorded, the skip must say how to record one: {reason}"
            );
            println!("SKIPPED (not passed): {reason}");
        }
        (GoldenOutcome::Matched, true) => println!("the lettered golden was compared: {} {} is in this build", face.family, face.style),
        (GoldenOutcome::Wrote, true) => println!("the lettered golden was rewritten (UPDATE_GOLDEN): {} {}", face.family, face.style),
        (other, _) => panic!("with CRAFT_FONTS empty the lettered golden cannot be compared, but the harness returned {other:?}"),
    }
}
