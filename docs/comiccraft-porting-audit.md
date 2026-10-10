# VectorCraft Baseline Audit

**Document type:** Repository audit and capability map (Phase 0 / M0 deliverable)
**Status:** Complete — baseline verified by reading the source
**Audited revision:** `storytold/vectorcraft` @ `3da4080b` (`main`, clean working tree)
**Companion documents:** [`comiccraft-design.md`](comiccraft-design.md), [`comiccraft-porting-plan.md`](comiccraft-porting-plan.md)

---

## 1. Scope and method

Every claim below was read from the source at the audited revision. Citations are
`path:line` relative to the VectorCraft repository root. Nothing in this document is
inferred from documentation alone, and no API is described here that was not found in
code.

Sections 4–9 state what exists. Section 10 is the **capability reuse matrix** that
`porting_plan.md` §3 and `comiccraft_design.md` §4 depend on. Section 11 records the
gaps and hazards that change the design.

---

## 2. Baseline

| Fact | Value | Citation |
|---|---|---|
| Repository | `https://github.com/storytold/vectorcraft` | `Cargo.toml:16` |
| Audited commit | `3da4080b` — *Merge pull request #935 from storytold/fix/918-place-psd* | `git log -1` |
| Branch / tree | `main`, no uncommitted changes | `git status` |
| Workspace version | `0.8.0` | `Cargo.toml:12` |
| Edition / MSRV | edition 2024, `rust-version = 1.95` | `Cargo.toml:13-14` |
| Licence | MIT OR Apache-2.0 | `Cargo.toml:15` |
| Resolver | 3 | `Cargo.toml:4` |
| Members | `crates/*`, `apps/*`, `xtask` | `Cargo.toml:5` |
| Default members | `crates/*`, `apps/vectorcraft`, `apps/vectorcraft-cli`, `xtask` (**the web app is not a default member**) | `Cargo.toml:6` |

CI gate (`cargo xtask ci`) runs, stopping at the first failure:
`fmt` → `clippy` → `test` → `assets` → `brands` → `layers` → `wasm`
(`xtask/src/main.rs:175-205`).

`plan/` is gitignored local session state (`AGENTS.md:12`); it is not part of the
repository.

---

## 3. Workspace and layering

Crates and applications at the baseline:

```text
crates/  affinity  brush  cad  color  doc  effects  engine  eps  format  geom  mcp
         metafile  pathops  pdf  plugins  render  svg  testkit  text  tools  trace  ui-egui
apps/    vectorcraft (desktop, eframe)   vectorcraft-cli   vectorcraft-web (wasm, Trunk)
xtask/
```

`cargo xtask layers` enforces the dependency order and the "no UI below L6" rule
(`xtask/src/layers.rs:9-57`):

| Layer | Crates |
|---|---|
| L0 | `geom`, `color` |
| L1 | `doc` |
| L2 | `pathops`, `brush`, `trace`, `text`, `effects`, `plugins` |
| L3 | `render`, `svg`, `pdf`, `format`, `cad`, `eps`, `metafile` |
| L4 | `tools` |
| L5 | `engine` |
| L6 | `ui-egui`, `mcp` |
| special | `testkit` (dev-dependency only) |
| standalone | `affinity` (may not depend on workspace crates) |
| exempt | `apps/*`, `xtask` |

Enforced rules: no upward dependencies, standalone crates depend on nothing,
`vectorcraft-testkit` only as a dev-dependency, and `egui`/`eframe`/`winit`/
`egui_kittest`/`rfd` allowed only at L6 and above (`xtask/src/layers.rs:66-98`,
`xtask/src/layers.rs:118-210`).

**Consequence for ComicCraft:** every new crate must be declared in
`xtask/src/layers.rs` or `cargo xtask ci` fails.

---

## 4. Visual document model — `crates/doc`

**Top-level type.** `Document`, with `artboards: Vec<Artboard>`,
`layers: Vec<Arc<Node>>`, `images: BTreeMap<String, ImageBlob>`, `next_id: u64`,
plus swatches, styles, text threading, symbols, guides, views, saved selections,
grid/setup, patterns, print/export state, and `unknown` / `extra`
(`crates/doc/src/lib.rs:525-676`).

**Units.** The model is always in points; `Unit` is display-only
(`crates/doc/src/lib.rs:87-88`). `Artboard.rect` is in document points
(`crates/doc/src/lib.rs:340-348`).

**Artboards.**

```rust
Artboard { id: u32, name: String, rect: Rect, show_center_mark: bool, show_cross_hairs: bool }
```

New ids come from `Document::next_artboard_id()` = `max(id) + 1`
(`crates/doc/src/lib.rs:340-348`, `crates/doc/src/lib.rs:1028-1030`).
Ids are serialized, so artboard identity survives save/reopen. There are **no**
bleed/trim/margin fields on `Artboard`.

