# ComicCraft Project Schema

**Document type:** Project and bundle format specification
**Status:** Frozen for M2 — version 2
**Owner:** `schema-architect` · **Approved by:** `comiccraft-lead`
**Depends on:** [`comiccraft-design.md`](comiccraft-design.md) §5–§7 (authoritative for
meaning; this document fixes the wire format) · [`comiccraft-porting-audit.md`](comiccraft-porting-audit.md)
§4–§5 (the visual document and native format this sits beside)
**Version history:** v1 frozen for M1 · **v2 for M2 adds exactly one field, `Panel.locked`
(§6.2)**, with its migration (§11) and its fixtures (§11.1). No other field, rule or file
changed with the bump.

---

## 1. Scope and status

This document fixes the **wire format** of a ComicCraft project: file layout, field types,
optionality, identity allocation, referential rules, the save protocol and the migration
contract. It does not restate the design's reasoning; where the two could be read
differently, [`comiccraft-design.md`](comiccraft-design.md) decides the meaning and this
document is corrected.

It is **frozen at version 2** for M2, and was frozen at version 1 for M1. A field added
after this point is a schema version increment with a migration and a fixture (§11). The v1 → v2
increment spent its budget on one field — `Panel.locked` (§6.2) — and nothing else: no entity, no
file and no rule was added with it, which is what keeps the migration small enough to test
byte-for-byte (§11.1).

Two rules from the design are structural and cannot be relaxed by this document:

1. **No comic data in the visual document.** No comic field in `crates/doc`, no comic
   section in `crates/format`, never `Document::unknown` / `Document::extra`
   (design ADR-013).
2. **No duplicated authority.** The artboard owns the page size; the document object owns
   frame geometry; the comic record owns the id and the meaning (design §6.1).

---

## 2. Versioning

| Version | Lives in | Meaning |
|---|---|---|
| `manifest.schema_version` | `manifest.json` | the comic bundle format (this document). Starts at `1`; **`2`** is what this document fixes |
| `format::File.version` | `document/comic.vectorcraft` | VectorCraft's native document format. Read from `crates/format`, not set here |
| `manifest.archive_version` | a packaged `.comiccraft` archive | present only in the single-file export (design §7.5) |

The two versions move **independently**. A newer VectorCraft may raise
`format::VERSION` and migrate the document inside a bundle whose `schema_version` is
unchanged, and vice versa. Neither migration may touch the other's fields.

Readers:

- Refuse a `schema_version` **greater** than the one they implement, with an actionable
  error naming the required version. Never partially load. This applies to **every file that
  declares one**, not only to the manifest: a `pages/page-<id>.json` declaring a version the reader
  does not implement may mean something else by its keys, and reading it forward would be exactly
  the partial load the rule forbids. Rule 3 (§13) governs the other direction — a file declaring a
  version *older* than the manifest's — and **reports** it, because that one is read forward.
- Accept any `schema_version` **less than or equal to** theirs, migrating forward (§11). A migrated
  load **reports** what it did (§11); it does not rewrite the bytes it read, and a save then writes
  the version the writer implements (§11.1).

---

## 3. Bundle layout

A working project is a **directory** (design ADR-012):

```text
example.comiccraft/
├── manifest.json                 # written LAST on save; the only file that declares contents
├── project.json                  # ComicProject
├── story/
│   ├── chapters.json
│   ├── scenes.json
│   ├── characters.json
│   ├── locations.json
│   └── styles.json
├── pages/
│   └── page-<PageId>.json        # Page + its Panels + Dialogue + Lettering
├── document/
│   └── comic.vectorcraft         # the ONE visual document; artboards are pages
├── assets/
│   ├── index.json                # the Asset registry
│   ├── references/
│   ├── panels/
│   ├── backgrounds/
│   └── lettering/
├── provenance/
│   └── generations.json          # GenerationJobs and ReviewDecisions
├── jobs/                         # one file per in-flight job, job-<JobId>.json (§8.3)
└── exports/                      # derived output only; deletable, never authoritative
```

Notes that matter for implementation:

- **Page files are keyed by `PageId`, not by an ordinal.** `pages/page-7.json` holds
  `PageId` 7. Renumbering a page's folio or moving it in reading order never moves a file.
  (The design's `page-003.json` example is illustrative padding; padding is cosmetic and
  the loader must not depend on it.)
- **`exports/` is never read as input.** It may be deleted at any time.
- **Comic paths are bundle-relative with `/` separators**, so a bundle moved on disk still
  resolves its assets. **This is not the convention `LinkInfo.relative` uses**: that field is
  relative to the *document's own folder* and may contain `..` to walk out of it
  (`crates/doc/src/links.rs:25-28`, `crates/engine/src/cmd/links.rs:627-645`). There are **three**
  bases and they are not interchangeable: `StorageRef::Linked` resolves against the bundle root,
  `StorageRef::Package` against the archive root (design §7.5), and `LinkInfo.relative` against
  `document/`. `StorageRef::Embedded` is a key, not a path. Confusing any two is a
  path-traversal bug, so §13 rule 9 names each base and requires the resolved result to be
  canonicalised before it is trusted.
- The directory extension is `.comiccraft`. The loader also accepts a directory whose name
  ends otherwise, because a bundle is identified by its `manifest.json`.

---

## 4. `manifest.json`

The manifest is the only file that declares what the bundle contains, and the last written
on save (§10).

