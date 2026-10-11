//! Reviewer tests (`code-reviewer`, M0 round 2) for the **hard rules** and the dependency trim.
//!
//! Round 1 checked these rules by reading; a rule that is only ever checked by reading regresses
//! silently. These tests make design §4.3 and §6.1, and round 1's finding 5 (unused dependencies),
//! into assertions that fail when the code drifts back.
//!
//! They are deliberately structural rather than behavioural: they read the three comic crates'
//! manifests and sources from disk, so they fail on a manifest edit or on a source line that
//! reaches for the visual document.

// The repository's policy (clippy.toml header) is that tests may panic; clippy only applies it to
// `#[cfg(test)]` modules, not to an integration-test crate, so this is the file-level equivalent.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// The three crates under review, by directory name.
const COMIC_CRATES: [&str; 3] = ["comic-model", "comic-ai", "comic-layout"];

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/crates/comic-layout`.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository root must be readable")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()))
}

fn manifest(crate_name: &str) -> String {
    read(&repo_root().join("crates").join(crate_name).join("Cargo.toml"))
}

fn source(crate_name: &str) -> String {
    read(&repo_root().join("crates").join(crate_name).join("src").join("lib.rs"))
}

/// Every `.rs` file under the crate's `src/`, as `(path, text)`, sorted by path.
///
/// M0's crates kept everything in `lib.rs`, so a rule checked there was a rule checked everywhere.
/// A crate with modules needs the whole tree read, or a rule silently stops covering the code that
/// moved out of `lib.rs`.
fn sources(crate_name: &str) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root().join("crates").join(crate_name).join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push((path.clone(), read(&path)));
            }
        }
    }
    out.sort();
    out
}

/// The crate's entity definitions, where a record-side rule lives: `src/entities.rs` when the
/// crate has one (it holds the comic records), else `src/lib.rs`, which is where M0's crates kept
/// everything they had.
fn entity_source(crate_name: &str) -> String {
    let dir = repo_root().join("crates").join(crate_name).join("src");
    let entities = dir.join("entities.rs");
    if entities.is_file() { read(&entities) } else { read(&dir.join("lib.rs")) }
}

/// The modules allowed to name the visual document, by file name.
///
/// `document.rs` is the boundary module `crates/comic-model` exists to have: `DocumentFacts` is the
/// *sanctioned* reach — the artboards and node ids the referential rules are handed instead of
/// reading `crates/format`, which sits at L3 and is off-limits to an L2 crate. Everything else must
/// reach the document by id (`NodeId`, `artboard_id`) and by nothing else.
const DOCUMENT_BOUNDARY_MODULES: &[&str] = &["document.rs"];

/// Every dependency name declared under one table header of a manifest.
///
/// A deliberately small reader rather than a TOML dependency: adding one would edit a
/// `Cargo.toml`, and this test file may edit nothing but tests.
fn declared_dependencies(manifest: &str, table: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_table = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == format!("[{table}]");
            continue;
        }
        if !in_table || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, _)) = line.split_once('=') else { continue };
        let name = name.trim();
        // A commented-out entry reads as a comment line and was skipped above; a bare key with
        // no `= value` is not a dependency and is skipped here.
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            names.push(name.to_string());
        }
    }
    names
}

/// The package name of every workspace member, read from `crates/*` and `apps/*`.
fn workspace_package_names() -> Vec<String> {
    let root = repo_root();
    let mut names = Vec::new();
    for group in ["crates", "apps"] {
        let Ok(entries) = fs::read_dir(root.join(group)) else { continue };
        for entry in entries.flatten() {
            let candidate = entry.path().join("Cargo.toml");
            if !candidate.is_file() {
                continue;
            }
            let text = read(&candidate);
            if let Some(name) = text.lines().find_map(|l| l.trim().strip_prefix("name = \"").and_then(|n| n.strip_suffix('"'))) {
                names.push(name.to_string());
            }
        }
    }
    assert!(!names.is_empty(), "no workspace packages found — the scan must not pass vacuously");
    names
}

/// The crate's own code with every `//`-prefixed line removed, so a rule that is *discussed* in a
/// doc comment is not mistaken for a rule that is *used* in code.
fn code_without_comments(src: &str) -> String {
    src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

#[test]
fn no_comic_crate_declares_a_dependency_on_the_visual_document_or_anything_above_the_command_layer() {
    // Design §4.3 rule 1 and §4.2: the arrow points one way, into the model. `vectorcraft-format`
    // is the document's own format, and `engine`, `ui-egui` and `mcp` sit above the command layer.
    //
    // `vectorcraft-doc` is not forbidden, and never was by the design: §4.2's table gives it to
    // `comic-model` (for `NodeId`) and to `comic-layout` (for `NodeId` **and** the text-object
    // types `vectorcraft-text::layout_with` takes — `TextObject`, `TextKind::Area`, `CharStyle`,
    // `ParaStyle`, `AreaFit`, which `text` does not re-export). What this test enforces is the
    // ban below: a comic crate must not hold the document, an artboard or frame geometry as
    // values. Design §9.3 still forbids `comic-ai` the document entirely, so for that crate the
    // dependency itself is banned.
    let forbidden = ["vectorcraft-format", "vectorcraft-engine", "vectorcraft-ui-egui", "vectorcraft-mcp"];
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name);
        for table in ["dependencies", "dev-dependencies"] {
            for declared in declared_dependencies(&text, table) {
                let comic_ai_reaches_the_document = declared == "vectorcraft-doc" && crate_name == "comic-ai";
                assert!(
                    !forbidden.contains(&declared.as_str()) && !comic_ai_reaches_the_document,
                    "crates/{crate_name} declares {declared} under [{table}]; design §4.3 forbids a comic crate reaching the document or the command layer, and §9.3 forbids `comic-ai` the document entirely"
                );
            }
        }
    }
}

#[test]
fn no_comic_crate_uses_a_document_type_in_its_code() {
    // The same rule from the other side: a dependency check cannot see comic metadata smuggled
    // into the document through a type that happens to already be in scope.
    //
    // Schema §6.2 gives `Panel` `frame_node_id: NodeId` and nothing else about the frame: geometry
    // belongs to the document object, and a comic record reaches it **by id**. §9 makes those ids
    // the one thing the model takes from the engine, so `NodeId` and `artboard_id` are the
    // spellings the schema mandates — and `VisualDocumentRef` is a *reference* (a path and a
    // version), not the document. What is banned is the document and the artboard as values, and
    // the geometry the design removed from `Panel` (design §5.2).
    //
    // **Every source file is read, not just the entity module.** Reading one file left a record
    // that moved into a new module ungoverned, so the ban now walks the whole `src/` tree.
    //
    // What it covers is the **dotted** path into the doc crate, in any file that is not the
    // boundary module — `vectorcraft_doc::Document`, `vectorcraft_doc::Artboard` — and that
    // includes the `as`-item form (`use vectorcraft_doc::Artboard as Art;`), because the alias's
    // source path is still in the text. What it does **not** cover, and cannot: the braced form
    // (`use vectorcraft_doc::{Document, NodeId};`), which never repeats the crate name next to the
    // item, and a crate alias (`use vectorcraft_doc as vd;` … `vd::Document`), which hides every
    // later use from any path-based rule. Both are the token-level test beside this one
    // (`no_comic_crate_reaches_a_document_type_by_a_brace_a_rename_or_a_space`), which is the ban
    // this rule actually needs; this one keeps the record-side token ban and `geometry_ref`.
    //
    // Residual, recorded rather than hidden: the exemption is by **file name**, so a future
    // `src/x/document.rs` inherits it, and `document.rs` itself could launder the crate with
    // `pub use vectorcraft_doc as vd;` — after which no textual rule can see `vd::Document`
    // anywhere. Closing that needs a review-time rule, not a test; the exemption is kept because
    // the boundary module is the one file whose job *is* the doc crate.
    let reaches_the_document = ["vectorcraft_doc::Document", "vectorcraft_doc::Artboard", "vectorcraft_doc::Rect", "vectorcraft_doc::ImageBlob"];
    let held_by_a_record = ["DocumentFacts", "ArtboardFacts", "geometry_ref"];
    for crate_name in COMIC_CRATES {
        for (path, text) in sources(crate_name) {
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if DOCUMENT_BOUNDARY_MODULES.contains(&file) {
                continue;
            }
            let code = code_without_comments(&text);
            for forbidden in reaches_the_document {
                assert!(
                    !code.contains(forbidden),
                    "crates/{crate_name}/{file} reaches `{forbidden}`; schema §9 lets a comic crate take `NodeId` from the engine and nothing else of the document"
                );
            }
        }
        // The records themselves hold no facts about the document either — they reach it by id.
        let records = code_without_comments(&entity_source(crate_name));
        for forbidden in held_by_a_record {
            assert!(
                !records.contains(forbidden),
                "crates/{crate_name}'s entity definitions name `{forbidden}` in code; design §6.1 forbids a comic record holding the document, an artboard or frame geometry"
            );
        }
    }
}

#[test]
fn no_comic_crate_reaches_a_document_type_by_a_brace_a_rename_or_a_space() {
    // The ban in the test above is a substring test on the **fully qualified dotted path**, so the
    // two most natural ways to write the violation walk straight past it:
    //
    //     use vectorcraft_doc::{Artboard, Document};      // no `vectorcraft_doc::Document` substring
    //     use vectorcraft_doc as vd;  ... vd::Document    // the crate name is never repeated
    //
    // and `use vectorcraft_doc :: Document;` is the same text with whitespace. `document.rs` stays
    // the one exempt boundary module; everywhere else the doc crate may be named only for an
    // identity or a text type (`NodeId`, `CharStyle`, …), never for the document itself, an
    // artboard, a frame rect or an image blob. The check is token-level rather than substring-level,
    // so an import's braces, spacing and order cannot hide an item, and an alias of the crate is
    // refused outright because it hides every later use from any path-based rule.
    let banned = ["Document", "Artboard", "ImageBlob", "Rect"];
    for crate_name in COMIC_CRATES {
        for (path, text) in sources(crate_name) {
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if DOCUMENT_BOUNDARY_MODULES.contains(&file) {
                continue;
            }
            for (index, line) in code_without_comments(&text).lines().enumerate() {
                if !line.contains("vectorcraft_doc") {
                    continue;
                }
                let words: Vec<&str> = line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
                for bad in banned {
                    assert!(
                        !words.contains(&bad),
                        "crates/{crate_name}/{file}:{} names `{bad}` from the doc crate; a braced import must not evade the path ban",
                        index + 1
                    );
                }
                assert!(
                    !words.contains(&"as"),
                    "crates/{crate_name}/{file}:{} aliases the doc crate; an alias hides every later use from the path ban",
                    index + 1
                );
            }
        }
    }
}

#[test]
fn no_comic_crate_owns_a_second_copy_of_the_page_size_or_a_dialogue() {
    // Design §6.1 rule 2: the artboard owns the page size, the document object owns the frame
    // geometry, a comic record owns the id and the meaning. A `page_width`/`page_size` field or a
    // dialogue-text field on a *record* would be a second authority. `PanelRect` is a computed
    // layout primitive and not a record, so only the record-side spellings are banned here.
    let banned = ["page_width", "page_height", "page_size", "dialogue_text", "balloon_text", "frame_rect"];
    for crate_name in COMIC_CRATES {
        let code = code_without_comments(&source(crate_name));
        for term in banned {
            assert!(!code.contains(term), "crates/{crate_name} declares `{term}`; design §6.1 allows one owner for page size and one for dialogue");
        }
    }
}

#[test]
fn each_comic_manifest_declares_only_what_its_code_uses() {
    // Round 1 finding 5: 13 of 18 declared dependencies were unused, and `serde_json` sat in
    // both `[dependencies]` and `[dev-dependencies]`. The rule that finding established is
    // **declared implies used** — not the literal list M0's empty crates happened to have, which
    // is why this no longer pins `vec!["serde"]`: each milestone brings the dependency its module
    // needs (M1 brings `vectorcraft-doc`, `vectorcraft-geom`, `thiserror` and `sha2` to
    // `comic-model`), and what must stay true is that nothing is declared that no code names.
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name);
        let prod = declared_dependencies(&text, "dependencies");
        let dev = declared_dependencies(&text, "dev-dependencies");
        assert!(prod.contains(&"serde".to_string()), "crates/{crate_name} must declare serde");
        assert!(dev.contains(&"serde_json".to_string()), "crates/{crate_name} must declare serde_json for its tests");
        assert!(text.contains("serde = { workspace = true }"), "crates/{crate_name} must still declare serde");
        assert!(text.contains("serde_json = { workspace = true }"), "crates/{crate_name} must still declare serde_json for its tests");

        // Every production dependency must be one the crate's own source names, so `cargo xtask
        // layers` reports only edges that really exist. `serde_json` is exempt: it is a
        // dev-dependency here and its users are in `tests/`, not `src/`.
        let code: String = sources(crate_name).into_iter().map(|(_, text)| code_without_comments(&text)).collect::<Vec<_>>().join("\n");
        for declared in prod {
            let named = declared.replace('-', "_");
            assert!(
                code.contains(&named),
                "crates/{crate_name} declares `{declared}` but no source under src/ names `{named}`: an unused dependency makes `cargo xtask layers` report an edge that does not exist"
            );
        }
    }
}

#[test]
fn every_crate_a_manifest_comment_names_as_a_future_dependency_exists_in_the_workspace() {
    // The manifests name the dependency each milestone brings, so a reader can check the plan
    // against the tree. A comment naming a crate that does not exist would send the next owner
    // looking for something that was never planned — so every name must be a real member.
    let packages = workspace_package_names();
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name);
        let mut named = Vec::new();
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix('#') else { continue };
            for word in rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
                if word.starts_with("vectorcraft-") {
                    named.push(word.to_string());
                }
            }
        }
        assert!(!named.is_empty(), "crates/{crate_name} names no future dependency; the comment that documents the plan is missing");
        for name in named {
            assert!(packages.contains(&name), "crates/{crate_name} names `{name}` as a future dependency, but no workspace package has that name");
        }
    }
}

#[test]
fn no_comic_crate_declares_an_http_client() {
    // ADR-015: no networking dependency anywhere, direct or transitive, and adding one must be a
    // deliberate gate-visible decision. The lockfile audit was round 1's; this is the manifest
    // half, which is the part a future author actually edits.
    let clients = ["reqwest", "hyper", "ureq", "isahc", "curl", "surf", "tokio-tungstenite", "attohttpc", "rustls", "native-tls", "openssl", "tokio"];
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name).to_lowercase();
        for client in clients {
            assert!(
                !text.contains(client),
                "crates/{crate_name} mentions `{client}`; ADR-015 forbids a networking dependency without a gate-visible decision"
            );
        }
    }
}

#[test]
fn every_comic_crate_forbids_unsafe_code() {
    // Design/AGENTS.md: no `unsafe` in these crates. `forbid` rather than `deny` also stops a
    // later `#[allow(unsafe_code)]` from quietly re-opening it.
    for crate_name in COMIC_CRATES {
        assert!(source(crate_name).contains("#![forbid(unsafe_code)]"), "crates/{crate_name}/src/lib.rs must keep `#![forbid(unsafe_code)]`");
    }
}
