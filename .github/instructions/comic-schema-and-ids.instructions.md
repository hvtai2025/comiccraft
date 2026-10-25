---
description: "Use when editing the ComicCraft comic domain schema, the project bundle, or the document model under shared/domain. Covers id allocation, link ownership, migrations, fixture requirements and the rule that comic data never enters the visual document."
applyTo: ["shared/domain/**", "schema/**"]
---
# Comic schema and ids

`shared/domain` is the whole team's contract, and it runs in two runtimes. A field change
here is a change to every agent's code *and* to both the server and the browser.

## Ids

- Allocate comic ids from the counter persisted in the manifest. **Never reuse an id.**
- Ids are typed strings with a type prefix (`page_`, `panel_`, `balloon_`). **Never use an
  array index** as a persistent identifier, and never `Math.random()`.
- Take visual-object identity from the document model: **numeric** document node ids and
  artboard ids, matching the reference format so a bundle round-trips without renumbering.
  Do not mint your own and do not switch them to strings.
- A node's `name` is a label, not an identity: user-editable, not unique.

## Links

One side owns each relationship (design §5.2.1). If you find yourself writing two fields
that must stay in step — `Dialogue.balloon_id` beside `Balloon.dialogue_id` — the model is
wrong. Keep the owning field and derive the other by lookup.

## Never duplicate

The artboard owns the page size. The document object owns the frame geometry. The comic
record owns the id and the meaning. `page_size`, `geometry_ref` and `bleed_and_trim` are
deliberately *not* fields here (design §6.1).

## Comic data never enters the visual document

No comic field in the document model, no comic section in its format, and never a
round-trip pocket meant for foreign data (ADR-013).

## The artboard-id hazard

The reference allocates `Artboard.id` as `max(id) + 1` with no counter repair, so a deleted
artboard's id can be reused. Keep `retired_artboard_ids` in the manifest, keep artboard
lifecycle in comic commands, and keep the validation cross-check (design §5.3.1). Do not
remove any of the three.

## Purity

Anything in `shared/domain` must load in Node **and** in the browser. No DOM global, no
`node:*` import, no `fetch`. Filesystem access, hashing and HTTP are injected by the caller
— `server/storage` supplies the first two, `server/ai` the third. If you need one, take it
as a parameter; do not import it.

Hashing is standard-library on both sides (`node:crypto`, `crypto.subtle`), which is why
`Asset.content_hash` needs no dependency.

## Migrations

Add the fixture for the previous version **before** changing the schema. A migration must
validate its input, refuse rather than guess, preserve every id, and preserve unknown
fields so a newer ComicCraft can still read an older file.

Every supported version has a fixture under `schema/`, and the fixture gate fails if one is
missing — a schema change without its fixture is not a complete change.

## Save

Write in the design's order (§7.2) and write the manifest **last**, so it can never
reference partially written data.