| Field | Type | Req | Notes |
|---|---|---|---|
| `schema_version` | `u32` | yes | §2 |
| `bundle_id` | `ComicId` | yes | stable for the life of the project; regenerating a bundle is a new project |
| `comic_id` | `ComicId` | yes | the `ComicProject.id` in `project.json`; duplicated here so the manifest alone identifies the project |
| `title` | `String` | yes | denormalised for humans and file pickers; `project.json` is authoritative |
| `next_comic_id` | `u64` | yes | the allocation counter (§9). Never decreases |
| `retired_artboard_ids` | `[u32]` | yes | artboards that belonged to deleted pages. Never pruned (design §5.3.1) |
| `document` | `VisualDocumentRef` | yes | `{ relative_path, format_version }` |
| `files` | `BTreeMap<String, FileEntry>` | yes | bundle-relative path → `FileEntry { sha256, bytes }`, for every file except the manifest **and except `exports/` and `.save-tmp/`** — those are never read as input (§3), so listing them would make deleting `exports/` break every load |
| `created` | `Timestamp` | yes | |
| `updated` | `Timestamp` | yes | |
| `app` | `{ name, version, schema }` | yes | what wrote it, for support |
| `unknown` | `Map<String, Value>` | no | preserved verbatim (§12) |

`files` is what makes the manifest a consistency record: a reader can detect a truncated or
tampered bundle without parsing every file.

---

## 5. `project.json` and the story files

### 5.1 `ComicProject`

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `ComicId` | yes | equals `manifest.comic_id` |
| `schema_version` | `u32` | yes | must equal `manifest.schema_version`; design §5.2 requires a schema version on the project, and §2 makes it the bundle's |
| `title` | `String` | yes | |
| `metadata` | `ProjectMetadata` | yes | §5.2 |
| `chapter_ids` | `[ChapterId]` | yes | order is the book order |
| `character_ids` | `[CharacterId]` | yes | |
| `location_ids` | `[LocationId]` | yes | |
| `style_profile_ids` | `[StyleProfileId]` | yes | |
| `asset_registry_id` | `AssetRegistryId` | yes | the `assets/index.json` registry |
| `settings` | `ProjectSettings` | yes | §5.3 |
| `visual_document` | `VisualDocumentRef` | yes | must equal `manifest.document` |
| `unknown` | `Map<String, Value>` | no | preserved verbatim |

### 5.2 `ProjectMetadata`

`author: Option<String>`, `language: Option<String>` (BCP-47), `genre: Option<String>`,
`reading_direction: ReadingDirection` (`ltr` | `rtl`, default `ltr`),
`created: Timestamp`, `updated: Timestamp`, `unknown`.

### 5.3 `ProjectSettings`

`default_page_size: Option<PageSize>` (points), `default_template: Option<String>`,
`default_lettering_style: Option<String>`, `unknown`.

`PageSize` is `{ width: f64, height: f64 }` in **document points**, mirroring
`Artboard.rect`. It is a *default for new pages*, never a second copy of a page's size.

**`default_lettering_style` is what makes the output comic-styled, and it is a name, not a
font file.** The ComicCraft defaults (ADR-017) name the two faces that live in
`storytold/craft-fonts` and are never committed here:

These are **names a project's default lettering style refers to**, not fields of a struct: no v1
struct declares them (§1 forbids adding one), and §6.6's per-lettering `font` is what actually
selects a face. They are listed so a project created without an explicit choice uses the comic
faces rather than the bundled body font.

| Setting | Face | Used for |
|---|---|---|
| `dialogue_font` | `Comic Neue` Regular | speech balloons, narration and caption text |
| `dialogue_font_emphasis` | `Comic Neue` Bold | emphasised words |
| `dialogue_font_aside` | `Comic Neue` Italic | asides and whispered lines |
| `display_font` | `Bangers` Regular | sound effects, titles and cover text |

Because these are *names*, a project can choose different faces, and a build without
`CRAFT_FONTS_DIR` still loads — the lettering then reports a missing font — a deterministic check
(design §12.1), reported by `comic.project.validate`, not a referential rule. A hard-coded face or a bundled font file would break both of those.

### 5.4 Story files

Each is `{ "schema_version": 7, "<entity>": [ … ], "unknown": { … } }` so a top-level unknown key
survives and rule 3 can check the version against the manifest.

| File | Element | Fields (all required unless marked) |
|---|---|---|
| `story/chapters.json` | `Chapter` | `id`, `project_id: ComicId` (design §5.2), `title`, `scene_ids: [SceneId]`, `order_index: u32`, `unknown?` |
| `story/scenes.json` | `Scene` | `id`, `chapter_id`, `title`, `synopsis`, `page_ids: [PageId]`, `location_id?`, `character_ids: [CharacterId]`, `order_index: u32`, `continuity_notes: String`, `unknown?` |
| `story/characters.json` | `Character` | `id`, `name`, `canonical_description`, `reference_asset_ids: [AssetId]`, `style_profile_id?`, `approved_design_version: Option<AssetId>`, `continuity_rules: [String]`, `unknown?` |
| `story/locations.json` | `Location` | `id`, `name`, `description`, `reference_asset_ids: [AssetId]`, `unknown?` |
| `story/styles.json` | `StyleProfile` | `id`, `name`, `description`, `palette: [String]` (hex), `line_weight: Option<f64>`, `rendering_notes: String`, `unknown?` |

`approved_design_version` is an **immutable** asset reference: approving a new design
creates a new `Asset` and repoints this field; it never rewrites the old asset (design §7.4).

---

### 5.5 Shared types

These are the value types the tables **in this document** reference — above and below this
section. Without them the v1 fixture (§11.1) cannot be authored, so they are part of the frozen
schema and not left to the reader. It holds the types **this document defines once and shares**
between sections; a type used by one section is defined in that section's own table (`PageSize` and
`TextLayoutSettings` §5.3/§6.6, `ArtworkRef` and `ContinuityConstraints` §6.2/§6.5,
`StorageRef` §7.1, `ContentHash` §7.2, `MediaMetadata` §7, `GenerationJob` and `ReviewDecision`
§8). The identifier newtypes (`ComicId`, `PageId`, … §9) and the enums a table names inline
(`JobStatus`, `PageStatus`, `LetteringKind`, …) are defined where they are used.

