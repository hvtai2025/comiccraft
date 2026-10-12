//! The project bundle: its manifest, its files, the reader and the ordered writer.
//!
//! Schema §3 (layout), §4 (`manifest.json`), §5.4/§6/§7/§8 (the per-file wrappers), §10 (the
//! save protocol) and §14 (what a load must tolerate).
//!
//! **One authority for the id counter.** It lives in [`Manifest::next_comic_id`] and nowhere
//! else, so an in-memory copy cannot drift from the persisted one; [`ComicBundle::allocate`]
//! issues from it and [`ComicBundle::repair_id_counter`] moves it up to cover the ids the bundle
//! already contains.
//!
//! **The loader reads the tree, not the manifest.** §10's step 4 opens a window in which the
//! content files are the new generation while `manifest.json` still describes the previous one,
//! and §14 says a `files` mismatch is reported rather than fatal. Reading the tree is what makes
//! that window survivable in the useful direction; the manifest is then *compared* with the tree
//! and any difference is a finding. A load never partially applies a manifest: a file it names
//! that is not there is reported, not invented.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::entities::{
    AppInfo, Asset, Chapter, Character, ComicProject, Dialogue, FileEntry, GenerationJob, Lettering, Location, Page, Panel, ReviewDecision, Scene,
    StyleProfile, Timestamp, Unknown, VisualDocumentRef, is_canonical_timestamp,
};
use crate::error::Error;
use crate::ids::{ComicId, IdNewtype, JobId, PageId};
use crate::json::{join_relative, path_within, scan_tree, to_canonical_bytes};
use crate::migrate::{self, Migration};
use crate::validate::Finding;
use crate::{DocumentPayload, SCHEMA_VERSION};

/// The manifest's file name. Written **last** (§10).
pub const MANIFEST_FILE: &str = "manifest.json";
/// The project root file.
pub const PROJECT_FILE: &str = "project.json";
/// The one visual document's directory (ADR-011, ADR-013).
pub const DOCUMENT_DIR: &str = "document";
/// Per-page files, keyed by `PageId` (§3, §6).
pub const PAGES_DIR: &str = "pages";
/// Per-job files, keyed by `JobId` (§8.3).
pub const JOBS_DIR: &str = "jobs";
/// The asset registry.
pub const ASSET_INDEX_FILE: &str = "assets/index.json";
/// Generation jobs and review decisions.
pub const PROVENANCE_FILE: &str = "provenance/generations.json";
/// The staging directory §10 serializes into.
pub const STAGING_DIR: &str = ".save-tmp";
/// Derived output, never read as input (§3).
pub const EXPORTS_DIR: &str = "exports";

/// `manifest.json` — the only file that declares what the bundle contains, and the last written
/// on save (§4).
///
/// Field order is §4's table, because §5.7 makes declared order canonical.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// The comic bundle format version (§2). Starts at `1`.
    pub schema_version: u32,
    /// Stable for the life of the project; regenerating a bundle is a new project.
    pub bundle_id: ComicId,
    /// The `ComicProject.id`, duplicated here so the manifest alone identifies the project.
    pub comic_id: ComicId,
    /// Denormalised for humans and file pickers; `project.json` is authoritative.
    pub title: String,
    /// The allocation counter (§9). Never decreases.
    pub next_comic_id: u64,
    /// Artboards that belonged to deleted pages. Never pruned (design §5.3.1).
    pub retired_artboard_ids: Vec<u32>,
    /// The one visual document.
    pub document: VisualDocumentRef,
    /// Bundle-relative path → hash and size, for every file except the manifest, `exports/` and
    /// `.save-tmp/` (§4).
    pub files: BTreeMap<String, FileEntry>,
    /// When the bundle was created.
    pub created: Timestamp,
    /// When it was last saved.
    pub updated: Timestamp,
    /// What wrote it, for support.
    pub app: AppInfo,
    /// Unknown keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Manifest {
    /// Retire an artboard id (§9.1 mitigation 2).
    ///
    /// Append-only by construction: retiring an id that is already retired changes nothing.
    /// Rule 17 requires both halves — the list is unique, and an id is never removed once
    /// retired, because removing it is exactly what would let a stale `Page.artboard_id` resolve
    /// again.
    pub fn retire_artboard_id(&mut self, id: u32) {
        if !self.retired_artboard_ids.contains(&id) {
            self.retired_artboard_ids.push(id);
        }
    }

    /// Whether `id` has been retired (rule 4, mitigation 4).
    #[must_use]
    pub fn is_artboard_retired(&self, id: u32) -> bool {
        self.retired_artboard_ids.contains(&id)
    }
}

macro_rules! story_file {
    ($(#[$meta:meta])* $name:ident, $field:ident, $element:ty) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            /// The bundle format version this file was written at (§5.4, rule 3).
            pub schema_version: u32,
            /// The file's elements, in order.
            pub $field: Vec<$element>,
            /// Unknown top-level keys, preserved (§5.4, §12).
            #[serde(flatten)]
            pub unknown: Unknown,
        }
    };
}

story_file!(
    /// `story/chapters.json` (§5.4).
    ChaptersFile, chapters, Chapter
);
story_file!(
    /// `story/scenes.json` (§5.4).
    ScenesFile, scenes, Scene
);
story_file!(
    /// `story/characters.json` (§5.4).
    CharactersFile, characters, Character
);
story_file!(
    /// `story/locations.json` (§5.4).
    LocationsFile, locations, Location
);
story_file!(
    /// `story/styles.json` (§5.4).
    StylesFile, styles, StyleProfile
);

/// `pages/page-<PageId>.json`: a page and everything grouped with it (§6).
///
/// Grouping a page's panels, dialogue and lettering in one file keeps a page's edits together and
/// makes the save set proportional to what changed. Cross-page references are still by id, so the
/// grouping is a storage detail, not a model change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageFile {
    /// The bundle format version this file was written at (rule 3).
    pub schema_version: u32,
    /// The page.
    pub page: Page,
    /// Its panels.
    pub panels: Vec<Panel>,
    /// Its dialogue.
    pub dialogue: Vec<Dialogue>,
    /// Its lettering.
    pub lettering: Vec<Lettering>,
    /// Unknown top-level keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// `assets/index.json` (§7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetIndexFile {
    /// The bundle format version this file was written at (rule 3).
    pub schema_version: u32,
    /// Every asset.
    pub assets: Vec<Asset>,
    /// Unknown top-level keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// `provenance/generations.json` (§8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceFile {
    /// The bundle format version this file was written at (rule 3).
    pub schema_version: u32,
    /// The job records.
    pub jobs: Vec<GenerationJob>,
    /// The review decisions.
    pub decisions: Vec<ReviewDecision>,
    /// Unknown top-level keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// `jobs/job-<JobId>.json`: the live copy of an in-flight job (§8.3).
///
/// The wrapper is the shape every other file uses, which is what lets rule 3 check this file's
/// version — §8.1's entity has no `schema_version` of its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobFile {
    /// The bundle format version this file was written at (rule 3).
    pub schema_version: u32,
    /// The job.
    pub job: GenerationJob,
    /// Unknown top-level keys, preserved (§12).
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A whole project: the manifest, the project, the story library, the pages, the assets, the
/// provenance and the in-flight jobs.
///
/// The page and job maps are keyed by id, which is what makes the file names derive from the
/// model instead of from an ordinal (§3).
#[derive(Clone, Debug, PartialEq)]
pub struct ComicBundle {
    /// The manifest (§4).
    pub manifest: Manifest,
    /// The project root (§5.1).
    pub project: ComicProject,
    /// `story/chapters.json`.
    pub chapters: ChaptersFile,
    /// `story/scenes.json`.
    pub scenes: ScenesFile,
    /// `story/characters.json`.
    pub characters: CharactersFile,
    /// `story/locations.json`.
    pub locations: LocationsFile,
    /// `story/styles.json`.
    pub styles: StylesFile,
    /// `pages/page-<PageId>.json`, by page id.
    pub pages: BTreeMap<PageId, PageFile>,
    /// `assets/index.json`.
    pub assets: AssetIndexFile,
    /// `provenance/generations.json`.
    pub provenance: ProvenanceFile,
    /// `jobs/job-<JobId>.json`, by job id.
    pub jobs: BTreeMap<JobId, JobFile>,
    /// The bundle-relative keys the **generation of record wrote** — what a save staged, as opposed
    /// to what `manifest.files` *describes*.
    ///
    /// `files` is built from a tree scan (§4), so it also names a `pages/page-<n>.json` that no save
    /// ever staged: copied in by hand, written by a newer ComicCraft, or produced outside the save
    /// protocol (§8.3 tolerates the third for `jobs/`). The prune must ask "did the generation of
    /// record **write** this?", not "does its manifest **describe** this?", and this field is that
    /// answer. [`load`] seeds it with the `files` map the manifest it read carries — the record that
    /// generation left behind — and every [`save`] replaces it with exactly the keys that save
    /// staged, so a file that appears *after* a save is in neither and is never pruned.
    ///
    /// Empty means **no generation of record**: a hand-built bundle, or one that has never been
    /// saved. There is nothing of record to protect then, so the prune cleans everything this
    /// version writes in the flat directories it owns.
    ///
    /// A caller that restores an **older copy** of this bundle (design §11.2's undo snapshots one)
    /// restores this field with it, so it must put the last *written* value back before saving
    /// again: otherwise the save prunes against a record older than the disk and leaves a file the
    /// undone save wrote, which the manifest the save writes then describes and a load adopts back.
    /// `comic.project.save` seeds the field from the model it last wrote, which no undo touches.
    ///
    /// Deliberately **not** part of [`Manifest`]: §4 fixes the wire format, and this is an in-memory
    /// record of what the tree's history wrote, not a claim about the bundle on disk.
    pub files_of_record: BTreeSet<String>,
}

impl ComicBundle {
    /// Every timestamp this bundle holds that is not in §5.5's canonical form, with the field that
    /// holds it.
    ///
    /// §5.7 gives the two halves of the format different duties: "A reader must accept any valid
    /// JSON; a **writer** must produce this form." So [`load`] does not refuse a sloppy timestamp —
    /// §14 requires a damaged bundle to stay openable and repairable rather than locking the
    /// creator out — and [`save`] refuses it, naming the field. This is how a caller that wants to
    /// warn the creator *when they open the project* asks for the same list the writer would
    /// refuse on; it is the one implementation of the question, so the reader and the writer cannot
    /// drift apart. Repairing a value like `2026-10-11 06:00:00` would mean guessing a timezone
    /// nobody recorded, which §11 forbids.
    #[must_use]
    pub fn non_canonical_timestamps(&self) -> Vec<(String, String)> {
        timestamps_written_verbatim(self)
    }

    /// The id counter (§9). The manifest is the only authority for it.
    #[must_use]
    pub fn next_comic_id(&self) -> u64 {
        self.manifest.next_comic_id
    }

