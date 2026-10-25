//! The `comic.*` command surface: the ComicCraft project, its pages, panels and lettering
//! (design §10.1, §10.2, §11.1, §11.2; schema `docs/comiccraft-project-schema.md` v1).
//!
//! Every command here is **additive**. No existing command changes meaning; a comic command may
//! *call* one (it does, for placing images), and the registry gains exactly one line. That is what
//! keeps upstream merges cheap.
//!
//! # Where the project lives
//!
//! An open project is [`OpenComic`]: the bundle directory, the loaded [`ComicBundle`] and the
//! findings the last load or validation produced. It is held on the `DocState` of the one visual
//! document the bundle names (ADR-011: one document per project, artboards are pages). So closing
//! the document closes the project, and the enablement predicate below — which gates every command
//! here — answers `no comic project open` rather than `no document open` when a plain document is
//! open. `find_command` and agent introspection therefore stay honest: a `comic.*` id is reported
//! disabled, with a reason, in any session that has no project.
//!
//! # Undo is one logical step (design §11.2)
//!
//! History is per-`DocState` and snapshot-based (ADR-011). A comic action usually changes two
//! things at once — the visual document (a frame object, a balloon outline) and the comic records
//! (a `Panel`, a `Lettering`) — and design §11.2 requires that be *one* undo entry, because
//! otherwise redo would put the document back without the record and the two would drift. So every
//! mutating command runs through [`comic_edit`], which snapshots document, selection and comic
//! model, lets the command edit whichever of them it needs, and then records **one** history entry
//! whose `comic` field is the comic model *before* the step. `edit.undo` / `edit.redo` swap that
//! field along with the document, so a step that changed only comic records (setting a shot
//! description, renumbering the reading order) is undone exactly like one that changed geometry.
//! The model is shared behind an `Arc`, so a snapshot costs a pointer, not a copy.
//!
//! # `comic.project.validate`, and what `Severity::Error` means
//!
//! `comic.project.validate` **checks and reports; it never repairs** (schema §13, design §12.1).
//! It runs `comic-model`'s eighteen referential rules, `comic-layout`'s checks over the loaded
//! pages, the artboard `rect` behind every page as information, and — in `notChecked` — the things
//! it *cannot* check and who owns them.
//!
//! **`Severity::Error` does not block a save, and that is deliberate.** Three reasons, in order of
//! weight:
//!
//! 1. Section 14 of the schema exists so "a creator can open a damaged bundle and repair it rather
//!    than being locked out", and refuses only two things: a document that will not parse and a
//!    comic file that will not parse. A save refused on a referential `Error` re-introduces exactly
//!    that lockout — and worse, it makes repair impossible, because the commands that repair a
//!    broken reference *are* the comic commands and they save.
//! 2. `Severity::Error` is **data**, not an exception: `validate` returns `Vec<Finding>` and its
//!    caller decides. A command that turned a finding into a refusal would be reclassifying a
//!    report as a gate, and §12.1's "returns findings as structured data" would stop being true.
//! 3. A save is local. It publishes nothing and spends nothing, and §10 already tolerates an
//!    interrupted save window.
//!
//! The one write this module *does* refuse is `comic.project.save` when two files in `pages/`
//! encode the same page id. That is not "there is an error, so refuse" — it is "this write is
//! **ambiguous**": §3 makes the padding cosmetic and rule 13 parses the number, so `page-7.json`
//! and `page-007.json` are indistinguishable, and the save would write one generation while the
//! next load read whichever file sorts first. The *bytes* are not at risk: `comic-model`'s prune
//! removes only what the generation of record wrote and this save dropped, so a file written since
//! the load survives either way.
//!
//! # What reports a non-finite or degenerate artboard
//!
//! Both crates are deliberately silent about it, and the silence is *named* rather than assumed:
//! `comic-layout`'s `validate::page` skips a page whose artboard is not finite or has no area (its
//! module doc says so, and the skip is the same one a degenerate artboard gets), and `comic-model`'s
//! rule 15 compares the artboard `rect` with `ProjectSettings.default_page_size` — a comparison that
//! returns early when there is no default to compare against. So with `default_page_size: None`
//! nothing reported it at all. `comic.project.validate` now does, as a **warning**: for each page
//! whose artboard is absent, non-finite or of no area it emits one finding naming the rect, saying
//! that the layout geometry check was skipped, and saying which of the two other layers is silent
//! and why. Warning, not error, because rule 15 — the other home for a page-size fact — also warns:
//! a page that deliberately differs from the default is legal, and two severities for one class of
//! fact would make `validate` unpredictable. Where `default_page_size` **is** set, rule 15 already
//! reports the difference and this finding stays out of the way, so one fact is never reported twice.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_comic_model::{ComicBundle, DialogueId, Finding, LetteringId, Migration, PageId, PanelId, Severity, Timestamp};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::Rect;

