---
description: "Use when adding or editing a ComicCraft engine command under crates/engine/src/cmd/comic. Covers the CommandSpec shape, parameter validation and documentation, enablement, undo grouping and additive-only rules."
applyTo: "crates/engine/src/cmd/comic/**"
---
# Comic command authoring

Every user-visible comic behaviour is a command here. The GUI, the control channel, the
CLI and MCP all reach it through this registry and nothing else.

## Shape

```rust
CommandSpec {
    id,        // "comic.<group>.<verb>" — no collision with existing ids
    label,     // user-visible; scanned by `cargo xtask brands`
    menu,      // menu path, or empty
    shortcut,  // rarely; comic commands mostly have none
    params,    // the documentation agents read — treat it as the schema
    enabled,   // honest: false without an open comic project
    run,       // the handler
    journal,   // present when the edit participates in history
}
```

## Rules

- **Additive only.** Never change an existing command's id, parameters, enablement or
  behaviour. That is what keeps upstream merges cheap (ADR-014).
- **`params` and the accepted JSON must agree.** If the documentation string is awkward to
  write, the parameter set is wrong — fix the parameters, not the wording.
- **Validate, then act.** Use the engine's helpers (`str_param`, `id_param`, `ids_param`,
  `f64_req`, `bad(..)`) and return `EngineError::BadParams` for bad input. `Other` is a
  last resort; an agent needs to know *which* parameter was wrong.
- **One user action, one undo step.** Apply document changes through the engine's edit
  path, and group multi-object edits with `UndoGroup`. Do not hand-roll history.
- **Jobs never write.** `comic.job.submit` returns an id; only `comic.job.apply` writes,
  and only after comparing `target_revision` with the document's current revision.
- **One module per command group**, contributing `specs()`, registered once in
  `command_specs()`.
- **Always add a malformed-input case** to `engine/tests/command_sweep.rs`.

## Before you call it done

```sh
cargo xtask ci
vectorcraft-cli commands          # the command and its params doc appear
vectorcraft-cli run --in FILE --cmd comic.<id> --params '{"…":…}'
```