**Object identity.** `NodeId(pub u64)` — *"Stable per-document object id. Never
reused."* Serialized transparently (`crates/doc/src/node.rs:19-20`). `Document`
carries `next_id`, `alloc_id()` and `fix_next_id()`; `fix_next_id()` runs after
loading foreign data so the counter exceeds every id in the tree
(`crates/doc/src/lib.rs:580-745`, `crates/doc/src/lib.rs:736-745`).

**Objects.** `Node` holds `id`, `name`, `visible`, `locked`, `opacity`, `blend`,
`isolate`, `knockout`, `knockout_shape`, `appearance`, `mask`, `trace`, `wrap`,
`graph`, `series_index`, `shaper`, `kind`, `graphic_style`, `attrs`, `slice`,
`bbox_angle`, `perspective` (`crates/doc/src/node.rs:496-540`).

`NodeKind` variants: `Layer`, `Group`, `Path`, `Compound`, `Text`, `Image`,
`SymbolInstance`, `Blend`, `Envelope`, `Mesh`, `Repeat`, `PlacedDocument`
(`crates/doc/src/node.rs:399-491`).

**Placed raster images.** `ImageObject { key, width, height, xf, link, placement }`
where `key` indexes `Document::images`, `link: Option<LinkInfo>` records a linked
(external) file, and `placement: PlacementOptions`
(`crates/doc/src/node.rs:381-395`). `PlacedDocument` adds `bounding: bool` for
Place-with-crop (`crates/doc/src/placed_document.rs:20-35`).

**Linked files already carry the fields an asset registry needs.** `LinkInfo` holds
the file's absolute `path`, a document-relative `relative` path (with `/` separators,
*"how the file is found after the document and its links move together"*), `modified`,
`size`, `hash`, and `page` for a linked document
(`crates/doc/src/links.rs:22-41`). A linked image keeps a low-resolution preview
(`PROXY_SIZE` = 256 px, `crates/doc/src/links.rs:15-16`) so it still draws when its file
cannot be read.

The hash is FNV-1a 64-bit (`hash_bytes`, `crates/doc/src/links.rs:76-79`) — good enough
for "has this file changed since I read it", which is its stated purpose, but **not
collision-resistant** and therefore not an integrity guarantee.

**Cropping, clipping and masking.**

| Mechanism | Field | Citation |
|---|---|---|
| Clipping path object | `Path.clipping: bool` | `crates/doc/src/node.rs:408-417` |
| Clip group (first child clips) | `Group.clip: bool` | `crates/doc/src/node.rs:404-407` |
| Layer clip | `Layer.clip: bool` | `crates/doc/src/node.rs:399-404` |
| Opacity mask | `Node.mask: Option<Box<OpacityMask>>` — `OpacityMask { art, clip, invert, disabled, linked }` | `crates/doc/src/node.rs:541-574` |
| Placed-file crop | `PlacedDocument.bounding` | `crates/doc/src/placed_document.rs:20-35` |

**Text.** `TextObject { vertical, kind, xf, runs, para, paras, area, path_effect,
path_align, path_spacing, wrap, cached_bounds, cached_baselines }`
(`crates/doc/src/text.rs:918-974`).
`TextKind::{Point, Area { frame }, OnPath { path, start, end }}`
(`crates/doc/src/text.rs:862-915`). `TextRun { text, style: CharStyle, inline:
Option<InlineArt> }` (`crates/doc/src/text.rs:791-818`). `cached_bounds` and
`cached_baselines` are layout caches and are not serialized
(`crates/doc/src/text.rs:948-956`).

**No string or UUID identifiers exist in the model.** `NodeId(u64)` and
`Artboard.id: u32` are the only identity types found.

---

## 5. Native format — `crates/format`

| Fact | Value | Citation |
|---|---|---|
| Extension | `vectorcraft`; legacy `drawcraft` | `crates/format/src/lib.rs:60-68` |
| Version | `VERSION = 4`, `MIN_VERSION = 1`; compression since v3 | `crates/format/src/lib.rs:49-60` |
| Encoding | UTF-8 JSON; v4 may append large blobs after `BLOB_MAGIC` (`\n\0VCBLOBS\0`) | `crates/format/src/lib.rs:1-14` |
| Inline threshold | data of `INLINE_MAX` (8 KiB) or more goes after the JSON, not in it as base64 | `crates/format/src/lib.rs:51`, `crates/format/src/lib.rs:27` |
| Compression | **opt-in** (`SaveOptions.compress`, default `false`); only version ≥ 3 readers open a compressed file | `crates/format/src/lib.rs:177`, `crates/format/src/lib.rs:192`, `crates/format/src/lib.rs:56` |
| Decompression guard | a compressed file may not unpack to more than `MAX_UNPACKED` = 512 MiB | `crates/format/src/lib.rs:63-64`, `crates/format/src/lib.rs:390-393` |
| Top-level struct | `File { format, version, document, images, profiles }` | `crates/format/src/lib.rs:161-168` |
| Blob record | `Image { mime, data, blob, proxy }` | `crates/format/src/lib.rs:69-98` |
| Version report | `FileInfo { version, legacy }` | `crates/format/src/lib.rs:414-424` |
| Load | `load_file()` rejects an unknown `format` or `version > VERSION` | `crates/format/src/lib.rs:438-442` |
| Migrations | after load: `drop_edit_modes`, `migrate_overprint_black`, `migrate_color_profiles`, `fix_next_id`, `tidy_saved_selections` | `crates/format/src/lib.rs:451-461` |