use super::*;
use crate::{EngineError, HistoryEntry, Result, Session};

pub mod lettering;
pub mod page;
pub mod panel;
pub mod project;

#[cfg(test)]
mod tests;

/// The ComicCraft project open in one document.
///
/// Cheap to clone: `DocState::comic` holds it behind an `Arc`, so an undo snapshot is a pointer
/// copy, not a deep copy of the whole model.
#[derive(Clone, Debug)]
pub struct OpenComic {
    /// The bundle directory (§3).
    pub root: PathBuf,
    /// The loaded model.
    pub bundle: ComicBundle,
    /// What the last load or `comic.project.validate` reported. `Severity::Error` here never blocks
    /// a save — see the module doc.
    pub findings: Vec<Finding>,
    /// What §11's migrations did when this project was **loaded**, in order — empty for a bundle
    /// that already declares this build's `schema_version`.
    ///
    /// Held here rather than recomputed because it is a fact about *this session's load*: the model
    /// has no record of having been read forward, and `comic-model` reports it as a value rather
    /// than a §14 finding ("a migrated bundle is older, not inconsistent"). It is surfaced by
    /// `comic.project.open` (where the creator learns about it) and by `comic.project.validate`
    /// (while it is still true), and `comic.project.save` clears it, because a project that has
    /// been written in this build's format is no longer one whose next save will write a different
    /// one.
    pub migrations: Vec<Migration>,
}

impl OpenComic {
    /// A project opened from `root`.
    #[must_use]
    pub fn new(root: PathBuf, bundle: ComicBundle, findings: Vec<Finding>, migrations: Vec<Migration>) -> Self {
        Self { root, bundle, findings, migrations }
    }
}

// ---------- registration ----------

pub fn specs() -> Vec<CommandSpec> {
    let mut v = project::specs();
    v.extend(page::specs());
    v.extend(panel::specs());
    v.extend(lettering::specs());
    v
}

// ---------- enablement ----------

/// Every command here is gated on an open comic project (design §10.1).
pub fn has_comic(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if st.comic.is_some() { Ok(()) } else { Err("no comic project open (comic.project.create or comic.project.open)".into()) }
}

// ---------- the open project ----------

pub(crate) fn open(s: &Session) -> Result<&OpenComic> {
    s.doc()?.comic.as_deref().ok_or_else(|| EngineError::Disabled("comic.*".into(), "no comic project open".into()))
}

/// The open project, ready to be mutated. `Arc::make_mut` clones only while a history snapshot
/// still points at it.
pub(crate) fn open_mut(s: &mut Session) -> Result<&mut OpenComic> {
    let st = s.doc_mut()?;
    match st.comic.as_mut() {
        Some(c) => Ok(Arc::make_mut(c)),
        None => Err(EngineError::Disabled("comic.*".into(), "no comic project open".into())),
    }
}

/// The bundle directory of the open project.
pub(crate) fn root(s: &Session) -> Result<PathBuf> {
    Ok(open(s)?.root.clone())
}

// ---------- time ----------

/// The canonical §5.5 timestamp for now: `YYYY-MM-DDTHH:MM:SSZ`, the one form the writer accepts.
///
/// A platform without a clock (the web build) reports the Unix epoch rather than a broken form, so
/// a save that cannot know the time still writes a canonical one instead of being refused.
pub(crate) fn now() -> Timestamp {
    vectorcraft_doc::metadata::iso8601(vectorcraft_doc::metadata::now_unix().unwrap_or(0))
}

// ---------- the document, as the model may see it ----------

/// The artboards and node ids of the loaded visual document (schema §9). Built here because
/// `crates/format` is L3 and `comic-model` is L2, so the model never parses the document.
pub(crate) fn facts(doc: &Document) -> vectorcraft_comic_model::DocumentFacts {
    vectorcraft_comic_model::DocumentFacts::from_document(doc)
}

// ---------- one logical undo step ----------