    /// Issue the next comic id, or refuse when the counter has reached the end of `u64`.
    ///
    /// Every allocation goes through here, so "never reuse an id" (§9) has one implementation.
    pub fn allocate<K: IdNewtype>(&mut self) -> Result<K, Error> {
        let mut allocator = crate::ids::IdAllocator::new(self.manifest.next_comic_id);
        let id = allocator.allocate::<K>()?;
        self.manifest.next_comic_id = allocator.next_raw();
        Ok(id)
    }

    /// The largest comic id anywhere in the bundle.
    ///
    /// Used to repair the counter upward (§9, §14 step 4). It covers the manifest's own ids, the
    /// libraries, every page's panels, dialogue and lettering, the assets, the provenance and the
    /// in-flight jobs — a hand edit that added an entity behind the counter's back is exactly what
    /// this catches.
    #[must_use]
    pub fn highest_comic_id(&self) -> u64 {
        let mut highest = 0;
        let mut take = |raw: u64| highest = highest.max(raw);
        take(self.manifest.bundle_id.0);
        take(self.manifest.comic_id.0);
        take(self.project.id.0);
        take(self.project.asset_registry_id.0);
        for id in &self.project.chapter_ids {
            take(id.0);
        }
        for id in &self.project.character_ids {
            take(id.0);
        }
        for id in &self.project.location_ids {
            take(id.0);
        }
        for id in &self.project.style_profile_ids {
            take(id.0);
        }
        for c in &self.chapters.chapters {
            take(c.id.0);
        }
        for s in &self.scenes.scenes {
            take(s.id.0);
        }
        for c in &self.characters.characters {
            take(c.id.0);
        }
        for l in &self.locations.locations {
            take(l.id.0);
        }
        for s in &self.styles.styles {
            take(s.id.0);
        }
        for file in self.pages.values() {
            take(file.page.id.0);
            for p in &file.panels {
                take(p.id.0);
            }
            for d in &file.dialogue {
                take(d.id.0);
            }
            for l in &file.lettering {
                take(l.id.0);
            }
        }
        for a in &self.assets.assets {
            take(a.id.0);
        }
        for j in &self.provenance.jobs {
            take(j.id.0);
        }
        for d in &self.provenance.decisions {
            take(d.id.0);
        }
        for file in self.jobs.values() {
            take(file.job.id.0);
        }
        highest
    }

    /// The artboards a new page may take: those the document holds and this manifest has **not**
    /// retired (design §5.3.1 mitigation 4).
    ///
    /// `Artboard.id` is allocated as `max(id) + 1` with no `fix_next_id()` equivalent, so deleting
    /// the highest artboard frees its number for the next one — and a page that took a retired
    /// number would be permanently untrusted (rule 4) and permanently unfixable (rule 17). So
    /// `comic.page.create` asks this, not "which artboards exist": it must allocate artboards
    /// until it gets an id that is not retired, which is what keeps rules 4 and 17 both true.
    #[must_use]
    pub fn assignable_artboards(&self, document: &crate::DocumentFacts) -> Vec<u32> {
        let mut out: Vec<u32> = document.artboards.iter().map(|a| a.id).filter(|id| !self.manifest.is_artboard_retired(*id)).collect();
        out.sort_unstable();
        out
    }

    /// Repair the counter upward so it is greater than every id present (§9, §14 step 4).
    ///
    /// Returns the new value when it changed. It never lowers the counter: an id that was issued
    /// and then deleted must stay unusable, which is the whole point of §9.
    pub fn repair_id_counter(&mut self) -> Option<u64> {
        let mut allocator = crate::ids::IdAllocator::new(self.manifest.next_comic_id);
        let repaired = allocator.repair_upward(self.highest_comic_id());
        self.manifest.next_comic_id = allocator.next_raw();
        repaired
    }

    /// `pages/page-<PageId>.json` for `id` (§3: keyed by id, never by an ordinal).
    #[must_use]
    pub fn page_file_key(id: PageId) -> String {
        format!("{PAGES_DIR}/page-{}.json", id.0)
    }

    /// `jobs/job-<JobId>.json` for `id` (§8.3).
    #[must_use]
    pub fn job_file_key(id: JobId) -> String {
        format!("{JOBS_DIR}/job-{}.json", id.0)
    }

    /// Every file of this bundle that declares a `schema_version`, with its bundle-relative key —
    /// the list rule 3 compares and the list §2's refusal walks.
    ///
    /// One authority for two questions: rule 3 asks whether each equals the manifest's
    /// `schema_version`, and [`load`] asks whether any is **greater** than the version this build
    /// implements, which §2 refuses rather than reading partially. Both must see every file, so a
    /// file added here is governed by both at once and neither list can drift from the other.
    ///
    /// `manifest.json` is not in it: the manifest **is** the version the others are compared
    /// against (§4), and §2's check of it is [`check_supported`]'s.
    #[must_use]
    pub fn declared_versions(&self) -> Vec<(String, u32)> {
        let mut declared: Vec<(String, u32)> = vec![
            (PROJECT_FILE.to_string(), self.project.schema_version),
            ("story/chapters.json".to_string(), self.chapters.schema_version),
            ("story/scenes.json".to_string(), self.scenes.schema_version),
            ("story/characters.json".to_string(), self.characters.schema_version),
            ("story/locations.json".to_string(), self.locations.schema_version),
            ("story/styles.json".to_string(), self.styles.schema_version),
            (ASSET_INDEX_FILE.to_string(), self.assets.schema_version),
            (PROVENANCE_FILE.to_string(), self.provenance.schema_version),
        ];
        for (id, file) in &self.pages {
            declared.push((Self::page_file_key(*id), file.schema_version));
        }
        for (id, file) in &self.jobs {
            declared.push((Self::job_file_key(*id), file.schema_version));
        }
        declared
    }

    /// Write this build's `schema_version` on every file that declares one (§2, §5.4, rule 3).
    ///
    /// §2 makes the manifest's version **the bundle's**: "the writer writes the version it
    /// implements". A file that declares its own copy must therefore be written at the same
    /// version, or the save would contradict rule 3 in the very bytes it wrote — a v2 manifest over
    /// v1 page files is exactly that contradiction, and it is what a save of a bundle read forward
    /// from v1 would produce without this.
    pub fn normalize_declared_versions(&mut self) {
        self.project.schema_version = SCHEMA_VERSION;
        self.chapters.schema_version = SCHEMA_VERSION;
        self.scenes.schema_version = SCHEMA_VERSION;
        self.characters.schema_version = SCHEMA_VERSION;
        self.locations.schema_version = SCHEMA_VERSION;
        self.styles.schema_version = SCHEMA_VERSION;
        self.assets.schema_version = SCHEMA_VERSION;
        self.provenance.schema_version = SCHEMA_VERSION;
        for file in self.pages.values_mut() {
            file.schema_version = SCHEMA_VERSION;
        }
        for file in self.jobs.values_mut() {
            file.schema_version = SCHEMA_VERSION;
        }
    }
}

/// Parse `pages/page-<PageId>.json` into the page id it encodes (§13 rule 13).
///
/// The number is **parsed, not compared**: padding is cosmetic (§3), so `page-7.json` and
/// `page-007.json` both mean page 7. The name encodes the *page* id and never the artboard id.
#[must_use]
pub fn parse_page_file_name(name: &str) -> Option<PageId> {
    parse_id_file_name(name, "page").map(PageId)
}

/// Parse `jobs/job-<JobId>.json` into the job id it encodes (§8.3).
#[must_use]
pub fn parse_job_file_name(name: &str) -> Option<JobId> {
    parse_id_file_name(name, "job").map(JobId)
}

fn parse_id_file_name(name: &str, prefix: &str) -> Option<u64> {
    let rest = name.strip_prefix(prefix)?.strip_prefix('-')?.strip_suffix(".json")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse::<u64>().ok()
}

/// A bundle that loaded, with what the load noticed.
#[derive(Clone, Debug)]
pub struct LoadedBundle {
    /// The project.
    pub bundle: ComicBundle,
    /// Every file in the bundle as §4 defines it, with the hash and size this load computed.
    pub files: BTreeMap<String, FileEntry>,
    /// What §14 steps 1 and 4 found. Referential rules 1–18 are not run here; see
    /// [`validate`](crate::validate).
    ///
    /// A **migration** is deliberately not a finding: §14's findings are the inconsistencies of a
    /// load, and a bundle read forward is not inconsistent — it is merely older. The migration is
    /// reported in [`migrations`](Self::migrations) instead, so it is neither silent nor filed
    /// with the damage.
    pub findings: Vec<Finding>,
    /// What §11's migrations did, in order — empty when the bundle already declares
    /// [`SCHEMA_VERSION`].
    ///
    /// §11 requires a migration to report rather than act silently, and this is that report: a
    /// caller is expected to surface it (a project that was read forward is a project whose next
    /// save will write a different format). Each entry names the version pair and carries one
    /// actionable note per observable effect.
    pub migrations: Vec<Migration>,
}

/// What [`save`] needs from the caller.
#[derive(Clone, Debug)]
pub struct SaveOptions {
    /// The canonical timestamp written to `manifest.updated` and `ProjectMetadata.updated`
    /// (§5.5, §5.7). A non-canonical value is refused, because a save that wrote one could not
    /// round-trip byte-identically.
    pub now: Timestamp,
    /// The visual document's bytes, serialized through `crates/format` **and verified by loading
    /// it back** (§10 steps 2–3). `None` leaves the document that is already on disk in place,
    /// which is what a comic-only edit wants.
    pub document: Option<DocumentPayload>,
}