There is **no third-party metadata section** in the format. Foreign data survives
only through `Document::unknown` and `Document::extra`, which exist to round-trip
data read from *other* applications (`crates/format/src/lib.rs:13-14`,
`crates/doc/src/lib.rs:662-676`).

**Consequence for ComicCraft:** storing comic semantics in `unknown`/`extra`, or
adding a `comic` section to `File`, both mean changing the native format. The
finalized design keeps comic metadata outside the visual document (see
`comiccraft_design.md` ADR-012/ADR-013).

---

## 6. Command engine — `crates/engine`

**Registration is a fixed, in-crate list.**

```rust
CommandSpec { id, label, menu, shortcut, params, enabled, run, journal }
```

(`crates/engine/src/cmd/mod.rs:43-58`). `command_specs()` concatenates each module's
`specs()`; `find_command(id)` looks up in that list
(`crates/engine/src/cmd/mod.rs:85-132`, `crates/engine/src/cmd/mod.rs:283-285`).
**No public hook exists for another crate to register commands at runtime.** Plugin
commands are one of the in-crate contributions (`plugin::specs()`), not an external
extension point (`crates/engine/src/cmd/mod.rs:265-276`,
`crates/engine/src/cmd/plugin.rs:194-231`).

**Invocation.** `Session::execute(&mut self, id: &str, params: &Value) -> Result<Value>`
— params arrive as `serde_json::Value` and are validated by helpers such as
`str_param`, `id_param`, `ids_param`, `f64_req` and `bad(..)` →
`EngineError::BadParams` (`crates/engine/src/lib.rs:1071-1095`,
`crates/engine/src/cmd/mod.rs:287-320`). Parameter documentation is the
`CommandSpec.params` string, surfaced to clients as `CommandInfo`
(`crates/engine/src/cmd/mod.rs:43-77`).

**Errors.** `EngineError::{UnknownCommand, Disabled, BadParams, NoDocument, NoNode,
Other, Internal}`; `type Result<T> = std::result::Result<T, EngineError>`
(`crates/engine/src/lib.rs:20-46`).

**Sessions.** `Session { docs: Vec<DocState>, active: Option<usize> }`
(`crates/engine/src/lib.rs:782-835`). `DocState` owns one document plus its own
`selection`, `history`, `revision`, `saved_doc`, `active_layer`, `layer_rows`,
`isolation`, `interaction`, `undo_group`, `last_transform`, `last_selection_cmd`,
`uid` (`crates/engine/src/lib.rs:83-183`).

**History is per document.**

- `HistoryEntry { label, doc, selection }`; `History { undo, redo, limit }`, snapshot
  based with structural sharing (`crates/engine/src/lib.rs:48-67`).
- `UndoGroup` makes several edits one step and is stored **per `DocState`**
  (`crates/engine/src/lib.rs:69-78`, `crates/engine/src/lib.rs:1346-1374`).
- `Session::edit(..)` edits the active document only (`self.doc_mut()?`) and pushes
  into that document's history (`crates/engine/src/lib.rs:955-1007`).
- **There is no cross-document undo or transaction mechanism.**

**Artboard commands.** `artboard.new`, `artboard.setProps` (rename, resize, move),
`artboard.delete`, `artboard.reorder`
(`crates/engine/src/cmd/layer.rs:124-143`, `crates/engine/src/cmd/layer.rs:734-823`,
`crates/engine/src/cmd/panelcmds.rs:13`, `crates/engine/src/cmd/panelcmds.rs:56-67`).

**Image crop and clip already exist.**

- `object.cropImage` — crops the selected image to `{rect}` or `trim`s it to the
  non-transparent pixels; defaults to the artboard the image sits on
  (`crates/engine/src/cmd/menucmds.rs:110-116`,
  `crates/engine/src/cmd/menucmds.rs:532-644`).
- A second command clips the selected image with a rectangle around it, selecting
  the resulting clipping path so its handles crop the image; one undo step
  (`crates/engine/src/cmd/menucmds.rs:118-123`).

---

## 7. Rendering — `crates/render`

Backend is `vello_cpu` with SIMD sparse strips; supported: multiple fills/strokes,
opacity, blend modes, stroke alignment, dashes, clip groups, gradients, images and
text (`crates/render/src/lib.rs:3-6`).

