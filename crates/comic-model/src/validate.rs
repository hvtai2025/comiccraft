//! The eighteen referential rules of schema §13, and nothing else.
//!
//! `comic.project.validate` **checks and reports; it never repairs.** Each rule is a
//! deterministic check from design §12.1, and every finding names the rule, the thing that
//! failed and why, so a creator can repair a damaged bundle rather than being locked out (§14).
//!
//! The rules are run against a *loaded* model plus the three facts this crate cannot read for
//! itself ([`DocumentFacts`]): which artboards exist (rules 4, 15), which node ids exist (rule 7)
//! and where the files are (rule 9). `crates/format` is L3 and this crate is L2, so the document
//! is never parsed here — see [`crate::document`].
//!
//! Rule 9 is the one rule with a security consequence, and it is implemented exactly as written:
//! resolve against the **declared base**, canonicalise, and only then check containment. A path
//! that escapes is never resolved, so that asset is reported unresolved (rule 1) and the rest of
//! the validation continues.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::bundle::{ComicBundle, MANIFEST_FILE};
use crate::document::DocumentFacts;
use crate::entities::{HASH_ALGORITHM_SHA256, StorageRef};
use crate::json::{PathResolution, resolve_path, sha256_hex};

/// How seriously a finding is meant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Recoverable, and legal in some projects: an orphan page (rule 14), a page whose artboard
    /// deliberately differs from the default (rule 15), a manifest that describes the previous
    /// generation (§14).
    Warning,
    /// A referential inconsistency. Reported, never repaired.
    Error,
}

/// One thing `comic.project.validate` found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The §13 rule that produced it, or `None` for a §14 load observation (the `files`/tree
    /// mismatch, a counter repaired upward) which is not one of the eighteen.
    pub rule: Option<u8>,
    /// How seriously to take it.
    pub severity: Severity,
    /// What it is about — a file key, an entity, a field.
    pub subject: String,
    /// Why it failed, in terms a creator can act on.
    pub message: String,
}

impl Finding {
    /// A referential-rule failure.
    #[must_use]
    pub fn error(rule: Option<u8>, subject: String, message: String) -> Self {
        Self { rule, severity: Severity::Error, subject, message }
    }

    /// Something recoverable.
    #[must_use]
    pub fn warning(rule: Option<u8>, subject: String, message: String) -> Self {
        Self { rule, severity: Severity::Warning, subject, message }
    }

    /// Whether this is an error rather than a warning.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rule {
            Some(n) => write!(f, "rule {n}: {} — {}", self.subject, self.message),
            None => write!(f, "{} — {}", self.subject, self.message),
        }
    }
}

/// What the eighteen rules need besides the model.
///
/// The document is supplied rather than read: see the module docs.
#[derive(Clone, Copy, Debug)]
pub struct ValidationInput<'a> {
    /// The bundle's directory, the base every `StorageRef::Linked` resolves against (rule 9).
    pub root: &'a Path,
    /// The artboards and node ids of the loaded visual document (rules 4, 7, 15).
    pub document: &'a DocumentFacts,
    /// The archive root a `StorageRef::Package` resolves against (design §7.5). `None` outside a
    /// packaged archive, where a `Package` reference cannot resolve at all — it names a
    /// different container, and resolving it against the bundle root is the path-traversal bug
    /// rule 9 exists to prevent.
    pub archive_root: Option<&'a Path>,
}

/// Run rules 1–18 and return what they found, in a stable order.
///
/// Nothing here mutates the bundle: a rule that reported *and* fixed would hide the
/// inconsistency from the creator, which §13 forbids.
#[must_use]
pub fn validate(bundle: &ComicBundle, input: &ValidationInput<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    let index = Index::build(bundle);
    rule_1_references(bundle, &index, &mut out);
    rule_2_duplicate_ids(bundle, &mut out);
    rule_3_versions_and_identity(bundle, &mut out);
    rule_4_page_artboards(bundle, input, &mut out);
    rule_5_denormalised_artboards(bundle, &mut out);
    rule_6_reading_order(bundle, &mut out);
    rule_7_node_ids(bundle, input, &mut out);
    rule_8_lettering_payload(bundle, &mut out);
    rule_9_paths(bundle, input, &mut out);
    rule_10_provenance(bundle, &mut out);
    rule_11_page_panel_membership(bundle, &mut out);
    rule_12_dialogue_speakers(bundle, &index, &mut out);
    rule_13_page_file_names(bundle, &mut out);
    rule_14_orphan_pages(bundle, &mut out);
    rule_15_page_size_against_the_default(bundle, input, &mut out);
    rule_16_hash_algorithms(bundle, &mut out);
    rule_17_retired_artboard_ids(bundle, &mut out);
    rule_18_job_files(bundle, &mut out);
    out.sort_by(|a, b| (a.rule.unwrap_or(0), &a.subject, &a.message).cmp(&(b.rule.unwrap_or(0), &b.subject, &b.message)));
    out
}

/// One page's visual home, reported as **information** (design §5.3.1 mitigation 3).
///
/// `comic.project.validate` verifies that every `Page.artboard_id` exists and is not retired, and
/// reports the artboard's `rect` alongside it. It checks **no size expectation**, because there is
/// none to hold one: the artboard *is* the page's size, and rule 15 only warns when it differs
/// from `ProjectSettings.default_page_size`. This is that report, kept separate from
/// [`Finding`] so that a clean project returns no findings at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageGeometry {
    /// The page.
    pub page_id: crate::PageId,
    /// The artboard that is its visual home.
    pub artboard_id: u32,
    /// The artboard's rectangle in document points — the page's size.
    pub rect: vectorcraft_geom::Rect,
}

/// The artboard rectangle behind each page, as information (design §5.3.1 mitigation 3).
///
/// A page whose artboard is missing or retired is skipped: that is a [`Finding`] from rule 4, and
/// reporting a rectangle that does not exist would be worse than reporting nothing.
#[must_use]
pub fn page_geometry(bundle: &ComicBundle, document: &DocumentFacts) -> Vec<PageGeometry> {
    let mut out: Vec<PageGeometry> = bundle
        .pages
        .values()
        .filter(|file| !bundle.manifest.is_artboard_retired(file.page.artboard_id))
        .filter_map(|file| document.artboard(file.page.artboard_id).map(|a| PageGeometry { page_id: file.page.id, artboard_id: a.id, rect: a.rect }))
        .collect();
    out.sort_by_key(|g| g.page_id.0);
    out
}

/// The id sets the reference rules need, built once.
struct Index {
    chapters: BTreeSet<u64>,
    scenes: BTreeSet<u64>,
    characters: BTreeSet<u64>,
    locations: BTreeSet<u64>,
    styles: BTreeSet<u64>,
    pages: BTreeSet<u64>,
    panels: BTreeSet<u64>,
    dialogue: BTreeSet<u64>,
    lettering: BTreeSet<u64>,
    assets: BTreeSet<u64>,
}

