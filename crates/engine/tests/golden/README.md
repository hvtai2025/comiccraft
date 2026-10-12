# The golden renders

The PNGs in this directory are the reference images of `tests/comic_golden_render.rs` (M2 W2). Each is
rendered at **1 pixel per document point** from the fixture that test builds, and compared with
`testkit::raster`'s perceptual comparison.

| File | What it shows | The test that compares it |
|---|---|---|
| `comic-page.png` | the representative comic page: a `grid-2x3` template at a 24 pt margin and 14 pt gutter, three panels carrying a linked PNG plate (`primary`, `primary`, `background`), one panel resized, one moved, and an explicit reading order — **no lettering**, so no pixel of it depends on a comic face | `the_representative_page_matches_the_geometry_golden` |

A golden whose pixels **do** depend on a comic face is named after that face
(`comic-page-lettered-comic-neue-regular.png`) and is compared only when this build has that face. None is
committed, because the comic faces live in `storytold/craft-fonts` and are not in this tree — with
`CRAFT_FONTS` empty the lettering path **skips** with a reason that names the missing face instead of
comparing a fallback-face render against a face-keyed reference.

## Regenerating

```sh
UPDATE_GOLDEN=1 cargo test -p vectorcraft-engine --test comic_golden_render
```

That rewrites `comic-page.png` (and any face-keyed golden whose face this build has) from a fresh render
and reports what it wrote. `UPDATE_GOLDEN` never writes a **face-keyed** golden in a build without that
face: a reference filed under Comic Neue must show Comic Neue.

## When updating one is legitimate

Only when the rendering of that page has **deliberately** changed in a production file — a template
rectangle, a frame, a gutter, a stroke or a rasteriser behaviour — and the change has been reviewed as a
fix (or an intended change). The updated PNG then belongs in the same change as the code, with the
production change named in the message.

It is **not** legitimate to regenerate a golden to make a red test green when nothing in the production
tree explains the difference, and it is not legitimate to widen the tolerance for that either: the
comparison's numbers are pinned by `the_tolerance_rejects_a_panel_shift_a_missing_border_and_a_narrowed_gutter`,
which requires the tolerance to keep rejecting a 1 pt panel shift, a missing frame border and a 2 pt
gutter error. If that test reds, the tolerance — not the golden — is what needs the explanation.

## Scope

The golden verifies the **page's geometry and placed artwork** as the renderer draws them. It does not
verify the words in a balloon (asserted at record level by the same test file and by M1's acceptance
suite), and it does not verify a page's lettering pixels without the lettering face present. See
`docs/reviews/M2-golden-render.md` for the tolerance's measurements, the mutations the golden rejects,
and the skip contract.
