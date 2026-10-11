//! Reviewer tests (`code-reviewer`, M1 round 1) for `crates/comic-model`.
//!
//! These are written against the frozen schema (`docs/comiccraft-project-schema.md`) and the
//! design, not against the implementation. Each targets a case the implementer's own suite does
//! not cover: the kind-correctness of rule 1's reference walk, the duplicate-page-file collapse,
//! the three bases of rule 9, the save order's crash windows, the prune step, the fixture's
//! fidelity, and the writer's timestamp contract.
//!
//! Tests whose name starts with `finding_` encode the behaviour the frozen schema requires and
//! are expected to **fail** on the revision under review; they are the reproduction for a round-1
//! finding. The rest are positive tests of a claim the implementer made.

// `clippy.toml` allows panics in tests, but clippy only applies that to `#[cfg(test)]` modules,
// not to an integration-test crate. This is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vectorcraft_comic_model::{
    ArtboardFacts, Asset, AssetType, ComicBundle, Consent, ContentHash, DocumentFacts, DocumentPayload, Error, FileEntry, Finding, JobId, NodeId,
    PageId, PageStatus, Provenance, ProviderId, SaveOptions, Severity, StorageRef, Unknown, ValidationInput, load, save, sha256_hex,
    to_canonical_bytes, validate,
};
use vectorcraft_geom::Rect;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v1/minimal.comiccraft");
const FIXTURE_UPDATED: &str = "2026-10-11T06:00:00Z";

fn fixture_root() -> PathBuf {
    PathBuf::from(FIXTURE)
}

/// A scratch directory under the target directory, so a test never writes outside the checkout.
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

/// What rules 4, 7 and 15 need, read from the fixture's document. The fixture's
/// `document/comic.vectorcraft` holds artboard 1 (612×792 pt) and nodes 2–13.
fn document_facts() -> DocumentFacts {
    DocumentFacts { artboards: vec![ArtboardFacts { id: 1, rect: Rect::new(0.0, 0.0, 612.0, 792.0) }], nodes: (2..=13u64).map(NodeId).collect() }
}

fn options() -> SaveOptions {
    SaveOptions { now: FIXTURE_UPDATED.to_string(), document: None }
}

fn rules(findings: &[Finding], rule: u8) -> Vec<&Finding> {
    findings.iter().filter(|f| f.rule == Some(rule)).collect()
}

// -------------------------------------------------------------------------------------------------
// Finding 1 — rule 1 accepts a "Page" reference that names a panel, a dialogue or a lettering.
// -------------------------------------------------------------------------------------------------

