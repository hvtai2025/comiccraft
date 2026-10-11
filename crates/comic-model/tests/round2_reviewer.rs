//! Reviewer tests (`code-reviewer`, M1 **round 2**) for `crates/comic-model`.
//!
//! Round 1 reported 8 findings on the original revision; the owner fixed all eight, and the seven
//! round-1 reproductions in `round1_reviewer.rs` now pass. This file is the round-2 attack **on the
//! fixes**, not a second copy of round 1:
//!
//! - F1 (one id set per kind): is rule 1 now checking *every* reference's kind, or only the two
//!   round 1 named?
//! - F2 (two files for one id): is the report right, and does the *deterministic* half hold — does
//!   the same file win on every load, and is it the sorted first?
//! - F3 (a refused non-canonical timestamp): is the check complete — every timestamp the writer
//!   emits verbatim, and exactly the two it derives from `now` excluded?
//! - F4 (a refused save leaves nothing behind): is every fallible check before the first mutation,
//!   and is `.save-tmp/` removed on both paths?
//! - F5 (`FileEntry.unknown` carried over): can the carry-over resurrect a key for a file that is
//!   gone?
//! - F6 / F7 (real paths in findings; both §14 halves one severity): is the asymmetry gone?
//!
//! A test whose name starts with `finding_` encodes behaviour the frozen schema requires and was
//! **expected to fail** on the revision round 2 reviewed; it is the reproduction for a round-2
//! finding. Everything else is a positive test of a claim a fix makes.

// `clippy.toml` allows panics in tests, but clippy only applies that to `#[cfg(test)]` modules,
// not to an integration-test crate. This is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vectorcraft_comic_model::{
    ApprovalStatus, ArtboardFacts, Asset, AssetId, AssetType, ChapterId, CharacterId, ComicBundle, ComicId, Consent, ContentHash, Decision,
    DialogueId, DocumentFacts, DocumentPayload, Error, FileEntry, Finding, GenerationJob, JobFile, JobId, JobOperation, JobStatus, LetteringId,
    LocationId, MediaMetadata, NodeId, PageFile, PageId, PageStatus, Panel, PanelId, Provenance, ProviderId, ReviewDecision, ReviewDecisionId,
    ReviewSubject, SaveOptions, SceneId, Severity, StorageRef, StyleProfileId, Unknown, ValidationInput, load, save, sha256_hex, to_canonical_bytes,
    validate,
};
use vectorcraft_geom::Rect;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";
/// A timestamp a human reads as one and the schema does not accept.
const NON_CANONICAL: &str = "2026-10-11 06:00:00";

/// The fixture's ids, so "an id of another kind" is a *real* entity of another kind:
/// Chapter 3 · Scene 4 · Character 5 · Location 6 · StyleProfile 7 · Page 8 · Panels 9–11 ·
/// Dialogue 12–13 · Lettering 14–16 · Asset 17 · Job 18 · Decision 19.
fn fixture_root() -> PathBuf {
    PathBuf::from(FIXTURE)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("destination");
    for entry in fs::read_dir(from).expect("source") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// The fixture's own copy, in a scratch directory, loaded.
fn loaded_fixture(name: &str) -> (PathBuf, ComicBundle) {
    let root = scratch(name);
    copy_tree(&fixture_root(), &root);
    let loaded = load(&root).expect("the fixture copy loads");
    (root, loaded.bundle)
}

/// What rules 4, 7 and 15 need. The fixture's document holds artboard 1 (612×792 pt) and nodes 2–13.
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn options() -> SaveOptions {
    SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None }
}

fn validate_fixture(root: &Path, bundle: &ComicBundle) -> Vec<Finding> {
    validate(bundle, &ValidationInput { root, document: &document_facts(), archive_root: None })
}

fn rules(findings: &[Finding], rule: u8) -> Vec<&Finding> {
    findings.iter().filter(|f| f.rule == Some(rule)).collect()
}

fn page_mut(bundle: &mut ComicBundle) -> &mut PageFile {
    bundle.pages.get_mut(&PageId(8)).expect("the fixture's page 8")
}

fn panel_mut(bundle: &mut ComicBundle) -> &mut Panel {
    page_mut(bundle).panels.first_mut().expect("panel 9")
}

fn job_file_mut(bundle: &mut ComicBundle) -> &mut JobFile {
    bundle.jobs.get_mut(&JobId(18)).expect("the fixture's job 18")
}

/// Every file under `root`, by `/`-separated key, with its bytes — so "the directory is exactly as
/// it was" is a comparison of everything, not of the two files a test remembered to look at.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("type").is_dir() {
                stack.push(path);
            } else {
                let key = path.strip_prefix(root).expect("inside").to_string_lossy().replace('\\', "/");
                out.insert(key, fs::read(&path).expect("readable"));
            }
        }
    }
    out
}

