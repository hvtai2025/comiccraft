# M5 (Node.js stack) — Agent-native comic operations

**Status:** Complete.
**Gate:** `npm run verify` green, including `test/agent.test.js` and the command-docs check.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §11.

## Exit criteria

| Criterion | Evidence |
|---|---|
| `mcp --headless` lists every comic command with parameter documentation | `server/mcp.js`; `toolList`; `node server/mcp.js --list` parses in the test; each tool carries the command's own `inputSchema` |
| A deterministic command produces identical results when replayed | same registry and dispatcher for every transport; golden render and story tests assert deterministic outputs |
| An agent can complete M1's acceptance list unaided | smoke test drives create → page → artwork → crop → clip → lettering → validate → PNG/PDF → save/reopen over HTTP; MCP calls use the same executor |
| A malformed parameter set cannot crash the dispatcher | `test/agent.test.js` sends five malformed payloads to **every** registered command and asserts a typed result |

## What landed

- `server/mcp.js`: newline-delimited JSON-RPC over stdio (`initialize`, `tools/list`, `tools/call`, `ping`), `--headless`, `--list`, `--project`.
- `comic.command.list` / `comic.command.describe`: parameter schema and retry class for automation.
- `comic.batch.run`: ordered commands through the same dispatcher, atomic by default; a failed batch restores the bundle and drops the undo steps it created.
- `docs/comiccraft-command-api.md`: generated from the registry (`npm run docs:commands`), checked for freshness in the gate (`npm run check:docs`).
- Every command has a malformed-input path in the sweep, not just a happy-path test.

## Notes

- Tool names are the command ids verbatim, so an agent can move between MCP, HTTP and the CLI without a translation table.
- A batch cannot contain another batch: recursion adds failure modes without adding capability.
- `serverOnly` is surfaced in the tool description and `_meta`; the dispatcher refuses those commands in the browser rather than failing halfway.