#[test]
fn finding_1_a_scene_page_id_that_names_a_panel_is_not_a_page_and_must_be_reported() {
    // §13 rule 1: "Every reference resolves: every `*_ids` entry and every singular foreign key".
    // `Scene.page_ids` holds `PageId`s, so `9` — which is a *Panel* in this fixture, not a page —
    // does not resolve. Nothing else in the eighteen rules cross-checks `Scene.page_ids` against
    // the real page set (rule 14 only warns about page files no scene reaches), so rule 1 is the
    // only guard and it must fire.
    let (root, mut bundle) = loaded_fixture("r1-scene-page-ids");
    bundle.scenes.scenes[0].page_ids = vec![PageId(9)];
    let document = document_facts();
    let input = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &input);
    assert!(
        rules(&findings, 1).iter().any(|f| f.subject.contains("page_ids")),
        "`Scene.page_ids` naming a Panel must be a rule-1 finding: {findings:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn finding_1_a_lettering_page_id_that_names_a_lettering_is_not_a_page_and_must_be_reported() {
    // The same hole from the other side: `Lettering.page_id` is declared as a `PageId` (§6.4) and
    // no rule cross-checks it against the containing file, so a dangling one is only visible to
    // rule 1 — which must therefore not accept a lettering id in its place.
    let (root, mut bundle) = loaded_fixture("r1-lettering-page-id");
    {
        let file = bundle.pages.get_mut(&PageId(8)).expect("page 8");
        file.lettering[0].page_id = PageId(14);
    }
    let document = document_facts();
    let input = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &input);
    assert!(
        rules(&findings, 1).iter().any(|f| f.subject.contains("Lettering(14)") && f.subject.contains("page_id")),
        "`Lettering.page_id` naming a Lettering must be a rule-1 finding: {findings:?}"
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// Finding 2 — two page files for one PageId are collapsed silently.
// -------------------------------------------------------------------------------------------------

#[test]
fn finding_2_two_page_files_for_one_page_id_must_not_be_silently_collapsed() {
    // §3: "padding is cosmetic and the loader must not depend on it", and rule 13 parses the
    // number, so `page-8.json` and `page-008.json` both mean page 8. §13 rule 2 forbids a duplicate
    // id within an entity kind, and §11 says a migration must "validate its input and refuse rather
    // than guess". Two files for one page is ambiguous input; the loader currently inserts both
    // under `PageId(8)` and the second silently wins, and when the manifest lists both files (a
    // coherent, hand-edited bundle) nothing at all is reported.
    let root = scratch("r1-duplicate-page-file");
    copy_tree(&fixture_root(), &root);

    let duplicate = fs::read(root.join("pages/page-8.json")).expect("readable");
    fs::write(root.join("pages/page-008.json"), &duplicate).expect("write the duplicate");

    // Keep the manifest coherent, so the §14 `files` mismatch cannot be what reports it.
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("manifest json");
    manifest["files"]["pages/page-008.json"] = serde_json::json!({ "sha256": sha256_hex(&duplicate), "bytes": duplicate.len() as u64 });
    fs::write(root.join("manifest.json"), serde_json::to_vec_pretty(&manifest).expect("json")).expect("write manifest");

    let loaded = load(&root).expect("the ambiguous bundle loads");
    assert_eq!(loaded.bundle.pages.len(), 1, "one page id");
    assert!(
        !loaded.findings.is_empty(),
        "two files encoding the same page id must be reported (rule 2), not collapsed silently: {:?}",
        loaded.findings
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// The save order (§10) — the crash windows, walked one at a time.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_crash_between_step_4_and_step_5_is_survivable_and_reports_the_mismatch() {
    // §10: step 4 replaces the content files and opens the window; step 5 writes the manifest. A
    // reader landing in the window must see the *new* content, must not refuse to load, and must
    // report the `files`/tree mismatch (§14). This simulates the window exactly: new content on
    // disk, the previous manifest untouched.
    let (root, mut bundle) = loaded_fixture("r1-crash-window");
    bundle.pages.get_mut(&PageId(8)).expect("page 8").page.status = PageStatus::Generated;
    let new_generation = to_canonical_bytes("a page file", &bundle.pages[&PageId(8)]).expect("serializes");
    fs::write(root.join("pages/page-8.json"), &new_generation).expect("step 4");

    let loaded = load(&root).expect("the window is survivable: §14 makes a mismatch a finding, not a refusal");
    assert_eq!(loaded.bundle.pages[&PageId(8)].page.status, PageStatus::Generated, "a reader in the window sees the new generation");
    assert!(
        loaded.findings.iter().any(|f| f.subject == "pages/page-8.json"),
        "the mismatch between the tree and the previous manifest must be reported: {:?}",
        loaded.findings
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_crash_before_step_4_leaves_the_previous_generation_untouched() {
    // §10: "A crash before step 4 leaves the previous manifest — and therefore the previous
    // consistent project — intact." Staging is written first, so a crash during steps 1–3 leaves
    // `.save-tmp/` behind; the load must ignore it entirely (§4 excludes it from `files`).
    let (root, _) = loaded_fixture("r1-crash-before-step-4");
    fs::create_dir_all(root.join(".save-tmp/story")).expect("staging");
    fs::write(root.join(".save-tmp/project.json"), b"{ not the project }").expect("staging");
    fs::write(root.join(".save-tmp/manifest.json"), b"{ not the manifest }").expect("staging");

    let loaded = load(&root).expect("staging is never read as input (§4)");
    assert_eq!(loaded.bundle.project.title, "Minimal v1 fixture");
    assert!(loaded.findings.is_empty(), "`.save-tmp/` is excluded from `files`, so it is not a mismatch: {:?}", loaded.findings);
    assert!(!loaded.files.keys().any(|k| k.starts_with(".save-tmp/")), "staging is never listed");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_truncated_manifest_is_the_one_commit_point_that_is_fatal() {
    // §10: "This is the commit point: it is the only file whose truncation can make the bundle
    // inconsistent." A half-written manifest must be a hard failure, not a half-loaded project.
    let (root, _) = loaded_fixture("r1-truncated-manifest");
    fs::write(root.join("manifest.json"), b"{\n  \"schema_version\": 1,\n").expect("truncate");
    match load(&root) {
        Err(Error::Parse { .. }) => {}
        other => panic!("a truncated manifest must fail the load, got {other:?}"),
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_window_between_the_prune_and_the_manifest_write_is_survivable() {
    // §10's window is not only "new content under the old manifest". Step 4 also *removes* the
    // files the new generation no longer has, and that prune happens before the manifest is
    // rewritten — so a reader landing between the two sees a manifest naming a file the tree no
    // longer has. §14 says that is reported and the load continues, and it is the same window the
    // schema describes, so it must not become a refusal.
    let (root, _) = loaded_fixture("r1-prune-window");
    fs::remove_file(root.join("pages/page-8.json")).expect("the file the prune would have removed");

    let loaded = load(&root).expect("the load continues through the window (§14)");
    let finding = loaded.findings.iter().find(|f| f.subject == "pages/page-8.json").expect("the missing declared file is reported");
    // Recorded as a preference in the round-1 report: this half of the §14 mismatch is reported as
    // `Severity::Error`, while the "unlisted file" half is a `Warning`, although §14 treats the
    // window as one benign, expected cause.
    assert!(finding.severity >= Severity::Warning, "the mismatch is reported at some severity: {finding:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_prune_step_removes_a_deleted_page_and_leaves_assets_alone() {
    // §10 step 4 replaces "the content files"; a page that is gone from the model must not keep a
    // file, or `manifest.files` would describe a page the project no longer has. The prune must
    // stop at `assets/`: an asset no page currently references is still an asset (content-addressed,
    // and deleting one would destroy approved artwork).
    let (root, mut bundle) = loaded_fixture("r1-prune");
    let orphan = root.join("assets/panels/orphan.png");
    fs::write(&orphan, b"\x89PNG orphan").expect("orphan asset");

    bundle.pages.remove(&PageId(8));
    save(&mut bundle, &root, &options()).expect("saves");

    assert!(!root.join("pages/page-8.json").exists(), "a deleted page's file must be pruned");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    let files = manifest["files"].as_object().expect("a files map");
    assert!(!files.contains_key("pages/page-8.json"), "the manifest must not describe a page whose file is gone");
    assert!(orphan.exists(), "an unreferenced asset is still an asset and must not be pruned");
    assert!(files.contains_key("assets/panels/orphan.png"), "and it must be listed: {files:?}");
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// Rule 9 — the three bases.
// -------------------------------------------------------------------------------------------------

#[test]
fn rule_9_embedded_is_a_key_not_a_path() {
    // §7.1: `Embedded` "is a key in the visual document's image map, for portability" — not a path
    // at all, so there is nothing to confine and rule 9 must say nothing about it.
    let (root, mut bundle) = loaded_fixture("r1-embedded");
    bundle.assets.assets[0].storage_reference = StorageRef::Embedded { image_key: "panel-1".into() };
    let document = document_facts();
    let input = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &input);
    assert!(rules(&findings, 1).is_empty(), "a key is not a reference to a file: {findings:?}");
    assert!(rules(&findings, 9).is_empty(), "an `Embedded` key is not a path and rule 9 has nothing to resolve: {findings:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rule_9_package_resolves_against_the_archive_root_and_nothing_else() {
    // §7.1 and rule 9: `Package`'s base is the **archive root**, "not the bundle root: a packaged
    // archive is a different container". Resolving it against the bundle root is the
    // path-traversal bug rule 9 exists to prevent, so the two outcomes must be distinguishable:
    // with an archive root the reference resolves, without one it cannot resolve at all.
    let (root, mut bundle) = loaded_fixture("r1-package");
    let archive = root.join("archive");
    fs::create_dir_all(&archive).expect("archive dir");
    let bytes = b"packaged bytes";
    fs::write(archive.join("a.png"), bytes).expect("archive file");

    bundle.assets.assets[0].storage_reference = StorageRef::Package { relative_path: "a.png".into() };
    bundle.assets.assets[0].content_hash = ContentHash { algorithm: "sha256".into(), value: sha256_hex(bytes) };
    let document = document_facts();

    let with_archive = ValidationInput { root: &root, document: &document, archive_root: Some(&archive) };
    assert!(rules(&validate(&bundle, &with_archive), 9).is_empty(), "against its own base it resolves");

    let without_archive = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &without_archive);
    assert!(
        rules(&findings, 9).iter().any(|f| f.message.contains("archive root")),
        "with no archive root a `Package` reference cannot resolve: {findings:?}"
    );

    // And a `Package` path that leaves the archive root is never resolved, exactly as a `Linked`
    // one that leaves the bundle root is not.
    bundle.assets.assets[0].storage_reference = StorageRef::Package { relative_path: "../../outside.png".into() };
    let findings = validate(&bundle, &with_archive);
    assert!(
        rules(&findings, 9).iter().any(|f| f.message.contains("escapes the archive root")),
        "escaping the archive root is a rule-9 finding: {findings:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[cfg(unix)]
#[test]
fn rule_9_reports_a_symlink_that_leaves_the_bundle() {
    // §13.1: "Canonicalising *before* the check defeats `..` and symlinks; a `..` denylist would
    // not." The unit test for `resolve_path` covers this; rule 9's own behaviour — the asset is
    // reported unresolved and the rest of the validation continues — is what a caller sees.
    let (root, mut bundle) = loaded_fixture("r1-symlink");
    let outside = root.parent().expect("a parent").join("r1-symlink-outside.png");
    fs::write(&outside, b"secret").expect("outside file");
    std::os::unix::fs::symlink(&outside, root.join("assets/panels/link.png")).expect("symlink");

    bundle.assets.assets[0].storage_reference = StorageRef::Linked { relative_path: "assets/panels/link.png".into() };
    let document = document_facts();
    let input = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &input);
    assert!(
        rules(&findings, 9).iter().any(|f| f.message.contains("escapes the bundle root")),
        "a symlink out of the bundle is an escape, not a resolvable asset: {findings:?}"
    );
    assert!(
        !rules(&findings, 9).iter().any(|f| f.message.contains("hashes")),
        "an escaping path is **never resolved**, so it is never read and never hashed: {findings:?}"
    );
    fs::remove_dir_all(&root).ok();
    fs::remove_file(&outside).ok();
}

// -------------------------------------------------------------------------------------------------
// Rule 3, and the finding subjects.
// -------------------------------------------------------------------------------------------------

#[test]
fn rule_3_checks_the_version_every_jobs_file_declares() {
    // §13 rule 3 names `every jobs/job-<id>.json` (§8.3) among the files whose `schema_version`
    // must equal the manifest's. §8.3 makes this the reason the wrapper exists at all.
    let (root, mut bundle) = loaded_fixture("r1-job-version");
    bundle.jobs.get_mut(&JobId(18)).expect("job 18").schema_version = 2;
    let document = document_facts();
    let input = ValidationInput { root: &root, document: &document, archive_root: None };
    let findings = validate(&bundle, &input);
    assert!(
        rules(&findings, 3).iter().any(|f| f.subject.starts_with("jobs/")),
        "a `jobs/` file declaring another version must be reported by rule 3: {findings:?}"
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// The fixture's fidelity, and the writer's canonical form.
// -------------------------------------------------------------------------------------------------

/// Every non-empty `unknown` map in a loaded bundle, as `(what it belongs to, the keys it holds)`.
fn unknown_sites(bundle: &ComicBundle) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut note = |what: String, unknown: &Unknown| {
        if !unknown.is_empty() {
            out.push((what, unknown.keys().cloned().collect()));
        }
    };
    note("manifest".into(), &bundle.manifest.unknown);
    note("manifest.document".into(), &bundle.manifest.document.unknown);
    note("manifest.app".into(), &bundle.manifest.app.unknown);
    for (key, entry) in &bundle.manifest.files {
        note(format!("manifest.files[{key}]"), &entry.unknown);
    }
    note("project".into(), &bundle.project.unknown);
    note("project.metadata".into(), &bundle.project.metadata.unknown);
    note("project.settings".into(), &bundle.project.settings.unknown);
    note("project.visual_document".into(), &bundle.project.visual_document.unknown);
    note("story/chapters.json".into(), &bundle.chapters.unknown);
    for c in &bundle.chapters.chapters {
        note(format!("Chapter({})", c.id.0), &c.unknown);
    }
    note("story/scenes.json".into(), &bundle.scenes.unknown);
    for s in &bundle.scenes.scenes {
        note(format!("Scene({})", s.id.0), &s.unknown);
    }
    note("story/characters.json".into(), &bundle.characters.unknown);
    for c in &bundle.characters.characters {
        note(format!("Character({})", c.id.0), &c.unknown);
    }
    note("story/locations.json".into(), &bundle.locations.unknown);
    for l in &bundle.locations.locations {
        note(format!("Location({})", l.id.0), &l.unknown);
    }
    note("story/styles.json".into(), &bundle.styles.unknown);
    for s in &bundle.styles.styles {
        note(format!("StyleProfile({})", s.id.0), &s.unknown);
    }
    note("assets/index.json".into(), &bundle.assets.unknown);
    for a in &bundle.assets.assets {
        note(format!("Asset({})", a.id.0), &a.unknown);
        if let Some(p) = &a.provenance {
            note(format!("Asset({}).provenance", a.id.0), &p.unknown);
            if let Some(c) = &p.consent {
                note(format!("Asset({}).provenance.consent", a.id.0), &c.unknown);
            }
        }
    }
    note("provenance/generations.json".into(), &bundle.provenance.unknown);
    for j in &bundle.provenance.jobs {
        note(format!("GenerationJob({})", j.id.0), &j.unknown);
        if let Some(p) = &j.progress {
            note(format!("GenerationJob({}).progress", j.id.0), &p.unknown);
        }
        if let Some(e) = &j.error {
            note(format!("GenerationJob({}).error", j.id.0), &e.unknown);
        }
        if let Some(c) = &j.consent {
            note(format!("GenerationJob({}).consent", j.id.0), &c.unknown);
        }
    }
    for d in &bundle.provenance.decisions {
        note(format!("ReviewDecision({})", d.id.0), &d.unknown);
    }
    for (id, file) in &bundle.pages {
        note(format!("pages/page-{}.json", id.0), &file.unknown);
        note(format!("Page({})", file.page.id.0), &file.page.unknown);
        for p in &file.panels {
            note(format!("Panel({})", p.id.0), &p.unknown);
        }
        for d in &file.dialogue {
            note(format!("Dialogue({})", d.id.0), &d.unknown);
        }
        for l in &file.lettering {
            note(format!("Lettering({})", l.id.0), &l.unknown);
        }
    }
    for (id, file) in &bundle.jobs {
        note(format!("jobs/job-{}.json", id.0), &file.unknown);
        note(format!("jobs GenerationJob({})", file.job.id.0), &file.job.unknown);
    }
    out.sort();
    out
}

#[test]
fn the_fixture_has_no_unintended_unknown_keys() {
    // The round trip cannot catch a fixture whose key spelling differs from the schema: a
    // misspelled field lands in `unknown` and is written straight back out, byte-identically. So
    // the fixture's only unknown keys must be the two it intends (the chapter's `pov_character`
    // and `revision_note`); anything else means a key the frozen schema names is being carried as
    // an unknown field, and no test would notice.
    let (root, bundle) = loaded_fixture("r1-fixture-unknown-keys");
    let expected: Vec<(String, Vec<String>)> = vec![("Chapter(3)".to_string(), vec!["pov_character".to_string(), "revision_note".to_string()])];
    assert_eq!(unknown_sites(&bundle), expected, "the fixture's only unknown keys are the chapter's two");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_generated_asset_with_provenance_and_consent_round_trips() {
    // The fixture carries one `source` asset and one job, so `Provenance` — a §5.5 shared type and
    // the whole point of §8 — is never exercised by it, and `Consent` only inside the job. §5.6
    // adds `Asset.provenance` for a reason: it must survive a save. The `request` map is the §5.7
    // exception whose keys are written **sorted**, which a fixture with a one-key map cannot show.
    let (root, mut bundle) = loaded_fixture("r1-provenance");
    bundle.assets.assets.push(Asset {
        id: vectorcraft_comic_model::AssetId(21),
        asset_type: AssetType::Generated,
        storage_reference: StorageRef::Linked { relative_path: "assets/panels/panel-1.png".into() },
        content_hash: ContentHash {
            algorithm: "sha256".into(),
            value: sha256_hex(&fs::read(root.join("assets/panels/panel-1.png")).expect("readable")),
        },
        media_metadata: vectorcraft_comic_model::MediaMetadata {
            mime: "image/png".into(),
            width: 8,
            height: 8,
            color_profile: Some("sRGB".into()),
            dpi: Some(72.0),
        },
        provenance: Some(Provenance {
            provider: ProviderId("local-diffusion".into()),
            model: "example-v1".into(),
            request: serde_json::json!({ "z_steps": 30, "a_prompt": "rooftop" }),
            input_asset_ids: vec![vectorcraft_comic_model::AssetId(17)],
            created: FIXTURE_UPDATED.into(),
            consent: Some(Consent { granted: FIXTURE_UPDATED.into(), statement: "Send it.".into(), unknown: Unknown::new() }),
            unknown: Unknown::new(),
        }),
        parent_asset_ids: vec![vectorcraft_comic_model::AssetId(17)],
        approval_status: vectorcraft_comic_model::ApprovalStatus::Pending,
        unknown: Unknown::new(),
    });

    save(&mut bundle, &root, &options()).expect("saves");
    let written = fs::read_to_string(root.join("assets/index.json")).expect("readable");
    let (a, z) = (written.find("\"a_prompt\"").expect("the request key"), written.find("\"z_steps\"").expect("the request key"));
    assert!(a < z, "a map-valued field's keys are written sorted (§5.7): {written}");
    assert!(written.contains("\"consent\""), "the consent record must be written: {written}");

    let reloaded = load(&root).expect("reloads");
    let asset = reloaded.bundle.assets.assets.iter().find(|a| a.id.0 == 21).expect("the generated asset");
    let provenance = asset.provenance.as_ref().expect("provenance survives a round trip");
    assert_eq!(provenance.provider, ProviderId("local-diffusion".into()));
    assert_eq!(provenance.request["a_prompt"], serde_json::json!("rooftop"));
    assert_eq!(provenance.consent.as_ref().map(|c| c.granted.as_str()), Some(FIXTURE_UPDATED), "the consent survives too");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn finding_3_the_writer_must_not_emit_a_non_canonical_timestamp() {
    // §5.7: "A save writes … Timestamps in the §5.5 canonical form." §5.5 fixes one form — RFC 3339
    // UTC, second precision, `Z` — "so a save/reload is byte-identical". Only `SaveOptions.now` is
    // checked (`is_canonical_timestamp` has one call site), so every other `Timestamp` in the model
    // — `manifest.created`, `ProjectMetadata.created/updated`, `Provenance.created`,
    // `Consent.granted`, `ReviewDecision.timestamp` — is written exactly as it was loaded. A
    // hand-edited or imported bundle therefore saves a non-canonical timestamp and the writer
    // silently violates the form it is supposed to produce.
    let (root, mut bundle) = loaded_fixture("r1-non-canonical-timestamp");
    bundle.manifest.created = "2026-10-11 06:00:00".into();

    if save(&mut bundle, &root, &options()).is_err() {
        fs::remove_dir_all(&root).ok();
        return;
    }
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    let created = manifest["created"].as_str().expect("a string");
    assert!(
        vectorcraft_comic_model::is_canonical_timestamp(created),
        "a save must write the one canonical timestamp form (§5.5, §5.7); it wrote {created:?}"
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// The save's error path.
// -------------------------------------------------------------------------------------------------

#[test]
fn finding_4_a_refused_save_does_not_leave_the_model_mutated() {
    // `save` derives the manifest's copies of `project.json`'s fields and overwrites `updated`
    // *before* it can fail. A caller that gets an error back must be able to retry with the model
    // it still holds; here the failure (a document path that is not bundle-relative) leaves
    // `project.visual_document.relative_path` — and the timestamps — rewritten in memory, and a
    // `.save-tmp/` directory behind on disk.
    let (root, mut bundle) = loaded_fixture("r1-refused-save");
    let before = bundle.project.visual_document.clone();

    let payload = DocumentPayload { relative_path: "../escape.vectorcraft".into(), format_version: 4, bytes: b"document".to_vec() };
    let result = save(&mut bundle, &root, &SaveOptions { now: FIXTURE_UPDATED.to_string(), document: Some(payload) });
    assert!(result.is_err(), "a document path that leaves the bundle must be refused");
    assert_eq!(bundle.project.visual_document, before, "a refused save must leave the model as it found it");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn finding_4_a_refused_save_cleans_up_its_staging_directory() {
    // The second half of the same error path: `.save-tmp/` is created before anything can fail and
    // is removed only on success, so a refused save leaves the previous run's staged files behind
    // for the next one to find.
    let (root, mut bundle) = loaded_fixture("r1-refused-save-staging");
    let payload = DocumentPayload { relative_path: "../escape.vectorcraft".into(), format_version: 4, bytes: b"document".to_vec() };
    assert!(
        save(&mut bundle, &root, &SaveOptions { now: FIXTURE_UPDATED.to_string(), document: Some(payload) }).is_err(),
        "a document path that leaves the bundle must be refused"
    );
    assert!(!root.join(".save-tmp").exists(), "a refused save must not leave its staging directory behind");
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// Finding 5 — a declared `unknown` map that no save can ever honour.
// -------------------------------------------------------------------------------------------------

#[test]
fn finding_5_an_unknown_key_in_a_manifest_file_entry_is_discarded_by_a_save() {
    // §12: a field a newer ComicCraft wrote must survive a load-and-save by an older one, and §11
    // states the same principle for migrations ("never silently discard them"). `FileEntry`
    // declares a flattened `unknown` (entities.rs), which is that intent — but `save` rebuilds the
    // whole `files` map from `scan_tree`, and `scan_tree` constructs every entry with an empty map,
    // so the key is dropped without a word. Either the field is a mistake (then `FileEntry` is a
    // derived value type like `ContentHash`, which carries none) or the save must preserve it; as
    // it stands the field cannot round-trip.
    let (root, mut bundle) = loaded_fixture("r1-file-entry-unknown");
    bundle.manifest.files.get_mut("project.json").expect("the entry").unknown.insert("mtime".into(), serde_json::json!(1));
    save(&mut bundle, &root, &options()).expect("saves");

    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("readable")).expect("json");
    assert_eq!(
        manifest["files"]["project.json"]["mtime"],
        serde_json::json!(1),
        "an `unknown` key on a `FileEntry` must survive a save, or the field should not exist"
    );
    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------------------------------------------------
// A guard against my own tests passing vacuously.
// -------------------------------------------------------------------------------------------------

#[test]
fn the_reviewers_own_harness_sees_the_fixture_it_thinks_it_does() {
    let (root, bundle) = loaded_fixture("r1-harness-sanity");
    assert_eq!(bundle.pages.len(), 1);
    assert_eq!(bundle.assets.assets.len(), 1);
    assert_eq!(bundle.jobs.len(), 1);
    let findings = validate(&bundle, &ValidationInput { root: &root, document: &document_facts(), archive_root: None });
    assert!(findings.is_empty(), "the fixture must be clean before a test mutates it: {findings:?}");
    assert_eq!(unknown_sites(&bundle).len(), 1, "the fixture's unknown-key set is stable");
    assert!(Severity::Error > Severity::Warning);
    let _: BTreeMap<String, FileEntry> = load(&root).expect("loads").files;
    fs::remove_dir_all(&root).ok();
}