impl Index {
    /// One set per entity kind, because rule 1 checks that a reference names the **right kind** of
    /// entity, not merely that some entity has that id.
    ///
    /// A single "everything under `pages/`" set — page, panel, dialogue and lettering ids together
    /// — would accept `Scene.page_ids` naming a panel, and `Lettering.page_id` naming a lettering.
    /// Nothing else in the eighteen rules governs either reference: `Panel.page_id` is cross-checked
    /// by rule 11 against its containing file, but `Scene.page_ids` and `Lettering.page_id` are
    /// guarded by rule 1 alone.
    fn build(bundle: &ComicBundle) -> Self {
        Self {
            chapters: bundle.chapters.chapters.iter().map(|c| c.id.0).collect(),
            scenes: bundle.scenes.scenes.iter().map(|s| s.id.0).collect(),
            characters: bundle.characters.characters.iter().map(|c| c.id.0).collect(),
            locations: bundle.locations.locations.iter().map(|l| l.id.0).collect(),
            styles: bundle.styles.styles.iter().map(|s| s.id.0).collect(),
            pages: bundle.pages.keys().map(|p| p.0).collect(),
            panels: bundle.pages.values().flat_map(|f| f.panels.iter().map(|p| p.id.0)).collect(),
            dialogue: bundle.pages.values().flat_map(|f| f.dialogue.iter().map(|d| d.id.0)).collect(),
            lettering: bundle.pages.values().flat_map(|f| f.lettering.iter().map(|l| l.id.0)).collect(),
            assets: bundle.assets.assets.iter().map(|a| a.id.0).collect(),
        }
    }
}

/// Rule 1 — every reference resolves. Singular foreign keys included, and a `null` key is not a
/// reference and is skipped (§13 rule 1).
fn rule_1_references(bundle: &ComicBundle, index: &Index, out: &mut Vec<Finding>) {
    let mut want = |owner: String, field: &str, id: u64, set: &BTreeSet<u64>, kind: &str| {
        if !set.contains(&id) {
            out.push(Finding::error(Some(1), format!("{owner}.{field}"), format!("{id} does not resolve to a {kind}")));
        }
    };
    let project = format!("ComicProject({})", bundle.project.id.0);
    for id in &bundle.project.chapter_ids {
        want(project.clone(), "chapter_ids", id.0, &index.chapters, "Chapter");
    }
    for id in &bundle.project.character_ids {
        want(project.clone(), "character_ids", id.0, &index.characters, "Character");
    }
    for id in &bundle.project.location_ids {
        want(project.clone(), "location_ids", id.0, &index.locations, "Location");
    }
    for id in &bundle.project.style_profile_ids {
        want(project.clone(), "style_profile_ids", id.0, &index.styles, "StyleProfile");
    }

    for c in &bundle.chapters.chapters {
        let owner = format!("Chapter({})", c.id.0);
        want(owner.clone(), "project_id", c.project_id.0, &BTreeSet::from([bundle.project.id.0]), "ComicProject");
        for id in &c.scene_ids {
            want(owner.clone(), "scene_ids", id.0, &index.scenes, "Scene");
        }
    }
    for s in &bundle.scenes.scenes {
        let owner = format!("Scene({})", s.id.0);
        want(owner.clone(), "chapter_id", s.chapter_id.0, &index.chapters, "Chapter");
        for id in &s.page_ids {
            want(owner.clone(), "page_ids", id.0, &index.pages, "Page");
        }
        for id in &s.character_ids {
            want(owner.clone(), "character_ids", id.0, &index.characters, "Character");
        }
        if let Some(id) = s.location_id {
            want(owner.clone(), "location_id", id.0, &index.locations, "Location");
        }
    }
    for c in &bundle.characters.characters {
        let owner = format!("Character({})", c.id.0);
        for id in &c.reference_asset_ids {
            want(owner.clone(), "reference_asset_ids", id.0, &index.assets, "Asset");
        }
        if let Some(id) = c.style_profile_id {
            want(owner.clone(), "style_profile_id", id.0, &index.styles, "StyleProfile");
        }
        if let Some(id) = c.approved_design_version {
            want(owner.clone(), "approved_design_version", id.0, &index.assets, "Asset");
        }
    }
    for l in &bundle.locations.locations {
        let owner = format!("Location({})", l.id.0);
        for id in &l.reference_asset_ids {
            want(owner.clone(), "reference_asset_ids", id.0, &index.assets, "Asset");
        }
    }
    for file in bundle.pages.values() {
        let owner = format!("Page({})", file.page.id.0);
        want(owner.clone(), "chapter_id", file.page.chapter_id.0, &index.chapters, "Chapter");
        for id in &file.page.scene_ids {
            want(owner.clone(), "scene_ids", id.0, &index.scenes, "Scene");
        }
        for id in &file.page.panel_ids {
            want(owner.clone(), "panel_ids", id.0, &index.panels, "Panel");
        }
        for id in &file.page.reading_order {
            want(owner.clone(), "reading_order", id.0, &index.panels, "Panel");
        }
        for p in &file.panels {
            let owner = format!("Panel({})", p.id.0);
            want(owner.clone(), "page_id", p.page_id.0, &index.pages, "Page");
            want(owner.clone(), "scene_id", p.scene_id.0, &index.scenes, "Scene");
            for a in &p.artwork_refs {
                want(owner.clone(), "artwork_refs[].asset_id", a.asset_id.0, &index.assets, "Asset");
            }
            for id in &p.dialogue_ids {
                want(owner.clone(), "dialogue_ids", id.0, &index.dialogue, "Dialogue");
            }
            for id in &p.lettering_ids {
                want(owner.clone(), "lettering_ids", id.0, &index.lettering, "Lettering");
            }
            for id in &p.continuity_constraints.characters {
                want(owner.clone(), "continuity_constraints.characters", id.0, &index.characters, "Character");
            }
        }
        for d in &file.dialogue {
            let owner = format!("Dialogue({})", d.id.0);
            want(owner.clone(), "panel_id", d.panel_id.0, &index.panels, "Panel");
        }
        for l in &file.lettering {
            let owner = format!("Lettering({})", l.id.0);
            want(owner.clone(), "page_id", l.page_id.0, &index.pages, "Page");
            want(owner.clone(), "panel_id", l.panel_id.0, &index.panels, "Panel");
            if let Some(id) = l.dialogue_id {
                want(owner.clone(), "dialogue_id", id.0, &index.dialogue, "Dialogue");
            }
        }
    }
    for a in &bundle.assets.assets {
        let owner = format!("Asset({})", a.id.0);
        for id in &a.parent_asset_ids {
            want(owner.clone(), "parent_asset_ids", id.0, &index.assets, "Asset");
        }
    }
    for j in &bundle.provenance.jobs {
        let owner = format!("GenerationJob({})", j.id.0);
        if let Some(id) = j.target_panel_id {
            want(owner.clone(), "target_panel_id", id.0, &index.panels, "Panel");
        }
        for id in j.input_asset_ids.iter().chain(j.result_asset_ids.iter()) {
            want(owner.clone(), "input_asset_ids/result_asset_ids", id.0, &index.assets, "Asset");
        }
    }
    for j in bundle.jobs.values() {
        let owner = format!("GenerationJob({})", j.job.id.0);
        if let Some(id) = j.job.target_panel_id {
            want(owner.clone(), "target_panel_id", id.0, &index.panels, "Panel");
        }
        for id in j.job.input_asset_ids.iter().chain(j.job.result_asset_ids.iter()) {
            want(owner.clone(), "input_asset_ids/result_asset_ids", id.0, &index.assets, "Asset");
        }
    }
    for d in &bundle.provenance.decisions {
        let owner = format!("ReviewDecision({})", d.id.0);
        if let Some(id) = d.superseded_asset_id {
            want(owner.clone(), "superseded_asset_id", id.0, &index.assets, "Asset");
        }
    }
    // `retired_artboard_ids` is deliberately **not** walked: it holds ids that must *not* resolve
    // (rule 4), and a rule that demanded they did would contradict it.
}

