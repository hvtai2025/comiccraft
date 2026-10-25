//! The ComicCraft project model: the bundle schema, stable comic ids and the narrative entities
//! a comic is made of.
//!
//! This crate is **pure data plus the bundle reader/writer**. It has no network, no AI and no UI,
//! and it never depends on `engine`, `ui-egui` or `mcp` — the arrow points one way, from the
//! command layer into this model. See `docs/comiccraft-design.md` §4.2 for the layer choice and
//! §5 for the entity set.
//!
//! The wire format is frozen in `docs/comiccraft-project-schema.md`; this crate implements it.
//! Two rules from the design are structural and are not relaxed here:
//!
//! 1. **No comic data in the visual document.** No comic field goes into `crates/doc`, no comic
//!    section into `crates/format`, and never `Document::unknown` / `Document::extra`. Comic
//!    metadata lives beside the document and reaches visual objects by `NodeId` and
//!    `artboard_id`.
//! 2. **No duplicated authority.** The artboard owns the page size, the document object owns frame
//!    geometry, and a comic record owns the id and the meaning.
//!
//! A third rule follows from the layer table: `crates/format` is L3 and this crate is L2, so the
//! visual document is **never parsed here**. The caller loads it and hands over
//! [`DocumentFacts`]; that is the one step of §10's save protocol this crate delegates, and it is
//! delegated to the only layer that can perform it.
//!
//! **The reader accepts, the writer refuses.** §5.7 gives the two halves of the format different
//! duties — "A reader must accept any valid JSON; a **writer** must produce this form" — and §14
//! requires a damaged bundle to stay openable and repairable rather than locking its creator out.
//! So [`load`] reports what it finds as [`Finding`]s and refuses only the two failures §14 names,
//! while [`save`] refuses to write a file that would not be canonical: a timestamp outside §5.5's
//! one form, or a path that is not bundle-relative. When a caller wants the writer's answer at
//! open time — to warn before the creator loses work — it asks the same function the writer uses:
//! [`ComicBundle::non_canonical_timestamps`].
//!
//! # Reading a bundle
//!
//! ```no_run
//! # use std::path::Path;
//! # use vectorcraft_comic_model::{DocumentFacts, ValidationInput, load, validate};
//! # fn main() -> Result<(), vectorcraft_comic_model::Error> {
//! let root = Path::new("example.comiccraft");
//! let loaded = load(root)?;
//! let document = DocumentFacts::default(); // built from the loaded `vectorcraft_doc::Document`
//! let findings = validate(&loaded.bundle, &ValidationInput { root, document: &document, archive_root: None });
//! for finding in findings {
//!     println!("{finding}");
//! }
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

pub mod bundle;
pub mod document;
pub mod entities;
pub mod error;
pub mod ids;
pub mod json;
pub mod migrate;
pub mod validate;

#[cfg(test)]
mod scratch;

pub use bundle::{
    AssetIndexFile, ChaptersFile, CharactersFile, ComicBundle, JobFile, LoadedBundle, LocationsFile, Manifest, PageFile, ProvenanceFile, SaveOptions,
    ScenesFile, StylesFile, check_supported, load, parse_job_file_name, parse_page_file_name, save,
};
pub use document::{ArtboardFacts, DocumentFacts, DocumentPayload};
pub use entities::*;
pub use error::Error;
pub use ids::{
    AssetId, AssetRegistryId, ChapterId, CharacterId, ComicId, DialogueId, IdAllocator, IdNewtype, JobId, LetteringId, LocationId, PageId, PanelId,
    ReviewDecisionId, SceneId, StyleProfileId,
};
pub use json::{join_relative, path_within, scan_tree, sha256_hex, to_canonical_bytes};
pub use migrate::{Migration, migrate};
pub use validate::{Finding, PageGeometry, Severity, ValidationInput, page_geometry, validate};

