//! The comic entity types — `docs/comiccraft-project-schema.md` §5.2–§5.6, §6, §7 and §8, field
//! for field.
//!
//! Two rules from the design are structural and are not relaxed here:
//!
//! 1. **No duplicated authority.** There is no `Page.page_size` (the artboard's `rect` is the
//!    page's size) and no `Panel.geometry_ref` (the frame object owns the geometry). A comic
//!    record holds the id and the meaning; the visual document holds the geometry (design §6.1).
//! 2. **One owner per link.** `Lettering.dialogue_id` is the only balloon↔dialogue link;
//!    `Dialogue` has no `balloon_id`. A field that needs a second field to stay consistent is the
//!    bug this architecture exists to prevent (design §5.2.1).
//!
//! Every **entity** struct carries `#[serde(flatten)] unknown`, so a field a newer ComicCraft
//! wrote survives a load-and-save by an older one (§12). The small value types §12 names —
//! `PageSize`, `TextLayoutSettings`, `ArtworkRef`, `ContinuityConstraints`, `StorageRef`,
//! `ContentHash`, `MediaMetadata` — deliberately do not: they are closed shapes whose extra keys
//! are more likely a typo than a future field.
//!
//! `NodeId` and the page's `artboard_id` are taken from `vectorcraft-doc` and `Artboard.id`
//! respectively; this crate never mints a visual-object id (§9).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use vectorcraft_geom::Size;

// Re-exported rather than wrapped: §9 takes visual-object identity from the engine, and a second
// `NodeId` here would be exactly the duplicated identity the schema forbids.
pub use vectorcraft_doc::NodeId;

use crate::ids::{
    AssetId, AssetRegistryId, ChapterId, CharacterId, ComicId, DialogueId, JobId, LetteringId, LocationId, PageId, PanelId, ReviewDecisionId,
    SceneId, StyleProfileId,
};

/// The flattened map every entity carries, so an unknown key survives a round trip (§12).
pub type Unknown = Map<String, Value>;

/// An RFC 3339 timestamp **in UTC, second precision, `Z` suffix** (§5.5), e.g.
/// `2026-10-10T21:52:00Z`.
///
/// Text, not an integer, so a bundle is readable without knowing an epoch convention. There is
/// one canonical form and no other, because §5.7's byte-identical round trip depends on it; the
/// writer refuses anything else ([`is_canonical_timestamp`]).
pub type Timestamp = String;

/// Whether `value` is the one canonical timestamp form §5.5 fixes.
///
/// `YYYY-MM-DDTHH:MM:SSZ`, ASCII, exactly 20 bytes, with the ranges a calendar and a clock
/// allow. A `+00:00` offset, a fractional second or a missing `Z` is not canonical, and a writer
/// that produced one could not round-trip byte-identically (§5.7).
#[must_use]
pub fn is_canonical_timestamp(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 20 || !value.is_ascii() {
        return false;
    }
    let digits_at = |range: std::ops::Range<usize>| b[range].iter().all(u8::is_ascii_digit);
    if !(digits_at(0..4) && digits_at(5..7) && digits_at(8..10) && digits_at(11..13) && digits_at(14..16) && digits_at(17..19)) {
        return false;
    }
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return false;
    }
    let field = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(u32::MAX);
    let (month, day, hour, minute, second) = (field(5..7), field(8..10), field(11..13), field(14..16), field(17..19));
    (1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60 && second < 60
}

/// How a book is read (§5.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingDirection {
    /// Left to right.
    #[default]
    Ltr,
    /// Right to left.
    Rtl,
}

/// Whether a record a human must approve has been (§5.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Waiting for a decision.
    #[default]
    Pending,
    /// Accepted.
    Approved,
    /// Turned down.
    Rejected,
}

/// Where a page is in production (§5.5, §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    /// Being composed.
    #[default]
    Draft,
    /// Artwork generated.
    Generated,
    /// Lettered.
    Lettered,
    /// Approved.
    Approved,
    /// Published.
    Published,
}

/// Whether a panel's artwork has been generated (§5.5, §6.2).
///
/// Distinct from [`JobStatus`]: this is the panel's own state, not the state of one request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStatus {
    /// No artwork requested.
    #[default]
    None,
    /// A request is waiting.
    Queued,
    /// A request is in flight.
    Running,
    /// Artwork exists.
    Completed,
    /// The request failed.
    Failed,
}