/// Read a bundle from a directory (§14).
///
/// What is fatal is what leaves nothing to report against: a missing or unparseable
/// `manifest.json` (§14 step 1), an unsupported `schema_version` (§2) — the manifest's or a **file's**
/// own, because §2 says "never partially load" — a missing or unparseable
/// `project.json`, a missing visual document, and a comic file that does not parse (§14 step 3).
/// Everything else — the `files`/tree mismatch, an **absent collection file**, a counter repaired
/// upward — is a [`Finding`] on a usable project, because §14's sentence exists so "a creator can
/// open a damaged bundle and repair it rather than being locked out".
///
/// A bundle that declares an **older** version is read forward (§2) and the migration is returned
/// in [`LoadedBundle::migrations`], not in `findings`: a project read forward is not inconsistent,
/// it is older (§11's reporting requirement is what puts it in the return value rather than in a
/// log line nobody reads).
///
/// The split is by **role**, not by convenience:
///
/// - **Identity files** — `manifest.json` and `project.json` — carry the project, the id counter
///   and the document reference. Without them there is no project, so nothing could be reported
///   and refusing is the only honest answer.
/// - **Collection files** — the five `story/*.json`, `assets/index.json` and
///   `provenance/generations.json` — hold lists. An absent one loads as an **empty collection**
///   and is reported: that is the *absence* of content, not invented content, and every
///   consequence is already governed elsewhere — a dangling `location_id` is a rule-1 finding, a
///   missing asset a rule-9 one, and the `files`/tree mismatch reports the absent path too. The
///   empty collection takes the manifest's `schema_version`, because a file that is not there
///   cannot disagree with it; reporting rule 3 as well would be two findings for one absent file.
///
/// The visual document is **not** parsed here: `crates/format` is L3 and this crate is L2, so the
/// caller loads it and passes the facts to [`validate`](crate::validate). Its bytes are hashed,
/// because `manifest.files` covers them.
pub fn load(root: &Path) -> Result<LoadedBundle, Error> {
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(Error::MissingManifest { path: root.to_path_buf() });
    }
    let manifest: Manifest = read_json(&manifest_path, "manifest")?;
    check_supported(manifest.schema_version)?;
    let declared_version = manifest.schema_version;

    // Identity: fatal when absent (see the doc comment above).
    let project: ComicProject = read_json(&root.join(PROJECT_FILE), "project")?;

    // Collections: absent becomes empty, and is reported below.
    let mut absent: Vec<String> = Vec::new();
    macro_rules! collection_file {
        ($key:expr, $what:literal, $file:ident, $field:ident) => {
            match read_collection(&root.join($key), $what)? {
                Some(file) => file,
                None => {
                    absent.push($key.to_string());
                    $file { schema_version: manifest.schema_version, $field: Vec::new(), unknown: Unknown::new() }
                }
            }
        };
    }
    let chapters: ChaptersFile = collection_file!("story/chapters.json", "chapters file", ChaptersFile, chapters);
    let scenes: ScenesFile = collection_file!("story/scenes.json", "scenes file", ScenesFile, scenes);
    let characters: CharactersFile = collection_file!("story/characters.json", "characters file", CharactersFile, characters);
    let locations: LocationsFile = collection_file!("story/locations.json", "locations file", LocationsFile, locations);
    let styles: StylesFile = collection_file!("story/styles.json", "styles file", StylesFile, styles);
    let assets: AssetIndexFile = collection_file!(ASSET_INDEX_FILE, "asset registry", AssetIndexFile, assets);
    // `provenance/generations.json` holds two collections, so it does not fit the macro's shape.
    let provenance: ProvenanceFile = match read_collection(&root.join(PROVENANCE_FILE), "provenance file")? {
        Some(file) => file,
        None => {
            absent.push(PROVENANCE_FILE.to_string());
            ProvenanceFile { schema_version: manifest.schema_version, jobs: Vec::new(), decisions: Vec::new(), unknown: Unknown::new() }
        }
    };

    let document_path = root.join(join_relative(&manifest.document.relative_path)?);
    if !document_path.is_file() {
        return Err(Error::Missing { path: document_path });
    }

    let files = scan_tree(root)?;

    // Two files can encode one id: §3 says padding is cosmetic and rule 13 parses the number, so
    // `pages/page-8.json` and `pages/page-008.json` both mean page 8. Which of them the project
    // means is not knowable, and §11 says to validate input and **refuse rather than guess** — so
    // the duplicate is reported rather than resolved by iteration order. The first file in sorted
    // key order wins, deterministically, so the same bundle always loads the same way.
    let mut pages = BTreeMap::new();
    let mut jobs = BTreeMap::new();
    let mut page_files: BTreeMap<PageId, String> = BTreeMap::new();
    let mut job_files: BTreeMap<JobId, String> = BTreeMap::new();
    let mut duplicates = Vec::new();
    let mut strays = Vec::new();
    for key in files.keys() {
        if let Some(name) = key.strip_prefix("pages/") {
            if name.contains('/') || !name.ends_with(".json") {
                continue;
            }
            // §14 names exactly **two** fatal load failures — the document will not parse and a
            // comic file will not parse — and says of everything else that it is "returned as a
            // finding on a usable project, **so a creator can open a damaged bundle and repair it
            // rather than being locked out**". A `pages/` file whose name is not
            // `page-<PageId>.json` is a damaged bundle, not a lockout: it is reported (rule 13's
            // subject matter — the file name encodes the page id) and skipped, and the rest of the
            // load continues.
            let Some(id) = parse_page_file_name(name) else {
                strays.push(Finding::error(
                    Some(13),
                    key.clone(),
                    format!("`{name}` is not `page-<PageId>.json`, so it encodes no page id (rule 13); the file is left where it is"),
                ));
                continue;
            };
            let file: PageFile = read_json(&root.join(join_relative(key)?), "page file")?;
            match page_files.get(&id) {
                Some(first) => duplicates.push(Finding::error(
                    Some(2),
                    key.clone(),
                    format!(
                        "encodes page {} as `{first}` does: two files for one page id, and the number is parsed, not compared (§3, rule 13), so they are indistinguishable",
                        id.0
                    ),
                )),
                None => {
                    page_files.insert(id, key.clone());
                    pages.insert(id, file);
                }
            }
        } else if let Some(name) = key.strip_prefix("jobs/") {
            if name.contains('/') || !name.ends_with(".json") {
                continue;
            }
            // Same tolerance as `pages/`. No *numbered* rule governs a `jobs/` file's name —
            // §8.3's "one file per job, `job-<JobId>.json`" is the only statement — so this is a
            // load finding rather than a rule-13 one.
            let Some(id) = parse_job_file_name(name) else {
                strays.push(Finding::error(
                    None,
                    key.clone(),
                    format!("`{name}` is not `job-<JobId>.json`, so it encodes no job id (§8.3); the file is left where it is"),
                ));
                continue;
            };
            let file: JobFile = read_json(&root.join(join_relative(key)?), "job file")?;
            match job_files.get(&id) {
                Some(first) => duplicates.push(Finding::error(
                    Some(2),
                    key.clone(),
                    format!("encodes job {} as `{first}` does: two files for one job id (§8.3)", id.0),
                )),
                None => {
                    job_files.insert(id, key.clone());
                    jobs.insert(id, file);
                }
            }
        }
    }

    let mut findings = files_mismatch_findings(&manifest.files, &files);
    for key in absent {
        // A `Warning`, like the `files`/tree half that reports the same absent path when the file
        // is declared: §14 makes the whole condition one non-fatal report, and a second severity
        // for one absent file would repeat round 1's F7. This finding is the *model's* half — it
        // says what the project opened as, which is the consequence the `files` finding does not.
        findings.push(Finding::warning(
            None,
            key,
            "the file is not in the bundle: it loaded as an empty collection, so the project opens and can be repaired (§14)".to_string(),
        ));
    }
    findings.extend(duplicates);
    findings.extend(strays);
    // The generation of record is the manifest this load read: its `files` map is the record that
    // generation left of what it wrote (see `ComicBundle::files_of_record`). A page or job file it
    // does not name was written since, and the prune must leave it alone.
    let files_of_record: BTreeSet<String> = manifest.files.keys().cloned().collect();
    let mut bundle =
        ComicBundle { manifest, project, chapters, scenes, characters, locations, styles, pages, assets, provenance, jobs, files_of_record };
    // §2's refusal, per file — after every file has parsed, so the refusal can name the one that is
    // unreadable rather than a prefix of the bundle. The manifest's own version is
    // `check_supported`'s; this is the same rule applied to the files that declare a copy of it.
    // Rule 3 still governs a file declaring an *older* version, which is read forward.
    for (key, found) in bundle.declared_versions() {
        if found > SCHEMA_VERSION {
            return Err(Error::FileSchemaTooNew { path: root.join(join_relative(&key)?), found, supported: SCHEMA_VERSION });
        }
    }
    if let Some(next) = bundle.repair_id_counter() {
        findings.push(Finding::warning(
            None,
            "manifest.next_comic_id".to_string(),
            format!("the counter was behind the ids the bundle contains; repaired upward to {next} (§9)"),
        ));
    }
    // §2/§11: a lesser version is accepted and migrated forward, and the migration is **reported**
    // rather than performed in the dark. The counter repair runs first, because it is a §14
    // consequence of what the files hold, not of the format version they were written at.
    let migrations: Vec<Migration> = migrate::migrate(&mut bundle, declared_version)?.into_iter().collect();
    Ok(LoadedBundle { bundle, files, findings, migrations })
}

/// Refuse a bundle this build cannot read, and accept an older one so it can be migrated (§2, §11).
///
/// The rule is asymmetric on purpose: a **greater** version is refused outright, naming the
/// required version, because reading it partially would be worse than not reading it; a **lesser**
/// or equal one is accepted and migrated by [`migrate::migrate`], which `load` runs and reports.
///
/// This is the **manifest's** half of §2. Every file that declares a `schema_version` of its own is
/// checked by the same rule — a greater one is [`Error::FileSchemaTooNew`], a lesser one is rule
/// 3's to report — because "never partially load" is a claim about the bundle, not about one file.
pub fn check_supported(found: u32) -> Result<(), Error> {
    if found > SCHEMA_VERSION {
        return Err(Error::SchemaTooNew { found, supported: SCHEMA_VERSION });
    }
    if found == 0 {
        return Err(Error::Migration { found, supported: SCHEMA_VERSION, reason: "version 0 was never a bundle format".into() });
    }
    Ok(())
}