| Entry point | Signature | Citation |
|---|---|---|
| Whole view | `Render(&mut self, doc, width: u32, height: u32, view: Affine, opts: &RenderOptions) -> Rendered` | `crates/render/src/lib.rs:471` |
| Region (a page) | `render_region(&mut self, doc, region: Rect, scale: f64, white: bool) -> Rendered` | `crates/render/src/lib.rs:611` |
| Region with options | `render_region_with(&mut self, doc, region, scale, opts)` | `crates/render/src/lib.rs:617` |
| Ink plate | `render_region_inks(..) -> Vec<u8>` | `crates/render/src/lib.rs:627` |
| Object thumbnail | `render_thumbnail(&mut self, doc, id: NodeId, size: u32) -> Option<Rendered>` | `crates/render/src/lib.rs:634` |
| Object | `render_node(&mut self, doc, n: &Arc<Node>, w: u16, h: u16, view: Affine)` | `crates/render/src/lib.rs:639` |
| Object thumbnail | `render_node_thumbnail(..)` | `crates/render/src/lib.rs:657` |

`RenderOptions` includes `trim`, `mask_view`, `progressive_placed`, `trace_views`,
`smooth_images`, `precise` (`crates/render/src/lib.rs:71-106`).

Opacity masks are turned into a `vello_cpu::Mask` (`crates/render/src/lib.rs:783-794`).
Clip groups are pushed as clip paths, with a fast path when the group is
clip-only (`crates/render/src/group.rs:20-27`, `crates/render/src/group.rs:57-74`);
blend modes are applied per layer through `push_layer` (`crates/render/src/group.rs:82-117`).

**Image clipping is a render-time parameter:** image content is painted through
`fill_path` when an `area: Option<&(BezPath, FillRule)>` is supplied, and through
`fill_rect` otherwise, using `ImageObject::xf`
(`crates/render/src/lib.rs:1493-1530`).

Progressive placed-document rendering runs on background workers with
`generation()` and `busy()` (`crates/render/src/placed_document.rs:69`,
`crates/render/src/placed_document.rs:74`,
`crates/render/src/placed_document.rs:267-304`).

---

## 8. Typography — `crates/text`

**Fonts.** `FontDb::global()` is the process-wide database (`crates/text/src/lib.rs:3-7`,
`crates/text/src/fontdb.rs:1441-1445`). The repository ships OFL faces in
`assets/fonts/`: Inter, JetBrains Mono, Source Sans 3, Source Serif 4. The optional
`CRAFT_FONTS_DIR` build input embeds a local `craft-fonts` checkout into
`vectorcraft_text::CRAFT_FONTS`; unset, `CRAFT_FONTS` is empty and the app uses its
own and installed fonts, and `CRAFT_FONTS_REQUIRED=1` turns a bad checkout into a
build error (`crates/text/build.rs:1-16`). wasm builds embed only the UI face
(`crates/text/build.rs:9-11`). Runtime hooks: `set_user_font_dirs`,
`set_app_font_dir`, `set_platform_font_files`, `add_font`, `load_system_fonts`,
`set_system_fallback` (`crates/text/src/fontdb.rs:1130-1178`,
`crates/text/src/fontdb.rs:1547-1550`, `crates/text/src/fontdb.rs:1601-1609`,
`crates/text/src/fontdb.rs:1429-1434`). Lazy system-font fallback exists
(`crates/text/src/fontdb.rs:1917-1948`).

**Layout.** `layout(db, &TextObject) -> TextLayout`
(`crates/text/src/layout.rs:133`) and `layout_with(db, t, &LayoutOptions)`
(`crates/text/src/layout.rs:194`).

```rust
LayoutOptions { rows, columns, gutter, inset, first_baseline, first_baseline_min,
                vertical_align, fit, composer, features }
```

(`crates/text/src/lib.rs:46-64`).

```rust
TextLayout { glyphs, lines, bounds, overflow, on_path, frames, fit_scale, inlines }
```

(`crates/text/src/lib.rs:159-183`).

**This is the lettering engine's foundation.** `TextLayout.overflow`,
`TextLayout.bounds` and `TextLayout.fit_scale` give balloon overflow detection and
shrink-to-fit without new layout code, and `layout_with` applies `shrink_to_fit`
when area text overflows (`crates/text/src/layout.rs:194-211`). `TextKind::Area`
and `TextKind::OnPath` are interpreted in text space, and `LayoutOptions::fit`
corresponds to area-type shrink-to-fit (`crates/text/src/lib.rs:59-61`).
Caret and hit testing exist: `caret_position`, `hit_byte`
(`crates/text/src/lib.rs:274`, `crates/text/src/lib.rs:294`).

No separate "fit text to a shape" or "text on path authoring" API was found beyond
these types.

---

## 9. Export and effects

**PDF.**

```rust
export(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError>
export_with_report(doc, opts) -> Result<ExportReport, PdfError>
page_areas(doc: &Document, opts: &PdfOptions) -> Result<Vec<Rect>, PdfError>
```

(`crates/pdf/src/export.rs:33`, `crates/pdf/src/export.rs:39`,
`crates/pdf/src/export.rs:119`).

