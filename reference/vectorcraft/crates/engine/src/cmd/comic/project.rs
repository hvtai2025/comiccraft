//! `comic.project.*`: create, open, save and validate the project bundle.
//!
//! The bundle's wire format is frozen in `docs/comiccraft-project-schema.md` v1; these commands
//! write it through `comic-model`'s reader and writer and never touch a file in the bundle
//! themselves. The one step the model delegates — serializing the visual document through
//! `crates/format` and verifying it by loading it back (schema §10 steps 2–3) — is performed here,
//! because `crates/format` is L3, the model is L2 and this layer sits above both.
//!
//! **Create, open and save are not undo steps**, exactly as `document.save` and `file.new` are not:
//! they open or write a project, and history is per-document. The edits *inside* a project are undo
//! steps, one each (§11.2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value, json};
use vectorcraft_comic_layout::validate as layout_validate;
use vectorcraft_comic_layout::validate::{Limits, PanelFrames};
use vectorcraft_comic_model::{
    AppInfo, AssetIndexFile, ChaptersFile, CharactersFile, ComicBundle, ComicId, ComicProject, DocumentPayload, Finding, LoadedBundle, LocationsFile,
    Manifest, PageId, ProjectMetadata, ProjectSettings, ProvenanceFile, SaveOptions, ScenesFile, StylesFile, ValidationInput, VisualDocumentRef,
    load, save as save_bundle,
};
use vectorcraft_doc::Document;
use vectorcraft_geom::{Rect, Size};

use super::*;
use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "comic.project.create",
            "New Comic Project",
            ["Window", "Comic"],
            None,
            "{path: the bundle directory to create, title?, author?, language? (BCP-47), genre?, readingDirection?: \"ltr\" (default)|\"rtl\", pageSize?: {width, height} (points; also the project's default for new pages, default 612x792), chapter?: the first chapter's title, scene?: its first scene's title} create the bundle: project.json, the story files, assets/index.json, provenance/generations.json, document/comic.vectorcraft, and one chapter and scene so pages have somewhere to go (M1 has no story commands). Opens it → {root, comic_id, document, artboards, chapter, scene, files}. Not an undo step: it opens a project",
            always,
            create
        ),
        cmd!(
            "comic.project.open",
            "Open Comic Project",
            ["Window", "Comic"],
            None,
            "{path: the bundle directory} read the bundle and its visual document and open them as a new tab. A damaged bundle opens with its findings rather than being refused (schema §14) → {root, comic_id, title, pages, migrations: [{from, to, notes, summary}] what §11's migrations did to an **older** bundle, in order — empty for a current one — migrationsNote: the sentence that goes with a non-empty list (null when there is none), warnings: the document's own from `crates/format`, findings}. A migration is a value and not a `finding`: a bundle read forward is older, not inconsistent, and `comic.project.save` writes this build's schema version, which is why it retires the entry. Not an undo step",
            always,
            open
        ),
        cmd!(
            "comic.project.save",
            "Save Comic Project",
            ["Window", "Comic"],
            // Deliberately **no shortcut**: `Cmd+S` is already the app's Save, and the shortcut
            // lookup takes the first matching spec without consulting enablement, so claiming it
            // here would shadow Save in a session that has no comic project open.
            None,
            "{path? (default: the open bundle)} serialize the visual document through the native format and verify it by loading it back (schema §10 steps 2-3), then write the bundle in §10 order with manifest.json last → {root, files: [the bundle-relative paths written], document}. **Refuses** when two files under pages/ or jobs/ encode one id, because the number is parsed, not compared (§3, rule 13): the two are indistinguishable and the next load would read whichever sorts first — and the save would prune the one it does not stage, so the refusal is what keeps that file's bytes until the pair is repaired by hand. A Severity::Error finding does *not* block a save (see the module doc). A pages/ or jobs/ file that appeared after the last save is **not** deleted: the prune removes only what the generation of record wrote and this save replaces or drops, never what it never knew. Not an undo step",
            has_comic,
            save
        ),
        cmd!(
            query "comic.project.validate",
            "Validate Comic Project",
            [],
            None,
            "{} run comic-model's eighteen referential rules, comic-layout's checks over the loaded pages, and report the artboard rect behind every page as information. Never repairs → {findings: [{rule, severity, subject, message}], errors, warnings, geometry: [{page, artboard_id, rect}], notChecked: [{what, why}], migrations: [{from, to, notes, summary}] plus migrationsNote, severityErrorBlocksSave: false}. The migrations are the **load's** (see comic.project.open) and are not findings: a bundle read forward is older, not inconsistent. Geometry findings name their panel as `Panel(<id>)` — the spelling `comic-layout`'s own page check uses — so a caller deduplicating on `(rule, subject)` sees one fault once",
            has_comic,
            validate
        ),
        cmd!(
            query "comic.project.settings.get",
            "Get Project Settings",
            ["Window", "Comic"],
            None,
            "{} read the project's defaults (schema §5.3) → {default_page_size: {width, height} or null, default_template or null, default_lettering_style or null}. `default_lettering_style` is the face the project letters its text in; `comic.project.settings.set` is how it is chosen. A query: it changes nothing and is not an undo step",
            has_comic,
            settings_get
        ),
        cmd!(
            "comic.project.settings.set",
            "Set Project Settings",
            ["Window", "Comic"],
            None,
            "{defaultLetteringStyle?: a face name (a string) to letter new balloons in, or null to clear it, defaultPageSize?: {width, height} or [width, height] in points, or null to clear it} set the project's defaults (schema §5.3). An omitted key is left alone; null clears it. `default_lettering_style` is read by the lettering commands — the family half of every kind's default face (design §8.3) — so a named face reaches the plan, the record's `text_layout_settings.font` and the missing-face report, and a project that names none keeps ADR-017's faces; `default_page_size` is what comic.page.create uses with no `size`, and what comic-model's rule 15 compares a page's artboard against. `default_template` is **deliberately not settable here**: nothing reads it (recorded as M2-W3-F2 in docs/reviews/M2-command-engineer-locking.md), and a setting no command reads is worse than one that is absent. → {default_page_size, default_template, default_lettering_style} after the change. One undo step",
            has_comic,
            settings_set
        ),
    ]
}

