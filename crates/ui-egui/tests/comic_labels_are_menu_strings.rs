//! Round-1 review of the W4 scope call on the i18n catalogs, by `code-reviewer`.
//!
//! The lead's question: would `menu: []` on the `comic.*` commands have been the better call, and so
//! removed the 20 new label rows (x 8 "complete menus" catalogs) the change had to author? This pins
//! the answer in the code: `menu_strings()` adds every registered command's `label` unconditionally,
//! whether or not the spec carries a menu path, so the labels are required either way — only not
//! registering the commands at all would avoid them.
//!
//! ## What each assertion can and cannot see (M2 W3 round 1, M2-W3-F1)
//!
//! The answer above rests on **one** property of `menu_strings()` — that it inserts every
//! `command_spec().label` unconditionally (`crates/ui-egui/src/menus.rs:3493-3496`) — and the
//! `missing` assertion below is what pins it: with that insertion made conditional on
//! `!c.menu.is_empty()` — the `menu: []` world this file exists to reject — `missing` goes red.
//! That was measured by mutation, not argued.
//!
//! What this file does **not** do is look at the catalogs. `menu_strings()` is computed from the
//! code, so a comic label is in it whether or not `es.tsv` has a row for it; the count pin below
//! fixes *how many* comic labels there are and says nothing about whether they are translated. The
//! obligation — *every `comic.*` label has a row in each `complete_menus` catalog* — is guarded one
//! file over, by `i18n::tests::complete_languages_translate_every_menu_string`: it asserts
//! `cat.plain(s).is_some()` for **every** string in `menu_strings()` in every `complete_menus`
//! language, and every command label is in that set. `i18n::tests::menu_catalogs_translate_every_menu_label`
//! covers the labels the menus actually show. **Not** `complete_languages_translate_every_message`,
//! which scans message literals (`.status(`, `Err(`, `#[error(` …) and can never see a `cmd!` label —
//! the W3 record and M2 §7.7 name that one, and it is the wrong test.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vectorcraft_engine::command_specs;

#[test]
fn a_registered_commands_label_is_a_menu_string_even_with_no_menu() {
    let strings = vectorcraft_ui_egui::menus::menu_strings();
    let specs = command_specs();

    let menu_less: Vec<&str> = specs.iter().filter(|c| c.menu.is_empty()).map(|c| c.label).collect();
    assert!(menu_less.len() > 20, "there are commands with no menu path to compare against: {}", menu_less.len());

    // **This is the assertion that answers the `menu: []` question**, and it can fail: make
    // `menu_strings()` insert a label only when the spec carries a menu path and it reds (reviewer
    // mutation F1, M2 W3 round 1). It is a guard on `menu_strings()`, not on the catalogs.
    let missing: Vec<&&str> = menu_less.iter().filter(|label| !strings.contains(**label)).collect();
    assert!(
        missing.is_empty(),
        "FAIL: `menu_strings()` does not require the labels of menu-less commands, so `menu: []` would have avoided \
         them: {missing:?}"
    );

    // The comic command labels specifically: one per registered `comic.*` command, and the `Comic`
    // group name is a menu string too, so the comic-related rows number one more than this. M1
    // registered 19 of them; M2's W3 added `comic.panel.set_locked`, `comic.project.settings.get`
    // and `comic.project.settings.set`, so the count is 22.
    //
    // **Pinned rather than derived, deliberately.** `assert_eq!(comic.len(), comic.len())` would be
    // `len == len` and would let a new command slip in uncatalogued; the number is the *catalog
    // obligation*, and growing the surface has to be a conscious edit here. The round-1 reviewer
    // changed 19 → 22 for M2 W3 and agrees with the pin. (The rows themselves are guarded by
    // `i18n::tests::complete_languages_translate_every_menu_string` — see this file's header.)
    let comic: Vec<&str> = specs.iter().filter(|c| c.id.starts_with("comic.")).map(|c| c.label).collect();
    assert_eq!(comic.len(), 22);
    // Weak, and it says so: `strings` is built from these same labels, so for commands that *carry* a
    // menu path this can only fail if `menu_strings()` stops inserting command labels altogether —
    // which `missing` above (and the catalog test) catch too. Kept as a cheap second net; it is a
    // *redundant* guard rather than a tautology: F1 read it as "cannot fail", and it can, never alone.
    let not_required: Vec<&&str> = comic.iter().filter(|label| !strings.contains(**label)).collect();
    assert!(not_required.is_empty(), "FAIL: these comic labels are not menu strings, so their catalog rows were optional: {not_required:?}");
}