The exporter writes **one page per artboard**, with the **trim box = the artboard**,
the **bleed box grown by the bleed**, and the **media box including printer's
marks**; it handles hidden/template/non-printing objects, layers, overprint,
thumbnails and linearisation (`crates/pdf/src/lib.rs:1-20`).
`PdfOptions { settings, artboards, title, created, native, thumbnails }`
(`crates/pdf/src/lib.rs:83-96`); `PdfSettings` carries `marks`, `bleed`,
`compression`, `create_layers`, `include_non_printing`, `output`, `advanced`,
`security`, `flattener_preset` (`crates/pdf/src/lib.rs:17-20`,
`crates/pdf/src/lib.rs:95-102`). Import supports `CropTo::{Bounding, Art, Crop,
Trim, Bleed, Media}` (`crates/pdf/src/lib.rs:109-154`).

**SVG.** `export(doc, &ExportOptions) -> String` (`crates/svg/src/lib.rs:317`),
`export_with_report(doc, opts) -> (String, Vec<String>)`
(`crates/svg/src/lib.rs:322`),
`import(svg) -> Result<Document, SvgError>` (`crates/svg/src/lib.rs:492`),
`import_with` (`crates/svg/src/lib.rs:503`). Web export can clip to a single
artboard (`crates/engine/src/cmd/webexport.rs:547-556`).

**Raster.** `Renderer::export_region(&mut self, doc, region, format:
RasterFormat, opts: &RasterExportOptions) -> Result<Vec<u8>, String>`
(`crates/render/src/encode/mod.rs:137-149`); `RasterExportOptions.ppi` with
`scale() = ppi / 72` (`crates/render/src/encode/mod.rs:22-58`,
`crates/render/src/encode/mod.rs:79-81`). PNG, JPEG, WebP, GIF, PNG8, TIFF, BMP,
TGA and PSD are written through the `document.export` / `document.serialize`
commands, which accept `ppi` or `scale`
(`crates/engine/src/cmd/fileio/export.rs:1-38`,
`crates/engine/src/cmd/fileio/encode.rs:138-232`,
`crates/engine/src/cmd/fileio/encode.rs:188-221`).

**Effects** are live, non-destructive records in appearance stacks
(`Effect { id, params, visible }`): Distort & Transform, Path, Convert to Shape,
Round Corners, Scribble, Warp, Drop Shadow, Inner/Outer Glow, Feather, Gaussian
Blur, the pixel filters, colour adjustments, and plugin effects. Baking and
expanding are explicit: `bake_appearance`, `expand_live`, `clip_outline`,
`evaluate_container`, `reshape`, `outline_text`
(`crates/effects/src/lib.rs:1-25`, `crates/effects/src/lib.rs:61-70`).

---

## 10. Interfaces

**MCP** (`crates/mcp`, documented in `docs/mcp.md`): newline-delimited JSON-RPC 2.0
over stdio; `PROTOCOL_VERSION = "2025-06-18"`, accepting `2026-07-28`,
`2025-03-26` and `2024-11-05` (`crates/mcp/src/server.rs:1-15`,
`crates/mcp/src/server.rs:69-88`). Advertised capabilities: `tools`, `resources`,
`prompts`, `completions`, `logging` (`crates/mcp/src/server.rs:175-192`).

- Discovery: `tools/list` returns name, title, description, `inputSchema` and
  annotations; the `command_list` / `list_commands` tools surface command ids,
  labels, parameter docs, enablement and menu paths
  (`crates/mcp/src/server.rs:193`, `crates/mcp/src/tools.rs:73-90`,
  `crates/mcp/src/tools.rs:650-741`).
- **`notifications/cancelled` and `notifications/progress` are accepted and
  ignored; there is no job, progress or cancel API**
  (`crates/mcp/src/server.rs:167-168`).
- Two modes: `Remote` drives a running desktop app over the loopback control
  channel, `Headless` hosts an in-process `Session` plus CPU `Renderer`
  (`crates/mcp/src/lib.rs:4-12`, `crates/mcp/src/headless.rs:14-17`,
  `crates/mcp/src/headless.rs:100-113`).

**Control channel** (`crates/ui-egui/src/control.rs:1-19`) — JSON lines over the
host transport. Methods: `engine.execute`, `ui.menu.invoke`, `engine.commands`,
`document.inspect`, `ui.inspect`, `ui.menu.list`, `ui.contextMenu.list`,
`ui.tool.select`, `ui.tool.list`, `ui.pointer`, `ui.key`, `ui.text`, `ui.move`,
`ui.click`, `ui.drag`, `ui.set`, `ui.dialog.set`, `ui.dialog.confirm`,
`ui.dialog.cancel`, `ui.resize`, `ui.focus`, `ui.screenshot`, `ui.render`,
`app.open`, `app.save`, `app.quit`, `app.export`. `ControlRequest { method,
params: Value }` dispatches through `app.run(...)`, so command parameters are JSON
(`crates/ui-egui/src/control.rs:37-64`, `crates/ui-egui/src/control.rs:133-156`).
`ui.screenshot` and `ui.render` can return PNG bytes inline as `pngBase64`
(`crates/ui-egui/src/control.rs:310-390`).

