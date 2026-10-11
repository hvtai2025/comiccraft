//! Round-1 review of the W4 scope call on the i18n catalogs, by `code-reviewer`.
//!
//! The lead's question: would `menu: []` on the `comic.*` commands have been the better call, and so
//! removed the 20 new label rows (x 8 "complete menus" catalogs) the change had to author? This pins
//! the answer in the code: `menu_strings()` adds every registered command's `label` unconditionally,
//! whether or not the spec carries a menu path, so the labels are required either way — only not
//! registering the commands at all would avoid them.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vectorcraft_engine::command_specs;

#[test]
fn a_registered_commands_label_is_a_menu_string_even_with_no_menu() {
    let strings = vectorcraft_ui_egui::menus::menu_strings();
    let specs = command_specs();

    let menu_less: Vec<&str> = specs.iter().filter(|c| c.menu.is_empty()).map(|c| c.label).collect();
    assert!(menu_less.len() > 20, "there are commands with no menu path to compare against: {}", menu_less.len());

    let missing: Vec<&&str> = menu_less.iter().filter(|label| !strings.contains(**label)).collect();
    assert!(
        missing.is_empty(),
        "FAIL: `menu_strings()` does not require the labels of menu-less commands, so `menu: []` would have avoided \
         them: {missing:?}"
    );

    // The nineteen comic labels specifically: the twentieth menu string is the `Comic` group name.
    let comic: Vec<&str> = specs.iter().filter(|c| c.id.starts_with("comic.")).map(|c| c.label).collect();
    assert_eq!(comic.len(), 19);
    let not_required: Vec<&&str> = comic.iter().filter(|label| !strings.contains(**label)).collect();
    assert!(not_required.is_empty(), "FAIL: these comic labels are not menu strings, so their catalog rows were optional: {not_required:?}");
}