/// Run `f` as **one** undo step covering both the visual document and the comic model (§11.2).
///
/// `f` may make as many document edits as it likes (each through `Session::edit`, each of which
/// would otherwise record its own step); this collapses them into one entry whose `comic` field is
/// the model *before* the step. On error everything — document, selection, model and history — goes
/// back to where it was, so a failed comic command leaves no half-applied change behind.
///
/// **The collapse cannot rely on the journal's length alone.** `push_undo` trims from the *front*
/// once an entry would exceed `history.limit`, so a command that edits at a full journal makes room
/// by evicting a step that was there before it — and a plain `truncate(undo_len)` afterwards then
/// has nothing left to remove, leaving the inner entry *and* the eviction: two undo steps for one
/// user action (§11.2), with an unrelated step gone. So the limit is lifted for the duration of the
/// closure, which makes every inner edit append, and the truncation below is exact. The limit is
/// restored before the single entry is pushed, so the eviction happens once, for this one step.
pub(crate) fn comic_edit<T>(s: &mut Session, label: &str, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let (before_doc, before_sel, before_comic, undo_len, limit) = {
        let st = s.doc()?;
        (st.doc.clone(), st.selection.clone(), st.comic.clone(), st.history.undo.len(), st.history.limit)
    };
    if let Some(st) = s.active_mut() {
        st.history.limit = usize::MAX;
    }
    match f(s) {
        Ok(v) => {
            let st = s.doc_mut()?;
            st.history.limit = limit;
            st.history.undo.truncate(undo_len);
            st.history.redo.clear();
            st.history.undo.push(HistoryEntry { label: label.to_string(), doc: before_doc, selection: before_sel, comic: before_comic });
            while st.history.undo.len() > st.history.limit {
                st.history.undo.remove(0);
            }
            st.revision += 1;
            Ok(v)
        }
        Err(e) => {
            if let Some(st) = s.active_mut() {
                st.history.limit = limit;
                st.doc = before_doc;
                st.selection = before_sel;
                st.comic = before_comic;
                st.history.undo.truncate(undo_len);
                st.revision += 1;
            }
            Err(e)
        }
    }
}

// ---------- model lookups ----------

pub(crate) fn page_file(b: &ComicBundle, page: PageId) -> Result<&vectorcraft_comic_model::PageFile> {
    b.pages.get(&page).ok_or_else(|| EngineError::Other(format!("no such page {}", page.0)))
}

pub(crate) fn page_file_mut(b: &mut ComicBundle, page: PageId) -> Result<&mut vectorcraft_comic_model::PageFile> {
    b.pages.get_mut(&page).ok_or_else(|| EngineError::Other(format!("no such page {}", page.0)))
}

/// The page a panel is in, and its position on that page.
pub(crate) fn locate_panel(b: &ComicBundle, panel: PanelId) -> Result<(PageId, usize)> {
    for (id, file) in &b.pages {
        if let Some(i) = file.panels.iter().position(|p| p.id == panel) {
            return Ok((*id, i));
        }
    }
    Err(EngineError::Other(format!("no such panel {}", panel.0)))
}

/// The page a lettering object is on, and its position on that page.
pub(crate) fn locate_lettering(b: &ComicBundle, lettering: LetteringId) -> Result<(PageId, usize)> {
    for (id, file) in &b.pages {
        if let Some(i) = file.lettering.iter().position(|l| l.id == lettering) {
            return Ok((*id, i));
        }
    }
    Err(EngineError::Other(format!("no such lettering {}", lettering.0)))
}

/// The page a dialogue entry is on, and its position on that page.
pub(crate) fn locate_dialogue(b: &ComicBundle, dialogue: DialogueId) -> Result<(PageId, usize)> {
    for (id, file) in &b.pages {
        if let Some(i) = file.dialogue.iter().position(|d| d.id == dialogue) {
            return Ok((*id, i));
        }
    }
    Err(EngineError::Other(format!("no such dialogue {}", dialogue.0)))
}

/// The artboard `rect` behind a page, which **is** the page's size (schema §6.1).
pub(crate) fn page_rect(s: &Session, page: PageId) -> Result<Rect> {
    let artboard = page_file(&open(s)?.bundle, page)?.page.artboard_id;
    s.doc()?
        .doc
        .artboards
        .iter()
        .find(|a| a.id == artboard)
        .map(|a| a.rect)
        .ok_or_else(|| EngineError::Other(format!("page {} names artboard {artboard}, which the document does not have", page.0)))
}

/// A page rectangle a layout can be built in, in points.
pub(crate) fn checked_page_rect(page: PageId, r: Rect) -> Result<Rect> {
    if !(r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()) {
        return Err(EngineError::Other(format!("page {}: the artboard rectangle is not finite ({r:?})", page.0)));
    }
    if !(r.width() > 0.0 && r.height() > 0.0) {
        return Err(EngineError::Other(format!("page {}: the artboard rectangle has no area ({r:?})", page.0)));
    }
    Ok(r)
}

/// A panel frame rectangle, refusing a non-finite, non-positive or unrepresentable one.
/// `comic-layout`'s `PanelRect` states and enforces that invariant, so it is reused rather than
/// repeated here.
pub(crate) fn checked_frame(cmd: &str, r: Rect) -> Result<vectorcraft_comic_layout::PanelRect> {
    vectorcraft_comic_layout::PanelRect::new(r.x0, r.y0, r.width(), r.height()).map_err(|e| bad(cmd, format!("{e}")))
}

// ---------- reading order (schema rule 6) ----------