/// Write a bundle in the order §10 fixes: content files first, **manifest last**.
///
/// Before §10's step 1, and still part of "refuse rather than guess" (§11): every check that can
/// fail runs **before anything is touched** — the timestamps the writer would emit verbatim
/// (§5.5, §5.7) and the document's path and existence. A refused save therefore leaves the
/// caller's model and the directory exactly as it found them, so a caller that gets an error back
/// can retry with the model it still holds.
///
/// **Only the writer refuses a timestamp.** §5.7 gives the two halves different duties — "A reader
/// must accept any valid JSON; a **writer** must produce this form" — and §14 requires a damaged
/// bundle to stay openable and repairable, so [`load`] does not refuse one. A caller that wants to
/// warn the creator when the project is *opened* asks [`ComicBundle::non_canonical_timestamps`],
/// which is the same function this refusal is built from, so the two cannot disagree.
///
/// Then §10's five steps, in its numbering:
///
/// 1. Every comic file is serialized into `<bundle>/.save-tmp/`.
/// 2. The caller's document bytes are staged alongside them. This crate cannot serialize the
///    document (`crates/format` is L3) and cannot verify it by loading it back, so the caller must
///    have done §10 step 3 before calling; that is the one step of the protocol this crate
///    delegates, and it is delegated to the only layer that can perform it.
/// 3. *(the caller's verification — see step 2)*
/// 4. The content files are replaced with per-file renames, and then the stale `.json` files in the
///    directories this crate owns are pruned. **This opens the window §10 describes**, and it is
///    why `manifest.json` is still the previous generation at this point.
/// 5. `manifest.json` is written last — with the new `updated`, `next_comic_id`,
///    `retired_artboard_ids` and `files` — and `.save-tmp/` is removed.
///
/// The manifest is written from a fresh scan of the tree rather than from what this function
/// believes it wrote, because §4's `files` exists to let a reader detect a truncated or tampered
/// bundle, and a list computed from intentions would not detect anything. The derived fields are
/// held in locals and assigned to `bundle` only once the whole write has succeeded, so a failure
/// at any step cannot leave a half-derived model behind.
pub fn save(bundle: &mut ComicBundle, root: &Path, options: &SaveOptions) -> Result<(), Error> {
    // Step 1 — refuse before touching anything.
    if let Some((field, value)) = non_canonical_timestamps(bundle, &options.now).into_iter().next() {
        return Err(Error::BadTimestamp { field, value });
    }
    let document_key = match &options.document {
        Some(payload) => payload.relative_path.clone(),
        None => bundle.project.visual_document.relative_path.clone(),
    };
    // `join_relative` refuses a path that is not bundle-relative, so a `relative_path` of
    // `../escape.vectorcraft` is rejected before a single byte is staged (§13 rule 9's order,
    // applied to the writer).
    let document_relative = join_relative(&document_key)?;
    let document_on_disk = root.join(&document_relative);
    if options.document.is_none() && !document_on_disk.is_file() {
        return Err(Error::Missing { path: document_on_disk });
    }

    // Step 1b — derive, in locals. The writer writes the version it implements (§2: the manifest's
    // version is the bundle's), and the manifest's copies of what `project.json` owns are derived
    // from the project.
    let mut project = bundle.project.clone();
    let mut manifest = bundle.manifest.clone();
    project.schema_version = SCHEMA_VERSION;
    project.metadata.updated = options.now.clone();
    if let Some(payload) = &options.document {
        project.visual_document.relative_path = payload.relative_path.clone();
        project.visual_document.format_version = payload.format_version;
    }
    manifest.schema_version = SCHEMA_VERSION;
    manifest.app.schema = SCHEMA_VERSION;
    manifest.comic_id = project.id;
    manifest.title = project.title.clone();
    manifest.document = project.visual_document.clone();
    manifest.updated = options.now.clone();

    let staging = root.join(STAGING_DIR);
    match write_generation(root, &staging, bundle, &project, &manifest, options, &document_key) {
        Ok((files, staged)) => {
            manifest.files = files;
            bundle.project = project;
            bundle.manifest = manifest;
            bundle.normalize_declared_versions();
            // The model now holds what was written, declared version included. A bundle read
            // forward from v1 was *read* at v1 (so a load never rewrites history — the files on
            // disk still say what they said) and is *written* at v2; the in-memory model follows
            // the bytes, because a caller that validates the model after a save must see the
            // bundle it just wrote and not the one it opened.
            // The new generation of record is this save: exactly the keys it staged, so the *next*
            // save prunes what this one replaced or dropped and leaves everything else alone —
            // including a file that appeared while this generation was the current one.
            bundle.files_of_record = staged;
            Ok(())
        }
        Err(error) => {
            // A refused or failed save leaves no staging directory behind for the next run to find.
            fs::remove_dir_all(&staging).ok();
            Err(error)
        }
    }
}

/// §10's steps 1, 2, 4 and 5, against derived copies of the project and the manifest.
///
/// Returns the `files` map that was written and the keys that were staged, so the caller can adopt
/// both — nothing here mutates `bundle`, and the manifest is written only after every content file
/// is in place. The staged keys are what the prune asks about next time (see
/// [`ComicBundle::files_of_record`]).
fn write_generation(
    root: &Path,
    staging: &Path,
    bundle: &ComicBundle,
    project: &ComicProject,
    manifest: &Manifest,
    options: &SaveOptions,
    document_key: &str,
) -> Result<(BTreeMap<String, FileEntry>, BTreeSet<String>), Error> {
    fs::create_dir_all(staging).map_err(|source| Error::Write { path: staging.to_path_buf(), source })?;

    let mut staged: BTreeSet<String> = BTreeSet::new();
    stage_versioned(staging, &mut staged, PROJECT_FILE, "project.json", project)?;
    stage_versioned(staging, &mut staged, "story/chapters.json", "story/chapters.json", &bundle.chapters)?;
    stage_versioned(staging, &mut staged, "story/scenes.json", "story/scenes.json", &bundle.scenes)?;
    stage_versioned(staging, &mut staged, "story/characters.json", "story/characters.json", &bundle.characters)?;
    stage_versioned(staging, &mut staged, "story/locations.json", "story/locations.json", &bundle.locations)?;
    stage_versioned(staging, &mut staged, "story/styles.json", "story/styles.json", &bundle.styles)?;
    stage_versioned(staging, &mut staged, ASSET_INDEX_FILE, "assets/index.json", &bundle.assets)?;
    stage_versioned(staging, &mut staged, PROVENANCE_FILE, "provenance/generations.json", &bundle.provenance)?;
    for (id, file) in &bundle.pages {
        stage_versioned(staging, &mut staged, &ComicBundle::page_file_key(*id), "a page file", file)?;
    }
    for (id, file) in &bundle.jobs {
        stage_versioned(staging, &mut staged, &ComicBundle::job_file_key(*id), "a job file", file)?;
    }
    if let Some(payload) = &options.document {
        stage_file(staging, &mut staged, document_key, &payload.bytes)?;
    }

    // Step 4: replace the content files, then drop what this crate owns and the bundle no longer
    // has. `assets/` subdirectories are content-addressed and are left alone: an unreferenced
    // asset is legal and deleting one would destroy approved artwork.
    for key in &staged {
        let from = staging.join(join_relative(key)?);
        let to = root.join(join_relative(key)?);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Write { path: parent.to_path_buf(), source })?;
        }
        fs::rename(&from, &to).map_err(|source| Error::Write { path: to.clone(), source })?;
    }
    prune_stale(root, &staged, &bundle.files_of_record)?;

    // Step 5: the manifest, from the tree as it now is, then the staging directory goes.
    let mut written = manifest.clone();
    written.files = files_from_tree(root, &manifest.files)?;
    let manifest_bytes = to_canonical_bytes(MANIFEST_FILE, &written)?;
    let staged_manifest = staging.join(MANIFEST_FILE);
    fs::write(&staged_manifest, &manifest_bytes).map_err(|source| Error::Write { path: staged_manifest.clone(), source })?;
    let manifest_path = root.join(MANIFEST_FILE);
    fs::rename(&staged_manifest, &manifest_path).map_err(|source| Error::Write { path: manifest_path.clone(), source })?;
    fs::remove_dir_all(staging).map_err(|source| Error::Write { path: staging.to_path_buf(), source })?;
    Ok((written.files, staged))
}

/// §4's `files`, with each entry's `unknown` map carried over from the manifest being replaced.
///
/// The hash and the size are **derived** — they describe the file as it now is — but the unknown
/// keys belong to the *entry*, not to the bytes, so a field a newer ComicCraft wrote on a
/// `manifest.files` entry survives the save (§12). Without this the declared `unknown` could never
/// round-trip, which would make declaring it a lie about the format.
fn files_from_tree(root: &Path, previous: &BTreeMap<String, FileEntry>) -> Result<BTreeMap<String, FileEntry>, Error> {
    let mut files = scan_tree(root)?;
    for (key, entry) in &mut files {
        if let Some(before) = previous.get(key) {
            entry.unknown = before.unknown.clone();
        }
    }
    Ok(files)
}

/// Record `value` under `field` when it is not in §5.5's canonical form.
fn check_timestamp(out: &mut Vec<(String, String)>, field: String, value: &str) {
    if !is_canonical_timestamp(value) {
        out.push((field, value.to_string()));
    }
}

/// Every timestamp the bundle holds that the writer would emit **verbatim from the model**.
///
/// `manifest.updated` and `ProjectMetadata.updated` are not here because the writer derives them
/// from [`SaveOptions::now`], which is checked separately; every other timestamp is written exactly
/// as the model holds it, so a hand-edited or imported one would go to disk uncanonical and break
/// the byte-identical round trip §5.7 exists to make possible.
///
/// This is the one function that answers "is this bundle's timestamps canonical", and both halves
/// of §5.7 call it: [`ComicBundle::non_canonical_timestamps`] is the answer a caller can ask for
/// when it opens a bundle, and [`save`] refuses on it. The **reader** does not refuse, because §5.7
/// gives the two halves different duties — "A reader must accept any valid JSON; a **writer** must
/// produce this form" — and §14 requires a damaged bundle to stay openable and repairable rather
/// than being locked out. The writer refuses rather than guesses, because interpreting
/// `2026-10-11 06:00:00` as UTC would be a guess about a timezone nobody recorded (§11: validate
/// the input and refuse).
fn timestamps_written_verbatim(bundle: &ComicBundle) -> Vec<(String, String)> {
    let mut out = Vec::new();
    check_timestamp(&mut out, "manifest.created".to_string(), &bundle.manifest.created);
    check_timestamp(&mut out, "project.metadata.created".to_string(), &bundle.project.metadata.created);
    for asset in &bundle.assets.assets {
        if let Some(provenance) = &asset.provenance {
            check_timestamp(&mut out, format!("Asset({}).provenance.created", asset.id.0), &provenance.created);
            if let Some(consent) = &provenance.consent {
                check_timestamp(&mut out, format!("Asset({}).provenance.consent.granted", asset.id.0), &consent.granted);
            }
        }
    }
    for job in bundle.provenance.jobs.iter().chain(bundle.jobs.values().map(|f| &f.job)) {
        check_timestamp(&mut out, format!("GenerationJob({}).created", job.id.0), &job.created);
        check_timestamp(&mut out, format!("GenerationJob({}).updated", job.id.0), &job.updated);
        if let Some(consent) = &job.consent {
            check_timestamp(&mut out, format!("GenerationJob({}).consent.granted", job.id.0), &consent.granted);
        }
    }
    for decision in &bundle.provenance.decisions {
        check_timestamp(&mut out, format!("ReviewDecision({}).timestamp", decision.id.0), &decision.timestamp);
    }
    out
}

/// Everything [`save`] refuses on: the model's timestamps, plus the caller's `now`.
fn non_canonical_timestamps(bundle: &ComicBundle, now: &Timestamp) -> Vec<(String, String)> {
    let mut out = Vec::new();
    check_timestamp(&mut out, "SaveOptions.now".to_string(), now);
    out.extend(timestamps_written_verbatim(bundle));
    out
}

/// Write one file into the staging directory, creating its parents.
fn stage_file(staging: &Path, staged: &mut BTreeSet<String>, key: &str, bytes: &[u8]) -> Result<(), Error> {
    let path = staging.join(join_relative(key)?);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Write { path: parent.to_path_buf(), source })?;
    }
    fs::write(&path, bytes).map_err(|source| Error::Write { path: path.clone(), source })?;
    staged.insert(key.to_string());
    Ok(())
}

/// A file that declares its own `schema_version` (rule 3).
///
/// Every such file is written at **the bundle's** version, because §2 makes the manifest's version
/// the bundle's: the writer "writes the version it implements". A bundle read forward from v1 has
/// files still declaring 1, and a save that left them there would write a v2 manifest over v1 page
/// files — rule 3's exact contradiction — and a canonical form no fresh save produces.
trait DeclaresVersion: Clone + Serialize {
    /// Set the version this file will be written at.
    fn set_declared_version(&mut self, version: u32);
}