// ---------- parameters ----------

/// A page size from `{width, height}` or `[width, height]`.
pub(crate) fn page_size_param(p: &Value, key: &str, cmd: &str) -> Result<Option<Size>> {
    let Some(v) = p.get(key).filter(|v| !v.is_null()) else { return Ok(None) };
    let (w, h) = match v {
        Value::Object(o) => (o.get("width").and_then(Value::as_f64), o.get("height").and_then(Value::as_f64)),
        Value::Array(a) if a.len() == 2 => (a[0].as_f64(), a[1].as_f64()),
        _ => (None, None),
    };
    let (Some(w), Some(h)) = (w, h) else { return Err(bad(cmd, format!("`{key}` must be {{width, height}} or [width, height]"))) };
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(bad(cmd, format!("`{key}` must be a finite, positive size, not {w}x{h}")));
    }
    Ok(Some(Size::new(w, h)))
}

fn reading_direction(p: &Value, cmd: &str) -> Result<vectorcraft_comic_model::ReadingDirection> {
    match str_param(p, "readingDirection") {
        None | Some("ltr") => Ok(vectorcraft_comic_model::ReadingDirection::Ltr),
        Some("rtl") => Ok(vectorcraft_comic_model::ReadingDirection::Rtl),
        Some(other) => Err(bad(cmd, format!("`readingDirection` is \"ltr\" or \"rtl\", not {other:?}"))),
    }
}