/// What a lettering object is (§5.5, §6.4). One entity covers balloons, captions and sound
/// effects because their difference is presentation, not structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LetteringKind {
    /// A speech balloon.
    Speech,
    /// A thought balloon.
    Thought,
    /// A caption box.
    Narration,
    /// A shout.
    Shout,
    /// A whisper.
    Whisper,
    /// A sound effect.
    SoundEffect,
}

impl LetteringKind {
    /// Whether this kind is *said*, so `Lettering.dialogue_id` must be set and `text` must be
    /// `null` — the linked `Dialogue` owns the words (§6.4, rule 8).
    #[must_use]
    pub fn is_spoken(self) -> bool {
        matches!(self, Self::Speech | Self::Thought | Self::Shout | Self::Whisper)
    }

    /// Whether this kind carries its own text, so `Lettering.text` must be set and `dialogue_id`
    /// must be `null` (§6.4, rule 8).
    #[must_use]
    pub fn carries_own_text(self) -> bool {
        matches!(self, Self::Narration | Self::SoundEffect)
    }
}

/// What to do when lettering does not fit its frame (§5.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowPolicy {
    /// Report it.
    #[default]
    Warn,
    /// Shrink the text to fit.
    Shrink,
    /// Resize the frame.
    ResizeFrame,
}

/// Text alignment inside a lettering frame (§5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    /// Ragged right.
    Left,
    /// Centred.
    Center,
    /// Ragged left.
    Right,
}

/// Where an asset came from (§5.5, §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    /// Imported.
    Source,
    /// Produced by a provider.
    Generated,
    /// Derived by an edit.
    Edited,
    /// A reference the prompts carry.
    Reference,
}

impl AssetType {
    /// Whether §13 rule 10 requires `Asset.provenance`: `generated` and `edited` must say what
    /// produced them, and every other type must not claim to.
    #[must_use]
    pub fn requires_provenance(self) -> bool {
        matches!(self, Self::Generated | Self::Edited)
    }
}

/// What an artwork node does inside a panel (§5.5, §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkRole {
    /// The panel's main artwork.
    Primary,
    /// Behind it.
    Background,
    /// In front of it.
    Overlay,
}

/// What a provider was asked to do (§5.5, §8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobOperation {
    /// Text to image.
    TextToImage,
    /// Conditioned on a reference.
    ReferenceConditioned,
    /// Image to image.
    ImageToImage,
    /// Inpainting.
    Inpaint,
    /// Variations of an existing image.
    Variations,
}

/// Where a generation job is in its life (§5.5, §8.1).
///
/// This is the bundle's wire enum, defined here because §5.5 says an enum a table names inline is
/// defined where it is used and §8.1 names it in `GenerationJob.status`.
///
/// **This is the one definition.** `crates/comic-ai` (L3) drives the job state machine and must
/// re-export this type rather than carrying its own copy: two enums with the same wire names are
/// the "two fields that must stay in step" defect §5.2.1 forbids. `crates/comic-ai` still has its
/// own copy from M0, before this crate had the type; removing it is a one-line re-export there,
/// and the doc-claim test that reads `crates/comic-ai/src/lib.rs` for `pub fn is_terminal` must
/// follow the predicate to this file in the same change. Both are the lead's, not this crate's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Accepted, not yet sent to a provider.
    Queued,
    /// Sent; awaiting a response.
    Running,
    /// A result exists and is being validated.
    Completed,
    /// The provider or the transport failed.
    Failed,
    /// A result exists and is waiting for a human decision (design §9.4).
    AwaitingReview,
    /// Stopped locally, and remotely where the provider supports it.
    Cancelled,
}

impl JobStatus {
    /// Whether the job has stopped and will not change again without a new job.
    ///
    /// `AwaitingReview` is deliberately **not** terminal: the result exists but the job is still
    /// awaiting a decision, and `comic.job.apply` or `comic.job.reject` may still act on it. Rule
    /// 18 is written in terms of this predicate, and §8.3 removes a job's file exactly at a
    /// terminal status.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled)
    }

    /// Whether a result asset is available to apply.
    #[must_use]
    pub fn has_result(self) -> bool {
        matches!(self, Self::Completed | Self::AwaitingReview)
    }
}

