# ComicCraft command API

> Generated from `shared/commands` by `npm run docs:commands`. Do not edit by hand.
> The parameter schema shown here is the same object `tools/list` publishes over MCP
> and the HTTP endpoint validates against, so a click, a `curl` and an agent call cannot disagree.

**95 commands.** Every command is enabled only on an open comic project unless stated otherwise.

Retry classes: **safe** commands are read-only; **idempotent** commands may be replayed with the same result; **state-changing** commands write project state and should be retried deliberately.

## `comic.artwork.*`

### `comic.artwork.clip`

**Clip Artwork to Panel** · retry: state-changing · undo: page

- `panelId`: string — required
- `inset`: number — min 0 — Shrink the clip rect inward by this many units, for a gutter-safe border.

### `comic.artwork.cover`

**Artwork Coverage** · retry: state-changing

- `panelId`: string — required

### `comic.artwork.crop`

**Crop Artwork** · retry: state-changing · undo: page

- `panelId`: string — required
- `x`: number — required — min 0 — max 1 — Left edge as a fraction of the image.
- `y`: number — required — min 0 — max 1
- `width`: number — required — min 0.001 — max 1 — Width as a fraction of the image.
- `height`: number — required — min 0.001 — max 1

### `comic.artwork.paint`

**Paint Local Drawing** · retry: state-changing · undo: page

- `panelId`: string — required
- `prompt`: string — What the panel shows. Defaults to the panel artwork prompt or shot description.
- `style`: string — one of "black ink", "manga", "noir", "watercolour", "retro"
- `name`: string

### `comic.artwork.place`

**Place Artwork** · retry: state-changing · undo: page

- `panelId`: string — required
- `name`: string — required — Human-readable asset name.
- `contentHash`: string — required — Hash of the asset bytes; the asset is stored under it.
- `data`: string — Asset payload as text, for a generated or uploaded file.
- `fit`: string — one of "cover", "contain" — How the artwork is fitted to the frame. Default cover.
- `clip`: boolean — Clip the artwork to the panel frame. Default true.
- `provider`: string — Who produced it, for provenance.
- `model`: string
- `request`: string

## `comic.asset.*`

### `comic.asset.approve`

**Approve Asset** · retry: state-changing · undo: project

- `assetId`: string — required
- `note`: string

### `comic.asset.get`

**Get Asset** · retry: safe

- `assetId`: string — required

### `comic.asset.list`

**List Assets** · retry: safe

- `assetType`: string — one of "source", "generated", "edited", "reference"
- `approvalStatus`: string — one of "pending", "approved", "rejected"

### `comic.asset.register`

**Register Asset** · retry: state-changing · undo: project

- `name`: string — required
- `contentHash`: string — required — Hash of the bytes; the identity of the asset.
- `data`: string — The bytes as a data: URL or text payload. Omit when the asset lives outside the bundle.
- `assetType`: string — one of "source", "generated", "edited", "reference"
- `mime`: string
- `provider`: string
- `model`: string
- `request`: string
- `inputAssetIds`: array
- `parentAssetIds`: array

### `comic.asset.reject`

**Reject Asset** · retry: idempotent · undo: project

- `assetId`: string — required
- `note`: string

## `comic.batch.*`

### `comic.batch.run`

**Run Command Batch** · retry: state-changing

- `commands`: array — required — An ordered list of { command, params } objects. Commands run through the same dispatcher as a click or a curl.
- `commands[].command`: string — required
- `commands[].params`: object
- `atomic`: boolean — Restore the whole bundle if any command fails. Default true.
- `stopOnError`: boolean — Stop at the first failure. Default true.

## `comic.book.*`

### `comic.book.export`

**Export Book** · server only · retry: state-changing

- `preset`: string — one of "screen", "print", "proofing"
- `path`: string — required — File for a PDF, or a directory for images.
- `format`: string — one of "pdf", "images", "both"
- `pageIds`: array
- `scale`: number — min 0.05 — max 4 — Preview scale for images. Defaults to the preset.
- `contactSheet`: boolean — Write a contact sheet beside the images. Default true for images and both.
- `allowMissingAssets`: boolean
- `allowMissingFonts`: boolean

### `comic.book.validate`

