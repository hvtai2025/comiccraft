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
    // Design §4.3 rule 1 and §4.2: the arrow points one way, into the model. `vectorcraft-doc`
    // and `vectorcraft-format` are the visual document; `engine`, `ui-egui` and `mcp` sit above
    // the command layer. Design §9.3 also forbids `comic-ai` knowing about the document at all.
    let forbidden = ["vectorcraft-doc", "vectorcraft-format", "vectorcraft-engine", "vectorcraft-ui-egui", "vectorcraft-mcp"];
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name);
        for table in ["dependencies", "dev-dependencies"] {
            for declared in declared_dependencies(&text, table) {
                assert!(
                    !forbidden.contains(&declared.as_str()),
                    "crates/{crate_name} declares {declared} under [{table}]; design §4.3 forbids a comic crate reaching the document or the command layer"
                );
            }
        }
    }
}

#[test]
fn no_comic_crate_uses_a_document_type_in_its_code() {
    // The same rule from the other side: a dependency check cannot see comic metadata smuggled
    // into the document through a type that happens to already be in scope. Schema §6.2 gives
    // `Panel` only `frame_node_id` — geometry belongs to the document object, and a comic record
    // must reach it by id alone, never by holding a `Document`/`Artboard`/`NodeId`.
    for crate_name in COMIC_CRATES {
        let code = code_without_comments(&source(crate_name));
        for forbidden in ["Document", "Artboard", "NodeId", "document", "artboard", "node_id"] {
            assert!(
                !code.contains(forbidden),
                "crates/{crate_name}/src/lib.rs refers to `{forbidden}` in code; design §6.1 forbids comic records holding a document type"
            );
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
    // both `[dependencies]` and `[dev-dependencies]`. The trimmed manifests declare exactly
    // `serde`; anything more must arrive with the module that needs it, so that `cargo xtask
    // layers` reports an edge that really exists.
    for crate_name in COMIC_CRATES {
        let text = manifest(crate_name);
        let prod = declared_dependencies(&text, "dependencies");
        let dev = declared_dependencies(&text, "dev-dependencies");
        assert_eq!(prod, vec!["serde".to_string()], "crates/{crate_name} must declare only the dependencies its code uses");
        assert_eq!(dev, vec!["serde_json".to_string()], "crates/{crate_name} must declare serde_json for tests only");
        // The other half of finding 5: trimming must not remove something the code needs. If
        // `serde` were dropped the crate would not compile at all, so asserting it is present is
        // the load-bearing half of this check.
        assert!(text.contains("serde = { workspace = true }"), "crates/{crate_name} must still declare serde");
        assert!(text.contains("serde_json = { workspace = true }"), "crates/{crate_name} must still declare serde_json for its tests");
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