// ---------- comic.project.create ----------

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.project.create";
    let path = str_param(p, "path").ok_or_else(|| bad(C, "missing `path` (the bundle directory to create)"))?;
    let root = PathBuf::from(path);
    if root.exists() {
        return Err(bad(C, format!("`{path}` already exists; open it with comic.project.open, or create the project elsewhere")));
    }
    let title = str_param(p, "title").unwrap_or("Untitled Comic").to_string();
    let size = page_size_param(p, "pageSize", C)?.unwrap_or(Size::new(612.0, 792.0));
    let direction = reading_direction(p, C)?;
    let stamp = now();

    // The counter (§9) issues each id once: 1 bundle, 2 comic/project, 3 registry, 4 chapter, 5 scene.
    let document_ref =
        VisualDocumentRef { relative_path: "document/comic.vectorcraft".into(), format_version: vectorcraft_format::VERSION, unknown: Map::new() };
    let manifest = Manifest {
        schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
        bundle_id: ComicId(1),
        comic_id: ComicId(2),
        title: title.clone(),
        next_comic_id: 6,
        retired_artboard_ids: Vec::new(),
        document: document_ref.clone(),
        files: BTreeMap::new(),
        created: stamp.clone(),
        updated: stamp.clone(),
        app: AppInfo {
            name: "ComicCraft".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            schema: vectorcraft_comic_model::SCHEMA_VERSION,
            unknown: Map::new(),
        },
        unknown: Map::new(),
    };
    let project = ComicProject {
        id: ComicId(2),
        schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
        title: title.clone(),
        metadata: ProjectMetadata {
            author: str_param(p, "author").map(str::to_string),
            language: str_param(p, "language").map(str::to_string),
            genre: str_param(p, "genre").map(str::to_string),
            reading_direction: direction,
            created: stamp.clone(),
            updated: stamp.clone(),
            unknown: Map::new(),
        },
        chapter_ids: vec![vectorcraft_comic_model::ChapterId(4)],
        character_ids: Vec::new(),
        location_ids: Vec::new(),
        style_profile_ids: Vec::new(),
        asset_registry_id: ComicId(3),
        settings: ProjectSettings {
            default_page_size: Some(size),
            default_template: None,
            // A *name*, not a font file (schema §5.3). No `StyleProfile` record exists yet, and
            // naming one that resolves to nothing would be a lie; `comic-layout`'s per-kind
            // lettering defaults already name the comic faces, so a project created here letters in
            // them without this field (ADR-017).
            default_lettering_style: None,
            unknown: Map::new(),
        },
        visual_document: document_ref,
        unknown: Map::new(),
    };
    let chapters = ChaptersFile {
        schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
        chapters: vec![vectorcraft_comic_model::Chapter {
            id: vectorcraft_comic_model::ChapterId(4),
            project_id: ComicId(2),
            title: str_param(p, "chapter").unwrap_or("Chapter 1").to_string(),
            scene_ids: vec![vectorcraft_comic_model::SceneId(5)],
            order_index: 0,
            unknown: Map::new(),
        }],
        unknown: Map::new(),
    };
    let scenes = ScenesFile {
        schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
        scenes: vec![vectorcraft_comic_model::Scene {
            id: vectorcraft_comic_model::SceneId(5),
            chapter_id: vectorcraft_comic_model::ChapterId(4),
            title: str_param(p, "scene").unwrap_or("Scene 1").to_string(),
            synopsis: String::new(),
            page_ids: Vec::new(),
            location_id: None,
            character_ids: Vec::new(),
            order_index: 0,
            continuity_notes: String::new(),
            unknown: Map::new(),
        }],
        unknown: Map::new(),
    };
    let mut bundle = ComicBundle {
        manifest,
        project,
        chapters,
        scenes,
        characters: CharactersFile { schema_version: vectorcraft_comic_model::SCHEMA_VERSION, characters: Vec::new(), unknown: Map::new() },
        locations: LocationsFile { schema_version: vectorcraft_comic_model::SCHEMA_VERSION, locations: Vec::new(), unknown: Map::new() },
        styles: StylesFile { schema_version: vectorcraft_comic_model::SCHEMA_VERSION, styles: Vec::new(), unknown: Map::new() },
        pages: BTreeMap::new(),
        assets: AssetIndexFile { schema_version: vectorcraft_comic_model::SCHEMA_VERSION, assets: Vec::new(), unknown: Map::new() },
        provenance: ProvenanceFile {
            schema_version: vectorcraft_comic_model::SCHEMA_VERSION,
            jobs: Vec::new(),
            decisions: Vec::new(),
            unknown: Map::new(),
        },
        jobs: BTreeMap::new(),
        // Nothing written yet: a bundle just created in memory has no generation of record, and
        // `write_bundle` sets the field to what the save it performs actually staged.
        files_of_record: BTreeSet::new(),
    };

    // A document needs an artboard, and `Document::new` gives it one at the project's page size.
    // The first `comic.page.create` claims it; see `page::create`.
    let mut doc = Document::new(size.width, size.height);
    doc.title = title;
    let relative = bundle.project.visual_document.relative_path.clone();
    let payload = serialize_and_verify(&doc, &root, &relative)?;
    let files = write_bundle(&mut bundle, &root, payload, &stamp)?;

    let document_file = document_path(&root, &relative).to_string_lossy().into_owned();
    s.add_document(doc, Some(document_file));
    {
        let st = s.doc_mut()?;
        st.comic = Some(Arc::new(OpenComic::new(root.clone(), bundle, Vec::new(), Vec::new())));
        // The bundle it just wrote is the saved state: a project nobody has edited since is not
        // unsaved work (`is_dirty` reads the model as well as the document).
        st.mark_comic_saved();
    }
    Ok(json!({
        "root": root.to_string_lossy(),
        "comic_id": 2,
        "document": relative,
        "artboards": 1,
        "chapter": 4,
        "scene": 5,
        "files": files,
    }))
}