**Validate for Publication** · retry: safe

- `preset`: string — one of "screen", "print", "proofing"
- `pageIds`: array
- `allowMissingAssets`: boolean
- `allowMissingFonts`: boolean
- `checkOverflow`: boolean — Report lettering overflow. Default true.

## `comic.chapter.*`

### `comic.chapter.add`

**Add Chapter** · retry: state-changing · undo: project

- `title`: string — required

## `comic.character.*`

### `comic.character.add`

**Add Character** · retry: state-changing · undo: project

- `name`: string — required
- `description`: string — Continuity note: what this character must look like in every panel.

### `comic.character.approveDesign`

**Approve Character Design** · retry: state-changing · undo: project

- `characterId`: string — required
- `assetId`: string — required
- `note`: string

### `comic.character.get`

**Get Character** · retry: safe

- `characterId`: string — required

### `comic.character.list`

**List Characters** · retry: safe

No parameters.

### `comic.character.reference.add`

**Add Character Reference** · retry: state-changing · undo: project

- `characterId`: string — required
- `assetId`: string — required

### `comic.character.update`

**Update Character** · retry: state-changing · undo: project

- `characterId`: string — required
- `name`: string
- `description`: string
- `canonicalDescription`: string — The appearance every panel must carry.
- `face`: string
- `body`: string
- `clothing`: string
- `palette`: string
- `continuityRules`: array
- `continuityExceptions`: array

## `comic.command.*`

### `comic.command.describe`

**Describe Command** · retry: safe

- `commandId`: string — required

### `comic.command.list`

**List Commands** · retry: safe

- `prefix`: string — Only ids starting with this prefix, e.g. comic.panel.

## `comic.edit.*`

### `comic.edit.history`

**History** · retry: state-changing

No parameters.

### `comic.edit.redo`

**Redo** · retry: state-changing

No parameters.

### `comic.edit.undo`

**Undo** · retry: state-changing

No parameters.

## `comic.export.*`

### `comic.export.pdf`

**Export PDF** · server only · retry: state-changing

- `path`: string — required
- `pageIds`: array — Pages to include, in order. Omit for every page in the book.

### `comic.export.png`

**Export PNG** · server only · retry: state-changing

- `pageId`: string — required
- `path`: string
- `scale`: number — min 0.05 — max 4 — Pixel scale. 1 renders one pixel per page unit.

### `comic.export.svg`

**Export SVG** · server only · retry: state-changing

- `pageId`: string — required
- `path`: string — Where to write it, relative to the storage root.

## `comic.fonts.*`

### `comic.fonts.list`

**Available Fonts** · retry: safe

No parameters.

## `comic.job.*`

### `comic.job.apply`

**Apply Generation Result** · retry: state-changing · undo: page

- `jobId`: string — required
- `note`: string

### `comic.job.cancel`

**Cancel Generation Job** · server only · retry: state-changing

- `jobId`: string — required

### `comic.job.list`

**List Generation Jobs** · retry: safe

- `status`: string — one of "queued", "running", "completed", "failed", "awaiting_review", "cancelled"

### `comic.job.reject`

**Reject Generation Result** · retry: idempotent · undo: page

- `jobId`: string — required
- `note`: string

### `comic.job.status`

**Generation Job Status** · retry: safe

- `jobId`: string

### `comic.job.submit`

**Submit Generation Job** · server only · retry: state-changing

- `operation`: string — required — one of "text-to-image", "reference-to-image", "image-to-image", "inpaint", "variation"
- `prompt`: string — required
- `panelId`: string — The panel the result is for. The job never writes to it.
- `inputAssetIds`: array
- `provider`: string
- `model`: string
- `parameters`: object — Provider parameters, recorded verbatim in provenance.

## `comic.lettering.*`

### `comic.lettering.add`

**Add Lettering** · retry: state-changing · undo: page

- `panelId`: string — required
- `kind`: string — required — one of "balloon", "caption", "sound-effect"
- `variant`: string — one of "speech", "thought", "shout", "whisper" — Presentation for a balloon: speech, thought, shout or whisper. Omit for speech.
- `text`: string — required — The lettering’s text. May be empty.
- `characterId`: string — Who says it, for a balloon. Creates the dialogue record.
- `overflowPolicy`: string — one of "warn", "shrink", "resize-frame"
- `x`: number
- `y`: number
- `width`: number — min 1
- `height`: number — min 1
- `tailToX`: number — Tip of the tail, for a balloon. Defaults to below the balloon.
- `tailToY`: number