/// A generated asset with provenance and consent, so the §5.5 shared types the fixture never writes
/// are in the model for a test to break.
fn provenance_asset(id: u64) -> Asset {
    Asset {
        id: AssetId(id),
        asset_type: AssetType::Generated,
        storage_reference: StorageRef::Linked { relative_path: "assets/panels/panel-1.png".into() },
        content_hash: ContentHash {
            algorithm: "sha256".into(),
            value: sha256_hex(&fs::read(fixture_root().join("assets/panels/panel-1.png")).expect("readable")),
        },
        media_metadata: MediaMetadata { mime: "image/png".into(), width: 8, height: 8, color_profile: None, dpi: Some(72.0) },
        provenance: Some(Provenance {
            provider: ProviderId("local-diffusion".into()),
            model: "example-v1".into(),
            request: serde_json::json!({ "prompt": "rooftop" }),
            input_asset_ids: vec![AssetId(17)],
            created: FIXTURE_UPDATED.into(),
            consent: Some(Consent { granted: FIXTURE_UPDATED.into(), statement: "Send it.".into(), unknown: Unknown::new() }),
            unknown: Unknown::new(),
        }),
        parent_asset_ids: vec![],
        approval_status: ApprovalStatus::Pending,
        unknown: Unknown::new(),
    }
}

fn extra_decision(id: u64) -> ReviewDecision {
    ReviewDecision {
        id: ReviewDecisionId(id),
        subject: ReviewSubject::StoryContent,
        decision: Decision::Approved,
        actor: "reviewer".into(),
        timestamp: FIXTURE_UPDATED.into(),
        note: None,
        superseded_asset_id: None,
        unknown: Unknown::new(),
    }
}

fn extra_job(id: u64) -> GenerationJob {
    GenerationJob {
        id: JobId(id),
        operation: JobOperation::TextToImage,
        target_panel_id: Some(PanelId(9)),
        input_asset_ids: vec![AssetId(17)],
        provider: ProviderId("local-diffusion".into()),
        model: "example-v1".into(),
        request: serde_json::json!({}),
        status: JobStatus::Running,
        progress: None,
        error: None,
        result_asset_ids: vec![],
        created: FIXTURE_UPDATED.into(),
        updated: FIXTURE_UPDATED.into(),
        target_revision: 1,
        consent: Some(Consent { granted: FIXTURE_UPDATED.into(), statement: "Send it.".into(), unknown: Unknown::new() }),
        unknown: Unknown::new(),
    }
}

// -------------------------------------------------------------------------------------------------
// F1 — is rule 1 now checking *every* reference's kind, or only the two round 1 named?
// -------------------------------------------------------------------------------------------------

/// One reference slot: the entity's spelling, the field, and the mutation that puts a real entity
/// of **another kind** in it.
type ReferenceSlot = (&'static str, &'static str, fn(&mut ComicBundle));