// ---------- comic.project.open ----------

fn open(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.project.open";
    let path = str_param(p, "path").ok_or_else(|| bad(C, "missing `path` (the bundle directory)"))?;
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(bad(C, format!("`{path}` is not a bundle directory")));
    }
    let LoadedBundle { bundle, findings, migrations, .. } = load(&root).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    let document_file = document_path(&root, &bundle.project.visual_document.relative_path);
    let name = document_file.to_string_lossy().into_owned();
    let bytes = super::super::fileio::read_file(&name)?;
    let super::super::fileio::Loaded { doc, warnings, .. } = super::super::fileio::load(&name, &bytes)?;

    let pages = bundle.pages.len();
    let comic_id = bundle.project.id.0;
    let title = bundle.project.title.clone();
    let root_string = root.to_string_lossy().into_owned();
    // What the load did to an older bundle, carried to the caller (W1's hand-off): `comic-model`
    // reports a migration as a **value** rather than a §14 finding — a bundle read forward is older,
    // not inconsistent — so if no command shows it, nobody is told that opening this project has
    // already moved it forward.
    let migrations_value = migrations_json(&migrations);
    let migrations_note_value = migrations_note(&migrations);
    s.add_document(doc, Some(name));
    {
        let st = s.doc_mut()?;
        st.comic = Some(Arc::new(OpenComic::new(root, bundle, findings.clone(), migrations)));
        // What the bundle on disk holds is what this opened: nothing is unsaved yet.
        st.mark_comic_saved();
    }
    Ok(json!({
        "root": root_string,
        "comic_id": comic_id,
        "title": title,
        "pages": pages,
        "migrations": migrations_value,
        "migrationsNote": migrations_note_value,
        "warnings": warnings,
        "findings": findings_json(&findings),
    }))
}

// ---------- comic.project.save ----------

/// Serialize the document through the native format and verify it by loading it back (schema §10
/// steps 2–3). The verification is **delegated to `format`**, not reimplemented here: the model is
/// L2 and this crate does not parse a document either.
fn serialize_and_verify(doc: &Document, root: &Path, relative: &str) -> Result<DocumentPayload> {
    let bytes = super::super::fileio::encode(doc, "vectorcraft", &json!({}))?;
    let _ = super::super::fileio::load(&document_path(root, relative).to_string_lossy(), &bytes)?;
    Ok(DocumentPayload { relative_path: relative.to_string(), format_version: vectorcraft_format::VERSION, bytes })
}

