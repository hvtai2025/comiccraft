# Releasing ComicCraft

ComicCraft is a Node.js process plus static client files. There is **no build step**, no
compiled artifact and no Rust toolchain. A release is a verified commit, a version bump and a
tag; publishing the files is a copy of the repository without `reference/` and `node_modules/`.

The gate is [`AGENTS.md`](../AGENTS.md) and the release criteria are
[`docs/milestones/M8-node.md`](milestones/M8-node.md).

## 1. Before you start

- Node **22 or newer** (CI runs 22 and 24).
- No font files, no browser and no credentials are required for the gate.
- A clean checkout of the commit you intend to release.

## 2. Run the gate

```sh
npm ci
npm run verify
```

`verify` runs, in order:

1. strict JSDoc type check (`tsc --checkJs --noEmit`) over `shared/`, `server/`, `tools/`;
2. the shared-boundary check (no runtime imports or DOM globals in `shared/`);
3. the brand check (no vendor names in shipped text);
4. `node --test` — every suite, including the release book;
5. schema fixtures;
6. command documentation freshness;
7. the server smoke test (boots the real server and drives the real HTTP surface).

It must be green, and it must stop at the first failure. Do not release on a partial run.

## 3. Performance

```sh
npm run perf                  # normal budgets
PERF_STRICT=1 npm run perf    # tighter release budgets
```

The numbers are printed as a table and as JSON with `--json`. Compare them with the table in
[`M8-node.md`](milestones/M8-node.md); a regression is a release blocker, not a curiosity.

## 4. Agent surface

```sh
node server/cli.js commands                 # every command id and label
node server/mcp.js --list                  # the MCP catalogue with parameter schemas
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | node server/mcp.js --headless
```

Every command in the catalogue must appear in `docs/comiccraft-command-api.md`
(`npm run check:docs` enforces it), and every command must answer a malformed call with a
typed failure rather than a crash.

## 5. Round trip a real book

```sh
node server/cli.js demo
node server/cli.js run --cmd comic.project.package --params '{"path":"release.book.comiccraft"}'
node server/cli.js run --cmd comic.project.unpack --params '{"archivePath":"release.book.comiccraft","path":"reopened"}'
node server/cli.js run --cmd comic.project.open --params '{"path":"reopened"}'
```

Then export it three ways and inspect the output:

```sh
node server/cli.js run --cmd comic.book.export --params '{"path":"out/screen","format":"images","preset":"screen"}'
node server/cli.js run --cmd comic.book.export --params '{"path":"out/print.pdf","format":"pdf","preset":"print"}'
node server/cli.js run --cmd comic.book.export --params '{"path":"out/proof","format":"images","preset":"proofing"}'
```

Check: the print PDF has trim, bleed and mark geometry; the proof has a contact sheet; the
project still validates after the export directory is deleted.

## 6. Walk through the studio

```sh
node server.js    # http://127.0.0.1:3000
```

Do this by hand with the representative book:

1. Create a page from a template; add and move a panel; resize it in the inspector.
2. Add a speech balloon, a thought balloon, a caption and a sound effect; set a face, size,
   alignment and overflow policy. Confirm the missing-font warning is visible when no face is
   installed — the export must still work with the standard face.
3. Import a story, review the proposal in **Review**, accept one scene, and confirm in the
   **Storyboard** that unrelated pages did not change.
4. Submit artwork generation on a server session; poll, cancel one job, apply another, reject a
   third. Confirm no job touches a panel until **Apply**.
5. Save locally, reload, reopen. Save to the filesystem through the CLI and reopen.
6. Validate for print and confirm the gate names the exact error (missing asset, unsupported
   page size) rather than exporting silently.

An agent walkthrough is the MCP test plus the CLI batch:

```sh
node server/cli.js run --cmd comic.batch.run --params '{"commands":[{"command":"comic.page.add","params":{"name":"Agent page","template":"three-up"}}]}'
```

## 7. Licence and provenance audit

- Every asset has a row in [`ASSETS.md`](../ASSETS.md); every dependency is accounted for.
- No font file is present in the repository.
- No Adobe iconography, screenshot, preset or copied wording is present.
- Asset records written by generation carry provider, model and request; credentials are never
  written to a bundle, job, export or log.
- `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` are current.

## 8. Version and tag

The version lives in exactly one place, `package.json`. Bump it there, commit the bump, and tag
that commit:

```sh
npm version patch --no-git-tag-version
npm run verify
git add package.json package-lock.json
git commit -m "Release: ComicCraft v$(node -p "require('./package.json').version")"
git tag "v$(node -p "require('./package.json').version")"
git push origin main --tags
```

CI (`.github/workflows/verify.yml`) runs the same gate on Node 22 and 24. A release is only
announced after that run is green on the tagged commit.

## 9. Do not

- Release with a red or partially-run gate.
- Add a runtime dependency without a recorded decision (the standard is `fastify` only).
- Ship a project bundle with credentials, a font file or an unlicensed asset.
- Call a release "print-ready" based on screen output alone.