/// Why a job failed (§5.5, §8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobErrorKind {
    /// The provider refused or errored.
    Provider,
    /// The request never arrived.
    Transport,
    /// A result came back that could not be used.
    InvalidOutput,
    /// Cancelled locally.
    Cancelled,
}

/// What a review decision is about (§5.5, §8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubject {
    /// A panel's artwork.
    PanelArtwork,
    /// Story content.
    StoryContent,
    /// A character design.
    CharacterDesign,
    /// A batch edit.
    BatchEdit,
    /// A publication.
    Publication,
}

/// What a reviewer decided (§5.5, §8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Accepted.
    Approved,
    /// Turned down.
    Rejected,
    /// Replaced by a later decision.
    Superseded,
}

/// A provider's identity, as recorded in every asset's provenance (§5.5).
///
/// Recorded verbatim with the model name and the exact request, so a generated asset can always
/// be traced back to what produced it (design §9.3).
///
/// **This is the one definition.** `crates/comic-ai` (L3) re-exports it; it does not carry its own
/// copy, because two newtypes with the same wire form are the "two fields that must stay in step"
/// defect §5.2.1 forbids. `crates/comic-ai` still has its own copy from M0, before this crate had
/// the type; removing it is a one-line re-export there, owned by the lead.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(pub String);

/// The informed-consent record design §9.3 and ADR-015 require before artwork leaves the machine
/// (§5.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Consent {
    /// When consent was given.
    pub granted: Timestamp,
    /// What was consented to.
    pub statement: String,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Where an asset's bytes live (§7.1).
///
/// An enum, so the storage policy is explicit and one place changes. The three bases are **not**
/// interchangeable and confusing any two is a path-traversal bug (§3, rule 9): `Linked` resolves
/// against the **bundle root**, `Package` against the **archive root**, and the document's own
/// `LinkInfo.relative` against the **document folder**. `Embedded` is a key in the visual
/// document's image map, not a path at all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageRef {
    /// A file under `assets/`, **bundle-relative**. The default.
    Linked {
        /// The path, `/`-separated, from the bundle root.
        relative_path: String,
    },
    /// A key in the visual document's image map, for portability.
    Embedded {
        /// The image map key.
        image_key: String,
    },
    /// A file inside a packaged archive (design §7.5), relative to the **archive** root.
    Package {
        /// The path, `/`-separated, from the archive root.
        relative_path: String,
    },
}

impl StorageRef {
    /// The path this reference names, when it names one, and the base it must be resolved
    /// against (§13 rule 9). `Embedded` names a key, not a path.
    #[must_use]
    pub fn path(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::Linked { relative_path } => Some(("bundle root", relative_path)),
            Self::Package { relative_path } => Some(("archive root", relative_path)),
            Self::Embedded { .. } => None,
        }
    }
}

/// The algorithm `ContentHash` uses today (§7.2).
pub const HASH_ALGORITHM_SHA256: &str = "sha256";

/// An asset's content hash (§7.2).
///
/// Not `LinkInfo.hash`: that is FNV-1a 64-bit and is a cheap change detector, not an integrity
/// guarantee.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash {
    /// The algorithm. A reader implements [`HASH_ALGORITHM_SHA256`]; rule 16 reports any other.
    pub algorithm: String,
    /// Lowercase hex.
    pub value: String,
}

/// What an asset's file is (§5.5, §7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    /// The MIME type.
    pub mime: String,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// The ICC profile, by name.
    pub color_profile: Option<String>,
    /// Resolution in dots per inch.
    pub dpi: Option<f64>,
}