fn write_bundle(bundle: &mut ComicBundle, root: &Path, payload: DocumentPayload, stamp: &str) -> Result<Vec<String>> {
    save_bundle(bundle, root, &SaveOptions { now: stamp.to_string(), document: Some(payload) })
        .map_err(|e| EngineError::Other(format!("{}: {e}", root.display())))?;
    Ok(bundle.manifest.files.keys().cloned().collect())
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let root = match str_param(p, "path") {
        Some(path) => PathBuf::from(path),
        None => super::root(s)?,
    };
    // **Refuse a lossy write.** Two `pages/` or `jobs/` files can encode one id (`page-7.json` and
    // `page-007.json`): §3 says the padding is cosmetic and rule 13 parses the number, so the two
    // are indistinguishable and which one the next load reads is decided by sort order, not by the
    // project — the save would confirm that and lose the intent.
    //
    // The refusal is also what protects the **loser's bytes**, and this is the half that is easy to
    // get wrong: a pair that was in the tree when the bundle was *loaded* is part of that
    // generation's record, so the manifest declares both files (`comic-model`'s `files_of_record`
    // is seeded from exactly that map) and the prune would delete the one this save does not stage
    // — silently, because the `files` it writes next would name the winner alone. Disable this
    // check and save a loaded pair, and the loser is gone. (A pair created *after* the load is the
    // other half: the new name is in no generation's record, so the prune leaves it; see
    // `prune_stale`'s doc. That half needs no refusal.) The message below names both files, because
    // the caller has to be able to find them.
    let duplicates = duplicate_files(&root)?;
    if !duplicates.is_empty() {
        return Err(EngineError::Other(format!(
            "not saving {}: two files encode one id and the next load would read whichever sorts first — {} (repair the bundle by hand, then save)",
            root.display(),
            duplicates.join("; ")
        )));
    }
    let (doc, relative) = {
        let oc = super::open(s)?;
        (s.doc()?.doc.clone(), oc.bundle.project.visual_document.relative_path.clone())
    };
    let payload = serialize_and_verify(&doc, &root, &relative)?;
    let stamp = now();
    // **The record of what was written follows the last write, not the last model state.** §11.2
    // makes an undo snapshot the whole `OpenComic`, so undoing an edit rewinds
    // `ComicBundle::files_of_record` with it — and a save would then prune against a record older
    // than what is on disk, leaving a `pages/page-<n>.json` the model no longer has (the manifest
    // describes it, so a reload adopts it back as a page: the undo silently fails to stick).
    // `saved_comic` is the model as `comic.project.save` last wrote it, and no undo touches *it*,
    // so the record is restored from there before the write. Seeding it is a no-op in the ordinary
    // case: between two saves the field is already the last write's.
    let record = s.doc()?.saved_comic.as_ref().map(|saved| saved.bundle.files_of_record.clone());
    let oc = open_mut(s)?;
    if let Some(record) = record {
        oc.bundle.files_of_record = record;
    }
    let files = write_bundle(&mut oc.bundle, &root, payload, &stamp)?;
    // The migration is spent. §11 requires a read-forward to be reported, and it was — at
    // `comic.project.open` — but this save has now written this build's `schema_version`, so a later
    // `comic.project.validate` still saying "read forward" would be asserting a fact about the file
    // that is no longer true. (That is M2-L3's shape: a message explaining a cause it has not
    // checked.) So the record of the load is retired by the write that makes it stale.
    oc.migrations.clear();
    {
        let st = s.doc_mut()?;
        st.mark_saved();
        // The model is written too, so the project stops counting as unsaved work (M1 W4).
        st.mark_comic_saved();
    }
    Ok(json!({
        "root": root.to_string_lossy(),
        "files": files,
        "document": relative,
    }))
}

