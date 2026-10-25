---
description: "Use when adding or editing a ComicCraft command in shared/commands or server/commands. Covers the command definition shape, parameter validation and documentation, enablement, undo grouping, transport parity and additive-only rules."
applyTo: ["shared/commands/**", "server/commands/**"]
---
# Comic command authoring

Every user-visible comic behaviour is a command here. The browser UI, the HTTP endpoint and
MCP all reach it through this registry and nothing else — so a command defined once behaves
identically for a click, a `curl` and an agent. That property is the point of the design,
not a side effect.

## Shape

```js
/**
 * @typedef {object} CommandSpec
 * @property {string}   id        // "comic.<group>.<verb>" — no collision with existing ids
 * @property {string}   label     // user-visible; scanned by `npm run check:brands`
 * @property {object}   params    // the JSON Schema agents read — treat it as the contract
 * @property {(ctx) => boolean} enabled   // honest: false without an open comic project
 * @property {(ctx, params) => Result} run
 * @property {boolean}  journal   // true when the edit participates in undo history
 * @property {boolean}  serverOnly // true when the handler must run on the server
 */
```

## Rules

- **Additive only.** Never change an existing command's id, parameters, enablement or
  behaviour. Agents and saved automations address commands by id.
- **`params` and the accepted JSON must agree.** The schema *is* the parameter
  documentation, and it is what MCP publishes. If the schema is awkward to write, the
  parameter set is wrong — fix the parameters, not the wording.
- **Validate, then act.** Check against the declared schema *before* the handler runs.
  Return a typed `BadParams` failure naming the offending parameter; never throw, and never
  treat an unknown extra field as acceptable. A malformed call must not be able to
  half-apply an edit.
- **One user action, one undo step.** Apply changes through the transaction API so a
  multi-object edit is grouped. Do not hand-roll history.
- **Jobs never write.** `comic.job.submit` returns an id; only `comic.job.apply` writes, and
  only after comparing `target_revision` with the project's current revision.
- **One module per command group**, registered once in the registry index.
- **A handler that needs the filesystem, a provider or a renderer is `serverOnly`** and
  lives in `server/commands/**`. Everything else must run in the browser too, which means it
  must not import a runtime (see `comiccraft-work.instructions.md` §3).
- **Always add a malformed-parameter case** to `test/fuzz.js`.

## Before you call it done

```sh
npm run verify
npm run cli -- commands                 # the command and its params doc appear
npm run cli -- run --project ./p --cmd comic.<id> --params '{"…":…}'
curl -s localhost:3000/api/comic/comic.<id> -d '{"…":…}'   # same result, same code path
```