/// Refuse a page whose `reading_order` is not a permutation of its `panel_ids`.
///
/// **A gap is a validation error, never a silent repair** (§11.1). The commands that renumber the
/// order (delete, reorder, `comic.reading_order.set`) run this first and refuse, so a hand-edited
/// page with a missing or duplicated panel is reported rather than quietly fixed by a comic
/// operation that was asked to do something else.
pub(crate) fn require_permutation(page: &vectorcraft_comic_model::Page) -> Result<()> {
    let mut members = page.panel_ids.clone();
    let mut order = page.reading_order.clone();
    members.sort_by_key(|p| p.0);
    order.sort_by_key(|p| p.0);
    if members != order {
        return Err(EngineError::Other(format!(
            "page {}: reading_order is not a permutation of panel_ids ({} panels, {} ordered) — schema rule 6, reported rather than repaired",
            page.id.0,
            page.panel_ids.len(),
            page.reading_order.len()
        )));
    }
    Ok(())
}

/// Make `order` the page's reading order and put every panel's `reading_order_index` in step
/// (schema rule 6). The caller has already established that `order` is a permutation of
/// `page.panel_ids`.
pub(crate) fn apply_reading_order(file: &mut vectorcraft_comic_model::PageFile, order: Vec<PanelId>) {
    file.page.reading_order = order.clone();
    let positions: BTreeMap<u64, u16> = order.iter().enumerate().map(|(i, id)| (id.0, u16::try_from(i).unwrap_or(u16::MAX))).collect();
    for panel in &mut file.panels {
        if let Some(i) = positions.get(&panel.id.0) {
            panel.reading_order_index = *i;
        }
    }
}

/// The page's reading order with one panel taken out, keeping every other panel's relative order.
pub(crate) fn order_without(file: &vectorcraft_comic_model::PageFile, removed: PanelId) -> Vec<PanelId> {
    file.page.reading_order.iter().copied().filter(|id| *id != removed).collect()
}

// ---------- document helpers ----------

/// Remove objects from the document, ignoring ones already gone.
pub(crate) fn drop_nodes(d: &mut Document, ids: &[NodeId]) {
    for id in ids {
        let _ = d.remove(*id);
    }
}

/// A node's frame rectangle, as a checked `PanelRect`. `None` when the node has no geometry a
/// panel can be checked against.
pub(crate) fn node_frame(d: &Document, id: NodeId) -> Option<vectorcraft_comic_layout::PanelRect> {
    let b = d.node(id)?.geometric_bounds()?;
    vectorcraft_comic_layout::PanelRect::new(b.x0, b.y0, b.width(), b.height()).ok()
}

/// The document's file inside the bundle, from its bundle-relative path.
pub(crate) fn document_path(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

// ---------- findings as data ----------

pub(crate) fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

pub(crate) fn finding_json(f: &Finding) -> Value {
    json!({ "rule": f.rule, "severity": severity_name(f.severity), "subject": f.subject, "message": f.message })
}

pub(crate) fn findings_json(fs: &[Finding]) -> Value {
    Value::Array(fs.iter().map(finding_json).collect())
}

/// What §11's migrations did, as JSON (W1's hand-off: `comic-model` reports them as a **value**, so
/// a command has to carry them to the caller or nobody reads them).
///
/// The shape is the model's own record plus the sentence it already knows how to write: `from`/`to`
/// is the version pair, `notes` is one actionable line per observable effect, and `summary` is
/// [`Migration`]'s `Display`. It is deliberately **not** a `Finding`: a project read forward is
/// older, not inconsistent, and filing it with the damage would make `comic.project.validate`'s
/// error count depend on the age of the file rather than on its contents.
pub(crate) fn migration_json(m: &Migration) -> Value {
    json!({ "from": m.from, "to": m.to, "notes": m.notes, "summary": m.to_string() })
}

pub(crate) fn migrations_json(ms: &[Migration]) -> Value {
    Value::Array(ms.iter().map(migration_json).collect())
}

/// The sentence that goes beside [`migrations_json`], or `null` when nothing was migrated — the
/// house style for a note that exists to explain a non-empty list.
pub(crate) fn migrations_note(ms: &[Migration]) -> Value {
    if ms.is_empty() {
        Value::Null
    } else {
        json!(format!(
            "the bundle declared an older schema_version and was read forward to {} (§2, §11); a migrated bundle is older, not inconsistent, so this is not a `finding`. Saving writes schema version {}",
            vectorcraft_comic_model::SCHEMA_VERSION,
            vectorcraft_comic_model::SCHEMA_VERSION
        ))
    }
}

/// The order a page's panels are read in, as bare ids.
pub(crate) fn reading_order_ids(file: &vectorcraft_comic_model::PageFile) -> Vec<u64> {
    file.page.reading_order.iter().map(|p| p.0).collect()
}
