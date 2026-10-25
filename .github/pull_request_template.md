## What and why

<!-- What does this change, and why? Link the issue if there is one. -->

## How it was tested

<!-- Tests added, and how you checked the behaviour (CLI, MCP, screenshots). -->

## Checklist

- [ ] `npm run verify` passes (type check, lint + boundaries, layering test, `node --test`,
      schema fixtures, server smoke test).
- [ ] No panics in shipped code: no `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or
      `unimplemented!` outside tests, and no unchecked indexing on data from files, parameters or
      messages. Errors are returned as `Result` and shown to the user
      ([why and how](https://github.com/storytold/vectorcraft/blob/main/docs/development.md#robustness-vectorcraft-never-crashes)).
- [ ] Clean-room: nothing copied from Adobe products or GPL/AGPL code ([AGENTS.md](https://github.com/storytold/vectorcraft/blob/main/AGENTS.md)).
- [ ] Every new asset has a row in `ASSETS.md`.