/// Rule 2 — no duplicate id within an entity kind, and none across kinds.
fn rule_2_duplicate_ids(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    let mut kinds: Vec<(&'static str, u64)> = Vec::new();
    kinds.push(("ComicProject", bundle.project.id.0));
    kinds.push(("AssetRegistry", bundle.project.asset_registry_id.0));
    kinds.push(("Bundle", bundle.manifest.bundle_id.0));
    for c in &bundle.chapters.chapters {
        kinds.push(("Chapter", c.id.0));
    }
    for s in &bundle.scenes.scenes {
        kinds.push(("Scene", s.id.0));
    }
    for c in &bundle.characters.characters {
        kinds.push(("Character", c.id.0));
    }
    for l in &bundle.locations.locations {
        kinds.push(("Location", l.id.0));
    }
    for s in &bundle.styles.styles {
        kinds.push(("StyleProfile", s.id.0));
    }
    for f in bundle.pages.values() {
        kinds.push(("Page", f.page.id.0));
        for p in &f.panels {
            kinds.push(("Panel", p.id.0));
        }
        for d in &f.dialogue {
            kinds.push(("Dialogue", d.id.0));
        }
        for l in &f.lettering {
            kinds.push(("Lettering", l.id.0));
        }
    }
    for a in &bundle.assets.assets {
        kinds.push(("Asset", a.id.0));
    }
    for j in &bundle.provenance.jobs {
        kinds.push(("GenerationJob", j.id.0));
    }
    for d in &bundle.provenance.decisions {
        kinds.push(("ReviewDecision", d.id.0));
    }
    // `manifest.comic_id` is deliberately absent: §4 makes it a *copy* of `ComicProject.id`, so
    // counting it would make every legal bundle a duplicate. Rule 3 checks that the copy agrees.
    // `jobs/` is absent too: §8.3 makes a job file the live copy of a record already in
    // `provenance/`, so the same `JobId` in both places is one job, not two.

    let mut by_id: BTreeMap<u64, BTreeSet<&'static str>> = BTreeMap::new();
    for (kind, id) in &kinds {
        by_id.entry(*id).or_default().insert(kind);
    }
    for (id, seen) in &by_id {
        if seen.len() > 1 {
            let names: Vec<&str> = seen.iter().copied().collect();
            out.push(Finding::error(
                Some(2),
                format!("id {id}"),
                format!("the same id is used by {} — ids are unique across kinds", names.join(", ")),
            ));
        }
    }
    let mut counted: BTreeMap<(&'static str, u64), usize> = BTreeMap::new();
    for entry in &kinds {
        *counted.entry(*entry).or_default() += 1;
    }
    for ((kind, id), count) in counted {
        if count > 1 {
            out.push(Finding::error(Some(2), format!("{kind}({id})"), format!("the id appears {count} times in one entity kind")));
        }
    }

    // A job file's name encodes the job's id (§8.3), exactly as a page file's does (§13 rule 13).
    for (key, file) in &bundle.jobs {
        if file.job.id != *key {
            out.push(Finding::error(
                Some(2),
                ComicBundle::job_file_key(*key),
                format!("the file name encodes job {} but the record's id is {} (§8.3: one file per job, `job-<JobId>.json`)", key.0, file.job.id.0),
            ));
        }
    }
}

/// Rule 3 — the manifest identifies the project, and every file that declares a `schema_version`
/// declares the manifest's.
fn rule_3_versions_and_identity(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    let version = bundle.manifest.schema_version;
    if bundle.manifest.comic_id != bundle.project.id {
        out.push(Finding::error(
            Some(3),
            MANIFEST_FILE.to_string(),
            format!("`comic_id` is {} but `project.id` is {}", bundle.manifest.comic_id.0, bundle.project.id.0),
        ));
    }
    if bundle.manifest.document != bundle.project.visual_document {
        out.push(Finding::error(
            Some(3),
            MANIFEST_FILE.to_string(),
            format!("`document` is {:?} but `project.visual_document` is {:?}", bundle.manifest.document, bundle.project.visual_document),
        ));
    }
    // The subject is the file's **real** path, not its template: a creator told that
    // "`jobs/job-<JobId>.json` declares 2" cannot tell which of several job files to fix.
    let mut declared: Vec<(String, u32)> = vec![
        ("project.json".to_string(), bundle.project.schema_version),
        ("story/chapters.json".to_string(), bundle.chapters.schema_version),
        ("story/scenes.json".to_string(), bundle.scenes.schema_version),
        ("story/characters.json".to_string(), bundle.characters.schema_version),
        ("story/locations.json".to_string(), bundle.locations.schema_version),
        ("story/styles.json".to_string(), bundle.styles.schema_version),
        ("assets/index.json".to_string(), bundle.assets.schema_version),
        ("provenance/generations.json".to_string(), bundle.provenance.schema_version),
    ];
    for (id, file) in &bundle.pages {
        declared.push((ComicBundle::page_file_key(*id), file.schema_version));
    }
    for (id, file) in &bundle.jobs {
        declared.push((ComicBundle::job_file_key(*id), file.schema_version));
    }
    for (what, found) in declared {
        if found != version {
            out.push(Finding::error(Some(3), what, format!("declares `schema_version` {found} but the manifest declares {version}")));
        }
    }
}

/// Rule 4 — every `Page.artboard_id` exists in the document and is not retired.
fn rule_4_page_artboards(bundle: &ComicBundle, input: &ValidationInput<'_>, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        let id = file.page.artboard_id;
        let subject = format!("Page({}).artboard_id", file.page.id.0);
        if bundle.manifest.is_artboard_retired(id) {
            out.push(Finding::error(
                Some(4),
                subject,
                format!(
                    "artboard {id} is in `retired_artboard_ids`: the page resolves to a deleted page's artboard and is broken, not trusted (§9.1)"
                ),
            ));
        } else if !input.document.has_artboard(id) {
            out.push(Finding::error(Some(4), subject, format!("artboard {id} does not exist in the visual document")));
        }
    }
}

/// Rule 5 — a panel's and a lettering's `artboard_id` equals their page's.
fn rule_5_denormalised_artboards(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        for p in &file.panels {
            match bundle.pages.get(&p.page_id) {
                Some(page) if page.page.artboard_id != p.artboard_id => out.push(Finding::error(
                    Some(5),
                    format!("Panel({}).artboard_id", p.id.0),
                    format!("is {} but Page({}).artboard_id is {}", p.artboard_id, p.page_id.0, page.page.artboard_id),
                )),
                _ => {}
            }
        }
        for l in &file.lettering {
            match bundle.pages.get(&l.page_id) {
                Some(page) if page.page.artboard_id != l.artboard_id => out.push(Finding::error(
                    Some(5),
                    format!("Lettering({}).artboard_id", l.id.0),
                    format!("is {} but Page({}).artboard_id is {}", l.artboard_id, l.page_id.0, page.page.artboard_id),
                )),
                _ => {}
            }
        }
    }
}

