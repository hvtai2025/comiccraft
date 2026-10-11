# Upstream proposal — a foreground save is never re-checked before the document closes

**Target:** VectorCraft, baseline `3da4080b`
**Branch in this fork:** `fix/quit-prompt-rechecks-after-foreground-save` (`086f6028`)
**Patch:** [`upstream-patches/0001-ui-egui-quit-prompt-rechecks-after-foreground-save.patch`](../upstream-patches/0001-ui-egui-quit-prompt-rechecks-after-foreground-save.patch)

## Why this is not ComicCraft work

ComicCraft found this while making `⌘S` save a comic *project* as well as a document, but the bug is
**not comic-specific and does not depend on anything ComicCraft added**. It is reachable in plain
VectorCraft, and ComicCraft's own fix is only what made it observable.

I am recording it separately rather than burying it in the M1 stack because it is **worth more upstream
than here**: ComicCraft's copy is one of five edits to pre-existing code, and the other four are
comic-gated.

## The defect

`crates/ui-egui/src/unsaved.rs`, `confirm` — answering the quit prompt:

```rust
let r = io::save(app, SaveMode::Save, &json!({}), false)?;
if r.get("pending").is_some() { return Ok(r); }
// Closing needs the file written: wait for a background save, and stop if it failed.
if r["background"] == true {              // <-- a foreground save never enters here
    crate::background::wait_all(app);
    if let Some(d) = app.session.documents().get(i).filter(|d| d.is_dirty()) {
        return Err(format!("{}: not saved, so not closed", d.title()));
    }
}
close_now(app, i)?;                       // <-- so this can run on a lie
```

The re-check that exists to stop a document being closed while unsaved **is guarded by the save having
gone to the background**. On the foreground path — the ordinary case — the document is closed without
anything being checked.

**Observable symptom:** the user is asked *"save your work?"*, answers **Save**, and the work is
discarded. `is_dirty()` is never consulted on the path that closes.

The guard is also self-defeating on its own terms: the comment says *"wait for a background save, and
stop if it failed"*, but a foreground save can fail or fail to clear `is_dirty()` exactly as a
background one can, and that is the case the guard skips.

## The fix — one hunk, one file

Move the check out of the conditional so it runs on **both** paths:

```rust
crate::background::wait_all(app);
if let Some(d) = app.session.documents().get(i).filter(|d| d.is_dirty()) {
    return Err(format!("{}: not saved, so not closed", d.title()));
}
```

`wait_all` needs no new guard: it is `while !app.background.jobs.is_empty() { … }`, so with nothing in
flight it returns immediately and the foreground path costs nothing.

## The behaviour change, which is upstream's call

This is not a pure bug-fix in one direction, and it should be reviewed as a behaviour change:

- **Before:** a foreground save that leaves the document dirty **closes it**, discarding the work.
- **After:** the close is **refused**, with an error naming the document.

Refusing is deliberate — a document still dirty after the save the prompt asked for must not be
discarded, and an error is a better answer than a silent loss. **But it is a change in what the quit
prompt can do:** if any reachable path leaves a document dirty after a save the prompt requested, the
user can no longer close the window by answering *Save*, and will see *"not saved, so not closed"*
instead.

ComicCraft's position: **that is the right trade**, because the alternative is data loss, and the
message names the document so the cause is visible. If upstream disagrees, the alternative is to keep
the check only where a save is expected to clear `is_dirty()`, and to fix the reason the flag survives
instead — but the current code does neither, and closes anyway.

## What has been verified, and where

| Claim | Where | Evidence |
|---|---|---|
| The bug is present in the pristine baseline | `3da4080b` | the guarded block is at `unsaved.rs:85-91` |
| The patch applies cleanly to the baseline | `3da4080b` | one hunk, `@@ -82,12 +82,20 @@` |
| It compiles on the baseline | `3da4080b` | `cargo check -p vectorcraft-ui-egui` → `Finished` |
| The fix prevents the loss | ComicCraft, M1 | `crates/ui-egui/tests/comic_save_keeps_model.rs` (5 tests): the prompt's Save writes, `is_dirty()` is false, the document closes; and the refusal arm, where a save made to fail leaves the close refused, the document open, still dirty, and **the edit still in the model** |
| The guard is load-bearing | ComicCraft, M1 | mutation **M-ROUTE** (`io.rs:285 → && false`), run by an independent reviewer, reds **2 of 3** tests, one of them via the refusal branch firing verbatim: `comic.vectorcraft: not saved, so not closed` |

## Honest limits of this proposal

- **The refusal branch itself is not pinned by a unit test.** It is reachable only when a save leaves
  `is_dirty()` true, which ComicCraft's fix makes hard to reach on purpose; a reviewer observed it firing
  under M-ROUTE and recorded it as **defence in depth, observed working, not pinned**. Pinning it needs a
  seam — extract `wait_all → re-check → Err` into a helper and unit-test that with an artificially dirty
  document. **I have not done that, in either tree.**
- **Verified on Linux only.**
- ComicCraft's own file carries a longer comment naming `comic.project.save`; the patch here is
  rewritten to stand alone, so the two are **not** byte-identical by design.
- `cargo check`, not the full baseline suite, was run against `3da4080b` — building the pristine
  baseline's whole test surface is what the branch is for, not something I did here.

## To consume

```sh
git fetch <this fork> fix/quit-prompt-rechecks-after-foreground-save
git show 086f6028          # one file, one hunk
git am upstream-patches/0001-ui-egui-quit-prompt-rechecks-after-foreground-save.patch
```