/// Where an asset came from and what it was made from (§5.5, §8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Who produced it.
    pub provider: ProviderId,
    /// The model's name.
    pub model: String,
    /// The exact request sent, kept verbatim. A map, so its keys are written sorted (§5.7).
    pub request: Value,
    /// The assets the request was conditioned on.
    pub input_asset_ids: Vec<AssetId>,
    /// When it was produced.
    pub created: Timestamp,
    /// The consent recorded before the artwork left the machine (ADR-015).
    pub consent: Option<Consent>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// How far a job has got (§5.5, §8.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Steps done.
    pub done: u32,
    /// Steps expected, when the provider says.
    pub total: Option<u32>,
    /// What is happening, for a human.
    pub note: Option<String>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Why a job failed (§5.5, §8.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobError {
    /// The failure's kind.
    pub kind: JobErrorKind,
    /// What happened.
    pub message: String,
    /// Whether trying again could help.
    pub retryable: bool,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// The default size for new pages, in **document points** (§5.3).
///
/// Mirrors `Artboard.rect`. It is a default for new pages, **never** a second copy of a page's
/// size: the artboard *is* the page's size, and rule 15 only warns when the two differ.
pub type PageSize = Size;

/// A page's visual home: the artboard's id, and the format version of the document that holds it
/// (§5.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisualDocumentRef {
    /// The document's path, from the bundle root, `/`-separated.
    pub relative_path: String,
    /// The `format::File.version` last written.
    pub format_version: u32,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// What wrote a bundle, for support (§5.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppInfo {
    /// The application's name.
    pub name: String,
    /// The application's version.
    pub version: String,
    /// The `schema_version` it wrote.
    pub schema: u32,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// One `manifest.files` value: a hash and a size (§5.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Lowercase hex sha256.
    pub sha256: String,
    /// The size in bytes.
    pub bytes: u64,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// The project root: title, settings, libraries and the visual document reference (§5.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComicProject {
    /// The project's id. Equals `manifest.comic_id` (rule 3).
    pub id: ComicId,
    /// Must equal `manifest.schema_version` (§5.1, rule 3).
    pub schema_version: u32,
    /// The project's title. `manifest.title` is a denormalised copy.
    pub title: String,
    /// Author, language, genre, reading direction and timestamps (§5.2).
    pub metadata: ProjectMetadata,
    /// The book order.
    pub chapter_ids: Vec<ChapterId>,
    /// Every character in the library.
    pub character_ids: Vec<CharacterId>,
    /// Every location in the library.
    pub location_ids: Vec<LocationId>,
    /// Every style profile in the library.
    pub style_profile_ids: Vec<StyleProfileId>,
    /// The `assets/index.json` registry (§5.5).
    pub asset_registry_id: AssetRegistryId,
    /// Defaults for new pages and lettering (§5.3).
    pub settings: ProjectSettings,
    /// The one visual document (ADR-011). Must equal `manifest.document` (rule 3).
    pub visual_document: VisualDocumentRef,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A project's descriptive metadata (§5.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// The author.
    pub author: Option<String>,
    /// The language, as a BCP-47 tag.
    pub language: Option<String>,
    /// The genre.
    pub genre: Option<String>,
    /// How the book is read. Defaults to `ltr` when absent.
    #[serde(default)]
    pub reading_direction: ReadingDirection,
    /// When the project was created.
    pub created: Timestamp,
    /// When it was last saved.
    pub updated: Timestamp,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Defaults for new pages and lettering (§5.3).
///
/// `default_lettering_style` is a **name**, not a font file: the faces live in
/// `storytold/craft-fonts` and are never committed here. A build without them still loads and
/// the missing face is a lettering check (design §12.1), not a referential rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// The default size for new pages, in points. Never a page's own size (§6.1).
    pub default_page_size: Option<PageSize>,
    /// The template new pages start from.
    pub default_template: Option<String>,
    /// The lettering style new lettering starts from, by name.
    pub default_lettering_style: Option<String>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A book-level division (§5.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    /// The chapter's id.
    pub id: ChapterId,
    /// The project it belongs to.
    pub project_id: ComicId,
    /// The chapter's title.
    pub title: String,
    /// Its scenes, in order.
    pub scene_ids: Vec<SceneId>,
    /// Its position in the book.
    pub order_index: u32,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A narrative unit that may span several pages (§5.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// The scene's id.
    pub id: SceneId,
    /// The chapter it belongs to.
    pub chapter_id: ChapterId,
    /// The scene's title.
    pub title: String,
    /// What happens, for the prompts.
    pub synopsis: String,
    /// The pages it spans.
    pub page_ids: Vec<PageId>,
    /// Where it happens.
    pub location_id: Option<LocationId>,
    /// Who is in it.
    pub character_ids: Vec<CharacterId>,
    /// Its position in the chapter.
    pub order_index: u32,
    /// Invariants the scene must keep.
    pub continuity_notes: String,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A character's identity, approved design and continuity rules (§5.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Character {
    /// The character's id.
    pub id: CharacterId,
    /// The character's name.
    pub name: String,
    /// The description every prompt carries.
    pub canonical_description: String,
    /// Reference images.
    pub reference_asset_ids: Vec<AssetId>,
    /// The style profile it is drawn in.
    pub style_profile_id: Option<StyleProfileId>,
    /// The approved design. An **immutable** asset reference: approving a new design creates a
    /// new `Asset` and repoints this field; it never rewrites the old asset (design §7.4).
    pub approved_design_version: Option<AssetId>,
    /// Invariants generation prompts must carry.
    pub continuity_rules: Vec<String>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A reusable setting (§5.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// The location's id.
    pub id: LocationId,
    /// The location's name.
    pub name: String,
    /// The description every prompt carries.
    pub description: String,
    /// Reference images.
    pub reference_asset_ids: Vec<AssetId>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Visual-style constraints for generation and lettering (§5.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleProfile {
    /// The profile's id.
    pub id: StyleProfileId,
    /// The profile's name.
    pub name: String,
    /// What the style is.
    pub description: String,
    /// Hex colours the style uses.
    pub palette: Vec<String>,
    /// The stroke weight, in points.
    pub line_weight: Option<f64>,
    /// Notes the prompts carry.
    pub rendering_notes: String,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A narrative page — one artboard (§6.1).
///
/// There is no `page_size` and no `visual_document_id`: the artboard owns the size and
/// `artboard_id` is the page's visual home (design §5.1.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// The page's id. The file name must agree (rule 13).
    pub id: PageId,
    /// The chapter it belongs to.
    pub chapter_id: ChapterId,
    /// The scenes it carries; a scene may span pages.
    pub scene_ids: Vec<SceneId>,
    /// `Artboard.id` in the visual document — the page's visual home.
    pub artboard_id: u32,
    /// The printed folio. May repeat across a book; **never** an identity.
    pub page_number: u32,
    /// Membership.
    pub panel_ids: Vec<PanelId>,
    /// Narrative order; must be a permutation of `panel_ids` (rule 6).
    pub reading_order: Vec<PanelId>,
    /// Where the page is in production.
    pub status: PageStatus,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A shot inside a page: frame, artwork and reading position (§6.2).
///
/// There is no `geometry_ref`: the frame object in the visual document owns the geometry, and the
/// panel reaches it by `frame_node_id` (design §6.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    /// The panel's id.
    pub id: PanelId,
    /// Must agree with the containing file (rule 11).
    pub page_id: PageId,
    /// Denormalised from the page for locality; must equal `Page.artboard_id` (rule 5).
    pub artboard_id: u32,
    /// The scene the panel belongs to.
    pub scene_id: SceneId,
    /// The frame object in the visual document.
    pub frame_node_id: NodeId,
    /// The artwork shown.
    pub artwork_refs: Vec<ArtworkRef>,
    /// The generation brief.
    pub shot_description: String,
    /// The dialogue in the panel.
    pub dialogue_ids: Vec<DialogueId>,
    /// The lettering in the panel.
    pub lettering_ids: Vec<LetteringId>,
    /// Must agree with `Page.reading_order` (rule 6).
    pub reading_order_index: u16,
    /// Where the character rules do not apply (§6.5).
    pub continuity_constraints: ContinuityConstraints,
    /// Whether the artwork has been generated.
    pub generation_status: GenerationStatus,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// One piece of artwork in a panel (§6.2). A closed shape, so it carries no `unknown` (§12).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtworkRef {
    /// The asset shown.
    pub asset_id: AssetId,
    /// The image object that shows it.
    pub node_id: NodeId,
    /// What it does in the panel.
    pub role: ArtworkRole,
}

/// Where a panel's character rules do not apply (§6.5). A closed shape (§12).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContinuityConstraints {
    /// Who is present.
    pub characters: Vec<CharacterId>,
    /// What they are wearing.
    pub wardrobe: Option<String>,
    /// When it happens.
    pub time_of_day: Option<String>,
    /// Earlier moments this echoes.
    pub callbacks: Vec<String>,
    /// Where the character rules do not hold ("she has the coat here", §5.6).
    pub exceptions: Vec<String>,
}

/// What is said, and who says it (§6.3).
///
/// There is no `balloon_id`: `Lettering.dialogue_id` is the single authoritative link
/// (design §5.2.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dialogue {
    /// The dialogue's id.
    pub id: DialogueId,
    /// The panel it is in.
    pub panel_id: PanelId,
    /// The speaker, or `null` for narration (§6.3).
    pub speaker_character_id: Option<CharacterId>,
    /// **The authoritative text.**
    pub text: String,
    /// The language, as a BCP-47 tag.
    pub language: Option<String>,
    /// Its position in the panel's reading order.
    pub reading_order_index: u16,
    /// Whether a human approved it.
    pub approval_status: ApprovalStatus,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// How dialogue is presented: a balloon, a caption or a sound effect (§6.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lettering {
    /// The lettering's id.
    pub id: LetteringId,
    /// The page it is on.
    pub page_id: PageId,
    /// The panel it is in.
    pub panel_id: PanelId,
    /// Must equal `Page.artboard_id` (rule 5).
    pub artboard_id: u32,
    /// What it is.
    pub kind: LetteringKind,
    /// Required for the spoken kinds; must be `null` for `narration`/`sound_effect` (rule 8).
    pub dialogue_id: Option<DialogueId>,
    /// Required for `narration`/`sound_effect`; must be `null` otherwise — the linked `Dialogue`
    /// owns the text (rule 8).
    pub text: Option<String>,
    /// The balloon outline or caption box.
    pub frame_node_id: NodeId,
    /// The tail path, for the kinds that have one.
    pub tail_node_id: Option<NodeId>,
    /// The area text object inside the frame.
    pub text_node_id: NodeId,
    /// Inputs to `vectorcraft_text` (§6.6).
    pub text_layout_settings: TextLayoutSettings,
    /// What to do when the text does not fit.
    pub overflow_policy: OverflowPolicy,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Inputs to `vectorcraft_text` (§6.6). A closed shape (§12).
///
/// The measured result (`TextLayout.bounds`, `.overflow`, `.fit_scale`) is never persisted
/// (design §8.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutSettings {
    /// A family name resolved through the font database.
    pub font: Option<String>,
    /// In points.
    pub size: Option<f64>,
    /// In points.
    pub leading: Option<f64>,
    /// In points.
    pub tracking: Option<f64>,
    /// Alignment.
    pub align: Option<TextAlign>,
    /// In points.
    pub padding: Option<f64>,
    /// Whether to scale the text to fit its frame.
    pub fit_to_frame: bool,
}

/// An imported or generated file, with provenance (§7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// The asset's id.
    pub id: AssetId,
    /// Where it came from.
    pub asset_type: AssetType,
    /// Where its bytes live.
    pub storage_reference: StorageRef,
    /// Its content hash.
    pub content_hash: ContentHash,
    /// What the file is.
    pub media_metadata: MediaMetadata,
    /// Required for `generated` and `edited`, forbidden otherwise (rule 10).
    pub provenance: Option<Provenance>,
    /// Lineage: the generation or edit this derived from.
    pub parent_asset_ids: Vec<AssetId>,
    /// Whether a human approved it.
    pub approval_status: ApprovalStatus,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// One asynchronous provider request (§8.1).
///
/// **Credentials never appear here.** No token, key or endpoint secret is written to a job
/// record, the bundle, an export or a log line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationJob {
    /// The job's id.
    pub id: JobId,
    /// What was asked for.
    pub operation: JobOperation,
    /// The panel the result belongs to, when it is a panel's artwork.
    pub target_panel_id: Option<PanelId>,
    /// What the request was conditioned on.
    pub input_asset_ids: Vec<AssetId>,
    /// Who ran it.
    pub provider: ProviderId,
    /// The model's name.
    pub model: String,
    /// The exact request sent, for provenance.
    pub request: Value,
    /// Where the job is in its life.
    pub status: JobStatus,
    /// How far it has got.
    pub progress: Option<Progress>,
    /// Why it failed.
    pub error: Option<JobError>,
    /// The assets it produced.
    pub result_asset_ids: Vec<AssetId>,
    /// When it started.
    pub created: Timestamp,
    /// When it last changed.
    pub updated: Timestamp,
    /// The `DocState.revision` the request was built against (design §11.3).
    pub target_revision: u64,
    /// The consent recorded before the request left the machine (ADR-015).
    pub consent: Option<Consent>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// An approval or rejection, with actor and time (§8.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReviewDecision {
    /// The decision's id.
    pub id: ReviewDecisionId,
    /// What was reviewed.
    pub subject: ReviewSubject,
    /// What was decided.
    pub decision: Decision,
    /// Who decided.
    pub actor: String,
    /// When.
    pub timestamp: Timestamp,
    /// Why, for a human.
    pub note: Option<String>,
    /// The artwork that was replaced, retained.
    pub superseded_asset_id: Option<AssetId>,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_terminal_job_statuses_are_terminal() {
        for status in [JobStatus::Queued, JobStatus::Running, JobStatus::Completed, JobStatus::AwaitingReview] {
            assert!(!status.is_terminal(), "{status:?} must keep its job file alive (rule 18)");
        }
        for status in [JobStatus::Failed, JobStatus::Cancelled] {
            assert!(status.is_terminal(), "{status:?} is terminal (rule 18)");
            assert!(!status.has_result(), "{status:?} has nothing to apply");
        }
        assert!(JobStatus::Completed.has_result());
        assert!(JobStatus::AwaitingReview.has_result());
    }

    #[test]
    fn lettering_kinds_split_into_spoken_and_self_texted() {
        for kind in [LetteringKind::Speech, LetteringKind::Thought, LetteringKind::Shout, LetteringKind::Whisper] {
            assert!(kind.is_spoken() && !kind.carries_own_text(), "{kind:?} is said, so a Dialogue owns its words");
        }
        for kind in [LetteringKind::Narration, LetteringKind::SoundEffect] {
            assert!(kind.carries_own_text() && !kind.is_spoken(), "{kind:?} carries its own text");
        }
    }

    #[test]
    fn only_generated_and_edited_assets_require_provenance() {
        assert!(AssetType::Generated.requires_provenance());
        assert!(AssetType::Edited.requires_provenance());
        assert!(!AssetType::Source.requires_provenance());
        assert!(!AssetType::Reference.requires_provenance());
    }

    #[test]
    fn storage_refs_name_their_base() {
        assert_eq!(StorageRef::Linked { relative_path: "assets/a.png".into() }.path(), Some(("bundle root", "assets/a.png")));
        assert_eq!(StorageRef::Package { relative_path: "a.png".into() }.path(), Some(("archive root", "a.png")));
        assert_eq!(StorageRef::Embedded { image_key: "k".into() }.path(), None, "a key is not a path");
    }

    #[test]
    fn canonical_timestamps_are_the_one_form() {
        assert!(is_canonical_timestamp("2026-10-10T21:52:00Z"));
        for bad in [
            "2026-10-10T21:52:00+00:00",
            "2026-10-10T21:52:00.000Z",
            "2026-10-10T21:52:00",
            "2026-10-10 21:52:00Z",
            "2026-13-10T21:52:00Z",
            "2026-10-10T24:00:00Z",
            "2026-10-10T21:52:60Z",
            "",
            "2026-10-10t21:52:00z",
        ] {
            assert!(!is_canonical_timestamp(bad), "{bad:?} is not the canonical form");
        }
    }

    #[test]
    fn enums_use_the_snake_case_wire_names() {
        assert_eq!(serde_json::to_string(&ReadingDirection::Ltr).expect("serializes"), "\"ltr\"");
        assert_eq!(serde_json::to_string(&PageStatus::Generated).expect("serializes"), "\"generated\"");
        assert_eq!(serde_json::to_string(&LetteringKind::SoundEffect).expect("serializes"), "\"sound_effect\"");
        assert_eq!(serde_json::to_string(&JobStatus::AwaitingReview).expect("serializes"), "\"awaiting_review\"");
        assert_eq!(serde_json::to_string(&JobErrorKind::InvalidOutput).expect("serializes"), "\"invalid_output\"");
        assert_eq!(serde_json::to_string(&ReviewSubject::PanelArtwork).expect("serializes"), "\"panel_artwork\"");
        assert_eq!(serde_json::to_string(&OverflowPolicy::ResizeFrame).expect("serializes"), "\"resize_frame\"");
        assert_eq!(serde_json::to_string(&JobOperation::TextToImage).expect("serializes"), "\"text_to_image\"");
        assert_eq!(serde_json::to_string(&ArtworkRole::Primary).expect("serializes"), "\"primary\"");
        assert_eq!(serde_json::to_string(&Decision::Superseded).expect("serializes"), "\"superseded\"");
    }

    #[test]
    fn a_reading_direction_defaults_to_left_to_right() {
        // §5.2: `reading_direction` is `ltr` | `rtl`, default `ltr`.
        let metadata: ProjectMetadata =
            serde_json::from_str(r#"{"author":null,"language":null,"genre":null,"created":"2026-10-10T21:52:00Z","updated":"2026-10-10T21:52:00Z"}"#)
                .expect("a metadata object without reading_direction loads");
        assert_eq!(metadata.reading_direction, ReadingDirection::Ltr);
    }

    #[test]
    fn an_unknown_field_survives_a_round_trip_on_an_entity() {
        // §12: a field a newer ComicCraft wrote must survive a load-and-save by an older one.
        let json = r#"{"id":7,"name":"Ada","canonical_description":"","reference_asset_ids":[],
            "style_profile_id":null,"approved_design_version":null,"continuity_rules":[],
            "hair_colour":"auburn"}"#;
        let character: Character = serde_json::from_str(json).expect("loads");
        assert_eq!(character.unknown.get("hair_colour").and_then(Value::as_str), Some("auburn"));
        let back = serde_json::to_string(&character).expect("serializes");
        assert!(back.contains("\"hair_colour\":\"auburn\""), "the unknown field must be written back: {back}");
    }

    #[test]
    fn a_closed_value_type_has_nowhere_to_keep_an_extra_key() {
        // §12: `PageSize` and `ContinuityConstraints` are two of the seven closed shapes, so an
        // extra key is a typo rather than a future field, and there is no map to keep it in.
        // §5.7 still requires the *reader* to accept any valid JSON, so the key loads and is
        // dropped — the difference from an entity is that it cannot come back.
        let size: PageSize = serde_json::from_str(r#"{"width":612.0,"height":792.0}"#).expect("loads");
        assert_eq!(size.width, 612.0);
        assert_eq!(size.height, 792.0);
        let constraints: ContinuityConstraints =
            serde_json::from_str(r#"{"characters":[],"wardrobe":null,"time_of_day":null,"callbacks":[],"exceptions":[],"note":"x"}"#)
                .expect("a reader accepts any valid JSON (§5.7)");
        let back = serde_json::to_string(&constraints).expect("serializes");
        assert!(!back.contains("note"), "a closed shape has nowhere to keep an extra key: {back}");
    }

    #[test]
    fn optional_fields_are_written_as_null_rather_than_omitted() {
        // §6.3 and §6.4 mark `Option` fields "Req: yes" — the key is present with `null`.
        let dialogue = Dialogue {
            id: DialogueId(1),
            panel_id: PanelId(2),
            speaker_character_id: None,
            text: "…".into(),
            language: None,
            reading_order_index: 0,
            approval_status: ApprovalStatus::Pending,
            unknown: Unknown::new(),
        };
        let json = serde_json::to_string(&dialogue).expect("serializes");
        assert!(json.contains("\"speaker_character_id\":null"), "{json}");
        assert!(json.contains("\"language\":null"), "{json}");
    }

    #[test]
    fn field_order_is_the_order_the_schema_declares() {
        // §5.7: a struct's keys are written in declared order, not sorted — which is what makes
        // one byte sequence canonical.
        let project = ComicProject {
            id: ComicId(1),
            schema_version: 1,
            title: "T".into(),
            metadata: ProjectMetadata {
                author: None,
                language: None,
                genre: None,
                reading_direction: ReadingDirection::Ltr,
                created: "2026-10-10T21:52:00Z".into(),
                updated: "2026-10-10T21:52:00Z".into(),
                unknown: Unknown::new(),
            },
            chapter_ids: vec![],
            character_ids: vec![],
            location_ids: vec![],
            style_profile_ids: vec![],
            asset_registry_id: ComicId(2),
            settings: ProjectSettings { default_page_size: None, default_template: None, default_lettering_style: None, unknown: Unknown::new() },
            visual_document: VisualDocumentRef { relative_path: "document/comic.vectorcraft".into(), format_version: 4, unknown: Unknown::new() },
            unknown: Unknown::new(),
        };
        let json = serde_json::to_string(&project).expect("serializes");
        let keys: Vec<&str> = [
            "\"id\"",
            "\"schema_version\"",
            "\"title\"",
            "\"metadata\"",
            "\"chapter_ids\"",
            "\"asset_registry_id\"",
            "\"settings\"",
            "\"visual_document\"",
        ]
        .into_iter()
        .filter(|k| json.contains(k))
        .collect();
        assert_eq!(keys.len(), 8);
        let positions: Vec<usize> = keys.iter().map(|k| json.find(k).unwrap_or(usize::MAX)).collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted, "declared order, not sorted: {json}");
    }
}