/// Rule 6 — `Page.reading_order` is a permutation of `Page.panel_ids`, and each panel's
/// `reading_order_index` agrees with its position.
fn rule_6_reading_order(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        let subject = format!("Page({}).reading_order", file.page.id.0);
        let membership: BTreeSet<u64> = file.page.panel_ids.iter().map(|p| p.0).collect();
        let order: BTreeSet<u64> = file.page.reading_order.iter().map(|p| p.0).collect();
        if membership != order || file.page.panel_ids.len() != file.page.reading_order.len() {
            out.push(Finding::error(
                Some(6),
                subject.clone(),
                format!(
                    "`reading_order` ({:?}) is not a permutation of `panel_ids` ({:?})",
                    file.page.reading_order.iter().map(|p| p.0).collect::<Vec<_>>(),
                    file.page.panel_ids.iter().map(|p| p.0).collect::<Vec<_>>()
                ),
            ));
        }
        for (position, id) in file.page.reading_order.iter().enumerate() {
            let Some(panel) = file.panels.iter().find(|p| p.id == *id) else { continue };
            let position = u16::try_from(position).unwrap_or(u16::MAX);
            if panel.reading_order_index != position {
                out.push(Finding::error(
                    Some(6),
                    format!("Panel({}).reading_order_index", panel.id.0),
                    format!("is {} but the panel is at position {position} in `Page.reading_order`", panel.reading_order_index),
                ));
            }
        }
    }
}

/// Rule 7 — every visual reference resolves to a node in the document.
fn rule_7_node_ids(bundle: &ComicBundle, input: &ValidationInput<'_>, out: &mut Vec<Finding>) {
    let mut want = |subject: String, node: crate::entities::NodeId| {
        if !input.document.nodes.contains(&node) {
            out.push(Finding::error(Some(7), subject, format!("node {} does not exist in the visual document", node.0)));
        }
    };
    for file in bundle.pages.values() {
        for p in &file.panels {
            want(format!("Panel({}).frame_node_id", p.id.0), p.frame_node_id);
            for a in &p.artwork_refs {
                want(format!("Panel({}).artwork_refs[].node_id", p.id.0), a.node_id);
            }
        }
        for l in &file.lettering {
            want(format!("Lettering({}).frame_node_id", l.id.0), l.frame_node_id);
            want(format!("Lettering({}).text_node_id", l.id.0), l.text_node_id);
            if let Some(tail) = l.tail_node_id {
                want(format!("Lettering({}).tail_node_id", l.id.0), tail);
            }
        }
    }
}

/// Rule 8 — `dialogue_id` is present exactly for the spoken kinds, and `text` exactly for the
/// kinds that carry their own.
fn rule_8_lettering_payload(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        for l in &file.lettering {
            let subject = format!("Lettering({})", l.id.0);
            if l.kind.is_spoken() && l.dialogue_id.is_none() {
                out.push(Finding::error(Some(8), subject.clone(), format!("`{:?}` is spoken, so `dialogue_id` is required", l.kind)));
            }
            if l.kind.carries_own_text() && l.dialogue_id.is_some() {
                out.push(Finding::error(Some(8), subject.clone(), format!("`{:?}` is not spoken, so `dialogue_id` must be null", l.kind)));
            }
            if l.kind.carries_own_text() && l.text.is_none() {
                out.push(Finding::error(Some(8), subject.clone(), format!("`{:?}` carries its own text, so `text` is required", l.kind)));
            }
            if l.kind.is_spoken() && l.text.is_some() {
                out.push(Finding::error(
                    Some(8),
                    subject.clone(),
                    format!("`{:?}` is spoken, so `text` must be null — the linked Dialogue owns the words", l.kind),
                ));
            }
        }
    }
}

/// Rule 9 — every path is resolved against its **declared base**, canonicalised, and only then
/// checked to be inside that base.
///
/// The three bases are not one base: `Linked` against the bundle root, `Package` against the
/// archive root, and the document's own `LinkInfo.relative` against the document folder (which
/// this crate never reads — it is `crates/format`'s and the engine's). `Embedded` is a key in the
/// document's image map, not a path, so there is nothing to confine.
///
/// A path that escapes is **never resolved**: the asset is reported unresolved (rule 1) and the
/// rest of the validation continues.
fn rule_9_paths(bundle: &ComicBundle, input: &ValidationInput<'_>, out: &mut Vec<Finding>) {
    for asset in &bundle.assets.assets {
        let subject = format!("Asset({})", asset.id.0);
        let Some((base_name, relative)) = asset.storage_reference.path() else { continue };
        let base = if matches!(&asset.storage_reference, StorageRef::Package { .. }) {
            match input.archive_root {
                Some(root) => root,
                None => {
                    out.push(Finding::error(
                        Some(9),
                        subject.clone(),
                        "is `Package`, whose base is the archive root (design §7.5); this is a directory bundle, so the reference cannot resolve"
                            .into(),
                    ));
                    continue;
                }
            }
        } else {
            input.root
        };
        match resolve_path(base, relative) {
            PathResolution::Resolved(path) => match fs::read(&path) {
                Ok(bytes) => {
                    let found = sha256_hex(&bytes);
                    if found != asset.content_hash.value {
                        out.push(Finding::error(
                            Some(9),
                            subject.clone(),
                            format!("`{relative}` hashes {found} but `content_hash.value` is {}", asset.content_hash.value),
                        ));
                    }
                }
                Err(source) => out.push(Finding::error(Some(9), subject.clone(), format!("`{relative}` cannot be read: {source}"))),
            },
            PathResolution::Escapes => {
                out.push(Finding::error(Some(9), subject.clone(), format!("`{relative}` escapes the {base_name} and is never resolved (§13.1)")))
            }
            PathResolution::Missing => out.push(Finding::error(Some(9), subject.clone(), format!("`{relative}` is not there under the {base_name}"))),
        }
    }
}

/// Rule 10 — provenance is present for `generated` and `edited`, and absent otherwise.
fn rule_10_provenance(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for asset in &bundle.assets.assets {
        let subject = format!("Asset({}).provenance", asset.id.0);
        if asset.asset_type.requires_provenance() && asset.provenance.is_none() {
            out.push(Finding::error(Some(10), subject.clone(), format!("`{:?}` must record what produced it", asset.asset_type)));
        }
        if !asset.asset_type.requires_provenance() && asset.provenance.is_some() {
            out.push(Finding::error(Some(10), subject.clone(), format!("`{:?}` must not claim a generation it did not have", asset.asset_type)));
        }
    }
}

/// Rule 11 — `Page.panel_ids` agrees with the panels in that page's file.
fn rule_11_page_panel_membership(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        let subject = format!("Page({}).panel_ids", file.page.id.0);
        let listed: BTreeSet<u64> = file.page.panel_ids.iter().map(|p| p.0).collect();
        let present: BTreeSet<u64> = file.panels.iter().map(|p| p.id.0).collect();
        if listed != present {
            out.push(Finding::error(Some(11), subject, format!("lists {listed:?} but the page's file holds {present:?}")));
        }
        for panel in &file.panels {
            if panel.page_id != file.page.id {
                out.push(Finding::error(
                    Some(11),
                    format!("Panel({}).page_id", panel.id.0),
                    format!("is {} but the panel is in the file of Page({})", panel.page_id.0, file.page.id.0),
                ));
            }
        }
    }
}

/// Rule 12 — every `speaker_character_id` that is **not `null`** resolves. `null` is legal and
/// means narration, so the rule checks the set values, not the field's presence.
fn rule_12_dialogue_speakers(bundle: &ComicBundle, index: &Index, out: &mut Vec<Finding>) {
    for file in bundle.pages.values() {
        for d in &file.dialogue {
            if let Some(speaker) = d.speaker_character_id
                && !index.characters.contains(&speaker.0)
            {
                out.push(Finding::error(
                    Some(12),
                    format!("Dialogue({}).speaker_character_id", d.id.0),
                    format!("{} does not resolve to a Character", speaker.0),
                ));
            }
        }
    }
}