**CLI** (`apps/vectorcraft-cli`): subcommands `mcp`, `run`, `commands`, `convert`,
`info`, `bench`, `perf` (`apps/vectorcraft-cli/src/main.rs:1-5`). `mcp` takes
`--connect ADDR | --headless` plus `--automation-read-root` /
`--automation-write-root`, which imply headless and confine file access
(`apps/vectorcraft-cli/src/main.rs:71-80`, `apps/vectorcraft-cli/src/main.rs:154-181`).
`run` batches `--in FILE`, repeated `--cmd ID [--params JSON]`, repeated
`--export FILE`, `--scale N`, printing one JSON line per step
(`apps/vectorcraft-cli/src/main.rs:271-284`). Exit codes are `SUCCESS`/`FAILURE`
(`apps/vectorcraft-cli/src/main.rs:16-33`, `apps/vectorcraft-cli/src/main.rs:143-147`).
Tests: `tests/cli.rs`, `tests/fileio.rs`, `tests/mcp_e2e.rs`.

**Desktop** (`apps/vectorcraft`): `VectorcraftApp` is driven inside an eframe app
through `logic()`, `ui()`, `on_exit()`; `--control <port>` starts the control
server (`apps/vectorcraft/src/main.rs:1-25`, `apps/vectorcraft/src/main.rs:46-73`,
`apps/vectorcraft/src/main.rs:572`).

**Web** (`apps/vectorcraft-web`): `web::start()` on wasm32 only; `WebRunner` loads
fonts and starts recovery locks. There is **no TCP control server**, and file
access goes through the browser picker and downloads
(`apps/vectorcraft-web/src/main.rs:1-12`, `apps/vectorcraft-web/src/web.rs:1-18`,
`apps/vectorcraft-web/src/web.rs:57-122`).

**UI** (`crates/ui-egui`): the egui frontend is its own crate, so the engine has no
UI dependency (`crates/ui-egui/src/lib.rs:1-6`). `UI_COMMANDS` in
`crates/ui-egui/src/menus.rs` holds UI-level commands; `chrome.rs` has document tabs
and the artboard navigator; `dock.rs` is the right dock; `theme.rs` defines
`Tokens`; panels read engine state and act through `app.run(id, params)`
(`crates/ui-egui/src/menus.rs:1-20`, `crates/ui-egui/src/chrome.rs:324-330`,
`crates/ui-egui/src/chrome.rs:495`, `crates/ui-egui/src/chrome.rs:868`,
`crates/ui-egui/src/dock.rs:10`, `crates/ui-egui/src/theme.rs:42-71`).

**There is no storyboard, page-thumbnail navigator or comic-style sequential view**
at the baseline — only document tabs, the artboard navigator, and saved views.

---

## 11. Capability reuse matrix

`R` = reuse unchanged, `A` = needs a thin adapter, `N` = new work in ComicCraft.