/// The comic bundle schema version this crate implements.
///
/// Must equal `manifest.schema_version` in `docs/comiccraft-project-schema.md` §2. Bumping it
/// requires a migration and a fixture for the previous version (schema §11); v2 is `Panel.locked`
/// (§6.2), and [`migrate`] is its v1 → v2 step, `tests/fixtures/v1/` its previous-version fixture
/// and `tests/fixtures/v2/` the canonical form it writes.
pub const SCHEMA_VERSION: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_the_frozen_schema() {
        // The schema document is frozen at the version this crate implements; the anti-drift half
        // of this test is `tests/schema_contract.rs`, which reads the document. v1 → v2 is the one
        // deliberate bump so far (`Panel.locked`, §6.2) and it came with a migration
        // (`migrate::migrate`) and both fixtures (§11.1), so this line makes the next bump loud:
        // an accidental change here is a bundle format nobody migrated.
        assert_eq!(SCHEMA_VERSION, 2);
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

    #[test]
    fn the_public_names_the_other_implementations_compile_against_are_exported() {
        // A compile-time check that the contract is exported. The other M1 implementations name
        // these types, so removing one is a breaking change they must hear about.
        let _: fn(&std::path::Path) -> Result<LoadedBundle, Error> = load;
        let _: fn(&mut ComicBundle, &std::path::Path, &SaveOptions) -> Result<(), Error> = save;
        let _: fn(&ComicBundle, &ValidationInput<'_>) -> Vec<Finding> = validate;
        let _: fn(&ComicBundle) -> u64 = ComicBundle::highest_comic_id;
        let _: fn(&mut ComicBundle) -> Option<u64> = ComicBundle::repair_id_counter;
        let _: fn(PageId) -> String = ComicBundle::page_file_key;
        let _: fn(JobId) -> String = ComicBundle::job_file_key;
        let _: fn() -> usize = std::mem::size_of::<Manifest>;
        let _: fn() -> usize = std::mem::size_of::<ComicProject>;
        let _: fn() -> usize = std::mem::size_of::<ProjectMetadata>;
        let _: fn() -> usize = std::mem::size_of::<ProjectSettings>;
        let _: fn() -> usize = std::mem::size_of::<Chapter>;
        let _: fn() -> usize = std::mem::size_of::<Scene>;
        let _: fn() -> usize = std::mem::size_of::<Page>;
        let _: fn() -> usize = std::mem::size_of::<Panel>;
        let _: fn() -> usize = std::mem::size_of::<Dialogue>;
        let _: fn() -> usize = std::mem::size_of::<Lettering>;
        let _: fn() -> usize = std::mem::size_of::<Character>;
        let _: fn() -> usize = std::mem::size_of::<Location>;
        let _: fn() -> usize = std::mem::size_of::<StyleProfile>;
        let _: fn() -> usize = std::mem::size_of::<Asset>;
        let _: fn() -> usize = std::mem::size_of::<GenerationJob>;
        let _: fn() -> usize = std::mem::size_of::<ReviewDecision>;
        let _: fn() -> usize = std::mem::size_of::<ArtworkRef>;
        let _: fn() -> usize = std::mem::size_of::<ContinuityConstraints>;
        let _: fn() -> usize = std::mem::size_of::<TextLayoutSettings>;
        let _: fn() -> usize = std::mem::size_of::<VisualDocumentRef>;
        let _: fn() -> usize = std::mem::size_of::<AppInfo>;
        let _: fn() -> usize = std::mem::size_of::<FileEntry>;
        let _: fn() -> usize = std::mem::size_of::<StorageRef>;
        let _: fn() -> usize = std::mem::size_of::<ContentHash>;
        let _: fn() -> usize = std::mem::size_of::<MediaMetadata>;
        let _: fn() -> usize = std::mem::size_of::<Provenance>;
        let _: fn() -> usize = std::mem::size_of::<DocumentFacts>;
        let _: fn() -> usize = std::mem::size_of::<DocumentPayload>;
        let _: fn() -> usize = std::mem::size_of::<IdAllocator>;
        let _: fn() -> usize = std::mem::size_of::<Finding>;
    }
}