/// Rule 13 — `Page.id` equals the number in its file name. The number is parsed, not compared:
/// padding is cosmetic, so `page-7.json` and `page-007.json` both mean page 7.
fn rule_13_page_file_names(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for (id, file) in &bundle.pages {
        if file.page.id != *id {
            out.push(Finding::error(
                Some(13),
                ComicBundle::page_file_key(*id),
                format!("the file name encodes page {} but the record's id is {}", id.0, file.page.id.0),
            ));
        }
    }
}

/// Rule 14 — a page file no scene reaches is a **warning**: an orphan is recoverable, and a
/// scene-less page is legal.
fn rule_14_orphan_pages(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    let reached: BTreeSet<u64> = bundle.scenes.scenes.iter().flat_map(|s| s.page_ids.iter().map(|p| p.0)).collect();
    for id in bundle.pages.keys() {
        if !reached.contains(&id.0) {
            out.push(Finding::warning(
                Some(14),
                ComicBundle::page_file_key(*id),
                "no `Scene.page_ids` reaches this page: an orphan is recoverable, and a scene-less page is legal".into(),
            ));
        }
    }
}

/// Rule 15 — a page artboard's `rect` differing from `ProjectSettings.default_page_size` is a
/// **warning**: the artboard *is* the page's size and a page may deliberately differ. There is no
/// per-page "expected size" field to compare against, which is why this cannot be an error.
fn rule_15_page_size_against_the_default(bundle: &ComicBundle, input: &ValidationInput<'_>, out: &mut Vec<Finding>) {
    let Some(default) = bundle.project.settings.default_page_size else { return };
    for file in bundle.pages.values() {
        let Some(artboard) = input.document.artboard(file.page.artboard_id) else { continue };
        let (width, height) = (artboard.rect.width(), artboard.rect.height());
        if width != default.width || height != default.height {
            out.push(Finding::warning(
                Some(15),
                format!("Page({}).artboard_id", file.page.id.0),
                format!(
                    "artboard {} is {width}×{height} pt but `default_page_size` is {}×{} pt — a page may deliberately differ",
                    file.page.artboard_id, default.width, default.height
                ),
            ));
        }
    }
}

/// Rule 16 — every asset's `content_hash.algorithm` is one this reader implements.
fn rule_16_hash_algorithms(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for asset in &bundle.assets.assets {
        if asset.content_hash.algorithm != HASH_ALGORITHM_SHA256 {
            out.push(Finding::error(
                Some(16),
                format!("Asset({}).content_hash.algorithm", asset.id.0),
                format!("`{}` is not an algorithm this reader implements (only `{HASH_ALGORITHM_SHA256}`)", asset.content_hash.algorithm),
            ));
        }
    }
}

/// Rule 17 — `retired_artboard_ids` is unique and append-only.
///
/// Uniqueness is checkable from one snapshot; "append-only" is not, because it is a property of
/// every *previous* state. It is enforced where it can be: [`Manifest::retire_artboard_id`] is
/// the only supported way to retire an id and it never removes one, because removing a retired
/// id is exactly what would let a stale `Page.artboard_id` resolve again.
fn rule_17_retired_artboard_ids(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    let mut seen = BTreeSet::new();
    for id in &bundle.manifest.retired_artboard_ids {
        if !seen.insert(*id) {
            out.push(Finding::error(
                Some(17),
                "manifest.retired_artboard_ids".to_string(),
                format!("artboard {id} is retired twice; the list is unique and append-only"),
            ));
        }
    }
}