| Comic requirement | Status | What exists | Citation |
|---|---|---|---|
| Project/page persistence container | A | native JSON format (v4) + appended blobs; no third-party sections | `crates/format/src/lib.rs:49-68`, `crates/format/src/lib.rs:161-168` |
| Page canvas with fixed page size | R | `Artboard.rect` in points; `artboard.new` / `artboard.setProps` | `crates/doc/src/lib.rs:340-348`, `crates/engine/src/cmd/layer.rs:124-143` |
| One document per project / page-per-artboard | R | multi-artboard documents, artboard navigator, per-artboard export | `crates/doc/src/lib.rs:525-676`, `crates/ui-egui/src/chrome.rs:495` |
| Multi-page PDF with trim/bleed/marks | R | one page per artboard, trim = artboard, bleed box, media box with marks | `crates/pdf/src/lib.rs:1-20`, `crates/pdf/src/export.rs:33` |
| Panel frames (rectangle / custom shape) | R | `Path` with `live` corner/rectangle shapes, `Compound` | `crates/doc/src/node.rs:399-491` |
| Panel artwork placement | R | `ImageObject` with `xf`; place/embed and link | `crates/doc/src/node.rs:381-395` |
| Panel artwork clipping | R | clip groups, `Path.clipping`, opacity masks, render-time `area` | `crates/doc/src/node.rs:404-417`, `crates/render/src/lib.rs:1493-1530` |
| Panel artwork cropping | R | `object.cropImage` (rect or trim-to-pixels, default artboard) | `crates/engine/src/cmd/menucmds.rs:110-116` |
| Panel thumbnails for a storyboard | R | `render_thumbnail(doc, NodeId, size)`, `render_node_thumbnail` | `crates/render/src/lib.rs:634`, `crates/render/src/lib.rs:657` |
| Page rendering at print resolution | R | `export_region` + `RasterExportOptions.ppi` | `crates/render/src/encode/mod.rs:137-149` |
| Speech balloons as editable vector shapes | R | paths, groups, live corners; `Path.clipping` for tails | `crates/doc/src/node.rs:408-417` |
| Balloon text with wrapping and alignment | R | `TextKind::Area { frame }` + `text::layout_with` | `crates/doc/src/text.rs:862-915`, `crates/text/src/layout.rs:194` |
| Balloon overflow detection | R | `TextLayout.overflow`, `bounds` | `crates/text/src/lib.rs:167-179` |
| Shrink text to fit a balloon | R | `LayoutOptions.fit` → `shrink_to_fit`, `TextLayout.fit_scale` | `crates/text/src/lib.rs:59-61`, `crates/text/src/layout.rs:194-211` |
| Type on a curved tail/path | R | `TextKind::OnPath`, `TextLayout.on_path` | `crates/doc/src/text.rs:862-915` |
| Comic lettering and SFX faces | N | bundled faces are UI/body text only; comic faces must come from `craft-fonts` | `crates/text/build.rs:1-16`, `assets/fonts/` |
| Missing-font detection | R | export missing-font warning; `FontDb` lookup and system fallback | `crates/text/src/fontdb.rs:1429-1434` |
| Reading-order-independent narrative order | N | no narrative model exists | — |
| Character/location/style libraries | N | no such model; `library.rs` / `stylelib.rs` are style/swatch libraries | `crates/engine/src/cmd/library.rs`, `crates/engine/src/cmd/stylelib.rs` |
| Asset registry with provenance and hashes | A | `LinkInfo` already holds a project-relative path, mtime, size and a change-detection hash, with a preview fallback; provenance and a cryptographic hash are new, and `sha2` is already a direct dependency of `crates/pdf` | `crates/doc/src/links.rs:22-41`, `crates/pdf/Cargo.toml:34` |
| Stable semantic↔visual references | R | `NodeId(u64)` never reused and serialized; `Artboard.id` serialized | `crates/doc/src/node.rs:19-20`, `crates/doc/src/lib.rs:340-348` |
| Undo/redo of comic operations | R | snapshot `History` per document + `UndoGroup` | `crates/engine/src/lib.rs:48-78` |
| Cross-page single undo step | A | per-document history means one *document* per project is required | `crates/engine/src/lib.rs:83-133`, `crates/engine/src/lib.rs:955-1007` |
| Agent access to comic commands | A | static in-crate registry; additive comic modules in `crates/engine` | `crates/engine/src/cmd/mod.rs:85-132` |
| Command discovery for agents | R | `command_list` surfaces ids, labels, param docs, enablement | `crates/mcp/src/tools.rs:650-741` |
| Long-running generation jobs | N | MCP ignores progress/cancellation; no job API | `crates/mcp/src/server.rs:167-168` |
| AI text/image providers | N | **no HTTP client dependency exists anywhere in the workspace** | `Cargo.toml`, workspace members |
| Deterministic page templates and gutters | A | artboards + `align`/`distribute`/path tools exist; grid templates are new | `crates/engine/src/cmd/*` |
| Golden-render regression tests | R | `testkit::raster::{render_region, render_artboard, assert_similar}` | `crates/testkit/src/raster.rs:43-65`, `crates/testkit/src/raster.rs:122-171` |
| Headless batch and scripting | R | `vectorcraft-cli run --in --cmd --params --export`, `mcp --headless` | `apps/vectorcraft-cli/src/main.rs:271-284` |
| Web/browser delivery | A | wasm app exists, but no control server, no file system; AI calls hit CORS | `apps/vectorcraft-web/src/main.rs:1-12` |
| Live non-destructive art effects | R | appearance `Effect` stacks, baking, expanding | `crates/effects/src/lib.rs:1-25` |

---

## 12. Gaps and hazards found

These are the findings that changed the finalized design. Each is a subsection so it
can be cited directly.

### 12.1 History is per document, not per session

`DocState` owns `history` (`crates/engine/src/lib.rs:83-133`), `Session::edit` touches
the active document only (`crates/engine/src/lib.rs:955-1007`), and `undo_group` is per
`DocState` (`crates/engine/src/lib.rs:1346-1374`). An operation spanning two open
documents cannot be one undo step. *This is the reason the finalized design puts a whole
comic project in one visual document with one artboard per page*
(`comiccraft_design.md` ADR-011).

### 12.2 The command registry has no external registration hook

`command_specs()` is a fixed concatenation inside `crates/engine`
(`crates/engine/src/cmd/mod.rs:85-132`). Comic commands therefore have to be added to
the engine crate as new modules contributing `specs()`. That is additive and small, but
it is a documented divergence from upstream (`comiccraft_design.md` ADR-014).

### 12.3 `Artboard.id` is allocated as `max(id) + 1`