### `comic.lettering.attachTail`

**Attach Balloon Tail** · retry: state-changing · undo: page

- `letteringId`: string — required
- `tipX`: number — required — The speaker the tail points at, in page units.
- `tipY`: number — required

### `comic.lettering.delete`

**Delete Lettering** · retry: idempotent · undo: page

- `letteringId`: string — required

### `comic.lettering.detachTail`

**Detach Balloon Tail** · retry: state-changing · undo: page

- `letteringId`: string — required

### `comic.lettering.edit`

**Edit Lettering** · retry: state-changing · undo: page

- `letteringId`: string — required
- `text`: string
- `family`: string
- `sizePt`: number — min 4 — max 96
- `align`: string — one of "left", "center", "right"
- `padding`: number — min 0 — max 80
- `bold`: boolean
- `italic`: boolean
- `color`: string
- `variant`: string — one of "speech", "thought", "shout", "whisper"
- `policy`: string — one of "warn", "shrink", "resize-frame"

### `comic.lettering.fit`

**Fit Lettering** · retry: state-changing · undo: page

- `letteringId`: string — required
- `policy`: string — one of "warn", "shrink", "resize-frame"

### `comic.lettering.list`

**List Lettering** · retry: safe

- `pageId`: string — required

### `comic.lettering.move`

**Move Lettering** · retry: state-changing · undo: page

- `letteringId`: string — required
- `dx`: number — required
- `dy`: number — required

### `comic.lettering.setFrame`

**Set Lettering Frame** · retry: state-changing · undo: page

- `letteringId`: string — required
- `x`: number — required
- `y`: number — required
- `width`: number — required — min 1
- `height`: number — required — min 1

### `comic.lettering.setOverflowPolicy`

**Set Overflow Policy** · retry: state-changing · undo: page

- `letteringId`: string — required
- `policy`: string — required — one of "warn", "shrink", "resize-frame"

### `comic.lettering.setStyle`

**Set Lettering Style** · retry: state-changing · undo: page

- `letteringId`: string — required
- `family`: string
- `sizePt`: number — min 4 — max 96
- `align`: string — one of "left", "center", "right"
- `padding`: number — min 0 — max 80
- `bold`: boolean
- `italic`: boolean
- `color`: string

### `comic.lettering.setText`

**Set Lettering Text** · retry: state-changing · undo: page

- `letteringId`: string — required
- `text`: string — required

### `comic.lettering.setVariant`

**Set Balloon Variant** · retry: state-changing · undo: page

- `letteringId`: string — required
- `variant`: string — required — one of "speech", "thought", "shout", "whisper"

## `comic.page.*`

### `comic.page.add`

**Add Page** · retry: state-changing · undo: project

- `name`: string — required — Shown in the page navigator.
- `sceneId`: string — Defaults to the first scene, creating one if the project has none.
- `template`: string — Panel template applied on creation. Defaults to the project setting.
- `width`: integer — min 16 — max 20000
- `height`: integer — min 16 — max 20000

### `comic.page.create`

**Create Page** · retry: state-changing · undo: project

- `name`: string — required
- `sceneId`: string
- `template`: string
- `width`: integer — min 16 — max 20000
- `height`: integer — min 16 — max 20000

### `comic.page.delete`

**Delete Page** · retry: idempotent · undo: project

- `pageId`: string — required

### `comic.page.export`

**Export Page** · server only · retry: state-changing

- `pageId`: string — required
- `path`: string — required
- `format`: string — one of "png", "svg", "pdf"
- `preset`: string — one of "screen", "print", "proofing"
- `scale`: number — min 0.05 — max 4
- `allowMissingAssets`: boolean
- `allowMissingFonts`: boolean

### `comic.page.info`

**Page Info** · retry: safe

- `pageId`: string — required

### `comic.page.render`

**Render Page** · server only · retry: state-changing

- `pageId`: string — required
- `format`: string — one of "png", "svg"
- `scale`: number — min 0.05 — max 4

