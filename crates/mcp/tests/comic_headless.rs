//! M1 acceptance test 12: **the same essential operations succeed through `mcp --headless`**.
//!
//! `vectorcraft-cli mcp --headless` is `Server::new(Box::new(Headless::with_document()))` reading
//! JSON-RPC 2.0 off stdio (`apps/vectorcraft-cli/src/main.rs::mcp`), so this file drives that exact
//! server through [`Server::handle_line`] — the same entry point `Server::serve` uses per line — with
//! `tools/call` → `run_command`, which is the path an agent takes. It then checks the *artefacts*
//! (the bundle on disk, `comic.project.validate`, `document.inspect`, the rendered PNG, the exported
//! PDF) rather than the replies, exactly as the engine-side acceptance tests do.
//!
//! Test 10 in `crates/engine/tests/comic_acceptance.rs` is the strict render/PDF geometry test; this
//! one proves the same outcomes are reachable *through MCP* with no engine back door.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::too_many_lines)]

use std::path::PathBuf;

use serde_json::{Value, json};
use vectorcraft_mcp::{Headless, PROTOCOL_VERSION, Server};

/// The `AssetId` the prepared bundle carries (`prepare_linked_asset`), and what the step below drives
/// `comic.panel.replace_art` with. Rule 2 makes ids unique **across** kinds, so it is well above every
/// id this test's pages, panels, dialogues and lettering already own — a low number collides.
const LINKED_ASSET: u64 = 1000;

/// An 8×8 opaque red PNG (75 bytes), embedded so the test needs no image encoder.
const PLATE_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR42mM4oaHxHx9mGBkKAOdkhcGWgnvEAAAAAElFTkSuQmCC";

/// The `sha256` of [`PLATE_PNG`]'s 75 bytes: what the prepared `assets/index.json` record has to carry
/// in `content_hash` (§5.5), which is what rule 5 checks against the file on disk.
const PLATE_SHA256: &str = "48375acbd6ca81fffa153a04795f25b7847b07dff0259d9848576c05e7b6627b";

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-acceptance-mcp").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn rpc(s: &mut Server, id: u64, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
    let reply = s.handle_line(&line).unwrap_or_else(|| panic!("{method} must be answered"));
    let v: Value = serde_json::from_str(&reply).expect("a JSON reply");
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], id);
    assert!(v.get("result").is_some() != v.get("error").is_some(), "exactly one of result/error: {v}");
    v
}

/// One MCP `tools/call`. Returns the tool result (`{content, isError}`).
fn tool(s: &mut Server, id: u64, name: &str, args: Value) -> Value {
    rpc(s, id, "tools/call", json!({"name": name, "arguments": args}))["result"].clone()
}

fn text_of(result: &Value) -> String {
    result["content"]
        .as_array()
        .expect("content")
        .iter()
        .filter(|c| c["type"] == "text")
        .map(|c| c["text"].as_str().unwrap_or("").to_string())
        .collect()
}

/// A command through MCP, unwrapped: the tool must not report an error and the text is the command's
/// JSON result.
fn run(s: &mut Server, id: u64, command: &str, params: Value) -> Value {
    let r = tool(s, id, "run_command", json!({"command": command, "params": params}));
    assert_eq!(r["isError"], json!(false), "{command} over MCP: {}", text_of(&r));
    serde_json::from_str(&text_of(&r)).unwrap_or_else(|e| panic!("{command} result is not JSON: {e}"))
}

/// A command through MCP that must fail, returning the tool's error text.
fn run_err(s: &mut Server, id: u64, command: &str, params: Value) -> String {
    let r = tool(s, id, "run_command", json!({"command": command, "params": params}));
    assert_eq!(r["isError"], json!(true), "{command} should have been refused over MCP");
    text_of(&r)
}