/// The `pages/` and `jobs/` files that encode an id another file also encodes (schema §3, rule 13:
/// the number is parsed, not compared).
fn duplicate_files(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (dir, parse) in [
        ("pages", vectorcraft_comic_model::parse_page_file_name as fn(&str) -> Option<PageId>),
        ("jobs", (|name: &str| vectorcraft_comic_model::parse_job_file_name(name).map(|j| PageId(j.0))) as fn(&str) -> Option<PageId>),
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        let mut seen: BTreeMap<u64, String> = BTreeMap::new();
        let mut names: Vec<String> = entries.flatten().filter_map(|e| e.file_name().into_string().ok()).collect();
        names.sort();
        for name in names {
            let Some(id) = parse(&name) else { continue };
            match seen.get(&id.0) {
                Some(first) => out.push(format!("{dir}/{first} and {dir}/{name} both encode id {}", id.0)),
                None => {
                    seen.insert(id.0, name);
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

// ---------- comic.project.validate ----------

fn validate(s: &mut Session, _: &Value) -> Result<Value> {
    let (root, bundle) = {
        let oc = super::open(s)?;
        (oc.root.clone(), oc.bundle.clone())
    };
    let doc = s.doc()?.doc.clone();
    let facts = super::facts(&doc);

    let mut findings = vectorcraft_comic_model::validate(&bundle, &ValidationInput { root: &root, document: &facts, archive_root: None });
    let geometry = vectorcraft_comic_model::page_geometry(&bundle, &facts);

    // The layout half of design §12.1. Its rule-7 and rule-6 findings repeat `comic-model`'s, so
    // they are deduplicated on `(rule, subject)` — the schema's own dedup key, as
    // `comic-layout`'s `validate::page` documents.
    let mut frames = PanelFrames::new();
    for file in bundle.pages.values() {
        for panel in &file.panels {
            if let Some(rect) = node_frame(&doc, panel.frame_node_id) {
                frames.insert(panel.frame_node_id, rect);
            }
        }
    }
    let layout_findings =
        layout_validate::page(&layout_validate::PageInput { pages: &bundle.pages, document: &facts, frames: &frames, limits: &Limits::default() });
    let mut seen: BTreeSet<(Option<u8>, String)> = findings.iter().map(|f| (f.rule, f.subject.clone())).collect();
    for f in layout_findings {
        if seen.insert((f.rule, f.subject.clone())) {
            findings.push(f);
        }
    }

    // **The artboard neither crate reports.** `comic-layout`'s `validate::page` skips a page whose
    // artboard is not finite or has no area, and `comic-model`'s rule 15 returns early when the
    // project names no `default_page_size` — so with no default, nothing reported it. One finding
    // per such page, at the severity rule 15 uses (a page-size fact is a warning).
    let mut not_checked: Vec<Value> = vec![
        json!({"what": "the visual document's own parse", "why": "crates/format is L3 and this crate is L2: re-reading a document for verification is delegated to format, which performs it on every load and save (schema §14 step 2)"}),
        json!({"what": "StorageRef::Package paths", "why": "a packaged archive is a different container (design §7.5) and M1 opens none, so such a reference is reported unresolved rather than resolved against the wrong base (schema rule 9)"}),
        json!({"what": "the page area layout would compute", "why": "comic-layout checks a *plan* before it is applied; a page with no frame rectangles in the document has no geometry to check (schema rule 7 reports the missing frames)"}),
    ];
    if bundle.project.settings.default_page_size.is_none() {
        not_checked.push(json!({"what": "page size against a default", "why": "the project names no default_page_size, so comic-model's rule 15 has nothing to compare an artboard against by design"}));
    }
    for (id, file) in &bundle.pages {
        let artboard_id = file.page.artboard_id;
        match facts.artboard(artboard_id) {
            None => not_checked.push(json!({
                "what": format!("Page({}).artboard_id", id.0),
                "why": format!("artboard {artboard_id} is not in the document; comic-layout's page check skips the page, and rule 4 reports the reference")
            })),
            Some(a) => {
                let r = a.rect;
                let usable = r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite() && r.width() > 0.0 && r.height() > 0.0;
                if !usable {
                    let covered = bundle.project.settings.default_page_size.is_some();
                    findings.push(Finding::warning(
                        None,
                        format!("Page({}).artboard_id", id.0),
                        format!(
                            "artboard {artboard_id} is {}, so the layout geometry check is skipped for this page; {}",
                            describe_rect(r),
                            if covered {
                                "rule 15 reports it against default_page_size".to_string()
                            } else {
                                "comic-model's rule 15 cannot report it either, because the project names no default_page_size".to_string()
                            }
                        ),
                    ));
                    not_checked.push(json!({
                        "what": format!("Page({}).artboard_id", id.0),
                        "why": format!("the artboard rectangle {} is not a usable page, so comic-layout's containment and gutter checks are skipped for it", describe_rect(r))
                    }));
                }
            }
        }
    }

    findings.sort_by(|a, b| (a.rule.unwrap_or(0), &a.subject, &a.message).cmp(&(b.rule.unwrap_or(0), &b.subject, &b.message)));
    let errors = findings.iter().filter(|f| f.is_error()).count();
    let warnings = findings.len() - errors;
    // The load's migrations, as information beside the findings (M2 §7.5, W1's hand-off) rather than
    // *as* findings: a migrated bundle is older, not inconsistent, and counting it as an error would
    // make `errors` depend on the age of the file. `comic.project.save` clears them, because a save
    // writes this build's version.
    let migrations = super::open(s)?.migrations.clone();
    Ok(json!({
        "findings": findings_json(&findings),
        "errors": errors,
        "warnings": warnings,
        "geometry": geometry.iter().map(|g| json!({
            "page": g.page_id.0,
            "artboard_id": g.artboard_id,
            "rect": [g.rect.x0, g.rect.y0, g.rect.width(), g.rect.height()],
        })).collect::<Vec<_>>(),
        "notChecked": not_checked,
        "migrations": migrations_json(&migrations),
        "migrationsNote": migrations_note(&migrations),
        "severityErrorBlocksSave": false,
    }))
}

// ---------- comic.project.settings ----------

/// The project's defaults, as JSON (schema §5.3).
fn settings_json(s: &ProjectSettings) -> Value {
    json!({
        "default_page_size": s.default_page_size.map(|z| json!({ "width": z.width, "height": z.height })),
        "default_template": s.default_template,
        "default_lettering_style": s.default_lettering_style,
    })
}

/// A name-valued optional setting: absent leaves it, `null` clears it, a string sets it.
///
/// A blank name is refused rather than treated as "clear": `comic-layout`'s face filter skips an
/// empty family (it is not a font, and `text.missingFonts` skips it too), so `""` would silently
/// mean the *default* face while looking like a chosen one. `null` says that on purpose.
fn optional_name(p: &Value, key: &str, cmd: &str) -> Result<Option<Option<String>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(Some(s.clone()))),
        Some(Value::String(_)) => Err(bad(cmd, format!("`{key}` is blank: give a face name, or null to clear the setting"))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a face name (a string), or null to clear it"))),
    }
}

fn settings_get(s: &mut Session, _: &Value) -> Result<Value> {
    let b = &super::open(s)?.bundle;
    Ok(settings_json(&b.project.settings))
}

fn settings_set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "comic.project.settings.set";
    // `page_size_param` answers `None` for both "absent" and "null", so the two are told apart by
    // whether the key is there at all.
    let size_present = p.get("defaultPageSize").is_some();
    let size = page_size_param(p, "defaultPageSize", C)?;
    let style = optional_name(p, "defaultLetteringStyle", C)?;
    // A `set` that sets nothing is refused rather than answered with `Ok`: a caller who mistyped the
    // key would otherwise be told the change was made while the settings came back untouched.
    if !size_present && style.is_none() {
        return Err(bad(
            C,
            "give `defaultLetteringStyle` (a face name, or null to clear it) or `defaultPageSize` ([width, height], or null to clear it); `comic.project.settings.get` shows the current values",
        ));
    }
    comic_edit(s, "Set Project Settings", |s| {
        let oc = open_mut(s)?;
        let settings = &mut oc.bundle.project.settings;
        if size_present {
            settings.default_page_size = size;
        }
        if let Some(style) = style.clone() {
            settings.default_lettering_style = style;
        }
        Ok(settings_json(settings))
    })
}

fn describe_rect(r: Rect) -> String {
    if !(r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()) {
        return format!("not finite ([{}, {}, {}, {}])", r.x0, r.y0, r.x1, r.y1);
    }
    format!("{}x{} pt", r.width(), r.height())
}