macro_rules! declares_version {
    ($($file:ty),+ $(,)?) => {
        $(impl DeclaresVersion for $file {
            fn set_declared_version(&mut self, version: u32) {
                self.schema_version = version;
            }
        })+
    };
}

declares_version!(
    ComicProject,
    ChaptersFile,
    ScenesFile,
    CharactersFile,
    LocationsFile,
    StylesFile,
    AssetIndexFile,
    ProvenanceFile,
    PageFile,
    JobFile
);

/// Stage one file at the version this build implements (§2, rule 3).
///
/// The copy is what the *disk* gets; the model's own [`ComicBundle::normalize_declared_versions`]
/// happens once, after the whole write has succeeded, so nothing here mutates `bundle` and a failed
/// save leaves the caller's model the way the refused-save rule promises.
fn stage_versioned<T: DeclaresVersion>(staging: &Path, staged: &mut BTreeSet<String>, key: &str, what: &str, file: &T) -> Result<(), Error> {
    let mut file = file.clone();
    file.set_declared_version(SCHEMA_VERSION);
    stage_file(staging, staged, key, &to_canonical_bytes(what, &file)?)
}

/// Whether `name`, in the directory `dir`, is a file **this version** writes there.
///
/// The prune must stop at what this version owns. §4 defines `manifest.files` as every file except
/// the manifest, `exports/` and `.save-tmp/`, so a `.json` in one of these directories that this
/// version does not name is a file a **newer** ComicCraft wrote — and §12's promise is that a
/// load-and-save by an older build does not destroy what it does not understand. Pruning by
/// directory would delete it and then drop its `files` entry, destroying data with no finding that
/// ever mentioned it. `story/relationships.json` from a future version survives; a page this
/// version deleted still goes.
fn is_written_by_this_version(dir: &str, name: &str) -> bool {
    match dir {
        PAGES_DIR => parse_page_file_name(name).is_some(),
        JOBS_DIR => parse_job_file_name(name).is_some(),
        "story" => matches!(name, "chapters.json" | "scenes.json" | "characters.json" | "locations.json" | "styles.json"),
        "provenance" => PROVENANCE_FILE.rsplit('/').next() == Some(name),
        _ => false,
    }
}

/// Remove the files this crate owns that the new generation does not contain, so `manifest.files`
/// cannot describe a page or a job whose file is gone.
///
/// Only the flat directories this crate writes are pruned, and inside them only the names this
/// version writes (see [`is_written_by_this_version`]). `assets/` is left alone entirely: it is
/// content-addressed, and an asset no page currently references is still an asset.
///
/// **A file the generation of record never wrote is not pruned.** `written` is
/// [`ComicBundle::files_of_record`] — what the previous save *staged* (or, for a bundle just loaded,
/// the `files` map the manifest it read carries) — and **not** `manifest.files`, which §4 derives
/// from a tree scan and so also names files no save ever wrote. A `pages/page-<n>.json` that no
/// generation wrote appeared *since* the last save: by hand, by a newer ComicCraft, or by a tool
/// outside the save protocol (§8.3 tolerates the third for `jobs/`). Deleting it would destroy bytes
/// this save never staged — and would do so *silently*, because the manifest it writes next would
/// simply omit the name. §12's promise that a load-and-save by this build does not destroy what it
/// does not understand applies to files as much as to fields.
///
/// Read against `manifest.files` instead, and the protection lasts exactly one save: the save that
/// keeps the file also *describes* it (§4), so the next save finds the name in its previous manifest
/// and deletes it. That is the defect of W4's round-2 review F9, and it is why the predicate is the
/// staged set and not the declared one. A name the generation of record **did** write and the new
/// one does not stage is a page or a job this save replaced or dropped, so it goes.
///
/// The one exception is a bundle with **no** generation of record (`written` empty: a hand-built
/// bundle, or one never saved): there is nothing of record to protect, so everything this version
/// writes in these directories is its own leftover and is cleaned.
fn prune_stale(root: &Path, keep: &BTreeSet<String>, written: &BTreeSet<String>) -> Result<(), Error> {
    let of_record = |key: &str| written.is_empty() || written.contains(key);
    for dir in [PAGES_DIR, JOBS_DIR, "story", "provenance"] {
        let path = root.join(dir);
        let Ok(entries) = fs::read_dir(&path) else { continue };
        for entry in entries {
            let entry = entry.map_err(|source| Error::Read { path: path.clone(), source })?;
            if !entry.file_type().map_err(|source| Error::Read { path: entry.path(), source })?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let key = format!("{dir}/{name}");
            if is_written_by_this_version(dir, &name) && !keep.contains(&key) && of_record(&key) {
                fs::remove_file(entry.path()).map_err(|source| Error::Write { path: entry.path(), source })?;
            }
        }
    }
    Ok(())
}