fn comic_ids(s: &mut Server, id: u64, enabled_only: bool) -> Vec<String> {
    let r = tool(s, id, "list_commands", json!({"filter": "comic.", "enabledOnly": enabled_only}));
    assert_eq!(r["isError"], json!(false), "{}", text_of(&r));
    let v: Value = serde_json::from_str(&text_of(&r)).expect("JSON");
    v["commands"].as_array().expect("commands").iter().map(|c| c["id"].as_str().unwrap().to_string()).collect()
}

fn inspect(s: &mut Server, id: u64) -> Value {
    let r = tool(s, id, "inspect_document", json!({}));
    assert_eq!(r["isError"], json!(false), "{}", text_of(&r));
    serde_json::from_str(&text_of(&r)).expect("JSON")
}

/// The width and height out of a PNG's IHDR, without an image decoder.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    assert_eq!(&bytes[12..16], b"IHDR", "no IHDR");
    (u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap()))
}

/// Prepare a bundle for the **one** step M1 has no command for: `comic.panel.replace_art` places a
/// `Linked` `Asset` inside a panel, and no `comic.asset.*` command registers one, so the record is
/// written the way the wire format writes it — `assets/plate.png` plus its §5.5 `assets/index.json`
/// entry (schema §7). This is a **fixture**, not an engine back door, exactly as opening a
/// `.vectorcraft` file is: every step of the operation itself is still `tools/call` → `run_command`.
fn prepare_linked_asset(root: &std::path::Path, id: u64) {
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets).expect("assets/");
    let bytes = vectorcraft_format::base64_decode(PLATE_PNG).expect("the embedded png decodes");
    assert_eq!(bytes.len(), 75, "the constant is the 75-byte plate its sha256 below was measured on");
    std::fs::write(assets.join("plate.png"), &bytes).expect("the linked asset's file");
    let index = json!({
        "schema_version": 1,
        "assets": [{
            "id": id,
            "asset_type": "source",
            "storage_reference": {"linked": {"relative_path": "assets/plate.png"}},
            "content_hash": {"algorithm": "sha256", "value": PLATE_SHA256},
            "media_metadata": {"mime": "image/png", "width": 8, "height": 8, "color_profile": null, "dpi": null},
            "provenance": null,
            "parent_asset_ids": [],
            "approval_status": "pending"
        }]
    });
    std::fs::write(assets.join("index.json"), serde_json::to_vec_pretty(&index).expect("JSON")).expect("assets/index.json");
}

/// The ids in the saved `assets/index.json` (§5.5), read from the bundle on disk.
fn asset_ids(root: &std::path::Path) -> Vec<u64> {
    let v: Value = serde_json::from_slice(&std::fs::read(root.join("assets").join("index.json")).expect("assets/index.json")).expect("JSON");
    v["assets"].as_array().expect("assets").iter().map(|a| a["id"].as_u64().expect("an asset id")).collect()
}

fn page_files(root: &std::path::Path) -> Vec<Value> {
    let dir = root.join("pages");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names.iter().map(|p| serde_json::from_slice(&std::fs::read(p).expect("read")).expect("the page file is JSON")).collect()
}

