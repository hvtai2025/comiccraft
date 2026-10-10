//! The ComicCraft project model: the bundle schema, stable comic ids and the narrative
//! entities a comic is made of.
//!
//! This crate is **pure data plus the bundle reader/writer**. It has no network, no AI and no
//! UI, and it never depends on `engine`, `ui-egui` or `mcp` — the arrow points one way, from
//! the command layer into this model. See `docs/comiccraft-design.md` §4.2 for the layer
//! choice and §5 for the entity set.
//!
//! The wire format is frozen in `docs/comiccraft-project-schema.md`; this crate implements it.
//! Two rules from the design are structural and are not relaxed here:
//!
//! 1. **No comic data in the visual document.** No comic field goes into `crates/doc`, no
//!    comic section into `crates/format`, and never `Document::unknown` / `Document::extra`.
//!    Comic metadata lives beside the document and reaches visual objects by `NodeId` and
//!    `artboard_id`.
//! 2. **No duplicated authority.** The artboard owns the page size, the document object owns
//!    frame geometry, and a comic record owns the id and the meaning.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The comic bundle schema version this crate implements.
///
/// Must equal `manifest.schema_version` in `docs/comiccraft-project-schema.md` §2. Bumping it
/// requires a migration and a fixture for the previous version (schema §11).
pub const SCHEMA_VERSION: u32 = 1;

/// A stable comic entity id.
///
/// Ids are opaque: never derive meaning from magnitude or order, never use an array index as
/// an id, and **never reuse** one (schema §9). Allocated from `manifest.next_comic_id`, which
/// only ever increases.
///
/// Serialized transparently, so a JSON id is a bare number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComicId(pub u64);

impl std::fmt::Display for ComicId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_the_frozen_schema() {
        // The schema document freezes version 1 for M1; a change here must come with a
        // migration and a fixture, so this test makes an accidental bump loud.
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn comic_id_serializes_transparently() {
        let json = serde_json::to_string(&ComicId(7)).expect("ComicId serializes");
        assert_eq!(json, "7", "an id must be a bare number in JSON, not a wrapped object");
        let back: ComicId = serde_json::from_str(&json).expect("ComicId deserializes");
        assert_eq!(back, ComicId(7));
    }

    #[test]
    fn comic_id_is_a_bare_number_not_a_string() {
        // A quoted id would silently break cross-references written by another tool.
        assert!(serde_json::from_str::<ComicId>("\"7\"").is_err());
    }
}