/// What §14 step 1 reports when `manifest.files` and the tree disagree.
///
/// A mismatch means the bundle was interrupted mid-save (§10), hand-edited, or a `jobs/` file was
/// written while the app ran outside the save protocol (§8.3) — the third cause is benign and
/// expected. So it is reported and the load continues: making it fatal would leave a project
/// nobody could open after a crash in the window between §10's steps 4 and 5.
fn files_mismatch_findings(declared: &BTreeMap<String, FileEntry>, actual: &BTreeMap<String, FileEntry>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (key, entry) in actual {
        match declared.get(key) {
            None => out.push(Finding::warning(
                None,
                key.clone(),
                "the file is in the bundle but `manifest.files` does not list it: the bundle was interrupted mid-save, hand-edited, or a job file was written outside the save protocol (§14)".into(),
            )),
            Some(declared_entry) if declared_entry.sha256 != entry.sha256 || declared_entry.bytes != entry.bytes => {
                out.push(Finding::warning(
                    None,
                    key.clone(),
                    format!(
                        "`manifest.files` records sha256 {} ({} bytes) but the file is {} ({} bytes): the manifest describes the previous generation (§14)",
                        declared_entry.sha256, declared_entry.bytes, entry.sha256, entry.bytes
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    for key in declared.keys() {
        if !actual.contains_key(key) {
            // Both halves of the mismatch are **warnings**, because §14 makes them one condition:
            // "A mismatch means the bundle was interrupted mid-save (§10), hand-edited, or a
            // `jobs/` file written while the app ran outside the save protocol (§8.3) — the third
            // cause is benign and expected, so it is reported as an inconsistency and the load
            // continues." Step 4 of a save both *adds* content files and, through the prune,
            // *removes* the ones the new generation dropped, so "a declared file is absent" is
            // the same mid-save window as "an unlisted file is present" — filing one as an error
            // would make the very window §14 says must stay open a blocker for the command layer.
            // The finding is still reported with its own subject, so a caller that wants to treat
            // a deletion as fatal can.
            out.push(Finding::warning(
                None,
                key.clone(),
                "`manifest.files` lists a file that is not in the bundle: the previous generation's manifest, read inside §10's save window (§14)"
                    .into(),
            ));
        }
    }
    out.sort_by(|a, b| a.subject.cmp(&b.subject));
    out
}

fn read_json<T: DeserializeOwned>(path: &Path, what: &'static str) -> Result<T, Error> {
    let bytes = fs::read(path).map_err(|source| Error::Read { path: path.to_path_buf(), source })?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Parse { path: path.to_path_buf(), what, source })
}

/// Read a **collection** file (§5.4, §7, §8), or `None` when it is not there.
///
/// §14's fatal failures are a file that will not parse and the two identity files that carry the
/// project; a collection file that is *absent* is neither, so [`load`] turns it into an empty
/// collection and a finding rather than locking the creator out. A file that is there but cannot
/// be read — permissions, or a directory where a file belongs — is still fatal: there is content
/// we cannot see, which is not the same as content that is not there.
fn read_collection<T: DeserializeOwned>(path: &Path, what: &'static str) -> Result<Option<T>, Error> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|source| Error::Parse { path: path.to_path_buf(), what, source }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Read { path: path.to_path_buf(), source }),
    }
}

/// A bundle path resolved against the bundle root, refusing to leave it (§13 rule 9).
#[must_use]
pub fn resolve_in_bundle(root: &Path, relative: &str) -> Option<PathBuf> {
    path_within(root, relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::*;
    use crate::ids::*;

    fn empty_bundle() -> ComicBundle {
        let unknown = Unknown::new();
        ComicBundle {
            manifest: Manifest {
                schema_version: SCHEMA_VERSION,
                bundle_id: ComicId(20),
                comic_id: ComicId(1),
                title: "T".into(),
                next_comic_id: 21,
                retired_artboard_ids: vec![],
                document: VisualDocumentRef { relative_path: "document/comic.vectorcraft".into(), format_version: 4, unknown: unknown.clone() },
                files: BTreeMap::new(),
                created: "2026-10-11T00:00:00Z".into(),
                updated: "2026-10-11T00:00:00Z".into(),
                app: AppInfo { name: "ComicCraft".into(), version: "0.8.0".into(), schema: SCHEMA_VERSION, unknown: unknown.clone() },
                unknown: unknown.clone(),
            },
            project: ComicProject {
                id: ComicId(1),
                schema_version: SCHEMA_VERSION,
                title: "T".into(),
                metadata: ProjectMetadata {
                    author: None,
                    language: None,
                    genre: None,
                    reading_direction: ReadingDirection::Ltr,
                    created: "2026-10-11T00:00:00Z".into(),
                    updated: "2026-10-11T00:00:00Z".into(),
                    unknown: unknown.clone(),
                },
                chapter_ids: vec![ChapterId(3)],
                character_ids: vec![],
                location_ids: vec![],
                style_profile_ids: vec![],
                asset_registry_id: ComicId(2),
                settings: ProjectSettings {
                    default_page_size: None,
                    default_template: None,
                    default_lettering_style: None,
                    unknown: unknown.clone(),
                },
                visual_document: VisualDocumentRef {
                    relative_path: "document/comic.vectorcraft".into(),
                    format_version: 4,
                    unknown: unknown.clone(),
                },
                unknown: unknown.clone(),
            },
            chapters: ChaptersFile {
                schema_version: SCHEMA_VERSION,
                chapters: vec![Chapter {
                    id: ChapterId(3),
                    project_id: ComicId(1),
                    title: "One".into(),
                    scene_ids: vec![],
                    order_index: 0,
                    unknown: unknown.clone(),
                }],
                unknown: unknown.clone(),
            },
            scenes: ScenesFile { schema_version: SCHEMA_VERSION, scenes: vec![], unknown: unknown.clone() },
            characters: CharactersFile { schema_version: SCHEMA_VERSION, characters: vec![], unknown: unknown.clone() },
            locations: LocationsFile { schema_version: SCHEMA_VERSION, locations: vec![], unknown: unknown.clone() },
            styles: StylesFile { schema_version: SCHEMA_VERSION, styles: vec![], unknown: unknown.clone() },
            pages: BTreeMap::new(),
            assets: AssetIndexFile { schema_version: SCHEMA_VERSION, assets: vec![], unknown: unknown.clone() },
            provenance: ProvenanceFile { schema_version: SCHEMA_VERSION, jobs: vec![], decisions: vec![], unknown: unknown.clone() },
            jobs: BTreeMap::new(),
            files_of_record: BTreeSet::new(),
        }
    }

    /// A minimal page file, so the save has a `pages/` entry to stage.
    fn page_file(id: PageId) -> PageFile {
        let unknown = Unknown::new();
        PageFile {
            schema_version: SCHEMA_VERSION,
            page: Page {
                id,
                chapter_id: ChapterId(3),
                scene_ids: vec![],
                artboard_id: 1,
                page_number: 1,
                panel_ids: vec![],
                reading_order: vec![],
                status: crate::entities::PageStatus::Draft,
                unknown: unknown.clone(),
            },
            panels: vec![],
            dialogue: vec![],
            lettering: vec![],
            unknown,
        }
    }

    /// The frozen v1 fixture, copied into `root` so a test can damage it.
    fn copy_fixture(root: &Path) {
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1/minimal.comiccraft");
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).expect("destination");
            for entry in fs::read_dir(from).expect("source") {
                let entry = entry.expect("entry");
                let target = to.join(entry.file_name());
                if entry.file_type().expect("type").is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    fs::copy(entry.path(), &target).expect("copy");
                }
            }
        }
        copy(&from, root);
    }

    /// A generated asset with provenance and consent, for the timestamp walk.
    fn asset_with_provenance() -> Asset {
        Asset {
            id: crate::AssetId(17),
            asset_type: crate::entities::AssetType::Generated,
            storage_reference: crate::entities::StorageRef::Linked { relative_path: "assets/panels/panel-1.png".into() },
            content_hash: crate::entities::ContentHash { algorithm: "sha256".into(), value: "00".repeat(32) },
            media_metadata: crate::entities::MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: None },
            provenance: Some(crate::entities::Provenance {
                provider: crate::entities::ProviderId("local".into()),
                model: "example".into(),
                request: serde_json::json!({}),
                input_asset_ids: vec![],
                created: "2026-10-11T06:00:00Z".into(),
                consent: Some(crate::entities::Consent { granted: "2026-10-11T06:00:00Z".into(), statement: "s".into(), unknown: Unknown::new() }),
                unknown: Unknown::new(),
            }),
            parent_asset_ids: vec![],
            approval_status: crate::entities::ApprovalStatus::Approved,
            unknown: Unknown::new(),
        }
    }

    /// A minimal in-flight job, for the timestamp walk.
    fn job(id: JobId) -> GenerationJob {
        GenerationJob {
            id,
            operation: crate::entities::JobOperation::TextToImage,
            target_panel_id: None,
            input_asset_ids: vec![],
            provider: crate::entities::ProviderId("local".into()),
            model: "example".into(),
            request: serde_json::json!({}),
            status: crate::entities::JobStatus::Running,
            progress: None,
            error: None,
            result_asset_ids: vec![],
            created: "2026-10-11T06:00:00Z".into(),
            updated: "2026-10-11T06:00:00Z".into(),
            target_revision: 1,
            consent: None,
            unknown: Unknown::new(),
        }
    }

    #[test]
    fn page_and_job_file_names_parse_the_number_not_the_string() {
        // Rule 13: padding is cosmetic (§3), so both spellings mean page 7.
        assert_eq!(parse_page_file_name("page-7.json"), Some(PageId(7)));
        assert_eq!(parse_page_file_name("page-007.json"), Some(PageId(7)));
        assert_eq!(parse_page_file_name("page-0.json"), Some(PageId(0)));
        assert_eq!(parse_page_file_name("page-18446744073709551615.json"), Some(PageId(u64::MAX)));
        for bad in ["page-.json", "page-7", "page-7.JSON", "page-+7.json", "page- 7.json", "page-7.json.bak", "page-7x.json", "artboard-7.json", ""] {
            assert_eq!(parse_page_file_name(bad), None, "{bad:?} does not encode a page id");
        }
        assert_eq!(parse_job_file_name("job-18.json"), Some(JobId(18)));
        assert_eq!(parse_page_file_name("job-18.json"), None, "a job file is not a page file");
    }

    #[test]
    fn the_file_name_for_an_id_is_the_one_the_parser_reads() {
        assert_eq!(ComicBundle::page_file_key(PageId(8)), "pages/page-8.json");
        assert_eq!(parse_page_file_name("page-8.json"), Some(PageId(8)));
        assert_eq!(ComicBundle::job_file_key(JobId(18)), "jobs/job-18.json");
        assert_eq!(parse_job_file_name("job-18.json"), Some(JobId(18)));
    }

    #[test]
    fn retiring_an_artboard_is_append_only_and_unique() {
        let mut manifest = empty_bundle().manifest;
        manifest.retire_artboard_id(1);
        manifest.retire_artboard_id(1);
        manifest.retire_artboard_id(2);
        assert_eq!(manifest.retired_artboard_ids, vec![1, 2], "unique and append-only (rule 17)");
        assert!(manifest.is_artboard_retired(1));
        assert!(!manifest.is_artboard_retired(3));
    }

    #[test]
    fn allocation_issues_from_the_manifest_and_never_repeats() {
        let mut bundle = empty_bundle();
        assert_eq!(bundle.allocate::<PageId>().expect("room"), PageId(21));
        assert_eq!(bundle.allocate::<PanelId>().expect("room"), PanelId(22));
        assert_eq!(bundle.next_comic_id(), 23, "the manifest is the only authority for the counter");
    }

    #[test]
    fn the_counter_is_repaired_upward_over_every_kind_present() {
        let mut bundle = empty_bundle();
        bundle.manifest.next_comic_id = 2;
        assert_eq!(bundle.highest_comic_id(), 20, "the manifest's own bundle id counts");
        assert_eq!(bundle.repair_id_counter(), Some(21));
        assert_eq!(bundle.repair_id_counter(), None, "already greater than every id present");
        assert_eq!(bundle.next_comic_id(), 21);
    }

    #[test]
    fn the_counter_is_never_lowered() {
        let mut bundle = empty_bundle();
        bundle.manifest.next_comic_id = 500;
        assert_eq!(bundle.repair_id_counter(), None);
        assert_eq!(bundle.next_comic_id(), 500, "a deleted id stays unusable (§9)");
    }

    #[test]
    fn a_newer_schema_version_is_refused_and_an_older_one_is_accepted() {
        assert!(check_supported(SCHEMA_VERSION).is_ok());
        assert!(check_supported(1).is_ok());
        match check_supported(SCHEMA_VERSION + 1) {
            Err(Error::SchemaTooNew { found, supported }) => {
                assert_eq!((found, supported), (SCHEMA_VERSION + 1, SCHEMA_VERSION));
            }
            other => panic!("a newer bundle must be refused with the version it needs, got {other:?}"),
        }
        assert!(check_supported(0).is_err(), "version 0 was never a bundle format");
    }

    #[test]
    fn declared_versions_covers_every_file_that_declares_one() {
        // `ComicBundle::declared_versions` is what rule 3 compares and what §2's refusal walks, so
        // a file missing from it would be invisible to both. The fixture is the checklist: every
        // `.json` it holds declares a `schema_version`, so every one must appear in the list.
        let root = crate::scratch::scratch_dir_created("declared-versions");
        copy_fixture(&root);
        let loaded = load(&root).expect("the v1 fixture loads");
        let listed: BTreeSet<String> = loaded.bundle.declared_versions().into_iter().map(|(key, _)| key).collect();
        for key in scan_tree(&root).expect("the tree").keys() {
            if key.ends_with(".json") && key != MANIFEST_FILE {
                assert!(listed.contains(key), "`{key}` declares a `schema_version` and `declared_versions` omits it");
            }
        }
        assert!(
            loaded.bundle.declared_versions().iter().all(|(_, version)| *version == 1),
            "the fixture is v1 in every file: {:?}",
            loaded.bundle.declared_versions()
        );
        let mut normalized = loaded.bundle.clone();
        normalized.normalize_declared_versions();
        for (file, version) in normalized.declared_versions() {
            assert_eq!(version, SCHEMA_VERSION, "`{file}` declares a version the writer would not write");
        }
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_migration_is_reported_and_a_current_bundle_is_not() {
        // §11: a migration must report rather than act in the dark. A bundle already at this
        // build's version is not a migration, and reporting one would put noise on every load.
        let root = crate::scratch::scratch_dir_created("migration-report");
        copy_fixture(&root);
        let loaded = load(&root).expect("the v1 fixture loads");
        assert_eq!(loaded.migrations.len(), 1, "a v1 bundle is migrated: {:?}", loaded.migrations);
        let migration = &loaded.migrations[0];
        assert_eq!((migration.from, migration.to), (1, 2));
        assert_eq!(
            migration.notes,
            vec!["v2 adds `Panel.locked` (default `false`); 3 panels take the default".to_string()],
            "the report names the field and what it did"
        );

        let mut current = loaded.bundle.clone();
        current.manifest.schema_version = SCHEMA_VERSION;
        assert_eq!(migrate::migrate(&mut current, SCHEMA_VERSION).expect("current is not a migration"), None);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_version_with_no_migration_step_refuses_rather_than_guesses() {
        // §11: validate the input and refuse rather than guess. There is no step older than v1, so
        // the only reachable shape is the unreachable one — pinned anyway, because the alternative
        // (a `match` arm that quietly returns `Ok`) is how a format silently drifts.
        let mut bundle = empty_bundle();
        match migrate::migrate(&mut bundle, 0) {
            Err(Error::Migration { found, supported, reason }) => {
                assert_eq!((found, supported), (0, SCHEMA_VERSION));
                assert!(reason.contains("no migration"), "{reason}");
            }
            other => panic!("a version with no step must refuse, got {other:?}"),
        }
    }

    #[test]
    fn a_save_refuses_a_timestamp_that_is_not_canonical() {
        // §5.7: one canonical form, or a save cannot round-trip byte-identically.
        let root = crate::scratch::scratch_dir_created("save-bad-timestamp");
        let mut bundle = empty_bundle();
        let options = SaveOptions { now: "2026-10-11T00:00:00+00:00".into(), document: None };
        assert!(matches!(save(&mut bundle, &root, &options), Err(Error::BadTimestamp { .. })));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_writes_the_manifest_last_and_describes_every_content_file() {
        let root = crate::scratch::scratch_dir_created("save-order");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("staging");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");

        let declared: Vec<&String> = bundle.manifest.files.keys().collect();
        assert!(!declared.iter().any(|k| k.as_str() == MANIFEST_FILE), "the manifest cannot hash itself");
        assert!(declared.iter().any(|k| k.as_str() == "document/comic.vectorcraft"), "the document is covered: {declared:?}");
        assert!(declared.iter().any(|k| k.as_str() == "project.json"), "the project is covered: {declared:?}");
        assert!(declared.iter().any(|k| k.as_str() == "story/chapters.json"), "the story files are covered: {declared:?}");
        assert!(!root.join(STAGING_DIR).exists(), "the staging directory is gone");
        assert!(root.join(MANIFEST_FILE).is_file());

        let loaded = load(&root).expect("loads back");
        assert!(loaded.findings.is_empty(), "a clean save has nothing to report: {:?}", loaded.findings);
        assert_eq!(loaded.bundle.manifest.files, bundle.manifest.files);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_removes_a_page_file_the_bundle_no_longer_has() {
        let root = crate::scratch::scratch_dir_created("save-prune");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("staging");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"d").expect("document");
        fs::create_dir_all(root.join(PAGES_DIR)).expect("pages");
        fs::write(root.join(PAGES_DIR).join("page-99.json"), b"{}").expect("stale page");
        let mut bundle = empty_bundle();
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");
        assert!(!root.join(PAGES_DIR).join("page-99.json").exists(), "a page the bundle no longer has must not be left behind");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_keeps_a_page_file_the_generation_of_record_never_wrote() {
        // Round-1 F7 of the W4 review, in the round-2 F9 shape: a `pages/page-<n>.json` written
        // *since* the load was written by no generation of record, and §4 defines `manifest.files`
        // as every file in the bundle — so the prune must leave it, on **every** later save, not
        // only on the first one. Pruning against `manifest.files` instead protects it for exactly
        // one save: the save that keeps it also describes it, so the next save finds the name in
        // its previous manifest and deletes it. The ordinary case (a page the generation of record
        // *wrote* and this save drops) still goes, which is what keeps round 0's
        // `a_save_removes_a_page_file_the_bundle_no_longer_has` green.
        let root = crate::scratch::scratch_dir_created("save-keeps-undeclared-page");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        bundle.pages.insert(PageId(8), page_file(PageId(8)));
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("the first save");
        assert!(root.join(PAGES_DIR).join("page-8.json").is_file(), "the first save wrote the page");

        let stray = root.join(PAGES_DIR).join("page-99.json");
        fs::copy(root.join(PAGES_DIR).join("page-8.json"), &stray).expect("a page file written since the load");
        save(&mut bundle, &root, &options).expect("the second save");
        assert!(stray.is_file(), "a save must not destroy a file the generation of record never wrote");
        let written: Manifest = read_json(&root.join(MANIFEST_FILE), "manifest").expect("the manifest is readable");
        assert!(
            written.files.contains_key("pages/page-99.json"),
            "§4: `files` describes every file, so the one that was kept is declared: {:?}",
            written.files.keys().collect::<Vec<_>>()
        );

        // The half that must still work: a page the generation of record wrote and this save drops
        // goes — and the kept stray still survives the same save (F9: it survived only one save
        // until the prune was reset to the *staged* set rather than the declared one).
        bundle.pages.remove(&PageId(8));
        save(&mut bundle, &root, &options).expect("the third save");
        assert!(!root.join(PAGES_DIR).join("page-8.json").exists(), "a page this save dropped is still pruned");
        assert!(stray.is_file(), "and a file no generation of record wrote is still not pruned, however many saves follow");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_writes_the_manifest_only_after_every_content_file() {
        // §10's whole guarantee is the *order*: content files first, the manifest last, so the
        // manifest can never describe partially written data. Asserting the contents does not test
        // that, so this makes step 4 fail — `project.json` cannot be replaced because a directory
        // sits where the file was — and then requires the **previous** manifest to still be on
        // disk. A writer that reached step 5 first would have overwritten it.
        let root = crate::scratch::scratch_dir_created("save-order-barrier");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("the first save succeeds");
        let previous_manifest = fs::read(root.join(MANIFEST_FILE)).expect("readable");

        fs::remove_file(root.join(PROJECT_FILE)).expect("remove the project");
        fs::create_dir(root.join(PROJECT_FILE)).expect("a directory where the project file was");
        let failed = save(&mut bundle, &root, &options);
        assert!(failed.is_err(), "step 4 cannot replace a file with a directory in the way");

        assert_eq!(
            fs::read(root.join(MANIFEST_FILE)).expect("readable"),
            previous_manifest,
            "the manifest is written LAST: a failure in step 4 must leave the previous generation's manifest in place"
        );
        assert!(!root.join(STAGING_DIR).exists(), "and no staging directory is left behind");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_refused_save_leaves_the_model_and_the_directory_as_it_found_them() {
        // A caller that gets an error back must be able to retry with the model it still holds, so
        // nothing may be derived onto `bundle` before the last fallible step has succeeded.
        let root = crate::scratch::scratch_dir_created("save-refused-atomic");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        let before = bundle.clone();
        let payload = DocumentPayload { relative_path: "../escape.vectorcraft".into(), format_version: 4, bytes: b"x".to_vec() };
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: Some(payload) };
        assert!(save(&mut bundle, &root, &options).is_err(), "a document path that leaves the bundle is refused");

        assert_eq!(bundle, before, "a refused save leaves the model exactly as it found it");
        assert!(!root.join(STAGING_DIR).exists(), "and no staging directory behind");
        assert!(!root.join(PROJECT_FILE).exists(), "and nothing written");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_writer_refuses_a_timestamp_that_is_not_canonical_anywhere_in_the_model() {
        // §5.7: "a **writer** must produce this form". Every timestamp the writer emits verbatim
        // from the model is checked, not only `SaveOptions.now`; repairing one would mean guessing a
        // timezone nobody recorded.
        let root = crate::scratch::scratch_dir_created("save-timestamp-fields");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };

        /// One field to make non-canonical, and the mutation that does it.
        type Case = (&'static str, fn(&mut ComicBundle));

        // One case per branch of `timestamps_written_verbatim`, so deleting any branch — including
        // the asset-provenance and job ones, which the first version of this test left unpinned —
        // reds this test.
        let cases: [Case; 8] = [
            ("manifest.created", |b| b.manifest.created = "2026-10-11 06:00:00".into()),
            ("project.metadata.created", |b| b.project.metadata.created = "2026-10-11T06:00:00+00:00".into()),
            ("Asset(17).provenance.created", |b| {
                let mut asset = asset_with_provenance();
                if let Some(provenance) = asset.provenance.as_mut() {
                    provenance.created = "2026-10-11 06:00:00".into();
                }
                b.assets.assets.push(asset);
            }),
            ("Asset(17).provenance.consent.granted", |b| {
                let mut asset = asset_with_provenance();
                if let Some(provenance) = asset.provenance.as_mut() {
                    provenance.consent =
                        Some(crate::entities::Consent { granted: "yesterday".into(), statement: "s".into(), unknown: Unknown::new() });
                }
                b.assets.assets.push(asset);
            }),
            ("GenerationJob(18).created", |b| {
                let mut in_flight = job(JobId(18));
                in_flight.created = "2026-10-11T06:00:00.5Z".into();
                b.provenance.jobs.push(in_flight);
            }),
            ("GenerationJob(18).updated", |b| {
                let mut in_flight = job(JobId(18));
                in_flight.updated = "2026-10-11 06:00:00".into();
                b.provenance.jobs.push(in_flight);
            }),
            ("GenerationJob(18).consent.granted", |b| {
                let mut in_flight = job(JobId(18));
                in_flight.consent =
                    Some(crate::entities::Consent { granted: "2026-10-11T06:00:00+00:00".into(), statement: "s".into(), unknown: Unknown::new() });
                b.provenance.jobs.push(in_flight);
            }),
            ("ReviewDecision(19).timestamp", |b| {
                b.provenance.decisions.push(ReviewDecision {
                    id: ReviewDecisionId(19),
                    subject: ReviewSubject::StoryContent,
                    decision: Decision::Approved,
                    actor: "x".into(),
                    timestamp: "yesterday".into(),
                    note: None,
                    superseded_asset_id: None,
                    unknown: Unknown::new(),
                });
            }),
        ];
        for (field, mutate) in cases {
            let mut bundle = empty_bundle();
            mutate(&mut bundle);
            match save(&mut bundle, &root, &options) {
                Err(Error::BadTimestamp { field: named, .. }) => assert_eq!(named, field, "the finding names the field"),
                other => panic!("a non-canonical `{field}` must be refused, got {other:?}"),
            }
        }

        // The job branch is reached through the live copy in `jobs/` too, not only through the
        // record: a job file written while the app ran carries the same fields.
        let mut bundle = empty_bundle();
        let mut in_flight = job(JobId(18));
        in_flight.created = "2026-10-11 06:00:00".into();
        bundle.jobs.insert(JobId(18), JobFile { schema_version: SCHEMA_VERSION, job: in_flight, unknown: Unknown::new() });
        match save(&mut bundle, &root, &options) {
            Err(Error::BadTimestamp { field, .. }) => assert_eq!(field, "GenerationJob(18).created"),
            other => panic!("the live copy in `jobs/` is checked too, got {other:?}"),
        }

        // And the timestamps the writer *derives* are not checked, because it overwrites them.
        let mut bundle = empty_bundle();
        bundle.manifest.updated = "nonsense".into();
        bundle.project.metadata.updated = "nonsense".into();
        save(&mut bundle, &root, &options).expect("`updated` is derived from `now`, so a stale one cannot be written");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unknown_key_on_a_manifest_file_entry_survives_a_save() {
        // §12: a field a newer ComicCraft wrote must survive a load-and-save by an older one. A
        // `manifest.files` entry's hash and size are derived, but its unknown keys belong to the
        // entry, so they must be carried over rather than rebuilt away.
        let root = crate::scratch::scratch_dir_created("save-file-entry-unknown");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("the first save succeeds");

        bundle.manifest.files.get_mut(PROJECT_FILE).expect("the entry").unknown.insert("mtime".into(), serde_json::json!(7));
        save(&mut bundle, &root, &options).expect("the second save succeeds");

        let written: Manifest = read_json(&root.join(MANIFEST_FILE), "manifest").expect("the manifest is readable");
        assert_eq!(written.files[PROJECT_FILE].unknown.get("mtime"), Some(&serde_json::json!(7)), "the entry's unknown key survives");
        assert_eq!(bundle.manifest.files[PROJECT_FILE].unknown.get("mtime"), Some(&serde_json::json!(7)));
        // The derived half is still derived: the hash describes the file as it now is.
        assert_eq!(written.files[PROJECT_FILE].sha256, crate::sha256_hex(&fs::read(root.join(PROJECT_FILE)).expect("readable")));

        // The other half: the carry-over looks each *present* key up in the previous map, so a key
        // for a file that is gone cannot come back. Carrying the whole map over would resurrect it.
        bundle.manifest.files.insert("pages/page-99.json".into(), FileEntry { sha256: "00".repeat(32), bytes: 1, unknown: Unknown::new() });
        save(&mut bundle, &root, &options).expect("the third save succeeds");
        let written: Manifest = read_json(&root.join(MANIFEST_FILE), "manifest").expect("the manifest is readable");
        assert!(!written.files.contains_key("pages/page-99.json"), "a key for a file that does not exist must not survive");
        assert!(!bundle.manifest.files.contains_key("pages/page-99.json"), "and the model must not keep it either");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn two_files_for_one_page_id_are_reported_rather_than_collapsed() {
        // §3 says padding is cosmetic and rule 13 parses the number, so `page-8.json` and
        // `page-008.json` are the same page. Which one the project means is not knowable, and §11
        // says to refuse rather than guess — so the loader reports the duplicate and keeps the
        // first in sorted key order, deterministically.
        let root = crate::scratch::scratch_dir_created("load-duplicate-page-file");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        bundle.pages.insert(PageId(8), page_file(PageId(8)));
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");
        // Make the two files differ, so *which* one wins is observable: a mutation that let the
        // last file win would otherwise be invisible.
        let mut padded = page_file(PageId(8));
        padded.page.status = crate::entities::PageStatus::Published;
        let padded_bytes = to_canonical_bytes("a page file", &padded).expect("serializes");
        fs::write(root.join("pages/page-008.json"), &padded_bytes).expect("the duplicate");

        let loaded = load(&root).expect("the ambiguous bundle still loads");
        assert_eq!(loaded.bundle.pages.len(), 1, "one page id");
        assert_eq!(
            loaded.bundle.pages[&PageId(8)].page.status,
            crate::entities::PageStatus::Published,
            "the first key in sorted order wins — `page-008.json` sorts before `page-8.json`"
        );
        assert!(
            loaded.findings.iter().any(|f| f.rule == Some(2) && f.subject == "pages/page-8.json"),
            "and the finding names the file that loses: {:?}",
            loaded.findings
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_load_reports_a_files_mismatch_rather_than_refusing() {
        // §14 step 1: the interrupted-save window must be survivable. Step 4 of a save both *adds*
        // content files and, through the prune, *removes* the ones the new generation dropped, so
        // both halves of the mismatch are the same condition — and §14 makes it one non-fatal
        // report, which is why both halves carry the same severity and the same absent rule.
        let root = crate::scratch::scratch_dir_created("load-mismatch");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        bundle.pages.insert(PageId(8), page_file(PageId(8)));
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");

        // A job file written while the app ran, outside the save protocol (§8.3) — the third,
        // benign cause — and a content file the prune would have removed before the manifest was
        // rewritten.
        fs::create_dir_all(root.join(JOBS_DIR)).expect("jobs dir");
        let in_flight = job(JobId(30));
        let file = JobFile { schema_version: SCHEMA_VERSION, job: in_flight, unknown: Unknown::new() };
        fs::write(root.join(JOBS_DIR).join("job-30.json"), serde_json::to_vec_pretty(&file).expect("json")).expect("job file");
        fs::remove_file(root.join("pages/page-8.json")).expect("the file the prune would have removed");

        let loaded = load(&root).expect("the load continues");
        let unlisted = loaded.findings.iter().find(|f| f.subject == "jobs/job-30.json").expect("the unlisted file is reported");
        let absent = loaded.findings.iter().find(|f| f.subject == "pages/page-8.json").expect("the absent declared file is reported");
        assert!(unlisted.message.contains("does not list it"), "{unlisted:?}");
        assert!(absent.message.contains("is not in the bundle"), "{absent:?}");
        for finding in [unlisted, absent] {
            assert_eq!(finding.severity, crate::Severity::Warning, "§14 makes the whole mismatch one non-fatal condition: {finding:?}");
            assert!(!finding.is_error(), "{finding:?}");
            assert_eq!(finding.rule, None, "no numbered rule governs the `files`/tree mismatch: {finding:?}");
        }
        assert!(loaded.bundle.jobs.contains_key(&JobId(30)), "and the job still loads");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_stray_json_file_in_pages_is_a_finding_not_a_lockout() {
        // §14 names exactly two fatal load failures and says everything else is "a finding on a
        // usable project, so a creator can open a damaged bundle and repair it rather than being
        // locked out". A `pages/` file a person dropped there must not make the bundle unopenable.
        let root = crate::scratch::scratch_dir_created("load-stray-page-file");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        bundle.pages.insert(PageId(8), page_file(PageId(8)));
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");
        fs::write(root.join(PAGES_DIR).join("notes.json"), b"{}\n").expect("a stray file");

        let loaded = load(&root).expect("§14: only a document or a comic file that will not parse is fatal");
        assert_eq!(loaded.bundle.pages.len(), 1, "the real page still loads");
        assert!(
            loaded.findings.iter().any(|f| f.rule == Some(13) && f.subject == "pages/notes.json"),
            "the stray file is reported under rule 13, which is the rule about page file names: {:?}",
            loaded.findings
        );
        assert!(root.join(PAGES_DIR).join("notes.json").is_file(), "and it is left where it is");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_keeps_a_declared_file_this_version_does_not_own() {
        // §12's promise for fields has the same shape for files: a `.json` in one of this crate's
        // directories whose name this version does not write is a **newer** ComicCraft's. §4 defines
        // `manifest.files` as covering every file, so a save must neither delete it nor drop its
        // entry — and the prune must still remove a page this version deleted.
        let root = crate::scratch::scratch_dir_created("save-keeps-foreign-file");
        fs::create_dir_all(root.join(DOCUMENT_DIR)).expect("dir");
        fs::write(root.join(DOCUMENT_DIR).join("comic.vectorcraft"), b"document bytes").expect("document");
        let mut bundle = empty_bundle();
        bundle.pages.insert(PageId(8), page_file(PageId(8)));
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("the first save succeeds");

        let foreign = b"{\n  \"schema_version\": 1,\n  \"relationships\": []\n}\n";
        fs::write(root.join("story/relationships.json"), foreign).expect("a file from a newer version");

        let mut loaded = load(&root).expect("the load accepts it");
        save(&mut loaded.bundle, &root, &options).expect("the second save succeeds");

        assert!(root.join("story/relationships.json").is_file(), "a save must not delete a file this version does not own");
        let written: Manifest = read_json(&root.join(MANIFEST_FILE), "manifest").expect("the manifest is readable");
        assert!(written.files.contains_key("story/relationships.json"), "and it stays declared: {:?}", written.files.keys().collect::<Vec<_>>());

        // The half that must still work: a page this version owns and the bundle no longer has goes.
        loaded.bundle.pages.remove(&PageId(8));
        save(&mut loaded.bundle, &root, &options).expect("the third save succeeds");
        assert!(!root.join("pages/page-8.json").exists(), "a deleted page's file is still pruned");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_collection_file_opens_as_an_empty_collection_with_a_finding() {
        // §14's sentence exists so "a creator can open a damaged bundle and repair it rather than
        // being locked out". A collection file that is *gone* is the absence of content, not
        // invented content: the empty collection is what the file said before anything was added
        // to it, the absent path is named, and every consequence is governed elsewhere — here the
        // fixture's `Scene.location_id` dangles, which rule 1 reports.
        let root = crate::scratch::scratch_dir_created("load-missing-collection");
        copy_fixture(&root);
        fs::remove_file(root.join("story/locations.json")).expect("remove the collection");

        let loaded = load(&root).expect("§14: a missing collection file is a finding, not a refusal");
        assert!(loaded.bundle.locations.locations.is_empty(), "it loaded as an empty collection");
        assert_eq!(
            loaded.bundle.locations.schema_version, loaded.bundle.manifest.schema_version,
            "an absent file cannot disagree with the manifest's version"
        );
        assert!(loaded.bundle.scenes.scenes[0].location_id.is_some(), "the scene still points at the location it had");
        // The absent path is named twice, on purpose: once by the `files`/tree mismatch (the file
        // is still declared) and once by this finding, which says what the *model* opened as. Both
        // are needed — the second is the consequence a creator has to act on — so the test selects
        // the model's half by its message.
        let finding = loaded
            .findings
            .iter()
            .find(|f| f.subject == "story/locations.json" && f.message.contains("empty collection"))
            .expect("the absent path is named, with what the project opened as");
        assert_eq!(finding.severity, crate::Severity::Warning, "§14 makes it one non-fatal report: {finding:?}");

        // The consequence is already governed: the dangling reference is a rule-1 finding.
        let document = crate::DocumentFacts::default();
        let findings = crate::validate(&loaded.bundle, &crate::ValidationInput { root: &root, document: &document, archive_root: None });
        assert!(
            findings.iter().any(|f| f.rule == Some(1) && f.subject == "Scene(4).location_id"),
            "a dangling `location_id` is rule 1's, which is why the empty collection is safe: {findings:?}"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_corrupt_collection_file_is_still_fatal() {
        // The other half of the role split, and the half that keeps it honest: an **absent**
        // collection file loads as empty, but one that is there and will not parse is one of the
        // two failures §14 makes fatal — "a comic file that will not parse". Treating it as absent
        // would silently drop content the file was declaring, which is the opposite of "a finding
        // on a usable project": the creator would see an empty library and no reason for it.
        let cases: [(&str, &[u8]); 3] = [
            ("story/locations.json", b"not json at all"),
            ("story/scenes.json", br#"{"schema_version": 1, "scenes": "not a list"}"#),
            ("assets/index.json", b"["),
        ];
        for (name, bytes) in cases {
            let root = crate::scratch::scratch_dir_created("load-corrupt-collection");
            copy_fixture(&root);
            fs::write(root.join(name), bytes).expect("write the broken file");
            match load(&root) {
                Err(Error::Parse { path, .. }) => assert_eq!(path, root.join(name), "the refusal names the file that will not parse"),
                other => panic!("a corrupt `{name}` must be fatal (§14 step 3), got {other:?}"),
            }
            fs::remove_dir_all(&root).ok();
        }
    }

    #[test]
    fn a_missing_identity_file_still_refuses() {
        // The other half of the split: `project.json` carries the project, the id counter and the
        // document reference, so without it there is nothing to report a finding against.
        let root = crate::scratch::scratch_dir_created("load-missing-identity");
        copy_fixture(&root);
        fs::remove_file(root.join(PROJECT_FILE)).expect("remove the project");

        match load(&root) {
            Err(Error::Read { path, .. }) => assert_eq!(path, root.join(PROJECT_FILE), "the refusal names the identity file"),
            other => panic!("a missing `project.json` must refuse, got {other:?}"),
        }

        // And `manifest.json` was already fatal, with its own, more specific error.
        let root = crate::scratch::scratch_dir_created("load-missing-manifest");
        copy_fixture(&root);
        fs::remove_file(root.join(MANIFEST_FILE)).expect("remove the manifest");
        assert!(matches!(load(&root), Err(Error::MissingManifest { .. })), "a bundle without its manifest is not a bundle");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_directory_without_a_manifest_is_not_a_bundle() {
        let root = crate::scratch::scratch_dir_created("load-no-manifest");
        assert!(matches!(load(&root), Err(Error::MissingManifest { .. })));
        fs::remove_dir_all(&root).ok();
    }
}
