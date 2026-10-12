# M2 W3 round-1 review — `cargo xtask ci`, raw

Run by the `code-reviewer` session that reviewed W3 (`docs/reviews/M2-command-engineer-locking-review.md`),
**after** the count pin in `crates/engine/tests/comic_commands_r1.rs` was re-pinned 19 → 22. Toolchain
`cargo 1.99.0` / `rustc 1.99.0`.

The gate was invoked five times because this environment kills a long command mid-flight (and a build killed
mid-compile leaves `target/` with `anon.<hash>.llvm.<hash>` link errors — M2 §7.7's *Error 2*); each re-run
resumed, and the run recorded here is the one that completed. This is that run's output, trimmed to the
step boundaries, the per-target results and the summary.

## 1. The 7 steps

```
=== ci: fmt ===
=== ci: clippy ===
=== ci: test ===
=== ci: assets ===
=== ci: brands ===
=== ci: layers ===
=== ci: wasm ===
CI summary: all 7 steps passed (fmt, clippy, test, assets, brands, layers, wasm)
EXIT 0
```

## 2. The comic and i18n targets

```
comic_acceptance.rs: test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
comic_acceptance_reviewer.rs: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
comic_commands_r1.rs: test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
comic_commands_r2.rs: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
comic_commands_r3.rs: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
comic_golden_render.rs: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
comic_headless.rs: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
comic_labels_are_menu_strings.rs: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
comic_quit_prompt.rs: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
comic_save_keeps_model.rs: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

## 3. Failures

```
(none — no `test result: FAILED` and no `error:` line anywhere in the run)
```

Every `test result:` line in the log is `ok`; the run ends `CI summary: all 7 steps passed`.