| Type | Shape |
|---|---|
| `Timestamp` | `String`, RFC 3339 **in UTC, second precision, `Z` suffix**, e.g. `2026-10-10T21:52:00Z`. No fractional seconds and no `+00:00` — one canonical form, so a save/reload is byte-identical (§5.7). Text, not an integer, so a bundle is readable without knowing an epoch convention |
| `AssetRegistryId` | `ComicId`, allocated once when the project is created and written to `ComicProject.asset_registry_id`; the registry file it names is `assets/index.json`. The id exists so a future project can hold more than one registry without a schema change |
| `FileEntry` | `{ sha256: String, bytes: u64 }` — one `manifest.files` value (lowercase hex, and the size in bytes) |
| `AppInfo` | `{ name: String, version: String, schema: u32 }` — what wrote the bundle and the `schema_version` it wrote, for support |
| `VisualDocumentRef` | `{ relative_path: String, format_version: u32 }` — `relative_path` from the bundle root, `format_version` the `format::File.version` last written |
| `ReadingDirection` | `"ltr"` \| `"rtl"` |
| `ApprovalStatus` | `"pending"` \| `"approved"` \| `"rejected"` |
| `PageStatus` | `"draft"` \| `"generated"` \| `"lettered"` \| `"approved"` \| `"published"` |
| `GenerationStatus` | `"none"` \| `"queued"` \| `"running"` \| `"completed"` \| `"failed"` |
| `LetteringKind` | `"speech"` \| `"thought"` \| `"narration"` \| `"shout"` \| `"whisper"` \| `"sound_effect"` |
| `OverflowPolicy` | `"warn"` \| `"shrink"` \| `"resize_frame"` |
| `TextAlign` | `"left"` \| `"center"` \| `"right"` |
| `AssetType` | `"source"` \| `"generated"` \| `"edited"` \| `"reference"` |
| `ArtworkRole` | `"primary"` \| `"background"` \| `"overlay"` |
| `Provenance` | `{ provider: ProviderId, model: String, request: Value, input_asset_ids: [AssetId], created: Timestamp, consent: Option<Consent> }` |
| `ProviderId` | `String`, the provider's own identifier, recorded verbatim (schema §8.1) |
| `Consent` | `{ granted: Timestamp, statement: String }` — the informed-consent record design §9.3 and ADR-015 require before artwork leaves the machine |
| `JobOperation` | `"text_to_image"` \| `"reference_conditioned"` \| `"image_to_image"` \| `"inpaint"` \| `"variations"` |
| `Progress` | `{ done: u32, total: Option<u32>, note: Option<String> }` |
| `JobError` | `{ kind: "provider" \| "transport" \| "invalid_output" \| "cancelled", message: String, retryable: bool }` |
| `ReviewSubject` | `"panel_artwork"` \| `"story_content"` \| `"character_design"` \| `"batch_edit"` \| `"publication"` |
| `Decision` | `"approved"` \| `"rejected"` \| `"superseded"` |

### 5.6 Where this document adds a field the design's prose list omits

The design's §5.2 lists each entity's fields in prose; this document fixes types and
optionality, which occasionally means adding or splitting one. Every such delta is listed here so
nothing is an undocumented invention. The list is small on purpose; a delta that is not here is a
defect in this document:

| Delta | Why |
|---|---|
| `ContinuityConstraints.exceptions` | Design §5.2 gives `Character.continuity_rules`; a *panel* also needs to record where the rules do not apply ("she has the coat here"), which belongs with the panel's constraints, not the character's |
| `GenerationJob.consent` and `Provenance.consent` | ADR-015 requires informed consent before artwork is sent to a provider, and design §9.3 requires it to be recorded. The design says the consent must be recorded but does not place it |
| `Asset.provenance: Option<Provenance>` | Optional at the type level, mandatory at the rule level: rule 10 requires it for `generated` and `edited` and forbids it otherwise |
| `StorageRef::Package` | Design §7.5 describes a packaged archive but does not name the variant; the export needs one so a path's base is explicit (rule 9) |
| `MediaMetadata` split into `mime`/`width`/`height`/`color_profile`/`dpi` | The design says "media metadata" without a shape |
| `ContinuityConstraints` as a named shape | Design §5.2 lists `continuity_constraints` as a panel field without a shape |
| `Panel.locked` (§6.2, v2) | The porting plan §8 requires panel **locking** but places no field for it, and design §5.2's `Panel` list has none either. This document places it: a `bool` on the panel, which is the layer that owns the decision. It arrived with the v1 → v2 migration (§11), and it is the only field the bump added |

### 5.7 Canonical form

§11.1 requires a fixture to round-trip **byte-identically**, which is only meaningful if one
byte sequence is canonical. A save writes:

- Object keys in the order the struct declares them (serde's default), not sorted.
- Two-space indentation, `\n` line endings, a trailing newline.
- Numbers in the shortest form that round-trips (`1`, not `1.0`, where the type is integral;
  `1.5` where it is not); no `-0`, no exponent where plain form fits.
- Strings UTF-8, with only the escapes JSON requires.
- Timestamps in the §5.5 canonical form.
- **Map-valued fields** (`unknown`, `provenance`'s `request: Value`, `manifest.files`) are the
  exception to declared order: their keys are written **sorted**, because `serde_json` sorts a
  `Map` and a fixture author cannot otherwise know what order a map with two keys takes. A struct
  field's order is declared; a map's is sorted. Both are canonical.

A reader must accept any valid JSON; a **writer** must produce this form. A fixture that differs
only in whitespace or number form is a writer bug, not a schema variation.

## 6. Per-page files

`pages/page-<PageId>.json`:

```json
{
  "schema_version": 2,
  "page": { … },
  "panels": [ … ],
  "dialogue": [ … ],
  "lettering": [ … ],
  "unknown": {}
}
```

Grouping a page's panels, dialogue and lettering in one file keeps a page's edits together
and makes the save set proportional to what changed. Cross-page references
(`Scene.page_ids`, `Page.reading_order`) are still by id, so the grouping is a storage
detail, not a model change.

### 6.1 `Page`

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `PageId` | yes | the file name must agree |
| `chapter_id` | `ChapterId` | yes | |
| `scene_ids` | `[SceneId]` | yes | a scene may span pages |
| `artboard_id` | `u32` | yes | `Artboard.id` in the visual document. **This is the page's visual home** — there is no `visual_document_id` and no `page_size` (design §5.1.1) |
| `page_number` | `u32` | yes | the printed folio. May repeat across a book; **never** used as an identity |
| `panel_ids` | `[PanelId]` | yes | membership |
| `reading_order` | `[PanelId]` | yes | narrative order; must be a permutation of `panel_ids` (§13) |
| `status` | `PageStatus` | yes | `draft` \| `generated` \| `lettered` \| `approved` \| `published` |
| `unknown` | `Map` | no | |

### 6.2 `Panel`

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `PanelId` | yes | |
| `page_id` | `PageId` | yes | must agree with the containing file |
| `artboard_id` | `u32` | yes | denormalised from the page for locality; must equal `Page.artboard_id` (§13) |
| `scene_id` | `SceneId` | yes | |
| `frame_node_id` | `NodeId` | yes | the frame object. **No `geometry_ref`**: the document owns the geometry |
| `artwork_refs` | `[ArtworkRef]` | yes | |
| `shot_description` | `String` | yes | the generation brief |
| `dialogue_ids` | `[DialogueId]` | yes | |
| `lettering_ids` | `[LetteringId]` | yes | |
| `reading_order_index` | `u16` | yes | must agree with `Page.reading_order` |
| `continuity_constraints` | `ContinuityConstraints` | yes | §6.5 |
| `generation_status` | `GenerationStatus` | yes | `none` \| `queued` \| `running` \| `completed` \| `failed` |
| `locked` | `bool` | yes | **v2.** Whether the creator pinned this panel against the commands that move, resize or delete it. `false` by default, and always written — a v1 file has no such key and reads as `false` (§11) |
| `unknown` | `Map` | no | |

`locked` is the **comic layer's** record, and the one the comic commands consult. It is not a second
authority over the document object's own lock flag (`crates/doc/src/node.rs:502`, which
`crates/doc/src/hit.rs` applies to clicks and Select All): that flag is the *document* editor's
session behaviour for a node, and this crate is L2 and never parses the document, so it cannot read
it at all. The panel's field is what a bundle durably says about a locked panel, and it survives
whatever happens to the frame object — a template re-apply, an artboard rebuild, a document written
by another tool. Whether the engine also mirrors it onto the frame node is the command layer's
decision and is outside this document (§6.2 is a wire-format statement, and this is the whole of
it).

**`locked` adds no referential rule.** Locking is not a relation: a locked panel makes nothing else
illegal, references nothing, and a bundle full of locked panels is as valid as one with none. §13
defines its rules as the deterministic consequences of a *reference* (rule 1), an id (2, 4, 17), a
declared agreement (3, 5, 6, 8, 11, 13), a path (9) or a lifecycle (10, 14, 15, 16, 18) — none of
which locking is. It constrains **commands** (what `comic.panel.move`/`resize`/`delete` may do), and
a constraint on a command is enforced by that command, not reported by `comic.project.validate`,
whose job is to tell a creator whether the *bundle* is coherent. §12.1's test for a rule is that it
is a deterministic check on the bundle's own data; "this panel is locked" is true or false and
inconsistent with nothing. So the rule set stays at eighteen.

`ArtworkRef` = `{ asset_id: AssetId, node_id: NodeId, role: ArtworkRole }` where
`ArtworkRole` is `primary` \| `background` \| `overlay`.

### 6.3 `Dialogue`

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `DialogueId` | yes | |
| `panel_id` | `PanelId` | yes | |
| `speaker_character_id` | `Option<CharacterId>` | yes | `null` for narration |
| `text` | `String` | yes | **the authoritative text.** No `balloon_id` field (design §5.2.1) |
| `language` | `Option<String>` | yes | BCP-47 |
| `reading_order_index` | `u16` | yes | |
| `approval_status` | `ApprovalStatus` | yes | `pending` \| `approved` \| `rejected` |
| `unknown` | `Map` | no | |

### 6.4 `Lettering`

One entity for balloons, captions and sound effects; `kind` distinguishes them.

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `LetteringId` | yes | |
| `page_id`, `panel_id` | ids | yes | |
| `artboard_id` | `u32` | yes | must equal `Page.artboard_id` |
| `kind` | `LetteringKind` | yes | `speech` \| `thought` \| `narration` \| `shout` \| `whisper` \| `sound_effect` |
| `dialogue_id` | `Option<DialogueId>` | yes | **required** for `speech`/`thought`/`shout`/`whisper`; **must be `null`** for `narration`/`sound_effect` (§13) |
| `text` | `Option<String>` | yes | required for `narration`/`sound_effect`; must be `null` otherwise — the linked `Dialogue` owns the text |
| `frame_node_id` | `NodeId` | yes | balloon outline or caption box |
| `tail_node_id` | `Option<NodeId>` | yes | required for kinds that have a tail |
| `text_node_id` | `NodeId` | yes | the area text object |
| `text_layout_settings` | `TextLayoutSettings` | yes | §6.6 |
| `overflow_policy` | `OverflowPolicy` | yes | `warn` \| `shrink` \| `resize_frame` |
| `unknown` | `Map` | no | |

### 6.5 `ContinuityConstraints`

`characters: [CharacterId]`, `wardrobe: Option<String>`, `time_of_day: Option<String>`,
`callbacks: [String]`, `exceptions: [String]`. A **closed shape**: it carries no `unknown` (§12).

### 6.6 `TextLayoutSettings`

`font: Option<String>` (a family name resolved through the font database),
`size: Option<f64>` (points), `leading: Option<f64>`, `tracking: Option<f64>`,
`align: Option<TextAlign>` (`left` \| `center` \| `right`),
`padding: Option<f64>`, `fit_to_frame: bool`. A **closed shape**: it carries no `unknown` (§12).

These are **inputs to** `vectorcraft_text`; the measured result (`TextLayout.bounds`,
`.overflow`, `.fit_scale`) is never persisted (design §8.3).

---

## 7. `assets/index.json`

```json
{ "schema_version": 2, "assets": [ … ], "unknown": {} }
```

| Field | Type | Req | Notes |
|---|---|---|---|
| `id` | `AssetId` | yes | |
| `asset_type` | `AssetType` | yes | `source` \| `generated` \| `edited` \| `reference` |
| `storage_reference` | `StorageRef` | yes | §7.1 |
| `content_hash` | `ContentHash` | yes | §7.2 |
| `media_metadata` | `MediaMetadata` | yes | `mime`, `width`, `height` (px), `color_profile: Option<String>`, `dpi: Option<f64>` |
| `provenance` | `Option<Provenance>` | yes | required for `generated` and `edited` (§8) |
| `parent_asset_ids` | `[AssetId]` | yes | lineage |
| `approval_status` | `ApprovalStatus` | yes | |
| `unknown` | `Map` | no | |

### 7.1 `StorageRef`

An enum, so the storage policy is explicit and one place changes:

- `Linked { relative_path: String }` — a file under `assets/`, **bundle-relative**. **The
  default.** Note the base is **not** `LinkInfo.relative`'s: that is document-folder-relative
  and may contain `..` (§3, rule 9). The adapter that writes the document's link converts
  between the two bases; they must never be used interchangeably.
- `Embedded { image_key: String }` — a key in the visual document's image map, for
  portability.

All three are **externally tagged** on the wire, as serde derives by default:
`{"linked": {"relative_path": "…"}}`, `{"embedded": {"image_key": "…"}}`,
`{"package": {"relative_path": "…"}}`. The v1 fixture freezes that form; it is written down here so
the fixture is not the only place it is defined.
- `Package { relative_path: String }` — a file inside a packaged archive (design §7.5). Its
  base is the **archive root**, not the bundle root: a packaged archive is a different container,
  and rule 9 resolves it against that base.

### 7.2 `ContentHash`

`{ algorithm: "sha256", value: String }` — lowercase hex.

`sha2` is already a direct dependency of `crates/pdf`, so this adds a workspace dependency
entry, not a new crate. `LinkInfo.hash` is FNV-1a 64-bit and is **not** used here: it is a
cheap change detector, not an integrity guarantee.

---

## 8. `provenance/generations.json`

```json
{ "schema_version": 2, "jobs": [ … ], "decisions": [ … ], "unknown": {} }
```

### 8.1 `GenerationJob`

`id: JobId`, `operation: JobOperation`, `target_panel_id: Option<PanelId>`,
`input_asset_ids: [AssetId]`, `provider: ProviderId`, `model: String`,
`request: Value` (the exact request sent, for provenance), `status: JobStatus`
(`queued` \| `running` \| `completed` \| `failed` \| `awaiting_review` \| `cancelled`),
`progress: Option<Progress>`, `error: Option<JobError>`, `result_asset_ids: [AssetId]`,
`created: Timestamp`, `updated: Timestamp`, `target_revision: u64`, `consent: Option<Consent>`,
`unknown`.

**Credentials never appear here.** No token, key or endpoint secret is written to a job
record, the bundle, an export or a log line.

### 8.2 `ReviewDecision`

`id: ReviewDecisionId`, `subject: ReviewSubject`, `decision: Decision`
(`approved` \| `rejected` \| `superseded`), `actor: String`, `timestamp: Timestamp`,
`note: Option<String>`, `superseded_asset_id: Option<AssetId>`, `unknown`.

---

### 8.3 `jobs/`

Durable metadata for a job that is **in flight**, so an interrupted session can report what was
running rather than losing it (design §10.3). One file per job, `jobs/job-<JobId>.json`, holding
a wrapper `{ "schema_version": 2, "job": { … }, "unknown": {} }` whose `job` is a §8.1
`GenerationJob`. The wrapper is the same shape every other file uses (§5.4, §6, §7, §8), which is
what lets rule 3 check this file's version — §8.1's entity has no `schema_version` of its own.

The split is deliberate: `jobs/` is the *live* copy, `provenance/generations.json` is the
*record*. A job is **terminal** when its `status` is `failed` or `cancelled` — the same predicate
the code implements (`JobStatus::is_terminal`, `crates/comic-ai/src/lib.rs`) — and a terminal
job's file is removed, its entry in `provenance/` becoming the only copy.

`completed` and `awaiting_review` are deliberately **not** terminal: a job holding a result a human
has not yet accepted legitimately keeps its file (design §9.4). So `jobs/` is not empty in a
quiescent project. A job's file is removed when the job becomes terminal (`failed`, `cancelled`),
 or when its result is **consumed** — `comic.job.apply` or `comic.job.reject` writes the
 `ReviewDecision` and removes the file. Rule 18 checks only the first half, because consumption is
 not visible in the status: a `completed` file whose result no one has decided yet is legal, and
 one whose result was consumed is removed by the command, not by a status change. A file whose
 status *is* terminal is the inconsistency rule 18 reports.

A job file is written **while the app runs**, outside the §10 save protocol, so it is in
`manifest.files` (§4) and a job started since the last save makes those hashes stale by design.
That is a third, benign cause of the `files` mismatch §14 step 1 reports, alongside an interrupted
save and a hand edit.

## 9. Identity

| Kind | Type | Allocation | Reuse |
|---|---|---|---|
| Comic entities | newtypes over `u64` (`ComicId`, `PageId`, …) | `manifest.next_comic_id`, incremented per allocation and persisted | **never** |
| Visual objects | `NodeId(u64)` (from `crates/doc`) | the document's own `next_id`, repaired by `fix_next_id()` after a foreign load | **never** |
| Page artboard | `u32` (`Artboard.id`) | the document's `max(id) + 1` — which the comic layer **never lets a page take when it is retired** (§9.1) | **must not be reused by a page** |

Every comic id is a `#[serde(transparent)]` newtype over `u64`, so a JSON id is a bare
number. Ids are **opaque**: never derive meaning from magnitude or order, and never use an
array index as an id.

`next_comic_id` must be greater than every id present in the bundle at load time; the loader
verifies this and repairs it upward (the `fix_next_id()` idea applied to the comic model)
rather than trusting a hand-edited manifest.

### 9.1 The artboard-id hazard

`Artboard.id` is allocated as `max(id) + 1`, and the engine gives no never-reuse guarantee:
deleting the highest-id artboard frees its number for the next one. The **comic layer closes
that hole itself** — a page is never given a retired number (mitigation 4 below) — because a
`Page.artboard_id` left over from a deleted page would otherwise silently resolve to a
**different page**, and rules 4 and 17 make such a page permanently unfixable.

Five mitigations, all mandatory (design §5.3.1):

1. **Comic commands own artboard lifecycle.** Only `comic.page.create`,
   `comic.page.delete` and `comic.page.reorder` add, remove or move a page's artboard while
   a project is open. `artboard.delete` is not offered in the comic workspace.
2. **Retired ids are remembered.** `comic.page.delete` appends the artboard id to
   `manifest.retired_artboard_ids`. A `Page` resolving to a retired id is **broken**, not
   trusted.
3. **Validation cross-checks.** `comic.project.validate` verifies every `Page.artboard_id`
   exists and is not retired, and **reports** the artboard's `rect` as information. It checks no
   size expectation, because there is none to hold one (rule 15): the artboard *is* the page's
   size. A retired or missing artboard is a repairable inconsistency, never auto-corrected.
4. **The comic layer never reuses a retired id.** Because `Artboard.id` is `max(id) + 1`
   (§9), deleting the highest artboard frees its id for the next one — and rule 17 makes a
   retired id permanently untrusted. So `comic.page.create` must **not accept an artboard whose
   id is in `retired_artboard_ids`**: it allocates artboards until it gets an id that is not
   retired, so rules 4 and 17 can both hold. Without this, deleting a page would permanently
   break the next page to reuse that number.
5. **Upstream fix proposed.** Allocate artboard ids from a persisted counter, as `NodeId` already does (audit §13 U1).
   Until it lands, mitigations 1–4 stand alone: 5 would remove the *cause* of the hazard, it would
   not replace the four guards that keep rules 4 and 17 consistent today.

---

## 10. Save protocol

Written in this order. Steps 4 and 5 are **both** observable, and the gap between them is the one
window a load must tolerate (§14):

1. Serialize every changed comic file into `<bundle>/.save-tmp/`.
2. Save the visual document through `crates/format` into the same staging directory.
3. **Verify** by loading the staged document back — `format::load_file` rejects an unknown
   `format` and any `version > VERSION`.
4. **Replace the content files** — every file *except* the manifest — with atomic per-file
   renames. **This opens the window.** From here until step 5 the content files are the new
   generation while `manifest.json` still describes the previous one, so its hashes do not match
   the files. That is expected and survivable: a reader that trusts the manifest sees the previous
   generation, and one that reads `pages/` directly sees new content under old hashes — which is
   exactly why only this document's loader may read a bundle, and why §14 reports a `files`
   mismatch instead of refusing to load.
5. **Write `manifest.json` last**, with the new `updated`, `next_comic_id`,
   `retired_artboard_ids` and `files` (`{sha256, bytes}` per file, §4), then remove
   `.save-tmp/`. This is the commit point: it is the only file whose truncation can make the
   bundle inconsistent, and it is written when everything it describes is already in place.

Because the manifest is written last and is the only file declaring the bundle's contents,
it can never reference partially written data. A crash before step 4 leaves the previous
manifest — and therefore the previous consistent project — intact. On Windows, where a
directory replace is not atomic, per-file replacement plus this ordering gives the same
guarantee at the granularity that matters.

Embedded images live in the document's blob section (managed by `crates/format`); linked
assets are content-addressed under `assets/`, so an interrupted save cannot overwrite an
approved asset — it writes a new file with a new hash.

---

## 11. Migrations

Migrations live in `crates/comic-model`, shaped like the engine's existing post-load
migrations (`format::load_file` runs `drop_edit_modes`, `migrate_overprint_black`,
`migrate_color_profiles`, `fix_next_id`, `tidy_saved_selections`).

A migration must:

- Validate its input and **refuse rather than guess**.
- Preserve every stable id, including `retired_artboard_ids`.
- Report incompatible data as a visible, actionable error.
- **Preserve unknown fields** — never silently discard them.
- Ship with a fixture for every previous version, exercised in CI.

**A migration that succeeds is reported too.** §11's third bullet is about data the migration
cannot handle, and that is an error; but a *successful* migration still moved the project, and a
caller that was not told would discover it when the next save wrote a format the creator never
chose. `crates/comic-model` therefore returns the report as a **value** — a `Migration` per step,
carrying the version pair and one actionable note per observable effect (which field a step added,
how many entities took its default, anything the step would otherwise have had to guess about) — on
the loaded bundle. It is deliberately **not** a §14 `Finding`: findings are the inconsistencies of a
load, and a bundle read forward is not inconsistent, only older.

**v1 → v2.**
One field: `Panel.locked` (§6.2), default `false`. A v1 file has no such key, so the value itself
comes from the field's serde default at parse time — that is the reader half of §5.7, and a
migration cannot be the thing that supplies it, because after parsing a `false` from the key and a
`false` from its absence are the same value. What the step does is the part a default cannot: it
states the field it added and its default in one place, **counts** the panels that took it, and
refuses to be silent about the one input a default would silently *accept* — a v1 bundle that
**declares** `locked`, which no v1 writer could have written, and which is preserved and reported as
the hand-edit it is. (After parsing, `locked: false` is indistinguishable from the key's absence, so
only a truthy value is reportable; the limit is stated rather than papered over.)

The load reads a v1 bundle as v2 **in memory only**: the bytes on disk are untouched, so the frozen
v1 fixture stays byte-identical and the migration has something left to be tested against. The
write side is what moves the version, and the writer writes the version it implements on **every**
file that declares one (§5.4, §7, §8.3, rule 3) — a v2 manifest over v1 page files would be rule 3's
own contradiction, in the bytes the save just wrote.

`crates/format` behaviour is **not** changed by ComicCraft. If the document itself needs a
migration, that is an upstream change with an upstream test.

### 11.1 Fixtures

```text
crates/comic-model/tests/fixtures/
├── v1/
│   └── minimal.comiccraft/      # the frozen v1 example: 1 chapter, 1 scene, 1 page, 3 panels,
│                                #   2 speech balloons, 1 caption, 1 linked asset, 1 job
└── v2/
    └── minimal.comiccraft/      # the v1 example read forward: every file declares
                                 #   `schema_version: 2` and every panel carries `locked: false`.
                                 #   Derived, never hand-edited — it is what saving v1 produces
```

The **fixture at the version the code writes** must load, validate and round-trip
byte-identically after a save, with ids unchanged: that is v2 today, and it is the anchor for
§5.7's canonical form. A fixture for an **older** version cannot round-trip byte-identically under
code that writes a newer one — the writer writes the version it implements (§2) — and pretending
otherwise would mean freezing a format nobody may write. Its job is the migration: it loads, it
gains the newer version's defaults, the load reports that, and **saving it produces the current
fixture byte for byte**. v1 is the migration anchor and stays byte-frozen; a schema bump never
rewrites history.

Both are exercised in CI: `crates/comic-model/tests/fixture_v1.rs` (the migration, and that the
frozen bytes never move) and `crates/comic-model/tests/fixture_v2.rs` (the round trip, the default,
the field's round trip, and the refusals). The v2 fixture has a generator, named in that file's
header and ignored by default, so it is reproducible rather than hand-maintained.

---

## 12. Unknown fields

Each top-level file struct carries a flattened map:

```rust
#[serde(flatten)]
pub unknown: serde_json::Map<String, serde_json::Value>,
```

so a field written by a **newer** ComicCraft survives a load-and-save by an older one. This
is deliberately *not* `Document::unknown` — that exists to round-trip foreign data inside the
visual document, which this schema never touches.

Which entities carry it is stated per entity above, and the answer is **not** "all of them":
the small value types (`PageSize`, `TextLayoutSettings`, `ArtworkRef`, `ContinuityConstraints`,
`StorageRef`, `ContentHash`, `MediaMetadata`) omit it deliberately, because they are closed
shapes whose extra keys are more likely to be a typo than a future field. Every *entity* struct
carries it. This is the list rule 1's reference walk skips, deliberately.

---

## 13. Referential rules

`comic.project.validate` checks these and **reports**; it never repairs. Each is a
deterministic rule from design §12.1.

**v2 added a field and no rule.** `Panel.locked` (§6.2) is not a reference, an id, a declared
agreement, a path or a lifecycle, so none of the eighteen below can say anything about it, and none
was added for it — see §6.2 for the argument against a "locked panels are consistent" rule. The
count stays at eighteen.

| # | Rule |
|---|---|
| 1 | Every reference resolves: every `*_ids` entry **and** every singular foreign key (`Chapter.project_id`, `Scene.chapter_id`, `Page.chapter_id`, `Panel.page_id`, `Panel.scene_id`, `ArtworkRef.asset_id`, `Lettering.dialogue_id`, `GenerationJob.target_panel_id`, `ReviewDecision.superseded_asset_id`, …). A key that is **`null` is not a reference** and is skipped, exactly as rule 12 does for a narration speaker. The plural glob alone left every singular key ungoverned. **`retired_artboard_ids` is not a reference list**: it holds artboard ids that must *not* resolve (rule 4) |
| 2 | No duplicate id within an entity kind, and none across kinds |
| 3 | `manifest.comic_id` equals `project.id`; `manifest.document` equals `project.visual_document`; and **every file that declares `schema_version` declares the manifest's value** — `project.json`, each `pages/page-<id>.json`, `story/chapters.json`, `story/scenes.json`, `story/characters.json`, `story/locations.json`, `story/styles.json` (§5.4), `assets/index.json`, `provenance/generations.json`, and every `jobs/job-<id>.json` (§8.3) |
| 4 | Every `Page.artboard_id` exists in the document and is **not** in `retired_artboard_ids` |
| 5 | `Panel.artboard_id` equals its `Page.artboard_id`; `Lettering.artboard_id` likewise |
| 6 | `Page.reading_order` is a permutation of `Page.panel_ids`; each `Panel.reading_order_index` agrees with its position |
| 7 | `Panel.frame_node_id`, every `ArtworkRef.node_id`, `Lettering.frame_node_id` / `.tail_node_id` / `.text_node_id` resolve to a node in the document |
| 8 | `Lettering.dialogue_id` is present exactly for `speech`/`thought`/`shout`/`whisper`, and `Lettering.text` exactly for `narration`/`sound_effect` |
| 9 | Every path is resolved against its **declared base**, then canonicalised, and only then checked to be inside that base. The bases are not one base: `Linked` resolves against the **bundle root**, `Package` against the **archive root** (design §7.5 — a packaged archive is a different container), and `LinkInfo.relative` against the **document folder** (§3). `Embedded` is a key in the document's image map, not a path at all. Canonicalising *before* the check defeats `..` and symlinks; a `..` denylist would not. A path that escapes is **never resolved**: rule 9 reports that asset unresolved and the rest of the
load continues. (It is rule 9's finding, not rule 1's: rule 1 walks *references*, and an escaped path
is a property of the path itself.) `.save-tmp/` is never a source. A `Linked` file must exist and its sha256 must match `content_hash` |
| 10 | Every `Asset.provenance` is present for `generated` and `edited`, and absent otherwise |
| 11 | `Page.panel_ids` agrees with the panels in that page's file |
| 12 | Every `Dialogue.speaker_character_id` that is **not `null`** resolves to a `Character`. `null` is legal and means narration (§6.3), so the rule checks the set values, not the field's presence |
| 13 | `Page.id` equals the number in its file name (`pages/page-<PageId>.json`) — the file name encodes the *page* id, never the artboard id. **Parse the number, not the string**: padding is cosmetic (§3), so `page-7.json` and `page-007.json` both mean page 7 |
| 14 | A file in `pages/` that no `Scene.page_ids` reaches is a **warning, not an error** — an orphan is recoverable, and a scene-less page is legal (a book may have an unassigned page) |
| 15 | A page artboard's `rect` differing from `ProjectSettings.default_page_size` is a **warning, not an error**: the artboard *is* the page's size and a page may deliberately differ. There is no per-page "expected size" field to compare against |
| 16 | Every `Asset.storage_reference` carries a `content_hash` whose `algorithm` the reader implements |
| 17 | `retired_artboard_ids` is **unique and append-only**: an id is never removed once retired, because removing it is exactly what would let a stale `Page.artboard_id` resolve again |
| 18 | Every file in `jobs/` holds a job whose `status` is **not terminal**. Terminal is exactly `failed` or `cancelled` (`JobStatus::is_terminal`, `crates/comic-ai/src/lib.rs`); `completed` and `awaiting_review` are not terminal, because a result awaiting a human decision still belongs to a live job (§8.3) |

### 13.1 Rule 9's limits, stated rather than papered over

Rule 9 must **confine every path to its own base** before reading — `Linked` to the bundle root,
`Package` to the archive root, `LinkInfo.relative` to the document folder. An imported bundle is
untrusted input, and a `relative_path` of `../../etc/passwd` must be rejected, not resolved. There
is no single "the bundle root" rule; a note that collapsed them would undo rule 9.

Two limits are real and are recorded rather than hidden:

- A **hard link** inside `assets/` is indistinguishable from a real asset — it has no path to
  canonicalise — so it is accepted, and integrity rests on `content_hash` (§7.2), not on the path
  check.
- Canonicalising and then opening is a **TOCTOU window**: between the two the path can be swapped
  for a symlink. The reader therefore opens with no-follow semantics where the platform offers
  them (`O_NOFOLLOW`) and re-checks that the opened file is inside the base. Where the platform
  offers neither, the window is documented and the bundle is treated as locally trusted.

---

## 14. Validation of a loaded bundle

A load is complete only when:

1. `manifest.json` parses and its `schema_version` is supported. `files` is then compared with
   the tree **as §4 defines it** — every file except the manifest, `exports/` and `.save-tmp/`,
   so a deleted `exports/` is not a mismatch. A mismatch means the bundle was interrupted
   mid-save (§10), hand-edited, or a `jobs/` file written while the app ran outside the save
   protocol (§8.3) — the third cause is benign and expected, so it is
   **reported as an inconsistency and the load continues** — it must not be fatal, or a crash in
   the window between step 4 and step 5 of a save would leave a project nobody could open.
   The same read also refuses a **file** that declares a greater `schema_version` than this build
   implements (§2): that is the one refusal here that is fatal, because reading such a file would be
   the partial load §2 forbids.
2. The document loads through `crates/format` and its `Artboard.id`s are readable.
3. The comic files parse, with unknown fields preserved.
4. `next_comic_id` is repaired upward if it is not greater than every id present.
5. Referential rules 1–18 run and their findings are returned to the caller.

Only step 2 (the document will not parse) and step 3 (a comic file will not parse) are fatal.
Everything else — the `files` mismatch, every referential finding, an escaped path, a retired
artboard — is **returned as a finding on a usable project**, so a creator can open a damaged
bundle and repair it rather than being locked out, and so the crash window in §10 is survivable.

---

## 15. Open items

| # | Item | Owner |
|---|---|---|
| S1 | **Closed** by rule 15: `PageSize` is in document points, mirroring `Artboard.rect`, and an artboard rect that differs is a warning because the artboard *is* the page's size. A UI path assuming pixels is a UI bug, not a schema question | closed |
| S2 | Whether `provenance/` should be split per job at scale, or stay one file | `schema-architect`, at M4 |
| S3 | `archive_version` numbering for the packaged export (design §7.5) | `publishing-engineer`, at M7 |
| S4 | **Closed at M2**: v2 landed with `Panel.locked` (§6.2), a v1 → v2 migration and both fixtures (§11.1). The next version is a fresh decision; a bump spends its budget on one field, and the migration step is where it is recorded | closed |