`crates/doc/src/lib.rs:1028-1030`. Unlike `NodeId` (never reused, with `fix_next_id()`
to repair the counter after a foreign load), deleting the highest-id artboard lets the
next one reuse that id. A comic `Page` that keys on `artboard_id` could then silently
point at a different page. Mitigations are required and are specified in
`comiccraft_design.md` §5.3.1.

### 12.4 No networking dependency exists — verified, not assumed

No HTTP client crate (`reqwest`, `ureq`, `hyper`, `curl`, `isahc`, `surf`, …) appears
anywhere in `Cargo.lock`, direct or transitive. Provider adapters therefore need a
genuinely new dependency. It must pass the licence and audit rules, must not break the
wasm gate, and must not be reachable from `crates/doc`/`crates/format`
(`comiccraft_design.md` ADR-015).

### 12.5 No comic lettering face is available

`assets/fonts/` holds Inter, JetBrains Mono, Source Sans 3 and Source Serif 4 — UI and
body text. Lettering and sound-effect faces belong in `storytold/craft-fonts`, never in
this repository (`AGENTS.md:31`, `crates/text/build.rs:1-16`), so ComicCraft must add
them there and must degrade gracefully with `CRAFT_FONTS` empty.

### 12.6 MCP has no job, progress or cancellation model

`crates/mcp/src/server.rs:167-168`. Generation jobs must therefore be exposed as
pollable commands (`comiccraft_design.md` ADR-016).

### 12.7 The native format has no third-party metadata section

`crates/format/src/lib.rs:161-168`; `Document::unknown`/`extra` exist to round-trip
foreign data, not to carry a new domain (`crates/doc/src/lib.rs:662-676`). Comic
metadata must live beside the visual document, not inside it.

### 12.8 The web build is a reduced surface

No control server, browser file access only, and one embedded font
(`apps/vectorcraft-web/src/main.rs:1-12`, `crates/text/build.rs:9-11`). Comic features
must declare their web behaviour.

### 12.9 Vendor-name gate

`cargo xtask brands` fails on the words `adobe`, `illustrator`, `photoshop`, `acrobat`,
`indesign`, `lightroom`, `dreamweaver`, `creative cloud`, `typekit`, `kuler`, `behance`,
`myriad`, `pantone`, `aicb` in user-visible Rust string literals, `Cargo.toml`
descriptions, `packaging/` and non-Markdown `docs/` files (`xtask/src/brands.rs:14-29`,
`xtask/src/brands.rs:1-14`). Comic command labels, parameter docs, panel text and MCP
tool definitions must respect it.

### 12.10 Every new crate must be added to `xtask/src/layers.rs`

`xtask/src/layers.rs:9-57`, or CI fails.

### 12.11 No-panic lints are workspace-wide

`unsafe_code = "deny"` and clippy denies `unwrap_used`, `expect_used`, `panic`, `todo`,
`unimplemented`, `unreachable` (`Cargo.toml:84-96`); `engine::guard` is the last-resort
rollback at every entry point (`AGENTS.md:41`).

### 12.12 The asset and clean-room rules apply to all new work

Every asset file needs a row in `ASSETS.md`, no Adobe iconography or GPL/AGPL code, and
original brand assets only — the ArtCraft logos in `docs/brand/` are trademarks that may
not be reused (`AGENTS.md:14-48`, `docs/brand/LICENSE-brand.txt`).

---

## 13. Uncertainty log

Recorded honestly. Items marked *resolved* were settled by reading code after this log
was first written; the rest were not resolved by the audit.

| # | Open item | Why it matters |
|---|---|---|
| U1 | Whether `Artboard` ids should be changed upstream to a never-reused counter (like `NodeId` via `fix_next_id`) | Removes hazard 3 entirely; needs an upstream contribution or a documented fork patch |
| U2 | Whether a single-file container (zip/deflate) is worth adding | The workspace has `flate2` but no archive dependency, and the app already writes its own container formats (`crates/metafile`, `crates/eps`, `crates/affinity/src/container.rs`) |
| U3 | Content-hash algorithm for the asset registry | **Mostly resolved:** `sha2` is already a direct dependency of `crates/pdf` (`crates/pdf/Cargo.toml:34`), so SHA-256 needs no new crate — only a workspace dependency entry. The document's existing `LinkInfo.hash` is FNV-1a 64-bit (`crates/doc/src/links.rs:76-79`): keep it for cheap change detection, but do not use it as an integrity guarantee |
| U4 | Whether MCP progress/cancellation should be implemented upstream | Determines whether generation progress is push or poll |
| U5 | Comic lettering faces currently available under an OSI/CC0 licence in `craft-fonts` | Blocking for a shippable lettering default |
| U6 | Which image/text provider APIs are acceptable and how their terms interact with the "no creative-work scraping" and provenance rules | Blocking for Phase 4 |
| U7 | Real cost/latency budgets for page generation | Determines job architecture and UI |
| U8 | Whether ComicCraft ships as a WebAssembly build at all | Determines which features need gating from day one |