#[test]
fn acceptance_12_the_essential_operations_succeed_through_mcp_headless() {
    let dir = scratch("t12");
    let root = dir.join("book.comiccraft");
    let mut s = Server::new(Box::new(Headless::with_document()));
    let mut id = 0;
    let mut next = || {
        id += 1;
        id
    };

    // ---------- the handshake, and the tool the whole surface is run through ----------
    let init = rpc(
        &mut s,
        next(),
        "initialize",
        json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "w5", "version": "1"}}),
    );
    assert!(init.get("result").is_some(), "the handshake answers: {init}");
    assert!(s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none(), "a notification is not answered");
    assert!(s.is_initialized(), "and the server knows the client is ready");
    let tools: Vec<String> = rpc(&mut s, next(), "tools/list", json!({}))["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(tools.contains(&"run_command".to_string()), "the tool the surface runs through: {tools:?}");

    // ---------- the comic commands are discoverable, and honest about being disabled ----------
    let before = comic_ids(&mut s, next(), false);
    assert_eq!(before.len(), 19, "nineteen comic.* commands over MCP: {before:?}");
    assert!(before.iter().all(|c| c.starts_with("comic.")), "{before:?}");
    let none_enabled = comic_ids(&mut s, next(), true);
    assert_eq!(
        none_enabled,
        ["comic.project.create", "comic.project.open"],
        "of the nineteen, only the two that open or create a project are enabled without one: {none_enabled:?}"
    );
    let refused = run_err(&mut s, next(), "comic.page.create", json!({}));
    assert!(refused.contains("no comic project open"), "the refusal names the way out: {refused}");

    // ---------- create, pages, panels, lettering ----------
    let created =
        run(&mut s, next(), "comic.project.create", json!({"path": root.to_string_lossy(), "title": "MCP book", "pageSize": [612.0, 792.0]}));
    assert_eq!(created["artboards"].as_u64(), Some(1), "a project open, and its document has an artboard");
    let enabled = comic_ids(&mut s, next(), true);
    assert_eq!(enabled.len(), 19, "with a project open, all nineteen are enabled: {enabled:?}");

    let one = run(&mut s, next(), "comic.page.create", json!({}))["page"].as_u64().expect("page 1");
    let two = run(&mut s, next(), "comic.page.create", json!({"size": [300.0, 400.0], "template": "two-tier"}))["page"].as_u64().expect("page 2");
    let three = run(&mut s, next(), "comic.page.create", json!({"size": [450.0, 250.0]}))["page"].as_u64().expect("page 3");
    assert_eq!([one, two, three].iter().copied().collect::<std::collections::BTreeSet<u64>>().len(), 3);

    let laid = run(&mut s, next(), "comic.page.template.apply", json!({"page": one, "template": "three-tier"}));
    let panels: Vec<u64> = laid["panels"].as_array().expect("panels").iter().map(|p| p["panel"].as_u64().unwrap()).collect();
    assert_eq!(panels.len(), 3, "three panels laid out: {panels:?}");
    run(&mut s, next(), "comic.panel.set_shot", json!({"panel": panels[0], "shot": "Rooftop, dawn."}));
    run(&mut s, next(), "comic.panel.move", json!({"panel": panels[1], "dx": 4.0, "dy": 3.0}));
    run(&mut s, next(), "comic.panel.resize", json!({"panel": panels[2], "width": 150.0, "height": 120.0}));

    let speech = run(&mut s, next(), "comic.lettering.add", json!({"page": one, "panel": panels[0], "kind": "speech", "text": "Over MCP."}));
    let caption = run(&mut s, next(), "comic.lettering.add", json!({"page": one, "panel": panels[1], "kind": "narration", "text": "A caption."}));
    let dialogue = speech["dialogue"].as_u64().expect("the dialogue");
    run(&mut s, next(), "comic.dialogue.update", json!({"dialogue": dialogue, "text": "Over MCP, revised."}));

    // ---------- artwork: place, crop, clip ----------
    let placed = run(&mut s, next(), "file.place", json!({"name": "plate.png", "dataBase64": PLATE_PNG, "at": [80.0, 80.0]}));
    let image = placed["ids"][0].as_u64().expect("the placed image");
    let cropped = run(&mut s, next(), "object.cropImage", json!({"rect": [80.0, 80.0, 8.0, 8.0]}));
    assert_eq!(cropped["id"].as_u64(), Some(image), "the crop keeps the image");
    let masked = run(&mut s, next(), "object.maskImage", json!({}));
    let clip = masked["id"].as_u64().expect("the clip group");
    let clipping_path = masked["path"].as_u64().expect("the clipping path");
    assert_ne!(clip, image, "the mask wraps the image in a clip group");
    assert_eq!(inspect(&mut s, next())["selection"], json!([clipping_path]), "and leaves the clipping path selected, whose handles crop it");

    // ---------- undo and redo over MCP, as one step each ----------
    let history_before = inspect(&mut s, next())["history"].as_array().expect("history").len();
    run(&mut s, next(), "comic.panel.set_shot", json!({"panel": panels[1], "shot": "A different brief."}));
    let history = inspect(&mut s, next())["history"].as_array().expect("history").len();
    assert_eq!(history, history_before + 1, "one action is one step, over MCP too");
    tool(&mut s, next(), "undo", json!({}));
    let after_undo = inspect(&mut s, next());
    assert_eq!(after_undo["history"].as_array().unwrap().len(), history_before, "undo takes the step back");
    tool(&mut s, next(), "redo", json!({}));
    assert!(inspect(&mut s, next())["history"].as_array().unwrap().len() > history_before, "and redo puts it back");

    // ---------- the model, as the comic command surface reports it ----------
    let v = run(&mut s, next(), "comic.project.validate", json!({}));
    assert_eq!(v["errors"].as_u64(), Some(0), "the project built over MCP validates: {}", v["findings"]);
    let geometry: Vec<(u64, u64, [f64; 2])> = v["geometry"]
        .as_array()
        .expect("geometry")
        .iter()
        .map(|g| {
            (
                g["page"].as_u64().unwrap(),
                u64::from(g["artboard_id"].as_u64().unwrap() as u32),
                [g["rect"][2].as_f64().unwrap(), g["rect"][3].as_f64().unwrap()],
            )
        })
        .collect();
    assert_eq!(geometry.len(), 3, "one artboard per page, reported as information (§9.1 mitigation 3)");
    assert_eq!(geometry.iter().map(|(_, a, _)| *a).collect::<std::collections::BTreeSet<u64>>().len(), 3, "each page its own artboard");

    // ---------- save, and the bundle on disk ----------
    let saved = run(&mut s, next(), "comic.project.save", json!({}));
    let files: Vec<String> = saved["files"].as_array().expect("files").iter().map(|f| f.as_str().unwrap().to_string()).collect();
    assert!(files.iter().any(|f| f.ends_with("document/comic.vectorcraft")), "the save wrote the document: {files:?}");
    assert!(files.iter().any(|f| f.starts_with("pages/page-")), "and every page: {files:?}");
    assert_eq!(inspect(&mut s, next())["dirty"], json!(false), "a saved project is not unsaved work");

    // Every id the commands handed out is in the page files, in the record the wire format puts it in.
    let pages = page_files(&root);
    assert_eq!(pages.len(), 3, "three page files");
    let artboard_of =
        |page: u64| pages.iter().find(|p| p["page"]["id"] == json!(page)).expect("the page file")["page"]["artboard_id"].as_u64().unwrap();
    assert_eq!([artboard_of(one), artboard_of(two), artboard_of(three)].iter().copied().collect::<std::collections::BTreeSet<u64>>().len(), 3);
    let page_one = pages.iter().find(|p| p["page"]["id"] == json!(one)).unwrap();
    assert_eq!(
        page_one["panels"].as_array().unwrap().iter().map(|p| p["id"].as_u64().unwrap()).collect::<Vec<u64>>(),
        panels,
        "the PanelIds the commands returned are the ids in the file"
    );
    assert_eq!(
        page_one["lettering"].as_array().unwrap().iter().map(|l| l["id"].as_u64().unwrap()).collect::<Vec<u64>>(),
        vec![speech["lettering"].as_u64().unwrap(), caption["lettering"].as_u64().unwrap()]
    );
    assert_eq!(page_one["dialogue"][0]["id"].as_u64(), Some(dialogue));
    assert_eq!(page_one["dialogue"][0]["text"], json!("Over MCP, revised."));
    assert_eq!(page_one["panels"][0]["shot_description"], json!("Rooftop, dawn."));

    // ---------- a prepared bundle, so `comic.panel.replace_art` can be driven over MCP ----------
    // R1-2: the artwork step above is the *document* path (`file.place` + `object.maskImage` +
    // `object.cropImage`), and it never establishes a `Panel.artwork_refs` link. The command that
    // does — `comic.panel.replace_art` — cannot create its `Linked` `Asset` over MCP (no
    // `comic.asset.*` in M1), but it *runs* over MCP against a bundle that has one. So the record is
    // written into the bundle's own wire format while the project is closed, and the project is then
    // reopened over MCP; everything the command does is a tool call.
    prepare_linked_asset(&root, LINKED_ASSET);

    // ---------- close and reopen, over MCP, with every id preserved ----------
    let before_reopen = serde_json::to_string(&geometry).unwrap();
    let r = tool(&mut s, next(), "open_file", json!({"path": root.join("document").join("comic.vectorcraft").to_string_lossy()}));
    assert_eq!(r["isError"], json!(false), "open_file over MCP: {}", text_of(&r));
    let opened_doc: Value = serde_json::from_str(&text_of(&r)).expect("JSON");
    assert_eq!(opened_doc["title"], json!("comic.vectorcraft"), "open_file names the document it opened, rather than any object: {opened_doc}");
    assert!(opened_doc["index"].as_u64().is_some(), "and it is the document in the session: {opened_doc}");
    let reopened_project = run(&mut s, next(), "comic.project.open", json!({"path": root.to_string_lossy()}));
    assert_eq!(reopened_project["pages"].as_u64(), Some(3));
    let v2 = run(&mut s, next(), "comic.project.validate", json!({}));
    assert_eq!(v2["errors"].as_u64(), Some(0), "the reopened bundle validates: {}", v2["findings"]);
    let geometry2: Vec<(u64, u64, [f64; 2])> = v2["geometry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| (g["page"].as_u64().unwrap(), g["artboard_id"].as_u64().unwrap(), [g["rect"][2].as_f64().unwrap(), g["rect"][3].as_f64().unwrap()]))
        .collect();
    assert_eq!(serde_json::to_string(&geometry2).unwrap(), before_reopen, "every page id, artboard id and page size survives reopening over MCP");
    // The ids are live, not merely written down: the reopened project still answers to them.
    run(&mut s, next(), "comic.panel.set_shot", json!({"panel": panels[0], "shot": "Still mine after reopen."}));
    run(&mut s, next(), "comic.lettering.set_text", json!({"lettering": caption["lettering"].as_u64().unwrap(), "text": "Still editable."}));

    // ---------- artwork placed and clipped **inside a panel**, over MCP (M1 §1) ----------
    // The document path above places a raster object on the canvas; this links a `Linked` asset to a
    // Panel's `artwork_refs` through the command that owns that link, which is the capability M1 §1
    // names ("place and clip raster artwork inside panels") and the one test 12 left uncertified.
    let artwork = run(&mut s, next(), "comic.panel.replace_art", json!({"panel": panels[0], "asset": LINKED_ASSET}));
    let group = artwork["node"].as_u64().expect("the clip group");
    let art_image = artwork["image"].as_u64().expect("the placed image");
    assert_eq!(artwork["panel"].as_u64(), Some(panels[0]));
    assert_eq!(artwork["asset"].as_u64(), Some(LINKED_ASSET), "the asset the prepared bundle names: {artwork}");
    assert_ne!(group, art_image, "the clip group and the image inside it are two objects");
    assert_eq!(inspect(&mut s, next())["selection"], json!([art_image]), "the placed image is selected, so object.cropImage acts on it");

    // **Clipped**: the artwork sits inside a clip group whose clipping path copies the panel's frame,
    // and the panel's record names it. Replacing it again reports what it replaced and keeps the
    // Asset record, so the lineage of §11.1 outlives the artwork (this is test 6's claim, over MCP).
    let again = run(&mut s, next(), "comic.panel.replace_art", json!({"panel": panels[0], "asset": LINKED_ASSET}));
    let group2 = again["node"].as_u64().expect("the new clip group");
    assert_eq!(again["previous"].as_u64(), Some(group), "it reports the artwork it replaced");
    assert_ne!(group2, group, "the replacement is a new object, not the old one re-used");
    let saved_art = run(&mut s, next(), "comic.project.save", json!({}));
    assert!(saved_art["files"].as_array().expect("files").iter().any(|f| f == &json!(format!("pages/page-{one}.json"))));

    let pages_after = page_files(&root);
    let page_one_after = pages_after.iter().find(|p| p["page"]["id"] == json!(one)).expect("page one's file");
    assert_eq!(
        page_one_after["panels"][0]["artwork_refs"],
        json!([{"asset_id": LINKED_ASSET, "node_id": group2, "role": "primary"}]),
        "the panel record on disk names the asset and the clip group, exactly once"
    );
    assert_eq!(asset_ids(&root), vec![LINKED_ASSET], "replacing art adds and removes no Asset record (§11.1)");
    assert_eq!(
        page_one_after["lettering"].as_array().expect("lettering").len(),
        page_one["lettering"].as_array().expect("lettering").len(),
        "and the page's lettering is untouched by the artwork step"
    );
    let v3 = run(&mut s, next(), "comic.project.validate", json!({}));
    assert_eq!(v3["errors"].as_u64(), Some(0), "and the bundle with artwork in the panel validates: {}", v3["findings"]);

    // ---------- render and PDF export ----------
    // Book order after a reorder: page 2 (300×400), page 1 (612×792), page 3 (450×250).
    let reordered = run(&mut s, next(), "comic.page.reorder", json!({"page": one, "to": 1}));
    assert_eq!(reordered["scene_order"], json!([two, one, three]));
    let book: [(u64, (u32, u32)); 3] = [(two, (300, 400)), (one, (612, 792)), (three, (450, 250))];

    for (i, (page, size)) in book.iter().enumerate() {
        let png = dir.join(format!("page-{page}.png"));
        let out = tool(&mut s, next(), "export", json!({"format": "png", "path": png.to_string_lossy(), "range": (i + 1).to_string()}));
        assert_eq!(out["isError"], json!(false), "{}", text_of(&out));
        assert_eq!(png_size(&std::fs::read(&png).expect("the png")), *size, "page {page}'s render is the page's dimensions");
    }

    // One PDF page per artboard, in the comic's reading order: exported without the native document,
    // so reopening it reads the *pages*, not a restored copy of the document.
    let pdf = dir.join("book.pdf");
    let out = tool(&mut s, next(), "export", json!({"format": "pdf", "path": pdf.to_string_lossy(), "options": {"preserveEditing": false}}));
    assert_eq!(out["isError"], json!(false), "{}", text_of(&out));
    let bytes = std::fs::read(&pdf).expect("the pdf");
    assert!(bytes.starts_with(b"%PDF-"), "a real PDF");
    let opened = run(&mut s, next(), "document.open", json!({"path": pdf.to_string_lossy()}));
    assert_eq!(opened["format"], json!("pdf"));
    assert_ne!(opened["restored"], json!(true), "a PDF exported without Preserve Editing has no native document to restore: {opened}");
    let pages_in_pdf = inspect(&mut s, next());
    let boxes: Vec<(u32, u32)> = pages_in_pdf["artboards"]
        .as_array()
        .expect("artboards")
        .iter()
        .map(|a| (a["width"].as_f64().unwrap().round() as u32, a["height"].as_f64().unwrap().round() as u32))
        .collect();
    assert_eq!(boxes, book.iter().map(|(_, s)| *s).collect::<Vec<(u32, u32)>>(), "one PDF page per artboard, each the page's size, in reading order");
}
