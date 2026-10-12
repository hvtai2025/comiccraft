# M2 W3 — `cargo xtask ci`, raw

Run by the `command-engineer` session that finished W3 (`docs/reviews/M2-command-engineer-locking.md`).
Toolchain `cargo 1.99.0` / `rustc 1.99.0`. This is the gate as it stands; **it is red on one test**,
and §2 says why it is not green and why the one line was not changed.

## 1. `cargo xtask ci` — raw

```
$ cargo xtask ci

=== ci: fmt ===
$ cargo fmt --all -- --check

=== ci: clippy ===
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.93s

=== ci: test ===
$ cargo test --workspace
…
     Running tests/comic_commands_r1.rs (target/debug/deps/comic_commands_r1-0282a4d8e993a9d7)

running 14 tests
test r1_every_comic_command_is_disabled_without_a_project ... FAILED
… 13 others ok …

failures:

---- r1_every_comic_command_is_disabled_without_a_project stdout ----

thread 'r1_every_comic_command_is_disabled_without_a_project' (7243) panicked at crates/engine/tests/comic_commands_r1.rs:547:5:
assertion `left == right` failed: the surface is nineteen commands: ["comic.project.create", "comic.project.open", "comic.project.save", "comic.project.validate", "comic.project.settings.get", "comic.project.settings.set", "comic.page.create", "comic.page.delete", "comic.page.reorder", "comic.page.template.apply", "comic.reading_order.set", "comic.panel.add", "comic.panel.move", "comic.panel.resize", "comic.panel.set_locked", "comic.panel.replace_art", "comic.panel.set_shot", "comic.panel.delete", "comic.lettering.add", "comic.lettering.set_text", "comic.lettering.set_style", "comic.dialogue.update"]
  left: 22
 right: 19
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    r1_every_comic_command_is_disabled_without_a_project

test result: FAILED. 13 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p vectorcraft-engine --test comic_commands_r1`

CI summary:
  ok    fmt
  ok    clippy
  FAIL  test: cargo test --workspace: exited with exit status: 101
  skip  assets
  skip  brands
  skip  layers
  skip  wasm
error: ci failed at `test`
```

**`fmt` and `clippy` are green**, and the `test` step's failure is one test. `ci` stops there, so the
last four steps are reported below from their own invocations (each exits 0).

## 2. The four steps `ci` skipped, each run on its own — raw

```
$ cargo xtask assets
assets: all 323 asset files attributed in ASSETS.md                              (exit 0)

$ cargo xtask brands
brands: no vendor names in user-visible text                                     (exit 0)

$ cargo xtask layers
vectorcraft-web              exempt         engine, format, text, ui-egui
xtask                        exempt

OK: 29 crates, no layering violations.                                           (exit 0)

$ cargo xtask wasm
  ok     vectorcraft-text
  ok     vectorcraft-tools
  ok     vectorcraft-trace
  ok     vectorcraft-ui-egui                                                     (exit 0)
```

## 3. Why the tree is not green, and what the one line is

`crates/engine/tests/comic_commands_r1.rs:547` is the round-1 `code-reviewer`'s pin of the comic
surface — *"the surface is nineteen commands"* — written when M1 had nineteen. W3 added three
(`comic.panel.set_locked`, `comic.project.settings.get`, `comic.project.settings.set`), so the pin is
stale by design, exactly like the two count pins that were updated (`docs/reviews/M2-command-engineer-locking.md`
§7).

**It was left unchanged on purpose: this brief forbids touching `crates/engine/tests/comic_commands_r*.rs`.**
That is a direct contradiction with the same brief's acceptance criterion (*"`cargo xtask ci` must be
GREEN on all 7 steps"*), and it is the same shape as M2 §7.7's *Error 1* — a brief that told W3 to fix
M2-L3 while forbidding W3's access to M2-L3's file. So the stronger, more specific constraint was
obeyed and the contradiction is reported rather than silently resolved: **this is the only thing
between this tree and a green gate.**

The fix is one integer and its prose (the doc comment at `:542` and the message at `:547` both say
*nineteen*); nothing else in that test depends on the number — the rest of it asserts that every comic
id is `Disabled` with a reason rather than `NoDocument`, which holds for all 22:

```diff
-/// §3 of the implementation: `has_comic` gates all nineteen, so a comic id is `Disabled` — never
+/// §3 of the implementation: `has_comic` gates all twenty-two, so a comic id is `Disabled` — never
-    assert_eq!(ids.len(), 19, "the surface is nineteen commands: {ids:?}");
+    assert_eq!(ids.len(), 22, "the surface is twenty-two commands: {ids:?}");
```

Its owner (the `code-reviewer` who wrote it, or the round-3 review M2 §3 schedules after W3) is the
right place to re-pin it, because the file is the reviewer's own evidence.

## 4. Full logs

`cargo xtask ci`'s full output is `target/m2w3/ci-final.log` (3 116 lines, git-ignored and
disposable, like `target/` itself), and the four separate gate runs are `target/m2w3/gates.log`.
The whole workspace suite under `--no-fail-fast`, which is what showed there were **no failures other
than this one**, is `target/m2w3/ws-tests.log`.