### `comic.page.reorder`

**Reorder Pages** · retry: state-changing · undo: project

- `sceneId`: string — required
- `order`: array — required — Every page id in the scene, in the intended order.

### `comic.page.setSize`

**Set Page Size** · retry: state-changing · undo: page

- `pageId`: string — required
- `width`: integer — required — min 16 — max 20000
- `height`: integer — required — min 16 — max 20000

### `comic.page.setStatus`

**Set Page Status** · retry: state-changing · undo: project

- `pageId`: string — required
- `status`: string — required — one of "draft", "generated", "lettered", "approved", "published"

### `comic.page.setTemplate`

**Apply Page Template** · retry: state-changing · undo: page

- `pageId`: string — required
- `template`: string — required — Replaces every panel on the page.

### `comic.page.template.apply`

**Apply Page Template** · retry: state-changing · undo: page

- `pageId`: string — required
- `template`: string — required
- `replace`: boolean — Replace every panel on the page. Default false, which refuses a page that already has panels.

## `comic.panel.*`

### `comic.panel.add`

**Add Panel** · retry: state-changing · undo: page

- `pageId`: string — required
- `x`: number — required — Left edge in page units.
- `y`: number — required — Top edge in page units.
- `width`: number — required — min 1 — Panel width, in page units. Must be positive.
- `height`: number — required — min 1 — Panel height, in page units. Must be positive.
- `after`: string — Panel id to insert after in reading order. Omit to append.

### `comic.panel.delete`

**Delete Panel** · retry: idempotent · undo: page

- `panelId`: string — required

### `comic.panel.list`

**List Panels** · retry: safe

- `pageId`: string — required

### `comic.panel.move`

**Move Panel** · retry: state-changing · undo: page

- `panelId`: string — required
- `dx`: number — required
- `dy`: number — required

### `comic.panel.readingOrder`

**Reading Order** · retry: state-changing

- `pageId`: string — required

### `comic.panel.reorder`

**Reorder Panels** · retry: state-changing · undo: page

- `pageId`: string — required
- `order`: array — required — Every panel id on the page, in the intended reading order.

### `comic.panel.resize`

**Resize Panel** · retry: state-changing · undo: page

- `panelId`: string — required
- `width`: number — required — min 1
- `height`: number — required — min 1
- `anchor`: string — one of "top-left", "center" — Which point stays put. Default top-left.

### `comic.panel.setArtworkRole`

**Set Artwork Role** · retry: state-changing · undo: page

- `panelId`: string — required
- `nodeId`: integer — required — min 1
- `role`: string — required — one of "primary", "background", "overlay"

### `comic.panel.setFrame`

**Set Panel Frame** · retry: state-changing · undo: page

- `panelId`: string — required
- `x`: number — required
- `y`: number — required
- `width`: number — required — min 1
- `height`: number — required — min 1

### `comic.panel.setLocked`

**Lock Panel** · retry: state-changing · undo: page

- `panelId`: string — required
- `locked`: boolean — required — A locked panel refuses frame and artwork changes.

### `comic.panel.setShot`

**Set Panel Shot** · retry: state-changing · undo: page

- `panelId`: string — required
- `shotDescription`: string
- `camera`: string
- `artworkPrompt`: string

## `comic.project.*`

### `comic.project.create`

**New Comic** · retry: state-changing

- `title`: string — required — The comic’s title.
- `pageWidth`: integer — min 16 — max 20000 — Default page width in page units.
- `pageHeight`: integer — min 16 — max 20000 — Default page height in page units.

### `comic.project.info`

**Project Info** · retry: safe

No parameters.

### `comic.project.open`

**Open Comic** · server only · retry: state-changing

- `path`: string — required — Project directory, relative to the storage root.

### `comic.project.package`

**Package Project** · server only · retry: state-changing

- `path`: string — required — The .comiccraft archive to write.

### `comic.project.save`

**Save Comic** · server only · retry: state-changing

- `path`: string — required — Project directory, relative to the storage root.

### `comic.project.serialize`

**Serialize Project** · retry: state-changing

No parameters.

### `comic.project.setDefaultTemplate`

**Set Default Template** · retry: state-changing · undo: project