/// Every reference the schema declares, with a value that is a **real entity of another kind**.
fn rule_1_cases() -> Vec<ReferenceSlot> {
    vec![
        ("ComicProject(1)", "chapter_ids", |b| b.project.chapter_ids = vec![ChapterId(8)]),
        ("ComicProject(1)", "character_ids", |b| b.project.character_ids = vec![CharacterId(8)]),
        ("ComicProject(1)", "location_ids", |b| b.project.location_ids = vec![LocationId(8)]),
        ("ComicProject(1)", "style_profile_ids", |b| b.project.style_profile_ids = vec![StyleProfileId(8)]),
        ("Chapter(3)", "project_id", |b| b.chapters.chapters[0].project_id = ComicId(8)),
        ("Chapter(3)", "scene_ids", |b| b.chapters.chapters[0].scene_ids = vec![SceneId(8)]),
        ("Scene(4)", "chapter_id", |b| b.scenes.scenes[0].chapter_id = ChapterId(8)),
        ("Scene(4)", "page_ids", |b| b.scenes.scenes[0].page_ids = vec![PageId(9)]),
        ("Scene(4)", "location_id", |b| b.scenes.scenes[0].location_id = Some(LocationId(8))),
        ("Scene(4)", "character_ids", |b| b.scenes.scenes[0].character_ids = vec![CharacterId(8)]),
        ("Character(5)", "reference_asset_ids", |b| b.characters.characters[0].reference_asset_ids = vec![AssetId(8)]),
        ("Character(5)", "style_profile_id", |b| b.characters.characters[0].style_profile_id = Some(StyleProfileId(8))),
        ("Character(5)", "approved_design_version", |b| b.characters.characters[0].approved_design_version = Some(AssetId(8))),
        ("Location(6)", "reference_asset_ids", |b| b.locations.locations[0].reference_asset_ids = vec![AssetId(8)]),
        ("Page(8)", "chapter_id", |b| page_mut(b).page.chapter_id = ChapterId(9)),
        ("Page(8)", "scene_ids", |b| page_mut(b).page.scene_ids = vec![SceneId(9)]),
        ("Page(8)", "panel_ids", |b| page_mut(b).page.panel_ids = vec![PanelId(12)]),
        ("Page(8)", "reading_order", |b| page_mut(b).page.reading_order = vec![PanelId(12)]),
        ("Panel(9)", "page_id", |b| panel_mut(b).page_id = PageId(12)),
        ("Panel(9)", "scene_id", |b| panel_mut(b).scene_id = SceneId(8)),
        ("Panel(9)", "artwork_refs[].asset_id", |b| panel_mut(b).artwork_refs[0].asset_id = AssetId(8)),
        ("Panel(9)", "dialogue_ids", |b| panel_mut(b).dialogue_ids = vec![DialogueId(9)]),
        ("Panel(9)", "lettering_ids", |b| panel_mut(b).lettering_ids = vec![LetteringId(9)]),
        ("Panel(9)", "continuity_constraints.characters", |b| panel_mut(b).continuity_constraints.characters = vec![CharacterId(8)]),
        ("Dialogue(12)", "panel_id", |b| page_mut(b).dialogue[0].panel_id = PanelId(8)),
        ("Lettering(14)", "page_id", |b| page_mut(b).lettering[0].page_id = PageId(14)),
        ("Lettering(14)", "panel_id", |b| page_mut(b).lettering[0].panel_id = PanelId(14)),
        ("Lettering(14)", "dialogue_id", |b| page_mut(b).lettering[0].dialogue_id = Some(DialogueId(9))),
        ("Asset(17)", "parent_asset_ids", |b| b.assets.assets[0].parent_asset_ids = vec![AssetId(8)]),
        ("GenerationJob(18)", "target_panel_id", |b| b.provenance.jobs[0].target_panel_id = Some(PanelId(8))),
        ("GenerationJob(18)", "input_asset_ids/result_asset_ids", |b| b.provenance.jobs[0].input_asset_ids = vec![AssetId(8)]),
        ("GenerationJob(18)", "target_panel_id", |b| job_file_mut(b).job.target_panel_id = Some(PanelId(8))),
        ("GenerationJob(18)", "input_asset_ids/result_asset_ids", |b| job_file_mut(b).job.result_asset_ids = vec![AssetId(8)]),
        ("ReviewDecision(19)", "superseded_asset_id", |b| b.provenance.decisions[0].superseded_asset_id = Some(AssetId(8))),
    ]
}

#[test]
fn rule_1_checks_every_reference_slot_against_its_own_kind() {
    // Round 1 named two slots (`Scene.page_ids` naming a panel, `Lettering.page_id` naming a
    // lettering) because `Index::build` collected page, panel, dialogue and lettering ids into one
    // set. The fix made one set per kind; this walks **every reference the schema declares** and
    // requires a rule-1 finding when the value is a real entity of another kind — the case a merged
    // set lets through. A slot still checked against a merged or wrong set shows up here.
    let mut survived = Vec::new();
    for (owner, field, mutate) in rule_1_cases() {
        let (root, mut bundle) = loaded_fixture("r2-rule-1-kinds");
        mutate(&mut bundle);
        let findings = validate_fixture(&root, &bundle);
        let kind = owner.split('(').next().unwrap_or(owner);
        let hit = rules(&findings, 1).iter().any(|f| f.subject.starts_with(kind) && f.subject.ends_with(field));
        if !hit {
            let seen: Vec<&str> = rules(&findings, 1).iter().map(|f| f.subject.as_str()).collect();
            survived.push(format!("{owner}.{field} — rule 1 reported {seen:?}"));
        }
        fs::remove_dir_all(&root).ok();
    }
    assert!(survived.is_empty(), "these reference slots were not checked against their own kind:\n  {}", survived.join("\n  "));
}