/// Rule 18 — every file in `jobs/` holds a job whose status is **not terminal**.
///
/// Terminal is exactly `failed` or `cancelled`. `completed` and `awaiting_review` are not
/// terminal: a job holding a result a human has not accepted legitimately keeps its file, so
/// `jobs/` is not empty in a quiescent project. Rule 18 checks only this half, because
/// consumption is not visible in the status — a `completed` file whose result was applied is
/// removed by the command, not by a status change.
fn rule_18_job_files(bundle: &ComicBundle, out: &mut Vec<Finding>) {
    for (id, file) in &bundle.jobs {
        if file.job.status.is_terminal() {
            out.push(Finding::error(
                Some(18),
                ComicBundle::job_file_key(*id),
                format!(
                    "holds a job whose status is `{:?}`, which is terminal: the file is removed when the job becomes terminal (§8.3)",
                    file.job.status
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{
        AssetIndexFile, ChaptersFile, CharactersFile, JobFile, LocationsFile, Manifest, PageFile, ProvenanceFile, SaveOptions, ScenesFile,
        StylesFile, save,
    };
    use crate::document::ArtboardFacts;
    use crate::entities::*;
    use crate::ids::*;

    fn unknown() -> Unknown {
        Unknown::new()
    }

    fn document() -> DocumentFacts {
        DocumentFacts {
            artboards: vec![ArtboardFacts { id: 1, rect: vectorcraft_geom::Rect::new(0.0, 0.0, 612.0, 792.0) }],
            nodes: [2u64, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13].into_iter().map(NodeId).collect(),
        }
    }

    fn project_settings() -> ProjectSettings {
        ProjectSettings {
            default_page_size: Some(PageSize::new(612.0, 792.0)),
            default_template: None,
            default_lettering_style: None,
            unknown: unknown(),
        }
    }

    fn bundle() -> ComicBundle {
        let mut chapters = ChaptersFile { schema_version: 1, chapters: vec![], unknown: unknown() };
        chapters.chapters.push(Chapter {
            id: ChapterId(3),
            project_id: ComicId(1),
            title: "One".into(),
            scene_ids: vec![SceneId(4)],
            order_index: 0,
            unknown: unknown(),
        });
        let scenes = ScenesFile {
            schema_version: 1,
            scenes: vec![Scene {
                id: SceneId(4),
                chapter_id: ChapterId(3),
                title: "Rooftop".into(),
                synopsis: "".into(),
                page_ids: vec![PageId(8)],
                location_id: Some(LocationId(6)),
                character_ids: vec![CharacterId(5)],
                order_index: 0,
                continuity_notes: "".into(),
                unknown: unknown(),
            }],
            unknown: unknown(),
        };
        let characters = CharactersFile {
            schema_version: 1,
            characters: vec![Character {
                id: CharacterId(5),
                name: "Ada".into(),
                canonical_description: "".into(),
                reference_asset_ids: vec![],
                style_profile_id: Some(StyleProfileId(7)),
                approved_design_version: None,
                continuity_rules: vec![],
                unknown: unknown(),
            }],
            unknown: unknown(),
        };
        let locations = LocationsFile {
            schema_version: 1,
            locations: vec![Location {
                id: LocationId(6),
                name: "Rooftop".into(),
                description: "".into(),
                reference_asset_ids: vec![],
                unknown: unknown(),
            }],
            unknown: unknown(),
        };
        let styles = StylesFile {
            schema_version: 1,
            styles: vec![StyleProfile {
                id: StyleProfileId(7),
                name: "Ink".into(),
                description: "".into(),
                palette: vec!["#000000".into()],
                line_weight: None,
                rendering_notes: "".into(),
                unknown: unknown(),
            }],
            unknown: unknown(),
        };
        let panels = vec![
            Panel {
                id: PanelId(9),
                page_id: PageId(8),
                artboard_id: 1,
                scene_id: SceneId(4),
                frame_node_id: NodeId(2),
                artwork_refs: vec![ArtworkRef { asset_id: AssetId(17), node_id: NodeId(13), role: ArtworkRole::Primary }],
                shot_description: "wide".into(),
                dialogue_ids: vec![DialogueId(12)],
                lettering_ids: vec![LetteringId(14)],
                reading_order_index: 0,
                continuity_constraints: ContinuityConstraints {
                    characters: vec![CharacterId(5)],
                    wardrobe: None,
                    time_of_day: None,
                    callbacks: vec![],
                    exceptions: vec![],
                },
                generation_status: GenerationStatus::None,
                unknown: unknown(),
            },
            Panel {
                id: PanelId(10),
                page_id: PageId(8),
                artboard_id: 1,
                scene_id: SceneId(4),
                frame_node_id: NodeId(3),
                artwork_refs: vec![],
                shot_description: "close".into(),
                dialogue_ids: vec![DialogueId(13)],
                lettering_ids: vec![LetteringId(15)],
                reading_order_index: 1,
                continuity_constraints: ContinuityConstraints {
                    characters: vec![],
                    wardrobe: None,
                    time_of_day: None,
                    callbacks: vec![],
                    exceptions: vec![],
                },
                generation_status: GenerationStatus::Completed,
                unknown: unknown(),
            },
            Panel {
                id: PanelId(11),
                page_id: PageId(8),
                artboard_id: 1,
                scene_id: SceneId(4),
                frame_node_id: NodeId(4),
                artwork_refs: vec![],
                shot_description: "insert".into(),
                dialogue_ids: vec![],
                lettering_ids: vec![LetteringId(16)],
                reading_order_index: 2,
                continuity_constraints: ContinuityConstraints {
                    characters: vec![],
                    wardrobe: None,
                    time_of_day: None,
                    callbacks: vec![],
                    exceptions: vec![],
                },
                generation_status: GenerationStatus::None,
                unknown: unknown(),
            },
        ];
        let dialogue = vec![
            Dialogue {
                id: DialogueId(12),
                panel_id: PanelId(9),
                speaker_character_id: Some(CharacterId(5)),
                text: "We open on the rooftop.".into(),
                language: Some("en".into()),
                reading_order_index: 0,
                approval_status: ApprovalStatus::Approved,
                unknown: unknown(),
            },
            Dialogue {
                id: DialogueId(13),
                panel_id: PanelId(10),
                speaker_character_id: None,
                text: "It is quieter up here.".into(),
                language: None,
                reading_order_index: 0,
                approval_status: ApprovalStatus::Pending,
                unknown: unknown(),
            },
        ];
        let layout = || TextLayoutSettings {
            font: Some("Comic Neue".into()),
            size: Some(12.0),
            leading: Some(14.4),
            tracking: None,
            align: Some(TextAlign::Center),
            padding: Some(4.0),
            fit_to_frame: true,
        };
        let lettering = vec![
            Lettering {
                id: LetteringId(14),
                page_id: PageId(8),
                panel_id: PanelId(9),
                artboard_id: 1,
                kind: LetteringKind::Speech,
                dialogue_id: Some(DialogueId(12)),
                text: None,
                frame_node_id: NodeId(5),
                tail_node_id: Some(NodeId(6)),
                text_node_id: NodeId(7),
                text_layout_settings: layout(),
                overflow_policy: OverflowPolicy::Shrink,
                unknown: unknown(),
            },
            Lettering {
                id: LetteringId(15),
                page_id: PageId(8),
                panel_id: PanelId(10),
                artboard_id: 1,
                kind: LetteringKind::Speech,
                dialogue_id: Some(DialogueId(13)),
                text: None,
                frame_node_id: NodeId(8),
                tail_node_id: Some(NodeId(9)),
                text_node_id: NodeId(10),
                text_layout_settings: layout(),
                overflow_policy: OverflowPolicy::Shrink,
                unknown: unknown(),
            },
            Lettering {
                id: LetteringId(16),
                page_id: PageId(8),
                panel_id: PanelId(11),
                artboard_id: 1,
                kind: LetteringKind::Narration,
                dialogue_id: None,
                text: Some("Three days earlier.".into()),
                frame_node_id: NodeId(11),
                tail_node_id: None,
                text_node_id: NodeId(12),
                text_layout_settings: layout(),
                overflow_policy: OverflowPolicy::Warn,
                unknown: unknown(),
            },
        ];
        let page = Page {
            id: PageId(8),
            chapter_id: ChapterId(3),
            scene_ids: vec![SceneId(4)],
            artboard_id: 1,
            page_number: 1,
            panel_ids: vec![PanelId(9), PanelId(10), PanelId(11)],
            reading_order: vec![PanelId(9), PanelId(10), PanelId(11)],
            status: PageStatus::Draft,
            unknown: unknown(),
        };
        let mut pages = BTreeMap::new();
        pages.insert(PageId(8), PageFile { schema_version: 1, page, panels, dialogue, lettering, unknown: unknown() });

        ComicBundle {
            manifest: Manifest {
                schema_version: 1,
                bundle_id: ComicId(20),
                comic_id: ComicId(1),
                title: "Minimal".into(),
                next_comic_id: 21,
                retired_artboard_ids: vec![],
                document: VisualDocumentRef { relative_path: "document/comic.vectorcraft".into(), format_version: 4, unknown: unknown() },
                files: BTreeMap::new(),
                created: "2026-10-11T00:00:00Z".into(),
                updated: "2026-10-11T00:00:00Z".into(),
                app: AppInfo { name: "ComicCraft".into(), version: "0.8.0".into(), schema: 1, unknown: unknown() },
                unknown: unknown(),
            },
            project: ComicProject {
                id: ComicId(1),
                schema_version: 1,
                title: "Minimal".into(),
                metadata: ProjectMetadata {
                    author: Some("fixture".into()),
                    language: Some("en".into()),
                    genre: None,
                    reading_direction: ReadingDirection::Ltr,
                    created: "2026-10-11T00:00:00Z".into(),
                    updated: "2026-10-11T00:00:00Z".into(),
                    unknown: unknown(),
                },
                chapter_ids: vec![ChapterId(3)],
                character_ids: vec![CharacterId(5)],
                location_ids: vec![LocationId(6)],
                style_profile_ids: vec![StyleProfileId(7)],
                asset_registry_id: ComicId(2),
                settings: project_settings(),
                visual_document: VisualDocumentRef { relative_path: "document/comic.vectorcraft".into(), format_version: 4, unknown: unknown() },
                unknown: unknown(),
            },
            chapters,
            scenes,
            characters,
            locations,
            styles,
            pages,
            assets: AssetIndexFile { schema_version: 1, assets: vec![], unknown: unknown() },
            provenance: ProvenanceFile { schema_version: 1, jobs: vec![], decisions: vec![], unknown: unknown() },
            jobs: BTreeMap::new(),
            files_of_record: BTreeSet::new(),
        }
    }

    /// A valid bundle plus its document file, saved, with the asset the panels reference.
    fn fixture(name: &str) -> (std::path::PathBuf, ComicBundle) {
        let root = crate::scratch::scratch_dir_created(name);
        std::fs::create_dir_all(root.join("document")).expect("dir");
        std::fs::create_dir_all(root.join("assets/panels")).expect("dir");
        std::fs::write(root.join("document/comic.vectorcraft"), b"document").expect("document");
        let png = b"\x89PNG\r\n\x1a\n fixture bytes";
        std::fs::write(root.join("assets/panels/panel-1.png"), png).expect("asset");
        let mut bundle = bundle();
        bundle.assets.assets.push(Asset {
            id: AssetId(17),
            asset_type: AssetType::Source,
            storage_reference: StorageRef::Linked { relative_path: "assets/panels/panel-1.png".into() },
            content_hash: ContentHash { algorithm: "sha256".into(), value: sha256_hex(png) },
            media_metadata: MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: None },
            provenance: None,
            parent_asset_ids: vec![],
            approval_status: ApprovalStatus::Approved,
            unknown: unknown(),
        });
        (root, bundle)
    }

    #[test]
    fn the_page_geometry_report_names_every_artboard_and_skips_a_retired_one() {
        let (root, mut bundle) = fixture("validate-geometry");
        let document = document();
        let report = page_geometry(&bundle, &document);
        assert_eq!(report.len(), 1);
        assert_eq!((report[0].page_id.0, report[0].artboard_id), (8, 1));
        assert_eq!((report[0].rect.width(), report[0].rect.height()), (612.0, 792.0), "the artboard *is* the page's size");

        bundle.manifest.retire_artboard_id(1);
        assert!(page_geometry(&bundle, &document).is_empty(), "a retired artboard is a rule-4 finding, not information");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_page_is_never_offered_a_retired_artboard() {
        // Design §5.3.1 mitigation 4: `comic.page.create` must allocate until it gets an id that
        // is not retired, or deleting a page would permanently break the next page to reuse that
        // number (rules 4 and 17 would leave it unfixable).
        let (root, mut bundle) = fixture("validate-assignable");
        let document = document();
        assert_eq!(bundle.assignable_artboards(&document), vec![1]);

        bundle.manifest.retire_artboard_id(1);
        assert_eq!(bundle.assignable_artboards(&document), Vec::<u32>::new(), "the only artboard is retired");

        let mut wider = document.clone();
        wider.artboards.push(ArtboardFacts { id: 2, rect: vectorcraft_geom::Rect::new(0.0, 0.0, 612.0, 792.0) });
        assert_eq!(bundle.assignable_artboards(&wider), vec![2], "the retired number is skipped, not reused");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_clean_bundle_has_no_findings() {
        let (root, bundle) = fixture("validate-clean");
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.is_empty(), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_reference_that_does_not_resolve_is_reported_but_nothing_is_repaired() {
        let (root, mut bundle) = fixture("validate-dangling");
        bundle.pages.get_mut(&PageId(8)).expect("page").panels[0].scene_id = SceneId(99);
        bundle.project.chapter_ids = vec![ChapterId(77)];
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject.contains("scene_id")), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject.contains("chapter_ids")), "{findings:?}");
        assert_eq!(bundle.project.chapter_ids, vec![ChapterId(77)], "a rule reports, it never repairs");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_1_checks_the_kind_of_the_entity_a_reference_names() {
        // A reference must name the **right kind** of entity, not merely an entity that happens to
        // have that id. `Scene.page_ids` and `Lettering.page_id` are guarded by rule 1 alone — no
        // other rule cross-checks them — so a single "everything under `pages/`" id set would let a
        // panel stand in for a page and nothing at all would be reported.
        let (root, mut bundle) = fixture("validate-rule-1-kinds");
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };

        // 9 is a Panel, 14 a Lettering, 12 a Dialogue: none of them is a Page.
        bundle.scenes.scenes[0].page_ids = vec![PageId(9)];
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject == "Scene(4).page_ids"), "a panel id is not a page id: {findings:?}");

        bundle.scenes.scenes[0].page_ids = vec![PageId(8)];
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.lettering[0].page_id = PageId(14);
            file.lettering[1].page_id = PageId(12);
        }
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject == "Lettering(14).page_id"), "a lettering id is not a page id: {findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject == "Lettering(15).page_id"), "a dialogue id is not a page id: {findings:?}");

        // And the kinds that were already separate stay separate: a page id is not a panel id.
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.lettering[0].page_id = PageId(8);
            file.lettering[1].page_id = PageId(8);
            file.panels[0].dialogue_ids = vec![DialogueId(8)];
        }
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(1) && f.subject == "Panel(9).dialogue_ids"), "a page id is not a dialogue id: {findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_2_reports_a_duplicate_within_a_kind_and_across_kinds() {
        let (root, mut bundle) = fixture("validate-duplicates");
        let mut second = bundle.chapters.chapters[0].clone();
        second.scene_ids.clear();
        bundle.chapters.chapters.push(second);
        bundle.pages.get_mut(&PageId(8)).expect("page").panels[0].id = PanelId(3);
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(2) && f.subject.contains("Chapter")), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(2) && f.message.contains("unique across kinds")), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_manifest_comic_id_is_a_copy_not_a_second_entity() {
        // §4 duplicates `comic_id` on purpose; rule 2 must not call every legal bundle a
        // duplicate.
        let (root, bundle) = fixture("validate-comic-id-copy");
        assert_eq!(bundle.manifest.comic_id, bundle.project.id);
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        assert!(!validate(&bundle, &input).iter().any(|f| f.rule == Some(2)), "a copy is not a duplicate");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_3_reports_a_version_or_identity_that_disagrees_with_the_manifest() {
        let (root, mut bundle) = fixture("validate-versions");
        bundle.chapters.schema_version = 2;
        bundle.manifest.comic_id = ComicId(999);
        bundle.project.visual_document.format_version = 3;
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(3) && f.message.contains("declares `schema_version` 2")), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(3) && f.message.contains("`comic_id` is 999")), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(3) && f.message.contains("visual_document")), "{findings:?}");

        // The subject is the file's **real** path, not its template: a creator told that
        // "`pages/page-<PageId>.json` declares 2" cannot tell which file to fix, and two files of
        // one kind must produce two differently-subjected findings.
        assert!(findings.iter().any(|f| f.rule == Some(3) && f.subject == "story/chapters.json"), "{findings:?}");
        assert!(!findings.iter().any(|f| f.subject.contains('<')), "no template survives as a subject: {findings:?}");

        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.schema_version = 2;
        }
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(3) && f.subject == "pages/page-8.json"), "{findings:?}");
        assert!(!findings.iter().any(|f| f.subject.contains('<')), "no template survives as a subject: {findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_4_reports_a_missing_artboard_and_a_retired_one() {
        let (root, mut bundle) = fixture("validate-artboards");
        bundle.manifest.retired_artboard_ids = vec![1];
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(
            findings.iter().any(|f| f.rule == Some(4) && f.message.contains("retired_artboard_ids")),
            "a page on a retired artboard is broken, not trusted: {findings:?}"
        );
        bundle.manifest.retired_artboard_ids.clear();
        bundle.pages.get_mut(&PageId(8)).expect("page").page.artboard_id = 42;
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(4) && f.message.contains("does not exist")), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_5_and_rule_11_report_denormalised_disagreement() {
        let (root, mut bundle) = fixture("validate-denormalised");
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.panels[0].artboard_id = 7;
            file.lettering[0].artboard_id = 7;
            file.page.panel_ids.pop();
        }
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().filter(|f| f.rule == Some(5)).count() == 2, "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(11)), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_6_reports_a_reading_order_that_is_not_a_permutation_and_an_index_that_disagrees() {
        let (root, mut bundle) = fixture("validate-reading-order");
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.page.reading_order = vec![PanelId(11), PanelId(9), PanelId(10)];
        }
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(6) && f.subject.contains("reading_order_index")), "{findings:?}");

        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.page.reading_order.pop();
        }
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(6) && f.message.contains("not a permutation")), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_7_reports_a_visual_reference_that_is_not_in_the_document() {
        let (root, mut bundle) = fixture("validate-nodes");
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.panels[0].frame_node_id = NodeId(404);
            file.lettering[2].tail_node_id = Some(NodeId(405));
            file.panels[0].artwork_refs[0].node_id = NodeId(406);
        }
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert_eq!(findings.iter().filter(|f| f.rule == Some(7)).count(), 3, "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_8_holds_dialogue_id_and_text_to_their_kinds() {
        let (root, mut bundle) = fixture("validate-lettering");
        {
            let file = bundle.pages.get_mut(&PageId(8)).expect("page");
            file.lettering[0].dialogue_id = None;
            file.lettering[0].text = Some("both".into());
            file.lettering[2].text = None;
            file.lettering[2].dialogue_id = Some(DialogueId(12));
        }
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert_eq!(findings.iter().filter(|f| f.rule == Some(8)).count(), 4, "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_9_reports_a_hash_mismatch_a_missing_file_and_an_escaping_path() {
        let (root, mut bundle) = fixture("validate-paths");
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let asset = bundle.assets.assets[0].clone();

        bundle.assets.assets[0].content_hash.value = "00".repeat(32);
        assert!(validate(&bundle, &input).iter().any(|f| f.rule == Some(9) && f.message.contains("hashes")), "a changed file is reported");

        bundle.assets.assets[0] = asset.clone();
        bundle.assets.assets[0].storage_reference = StorageRef::Linked { relative_path: "assets/panels/nope.png".into() };
        assert!(validate(&bundle, &input).iter().any(|f| f.rule == Some(9) && f.message.contains("is not there")), "a missing file is reported");

        bundle.assets.assets[0] = asset.clone();
        bundle.assets.assets[0].storage_reference = StorageRef::Linked { relative_path: "../../etc/passwd".into() };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(9) && f.message.contains("escapes")), "{findings:?}");

        bundle.assets.assets[0] = asset.clone();
        bundle.assets.assets[0].storage_reference = StorageRef::Linked { relative_path: "/etc/passwd".into() };
        assert!(validate(&bundle, &input).iter().any(|f| f.rule == Some(9) && f.message.contains("escapes")), "an absolute path escapes its base");

        bundle.assets.assets[0] = asset;
        bundle.assets.assets[0].storage_reference = StorageRef::Package { relative_path: "a.png".into() };
        assert!(
            validate(&bundle, &input).iter().any(|f| f.rule == Some(9) && f.message.contains("archive root")),
            "a Package reference cannot resolve against the bundle root"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_10_requires_provenance_exactly_where_it_belongs() {
        let (root, mut bundle) = fixture("validate-provenance");
        bundle.assets.assets[0].asset_type = AssetType::Generated;
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        assert!(validate(&bundle, &input).iter().any(|f| f.rule == Some(10) && f.message.contains("must record")), "generated without provenance");

        bundle.assets.assets[0].asset_type = AssetType::Source;
        bundle.assets.assets[0].provenance = Some(Provenance {
            provider: ProviderId("p".into()),
            model: "m".into(),
            request: serde_json::json!({"a": 1}),
            input_asset_ids: vec![],
            created: "2026-10-11T00:00:00Z".into(),
            consent: None,
            unknown: unknown(),
        });
        assert!(validate(&bundle, &input).iter().any(|f| f.rule == Some(10) && f.message.contains("must not claim")), "source with provenance");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_12_lets_narration_be_null_and_checks_only_the_set_values() {
        let (root, mut bundle) = fixture("validate-speakers");
        bundle.pages.get_mut(&PageId(8)).expect("page").dialogue[0].speaker_character_id = Some(CharacterId(404));
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert_eq!(findings.iter().filter(|f| f.rule == Some(12)).count(), 1, "the null speaker in panel 10 is legal: {findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_13_compares_the_parsed_number_and_rule_14_warns_about_an_orphan() {
        let (root, mut bundle) = fixture("validate-page-names");
        let file = bundle.pages.remove(&PageId(8)).expect("page");
        // `page-007.json` and `page-7.json` both mean page 7 (§3).
        bundle.pages.insert(PageId(7), file);
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(13) && f.subject == "pages/page-7.json"), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(14) && f.severity == Severity::Warning), "an orphan page is a warning: {findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_15_warns_when_a_page_artboard_differs_from_the_default() {
        let (root, mut bundle) = fixture("validate-page-size");
        bundle.project.settings.default_page_size = Some(PageSize::new(612.0, 1000.0));
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(15) && f.severity == Severity::Warning), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_16_and_rule_17_report_an_unknown_algorithm_and_a_repeated_retirement() {
        let (root, mut bundle) = fixture("validate-hash-retired");
        bundle.assets.assets[0].content_hash.algorithm = "blake3".into();
        bundle.manifest.retired_artboard_ids = vec![1, 1];
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(16)), "{findings:?}");
        assert!(findings.iter().any(|f| f.rule == Some(17)), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rule_18_reports_a_terminal_job_file_and_accepts_awaiting_review() {
        let (root, mut bundle) = fixture("validate-jobs");
        let job = GenerationJob {
            id: JobId(18),
            operation: JobOperation::TextToImage,
            target_panel_id: Some(PanelId(10)),
            input_asset_ids: vec![AssetId(17)],
            provider: ProviderId("local".into()),
            model: "example".into(),
            request: serde_json::json!({"prompt": "rooftop"}),
            status: JobStatus::AwaitingReview,
            progress: None,
            error: None,
            result_asset_ids: vec![],
            created: "2026-10-11T00:00:00Z".into(),
            updated: "2026-10-11T00:00:00Z".into(),
            target_revision: 1,
            consent: None,
            unknown: unknown(),
        };
        bundle.provenance.jobs.push(job.clone());
        bundle.jobs.insert(JobId(18), JobFile { schema_version: 1, job, unknown: unknown() });
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        assert!(!validate(&bundle, &input).iter().any(|f| f.rule == Some(18)), "awaiting_review is not terminal: its file legitimately stays (§8.3)");

        if let Some(file) = bundle.jobs.get_mut(&JobId(18)) {
            file.job.status = JobStatus::Failed;
        }
        let findings = validate(&bundle, &input);
        assert!(findings.iter().any(|f| f.rule == Some(18)), "{findings:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_validation_of_a_saved_bundle_matches_the_one_of_the_model() {
        let (root, mut bundle) = fixture("validate-round-trip");
        let options = SaveOptions { now: "2026-10-11T06:00:00Z".into(), document: None };
        save(&mut bundle, &root, &options).expect("saves");
        let loaded = crate::load(&root).expect("loads");
        let document = document();
        let input = ValidationInput { root: &root, document: &document, archive_root: None };
        let findings = validate(&loaded.bundle, &input);
        assert!(findings.is_empty(), "{findings:?}");
        assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
        std::fs::remove_dir_all(&root).ok();
    }
}