- `template`: string | null — required — one of "single", "two-up", "three-up", "two-by-two", "three-by-two", "five-grid", "splash", "six-grid", null — Template used when a new page does not name one, or null to clear it.

### `comic.project.setLetteringDefault`

**Set Default Lettering Style** · retry: state-changing · undo: project

- `family`: string
- `sizePt`: number — min 4 — max 96
- `align`: string — one of "left", "center", "right"
- `padding`: number — min 0 — max 80
- `bold`: boolean
- `italic`: boolean

### `comic.project.unpack`

**Unpack Project** · server only · retry: state-changing

- `archivePath`: string — required
- `path`: string — required — Directory to unpack into; open this path afterwards.

### `comic.project.validate`

**Validate Comic** · retry: safe

No parameters.

## `comic.provider.*`

### `comic.provider.list`

**List Providers** · retry: safe

No parameters.

## `comic.reading.*`

### `comic.reading.set`

**Set Reading Order** · retry: state-changing · undo: page

- `pageId`: string — required
- `order`: array — required

### `comic.reading.setDirection`

**Set Reading Direction** · retry: state-changing · undo: project

- `direction`: string — required — one of "ltr", "rtl"

### `comic.reading.validate`

**Validate Reading Order** · retry: safe

- `pageId`: string — required

## `comic.scene.*`

### `comic.scene.add`

**Add Scene** · retry: state-changing · undo: project

- `chapterId`: string — required
- `name`: string — required

### `comic.scene.list`

**List Scenes** · retry: safe

- `chapterId`: string

## `comic.story.*`

### `comic.story.design`

**Design Comic** · retry: state-changing

- `ideas`: string — required — The story, idea, outline or manuscript to turn into a comic.
- `title`: string
- `style`: string — Visual direction, e.g. black ink, manga, watercolour, noir.
- `audience`: string — one of "all ages", "teen", "mature"
- `characters`: array — Names, optionally Name: appearance.
- `maxPanelsPerPage`: integer — min 1 — max 6 — Default 3.
- `provider`: string — one of "local", "http" — local runs the deterministic director in any runtime; http is server-only and needs configuration.

### `comic.story.get`

**Get Story Proposal** · retry: safe

- `proposalId`: string — required

### `comic.story.import`

**Import Story** · retry: state-changing · undo: project

- `text`: string — required — A prompt, synopsis or manuscript. Headings, NAME: lines, CAPTION: and SFX: are recognised.
- `title`: string
- `maxPanelsPerPage`: integer — min 1 — max 6 — Default 4.

### `comic.story.providers`

**Story Design Providers** · retry: state-changing

No parameters.

### `comic.story.validate`

**Validate Story Proposal** · retry: safe

- `proposalId`: string — required

## `comic.storyboard.*`

### `comic.storyboard.accept`

**Accept Storyboard** · retry: state-changing · undo: project

- `proposalId`: string — required
- `sceneKeys`: array — Scenes to accept. Omit for every scene. Unlisted scenes are left completely untouched.
- `chapterId`: string — Chapter to place new scenes in. Defaults to the first chapter.
- `preserveExisting`: boolean — Keep approved content on a page that already exists and refresh only descriptions. Default true.
- `note`: string

### `comic.storyboard.generate`

**Generate Storyboard** · retry: state-changing · undo: project

- `proposalId`: string — Regenerate this proposal from its stored source, carrying its ids.
- `text`: string — New source instead of a stored proposal.
- `title`: string
- `maxPanelsPerPage`: integer — min 1 — max 6

### `comic.storyboard.list`

**List Storyboards** · retry: safe

- `status`: string — one of "proposed", "approved", "rejected", "superseded"
- `includeText`: boolean

### `comic.storyboard.reject`

**Reject Storyboard** · retry: idempotent · undo: project

- `proposalId`: string — required
- `note`: string

## Transports

- **HTTP:** `POST /api/comic/<command>` with a JSON body; `GET` for read-only calls.
- **MCP:** `node server/mcp.js --headless`; `tools/list` and `tools/call` are generated from this registry.
- **CLI:** `node server/cli.js run --cmd <id> --params '<json>'`.
- **Browser:** the same dispatch function in `public/app.js`.