/// `Dialogue.speaker_character_id` is governed by rule 12, and rule 12 is *not* rule 1 (§13: "the
/// rule checks the set values, not the field's presence"). Once the id sets are separate the two
/// must not be confused in either direction.
#[test]
fn the_speaker_reference_is_rule_12s_and_not_rule_1s() {
    let (root, mut bundle) = loaded_fixture("r2-speaker-kind");
    page_mut(&mut bundle).dialogue[0].speaker_character_id = Some(CharacterId(8));
    let findings = validate_fixture(&root, &bundle);
    assert_eq!(rules(&findings, 1).len(), 0, "rule 1 does not walk `speaker_character_id`: {findings:?}");
    assert!(rules(&findings, 12).iter().any(|f| f.subject == "Dialogue(12).speaker_character_id"), "{findings:?}");
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// F2 — the duplicate report and the deterministic half.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_duplicate_is_reported_against_the_file_that_loses_and_the_sorted_first_wins() {
    // The fix reports the duplicate and keeps "the first in sorted key order". Both halves are
    // testable only if the two files **differ**: with identical bytes "which one won" is invisible
    // and any iteration order passes. `page-008.json` sorts before `page-8.json`, so the padded
    // spelling must be the one the loader keeps, and the canonical one the file it reports.
    let (root, mut bundle) = loaded_fixture("r2-duplicate-winner");
    page_mut(&mut bundle).page.status = PageStatus::Generated;
    let generated = to_canonical_bytes("a page file", &bundle.pages[&PageId(8)]).expect("serializes");
    let mut draft_model = bundle.clone();
    page_mut(&mut draft_model).page.status = PageStatus::Draft;
    let draft = to_canonical_bytes("a page file", &draft_model.pages[&PageId(8)]).expect("serializes");
    fs::write(root.join("pages/page-008.json"), &generated).expect("the padded spelling");
    fs::write(root.join("pages/page-8.json"), &draft).expect("the unpadded spelling");

    let loaded = load(&root).expect("the ambiguous bundle still loads");
    assert_eq!(loaded.bundle.pages[&PageId(8)].page.status, PageStatus::Generated, "the sorted-first file (`page-008.json`) must win");
    let subjects: Vec<&str> = rules(&loaded.findings, 2).iter().map(|f| f.subject.as_str()).collect();
    assert_eq!(subjects, vec!["pages/page-8.json"], "the report must name the file that lost, and only it: {:?}", loaded.findings);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_duplicate_resolves_the_same_way_on_every_load() {
    // "Deterministic" must mean more than "it happened to work once": the same bundle loaded twice
    // must not change which generation the caller sees, nor what is reported.
    let (root, mut bundle) = loaded_fixture("r2-duplicate-determinism");
    page_mut(&mut bundle).page.status = PageStatus::Generated;
    let generated = to_canonical_bytes("a page file", &bundle.pages[&PageId(8)]).expect("serializes");
    let mut draft_model = bundle.clone();
    page_mut(&mut draft_model).page.status = PageStatus::Draft;
    let draft = to_canonical_bytes("a page file", &draft_model.pages[&PageId(8)]).expect("serializes");
    fs::write(root.join("pages/page-008.json"), &generated).expect("padded");
    fs::write(root.join("pages/page-8.json"), &draft).expect("unpadded");

    let first = load(&root).expect("loads");
    let second = load(&root).expect("loads again");
    assert_eq!(first.bundle.pages[&PageId(8)], second.bundle.pages[&PageId(8)], "the same bundle must load the same way twice");
    assert_eq!(first.findings, second.findings, "and report the same thing twice");
    assert_eq!(first.bundle.pages[&PageId(8)].page.status, PageStatus::Generated, "the winner is the sorted-first key, not the last one read");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn two_job_files_for_one_job_id_are_reported_too() {
    // §8.3 makes a job file the live copy and names it `job-<JobId>.json`, so the same padding is
    // the same ambiguity — `jobs/` and `pages/` must not have diverged. `jobs/job-018.json` sorts
    // before `jobs/job-18.json`, so the *declared, canonical* name is the one that loses here: the
    // winner is chosen by sorted key order, not by which file `manifest.files` lists.
    let (root, mut bundle) = loaded_fixture("r2-duplicate-job");
    save(&mut bundle, &root, &options()).expect("saves");
    fs::copy(root.join("jobs/job-18.json"), root.join("jobs/job-018.json")).expect("the duplicate");

    let loaded = load(&root).expect("loads");
    assert_eq!(loaded.bundle.jobs.len(), 1, "one job id");
    assert!(
        rules(&loaded.findings, 2).iter().any(|f| f.subject == "jobs/job-18.json"),
        "the job duplicate must be reported against the file that lost: {:?}",
        loaded.findings
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// F3 — is the refusal complete, and are exactly the two derived timestamps excluded?
// -------------------------------------------------------------------------------------------------

/// One field to make non-canonical and the mutation that does it.
type TimestampCase = (&'static str, fn(&mut ComicBundle));

/// A bundle carrying every timestamp the writer emits verbatim, so each branch of the check can be
/// exercised: an asset with provenance **and** consent (the fixture's only asset has neither), a job
/// with consent, and a second review decision.
fn timestamp_model(name: &str) -> (PathBuf, ComicBundle) {
    let (root, mut bundle) = loaded_fixture(name);
    bundle.assets.assets.push(provenance_asset(21));
    bundle.provenance.jobs[0].consent = Some(Consent { granted: FIXTURE_UPDATED.into(), statement: "Send it.".into(), unknown: Unknown::new() });
    bundle.provenance.decisions.push(extra_decision(22));
    (root, bundle)
}

#[test]
fn every_timestamp_the_writer_emits_verbatim_is_checked() {
    // §5.7: "a **writer** must produce this form". The owner's new test covers four fields
    // (`manifest.created`, `project.metadata.created`, one job's `created`, one decision's
    // `timestamp`), leaving the whole asset-provenance branch, a job's `updated` and every
    // `consent.granted` unexercised — mutations that delete those branches leave the owner's suite
    // green (round-2 finding 4). Every verbatim timestamp is here.
    let cases: Vec<TimestampCase> = vec![
        ("SaveOptions.now", |_| {}),
        ("manifest.created", |b| b.manifest.created = NON_CANONICAL.into()),
        ("project.metadata.created", |b| b.project.metadata.created = NON_CANONICAL.into()),
        ("Asset(21).provenance.created", |b| b.assets.assets[1].provenance.as_mut().expect("provenance").created = NON_CANONICAL.into()),
        ("Asset(21).provenance.consent.granted", |b| {
            b.assets.assets[1].provenance.as_mut().expect("provenance").consent.as_mut().expect("consent").granted = NON_CANONICAL.into();
        }),
        ("GenerationJob(18).created", |b| b.provenance.jobs[0].created = NON_CANONICAL.into()),
        ("GenerationJob(18).updated", |b| b.provenance.jobs[0].updated = NON_CANONICAL.into()),
        ("GenerationJob(18).consent.granted", |b| b.provenance.jobs[0].consent.as_mut().expect("consent").granted = NON_CANONICAL.into()),
        ("ReviewDecision(22).timestamp", |b| b.provenance.decisions[1].timestamp = NON_CANONICAL.into()),
    ];

    let mut unreported = Vec::new();
    for (field, mutate) in cases {
        let root = scratch("r2-timestamp-fields");
        let (_fixture, mut bundle) = timestamp_model("r2-timestamp-model");
        mutate(&mut bundle);
        let save_options = if field == "SaveOptions.now" { SaveOptions { now: NON_CANONICAL.into(), document: None } } else { options() };
        match save(&mut bundle, &root, &save_options) {
            Err(Error::BadTimestamp { field: named, .. }) if named == field => {}
            Err(Error::BadTimestamp { field: named, .. }) => unreported.push(format!("{field}: refused under the wrong name `{named}`")),
            other => unreported.push(format!("{field}: not refused at all — {other:?}")),
        }
        fs::remove_dir_all(&root).ok();
    }
    assert!(unreported.is_empty(), "these timestamps would be written verbatim without being checked:\n  {}", unreported.join("\n  "));
}

#[test]
fn the_two_derived_timestamps_are_replaced_by_now_and_are_not_refused() {
    // `manifest.updated` and `ProjectMetadata.updated` are the only two the writer derives, so a
    // stale one must be **overwritten** with `now` rather than refused — and the value that lands on
    // disk must be `now`, canonical, in both files.
    let (root, mut bundle) = loaded_fixture("r2-derived-timestamps");
    bundle.manifest.updated = NON_CANONICAL.into();
    bundle.project.metadata.updated = "nonsense".into();
    save(&mut bundle, &root, &options()).expect("the two derived timestamps are overwritten, not checked");

    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    let project: serde_json::Value = serde_json::from_slice(&fs::read(root.join("project.json")).expect("readable")).expect("json");
    assert_eq!(manifest["updated"], serde_json::json!(FIXTURE_UPDATED), "the writer derives it from `now`");
    assert_eq!(project["metadata"]["updated"], serde_json::json!(FIXTURE_UPDATED), "and so is the project's copy");
    fs::remove_dir_all(&root).ok();
}

/// Every string in `value` that reads like a date, checked against the one canonical form.
fn collect_timestamp_lookalikes(value: &serde_json::Value, key: &str, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            let looks_like_a_date = s.len() >= 10 && s.as_bytes()[..4].iter().all(u8::is_ascii_digit) && s.as_bytes()[4] == b'-';
            if looks_like_a_date && !vectorcraft_comic_model::is_canonical_timestamp(s) {
                out.push(format!("{key}: {s:?}"));
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_timestamp_lookalikes(v, key, out)),
        serde_json::Value::Object(map) => map.values().for_each(|v| collect_timestamp_lookalikes(v, key, out)),
        _ => {}
    }
}

#[test]
fn no_non_canonical_timestamp_can_reach_the_disk_at_all() {
    // The end-to-end half of F3: rather than checking that the writer *refuses* the fields a test
    // remembered, read every string the save produced and require the canonical form of any that
    // reads like a date. A field the check forgets shows up here even though no test names it.
    let (root, mut bundle) = timestamp_model("r2-timestamp-every-file");
    bundle.assets.assets.push(provenance_asset(23));
    bundle.provenance.decisions.push(extra_decision(24));
    save(&mut bundle, &root, &options()).expect("a canonical model saves");

    let mut offenders = Vec::new();
    for (key, bytes) in snapshot(&root) {
        if !key.ends_with(".json") {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("canonical json");
        collect_timestamp_lookalikes(&value, &key, &mut offenders);
    }
    assert!(offenders.is_empty(), "a save wrote a timestamp that is not canonical: {offenders:?}");
    fs::remove_dir_all(&root).ok();
}

/// `manifest.json` rewritten with `f`, for a test that must change the file rather than the model.
fn rewrite_manifest(root: &Path, f: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    f(&mut value);
    serde_json::to_vec_pretty(&value).expect("json")
}

#[test]
fn the_reader_accepts_a_timestamp_the_writer_refuses() {
    // The new failure path is one-sided. §14 says a damaged bundle must be **openable and
    // repairable** — "a creator can open a damaged bundle and repair it rather than being locked
    // out" — and §11 requires a migration to "report incompatible data as a visible, actionable
    // error". `load` accepts `2026-10-11 06:00:00` with **no finding at all** (no rule 1–18 covers a
    // timestamp, and §14's five steps do not either), so the creator's only warning is `save`
    // failing — after they have done the work. The reader and the writer disagree about one value.
    let (root, mut bundle) = loaded_fixture("r2-timestamp-asymmetry");
    bundle.manifest.created = NON_CANONICAL.into();
    fs::write(root.join("manifest.json"), rewrite_manifest(&root, |m| m["created"] = serde_json::json!(NON_CANONICAL))).expect("write the manifest");

    // This is a **characterisation** test, not a reproduction: it pins the asymmetry as it is, so
    // that the finding is evidence and not an opinion. A `finding_`-prefixed test would have to
    // assert the repair (a load-time finding) and fail; this one asserts the current behaviour.
    let loaded = load(&root).expect("§14 step 1 does not refuse it");
    assert!(loaded.findings.is_empty(), "the load reports nothing about a timestamp the writer will refuse: {:?}", loaded.findings);
    let mut model = loaded.bundle.clone();
    let refused = save(&mut model, &root, &options());
    assert!(matches!(refused, Err(Error::BadTimestamp { .. })), "…and then the save refuses the model it just handed over: {refused:?}");
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// F4 — every fallible check before the first mutation, and `.save-tmp/` gone on both paths.
// -------------------------------------------------------------------------------------------------

#[test]
fn every_way_a_save_can_refuse_leaves_the_whole_model_and_the_whole_directory_untouched() {
    // Round 1's reproduction checked `visual_document.relative_path`. "Leaves nothing behind" is a
    // stronger claim: the *whole* model (`ComicBundle` derives `PartialEq`) and *every* byte of the
    // directory — including a stale `.save-tmp/` an earlier killed run left, which a refused save
    // must not have to clean up because it never created it.
    type Case = (&'static str, fn(&mut ComicBundle) -> SaveOptions);
    let cases: Vec<Case> = vec![
        ("a non-canonical now", |_| SaveOptions { now: NON_CANONICAL.into(), document: None }),
        ("a document path that walks up out of the bundle", |_| SaveOptions {
            now: FIXTURE_UPDATED.into(),
            document: Some(DocumentPayload { relative_path: "../escape.vectorcraft".into(), format_version: 4, bytes: b"x".to_vec() }),
        }),
        ("a document path that is absolute", |_| SaveOptions {
            now: FIXTURE_UPDATED.into(),
            document: Some(DocumentPayload { relative_path: "/etc/passwd".into(), format_version: 4, bytes: b"x".to_vec() }),
        }),
        ("a document the model names that is not on disk", |b| {
            b.project.visual_document.relative_path = "document/absent.vectorcraft".into();
            options()
        }),
    ];

    let mut failures = Vec::new();
    for (label, make) in cases {
        let (root, mut bundle) = loaded_fixture("r2-refused-save");
        fs::create_dir_all(root.join(".save-tmp/story")).expect("a stale staging directory, as a killed run leaves");
        fs::write(root.join(".save-tmp/story/chapters.json"), b"stale").expect("stale");
        // The case's own model change (a document the model names) happens *before* the snapshot;
        // what is compared is only what the save itself did.
        let save_options = make(&mut bundle);
        let before = snapshot(&root);
        let model_before = bundle.clone();
        let result = save(&mut bundle, &root, &save_options);
        if result.is_ok() {
            failures.push(format!("{label}: the save was not refused — {result:?}"));
        }
        if bundle != model_before {
            failures.push(format!("{label}: a refused save mutated the model"));
        }
        let after = snapshot(&root);
        if after != before {
            let changed: Vec<&String> = after.keys().chain(before.keys()).filter(|k| before.get(*k) != after.get(*k)).collect();
            failures.push(format!("{label}: the directory changed — {changed:?}"));
        }
        fs::remove_dir_all(&root).ok();
    }
    assert!(failures.is_empty(), "a refused save is not clean:\n  {}", failures.join("\n  "));
}

#[test]
fn a_failure_in_step_4_leaves_the_previous_manifest_untouched() {
    // §10: "manifest.json is written last" is the whole guarantee — a reader that lands inside the
    // window sees the *new* content under the *previous* manifest, which §14 makes survivable. The
    // owner's new barrier test asserts the manifest bytes are unchanged after a step-4 failure, but
    // its two saves use the same `now` and the same content, so the manifest it would rewrite is
    // byte-identical and the assertion cannot tell "not written" from "written with the same
    // bytes". Here the second save has a later `now` and a changed page, so *any* manifest write
    // during the failed save changes what is on disk.
    let (root, mut bundle) = loaded_fixture("r2-step-4-manifest");
    save(&mut bundle, &root, &options()).expect("the first save succeeds");
    let previous = fs::read(root.join("manifest.json")).expect("readable");

    page_mut(&mut bundle).page.status = PageStatus::Generated;
    fs::remove_file(root.join("project.json")).expect("remove the project");
    fs::create_dir(root.join("project.json")).expect("a directory where the project file was");
    let later = SaveOptions { now: "2026-10-11T07:30:00Z".into(), document: None };
    assert!(save(&mut bundle, &root, &later).is_err(), "step 4 cannot replace a file with a directory in the way");

    assert_eq!(
        fs::read(root.join("manifest.json")).expect("readable"),
        previous,
        "§10: the manifest is the commit point and is written LAST, so a failure in step 4 must leave the previous generation's manifest byte for byte in place"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_failure_after_staging_still_removes_the_staging_directory_and_stays_loadable() {
    // The other half of "`.save-tmp/` is removed on both paths": the failure must happen *after*
    // step 1 has staged files, or the test proves nothing about the error arm. A regular file where
    // the `pages/` directory belongs makes step 4's `create_dir_all` fail once other files have
    // already been renamed into place.
    let (root, mut bundle) = loaded_fixture("r2-failure-after-staging");
    page_mut(&mut bundle).page.status = PageStatus::Generated;
    fs::remove_dir_all(root.join("pages")).expect("remove the pages directory");
    fs::write(root.join("pages"), b"a file where the pages directory belongs").expect("block step 4");

    let result = save(&mut bundle, &root, &options());
    assert!(result.is_err(), "step 4 cannot replace a file when a file sits where its directory goes: {result:?}");
    assert!(!root.join(".save-tmp").exists(), "the error arm must remove the staging directory it created");

    let loaded = load(&root).expect("§14: a bundle interrupted mid-save must still load");
    assert!(!loaded.findings.is_empty(), "and the `files`/tree mismatch must be reported: {:?}", loaded.findings);
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// F5 — can the carry-over resurrect a key for a file that is gone?
// -------------------------------------------------------------------------------------------------

#[test]
fn the_carry_over_cannot_resurrect_a_key_for_a_file_that_no_longer_exists() {
    // The carry-over walks the *new* scan, so a key that is not in the tree cannot come back. The
    // sharp case is the prune's: a page the bundle no longer has is deleted by step 4, and its
    // `files` entry — unknown keys and all — must go with it, or the manifest would describe a file
    // that is not there.
    let (root, mut bundle) = loaded_fixture("r2-carry-over-prune");
    bundle.manifest.files.insert(
        "pages/page-99.json".into(),
        FileEntry { sha256: sha256_hex(b"gone"), bytes: 4, unknown: Unknown::from_iter([("mtime".to_string(), serde_json::json!(1))]) },
    );
    bundle.manifest.files.get_mut("project.json").expect("the entry").unknown.insert("mtime".into(), serde_json::json!(2));

    save(&mut bundle, &root, &options()).expect("saves");

    let written: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    let files = written["files"].as_object().expect("a map");
    assert!(!files.contains_key("pages/page-99.json"), "a key for a file that does not exist must not be resurrected: {files:?}");
    assert_eq!(files["project.json"]["mtime"], serde_json::json!(2), "the carry-over must still honour a key for a file that does exist");
    assert!(files["pages/page-8.json"].get("mtime").is_none(), "a file with no previous entry gets a fresh one");
    assert!(!bundle.manifest.files.contains_key("pages/page-99.json"), "and the model's own map must not keep it either");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_carried_over_entry_key_survives_load_save_load() {
    // Round 1's reproduction asserted one save. The field only earns its place if the key survives
    // the whole cycle — load, save, load again — because that is what §12 promises a newer
    // ComicCraft's field.
    let (root, mut bundle) = loaded_fixture("r2-carry-over-cycle");
    bundle.manifest.files.get_mut("pages/page-8.json").expect("the entry").unknown.insert("writer".into(), serde_json::json!("v9"));
    save(&mut bundle, &root, &options()).expect("saves");

    let reloaded = load(&root).expect("reloads");
    assert_eq!(
        reloaded.bundle.manifest.files["pages/page-8.json"].unknown.get("writer"),
        Some(&serde_json::json!("v9")),
        "the key must survive a load after the save, not only the save itself"
    );
    assert!(reloaded.findings.is_empty(), "and the bundle must still be consistent: {:?}", reloaded.findings);
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// F6 / F7 — real subjects, and one severity for one §14 condition.
// -------------------------------------------------------------------------------------------------

#[test]
fn rule_3_names_the_file_that_declares_the_wrong_version() {
    // `jobs/job-<JobId>.json` was the subject for *every* job file, so a creator with several could
    // not tell which one to fix; `pages/page-<PageId>.json` was the same. Both must now be the real
    // key, and two files of one kind must produce two differently-subjected findings.
    let (root, mut bundle) = loaded_fixture("r2-rule-3-subjects");
    bundle.pages.get_mut(&PageId(8)).expect("page 8").schema_version = 2;
    bundle.provenance.jobs.push(extra_job(30));
    bundle.jobs.insert(JobId(30), JobFile { schema_version: 3, job: extra_job(30), unknown: Unknown::new() });

    let findings = validate_fixture(&root, &bundle);
    let subjects: Vec<&str> = rules(&findings, 3).iter().map(|f| f.subject.as_str()).collect();
    assert!(subjects.contains(&"pages/page-8.json"), "the page finding must name the real file: {subjects:?}");
    assert!(subjects.contains(&"jobs/job-30.json"), "the job finding must name the real file: {subjects:?}");
    assert!(!subjects.iter().any(|s| s.contains("page-<") || s.contains("job-<")), "a template string is not an actionable subject: {subjects:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn both_halves_of_the_files_mismatch_are_the_same_non_blocking_severity() {
    // §14 makes the two directions of the `files`/tree mismatch **one** condition with three causes,
    // "the third cause is benign and expected", so filing one half as an error and the other as a
    // warning would make the schema's own survivable window a blocker for whatever gates on
    // `is_error`. Round 1's test only checked that both are reported; this checks the severity, that
    // neither carries a rule number (it is a §14 observation, not one of the eighteen), and that the
    // two directions of one condition agree.
    let (root, mut bundle) = loaded_fixture("r2-mismatch-severity");
    save(&mut bundle, &root, &options()).expect("saves");
    fs::write(root.join("story/notes.json"), b"{}\n").expect("a file the manifest does not list");
    fs::remove_file(root.join("jobs/job-18.json")).expect("a file the manifest does list, gone");

    let loaded = load(&root).expect("§14: the mismatch is a finding, not a refusal");
    let mismatches: Vec<&Finding> = loaded.findings.iter().filter(|f| f.subject == "story/notes.json" || f.subject == "jobs/job-18.json").collect();
    assert_eq!(mismatches.len(), 2, "both halves must be reported: {:?}", loaded.findings);
    for finding in mismatches {
        assert_eq!(finding.severity, Severity::Warning, "§14 makes both halves one non-blocking condition: {finding:?}");
        assert!(!finding.is_error(), "{finding:?}");
        assert_eq!(finding.rule, None, "it is a §14 load observation, not one of the eighteen rules: {finding:?}");
    }
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// Round-2 edges: the code the fixes rewrote, walked from the outside.
// -------------------------------------------------------------------------------------------------

#[test]
fn finding_1_a_stray_json_file_in_pages_is_a_fatal_load_error() {
    // §14 names **two** fatal load failures and says of everything else that it is "returned as a
    // finding on a usable project, so a creator can open a damaged bundle and repair it rather than
    // being locked out". `pages/` is also where a person drops things; a file whose name is not
    // `page-<id>.json` currently makes the whole bundle un-openable, so the creator cannot even
    // reach the tools that would repair it.
    let (root, _bundle) = loaded_fixture("r2-stray-page-file");
    fs::write(root.join("pages/notes.json"), b"{}\n").expect("a stray file");

    let loaded = load(&root);
    assert!(loaded.is_ok(), "§14: only a document or a comic file that will not parse is fatal: {:?}", loaded.err());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn finding_2_a_save_deletes_a_declared_json_file_it_does_not_own() {
    // `manifest.files` exists so "a reader can detect a truncated or tampered bundle", and §4
    // defines it as "bundle-relative path → `FileEntry`, for every file except the manifest and
    // except `exports/` and `.save-tmp/`" — so a `files` entry the schema does not name is a file a
    // **newer** ComicCraft wrote. The prune removes every `.json` in `pages/`, `jobs/`, `story/` and
    // `provenance/` that the new generation does not stage, which includes such a file: the save
    // deletes a file the manifest declares, and the next load reports it as absent. §12's promise
    // ("a field written by a newer ComicCraft survives a load-and-save") has no analogue for files.
    let (root, _bundle) = loaded_fixture("r2-prune-declared");
    let bytes = b"{\n  \"schema_version\": 1,\n  \"relationships\": []\n}\n";
    fs::write(root.join("story/relationships.json"), bytes).expect("a file from a newer version");
    let declared = rewrite_manifest(&root, |m| {
        m["files"]["story/relationships.json"] = serde_json::json!({ "sha256": sha256_hex(bytes), "bytes": bytes.len() });
    });
    fs::write(root.join("manifest.json"), declared).expect("declare it, as a newer version would");

    let loaded = load(&root).expect("the load accepts it: declared and present");
    assert!(loaded.findings.is_empty(), "the fixture plus the declared file is a consistent bundle: {:?}", loaded.findings);
    let mut bundle = loaded.bundle;

    save(&mut bundle, &root, &options()).expect("saves");
    assert!(root.join("story/relationships.json").is_file(), "a save must not delete a file the manifest declares and the load accepted");
    fs::remove_dir_all(&root).ok();
}
