//! Round-1 reviewer tests for M1 W5 (`docs/reviews/M1-acceptance.md`).
//!
//! `comic_acceptance.rs` pins mitigation 4 with `artboard.new` resurrecting the retired number, so
//! its `free_artboards` filter is the half a wrong implementation breaks there. Mutation testing
//! found the other half unheld: removing the retirement check from `claim_artboard`'s allocation
//! **loop** leaves all fifteen acceptance tests green (`crates/engine/src/cmd/comic/tests.rs`
//! catches it, the acceptance suite does not), and the loop is the only half that runs when the
//! retired number is not in the document. These three tests close both paths, plus the hazard's
//! consequence — which test 11's failure message asserts in prose and nothing verified.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::comic::OpenComic;

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("target"));
    let dir = base.join("comic-acceptance-reviewer").join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn exec(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, &params).unwrap_or_else(|e| panic!("{id} rejected {params}: {e}"))
}

fn project(tag: &str) -> (Session, PathBuf) {
    let mut s = Session::new();
    let root = scratch(tag).join("book.comiccraft");
    exec(&mut s, "comic.project.create", json!({"path": root.to_string_lossy(), "title": "Reviewer"}));
    (s, root)
}

fn new_page(s: &mut Session, params: Value) -> u64 {
    exec(s, "comic.page.create", params)["page"].as_u64().expect("a page id")
}

fn open_comic(s: &Session) -> &OpenComic {
    s.doc().expect("a document").comic.as_deref().expect("an open project")
}

fn open_comic_mut(s: &mut Session) -> &mut OpenComic {
    Arc::make_mut(s.doc_mut().expect("a document").comic.as_mut().expect("an open project"))
}

fn artboard_of(s: &Session, page: u64) -> u32 {
    open_comic(s).bundle.pages.get(&vectorcraft_comic_model::PageId(page)).expect("the page file").page.artboard_id
}

fn named_artboards(s: &Session) -> Vec<u32> {
    open_comic(s).bundle.pages.values().map(|f| f.page.artboard_id).collect()
}

fn retired(s: &Session) -> Vec<u32> {
    open_comic(s).bundle.manifest.retired_artboard_ids.clone()
}

fn artboard_ids(s: &Session) -> Vec<u32> {
    s.doc().expect("a document").doc.artboards.iter().map(|a| a.id).collect()
}

fn validate(s: &mut Session) -> Value {
    exec(s, "comic.project.validate", json!({}))
}

/// The two halves of §5.3.1 mitigation 4, in the order a wrong implementation can break them.
///
/// The **fast path** (`free_artboards`) is what runs when a retired artboard is back in the
/// document and free: `artboard.new` gives the number straight back, and the filter is the only
/// thing between it and a page. `comic_acceptance.rs` covers that (`M-W5-1`).
///
/// The **loop** is what runs when no free artboard is left at all: the retired number is then the
/// `next_artboard_id`, and the loop's own check is the only thing that steps over it. Nothing in the
/// acceptance suite covers it, so dropping it leaves the whole suite green — this is the case that
/// closes it. Both halves must hold, so the test failing on either is the point.
#[test]
fn reviewer_a_retired_artboard_is_refused_by_both_allocation_paths() {
    // ---- the fast path: a retired artboard is free in the document, next to a free one ----
    let (mut s, _root) = project("fast-path");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!(artboard_of(&s, second), 2, "the second page takes the highest artboard");
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert_eq!(retired(&s), vec![2], "deleting the page retires its artboard id");
    // The ordinary command hands the number back, and a second one adds a genuinely free artboard,
    // so `free_artboards` has a usable answer that is *not* the retired id.
    exec(&mut s, "artboard.new", json!({"x": 0.0, "y": 0.0, "width": 200.0, "height": 200.0}));
    exec(&mut s, "artboard.new", json!({"x": 0.0, "y": 300.0, "width": 200.0, "height": 200.0}));
    assert_eq!(artboard_ids(&s), vec![1, 2, 3], "the retired 2 is back, and 3 is free for the taking");

    let third = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    let taken = artboard_of(&s, third);
    assert!(!retired(&s).contains(&taken), "a claim must not be a retired id, whichever path offered it");
    assert_eq!(taken, 3, "the fast path must skip the retired 2 and take the free 3, not allocate past it");
    assert_eq!(named_artboards(&s), vec![1, 3], "and the retired number is still no page's page");
    assert_eq!(validate(&mut s)["errors"].as_u64(), Some(0), "nothing is reported, because nothing drifted");
    assert_eq!(artboard_of(&s, first), 1, "the first page is untouched");

    // ---- the loop: no free artboard at all, so the retired number is the next candidate ----
    let (mut s, _root) = project("loop");
    let first = new_page(&mut s, json!({}));
    assert_eq!(artboard_of(&s, first), 1, "the first page claims the artboard the document already had");
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!(artboard_of(&s, second), 2, "the second page takes the highest artboard");
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert_eq!(artboard_ids(&s), vec![1], "the deleted page's artboard is gone, and 2 is retired");
    assert_eq!(retired(&s), vec![2]);
    // No `artboard.new` here: 1 is named by the first page, so there is nothing free, and the only
    // candidate `next_artboard_id` offers is the retired 2.
    let third = exec(&mut s, "comic.page.create", json!({"size": [250.0, 250.0]}));
    let taken = third["artboard_id"].as_u64().expect("an artboard id");
    assert!(!retired(&s).contains(&(taken as u32)), "the allocation loop must step over the retired number too");
    assert_eq!(taken, 3, "so it allocates past the picket, exactly as it does past a retired id");
    assert_eq!(third["skipped_retired_artboards"].as_u64(), Some(1), "and it says what it stepped over");
    assert_eq!(named_artboards(&s), vec![1, 3], "two pages, two artboards, neither of them retired");
}

