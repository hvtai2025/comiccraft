---
description: "Use when editing the ComicCraft comic domain schema or project bundle under crates/comic-model. Covers id allocation, link ownership, migrations, fixture requirements and the rule that comic data never enters the visual document."
applyTo: "crates/comic-model/**"
---
# Comic schema and ids

This crate is the whole team's contract. A field change here is a change to every agent's
code.

## Ids

- Allocate comic ids from the counter persisted in the manifest. **Never reuse an id.**
- **Never use an array index** as a persistent identifier.
- Take visual-object identity from the engine: `NodeId(u64)` for objects
  (`crates/doc/src/node.rs`), `Artboard.id: u32` for a page's artboard. Do not mint your
  own.
- `Node.name` is a label, not an identity: user-editable, not unique.

## Links

One side owns each relationship (design §5.2.1). If you find yourself writing two fields
that must stay in step — `Dialogue.balloon_id` beside `Balloon.dialogue_id` — the model is
wrong. Keep the owning field and derive the other by lookup.

## Never duplicate

The artboard owns the page size. The document object owns the frame geometry. The comic
record owns the id and the meaning. `page_size`, `geometry_ref` and `bleed_and_trim` are
deliberately *not* fields here (design §6.1).

## Comic data never enters the visual document

No comic field in `crates/doc`, no comic section in `crates/format`, and never
`Document::unknown` / `Document::extra` — those round-trip foreign data (ADR-013).

## The artboard-id hazard

`Artboard.id` is `max(id) + 1` with no `fix_next_id()` equivalent, so a deleted artboard's
id can be reused. Keep `retired_artboard_ids` in the manifest, keep artboard lifecycle in
comic commands, and keep the validation cross-check (design §5.3.1). Do not remove any of
the three.

## Migrations

Add the fixture for the previous version **before** changing the schema. A migration must
validate its input, refuse rather than guess, preserve every id, and preserve unknown
fields so a newer ComicCraft can still read an older file.

## Save

Write in the design's order (§7.2) and write the manifest **last**, so it can never
reference partially written data.