/// The `named` half of the same rule, which `claim_artboard`'s own doc paragraph describes and no
/// delivered test exercises.
///
/// `artboard.delete` (`Window > Artboards`) removes an artboard *without retiring it* and stays
/// reachable while a project is open. A page whose artboard that command deleted still names the
/// number, so handing that exact number to a new page would make two pages resolve to one artboard
/// **and** delete the rule-4 report that was already showing the first page's breakage. The number
/// must therefore be skipped even though the retired list does not mention it — the conservative
/// half of mitigation 4, and this is the only test that holds it.
#[test]
fn reviewer_an_artboard_a_page_still_names_is_never_handed_to_another_page() {
    let (mut s, _root) = project("named");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!((artboard_of(&s, first), artboard_of(&s, second)), (1, 2));
    assert_eq!(retired(&s), Vec::<u32>::new(), "nothing is retired yet");

    // Delete the **highest** artboard — the one page 7 names — through the ordinary document
    // command. It is not retired, so `retired_artboard_ids` cannot be what saves the number.
    exec(&mut s, "artboard.delete", json!({"index": 1}));
    assert_eq!(artboard_ids(&s), vec![1], "artboard 2 is gone from the document");
    assert_eq!(named_artboards(&s), vec![1, 2], "but page 7 still names it, and it is not retired");
    let broken = validate(&mut s);
    assert!(broken["errors"].as_u64().expect("errors") >= 1, "the dangling page is already reported: {}", broken["findings"]);

    let third = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    let taken = artboard_of(&s, third);
    assert_ne!(taken, 2, "a page may not take a number another page still names, retired or not");
    let named = named_artboards(&s);
    assert_eq!(named.iter().copied().collect::<BTreeSet<u32>>().len(), named.len(), "one artboard per page (design §5.1.1): {named:?}");
    let after = validate(&mut s);
    assert_eq!(
        after["errors"].as_u64(),
        broken["errors"].as_u64(),
        "and the new page adds no report of its own — the pre-existing breakage is unchanged: {}",
        after["findings"]
    );
}

/// What test 11's failure message asserts in prose, and nothing checked: a page that *does* take a
/// retired id is reported by rule 4, and stays reported.
///
/// This is why mitigation 4 is worth a test at all — the number is not merely untidy, it is the one
/// state a creator cannot reach the end of. The violation is written straight into the model,
/// because no M1 command can produce it (that is the whole point), and then the project is asked to
/// survive a save and a reopen with it.
#[test]
fn reviewer_a_page_that_took_a_retired_artboard_is_reported_and_stays_reported() {
    let (mut s, root) = project("consequence");
    let first = new_page(&mut s, json!({}));
    let second = new_page(&mut s, json!({"size": [200.0, 200.0]}));
    assert_eq!(artboard_of(&s, second), 2);
    exec(&mut s, "comic.page.delete", json!({"page": second}));
    assert_eq!(retired(&s), vec![2], "2 means never again");
    assert_eq!(validate(&mut s)["errors"].as_u64(), Some(0), "and the project is clean");

    // The wrong allocation M-W5-1 produces, written by hand: page 6's page resolves to the id the
    // deleted page 7 used.
    {
        let page = open_comic_mut(&mut s).bundle.pages.get_mut(&vectorcraft_comic_model::PageId(first)).expect("the page file");
        page.page.artboard_id = 2;
    }

    let reported = validate(&mut s);
    let rule_four: Vec<&Value> =
        reported["findings"].as_array().expect("findings").iter().filter(|f| f["rule"] == json!(4) && f["severity"] == json!("error")).collect();
    assert!(!rule_four.is_empty(), "rule 4 reports a retired artboard, and as an error: {}", reported["findings"]);
    assert!(
        rule_four.iter().any(|f| f["message"].as_str().unwrap_or("").contains("retired_artboard_ids")),
        "and it names the list that makes the number retired: {rule_four:?}"
    );
    assert!(reported["errors"].as_u64().expect("errors") >= 1, "so the project is no longer clean");

    // It is not a transient: the report is on disk, and a reopen finds it again.
    exec(&mut s, "comic.project.save", json!({}));
    assert!(s.close_document(0), "the document closes");
    exec(&mut s, "comic.project.open", json!({"path": root.to_string_lossy()}));
    let reopened = validate(&mut s);
    assert!(
        reopened["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|f| f["rule"] == json!(4) && f["message"].as_str().unwrap_or("").contains("retired_artboard_ids")),
        "the same rule-4 report comes back with the bundle: {}",
        reopened["findings"]
    );

    // And creating a page does not launder it: the retired number is still refused, so the breakage
    // cannot spread to a second page.
    let third = new_page(&mut s, json!({"size": [250.0, 250.0]}));
    assert_ne!(artboard_of(&s, third), 2, "no command can hand out a retired id either");
    assert!(
        validate(&mut s)["findings"].as_array().expect("findings").iter().any(|f| f["rule"] == json!(4)),
        "the page that took it is still reported after another page is created"
    );
}
